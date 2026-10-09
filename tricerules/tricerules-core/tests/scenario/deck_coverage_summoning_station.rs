//! Actual Summoning Station, exact nonartifact Pincher and artifact-death untap evidence.

use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, CounterKind, EffectDuration};
use tricerules_core::state::{ActiveDeathReplacement, AffectedScope, ContinuousEffect};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChoiceKind, DevCommand, DevMoveCard, DevZone,
    ResolutionChoiceDecision, RuledEventBatch,
};

const STATION: &str = "summoning_station";
const PINCHER: &str = "pincher_c_2_2";

fn setup() -> (GameEngine, u32) {
    let deck = deck_with("mountain", &[]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_701,
        &[10, 20, 30],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    let station = inject_card_into_hand(&mut e, 0, STATION);
    give_mana(
        &mut e,
        10,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, STATION);
    e.apply_command(10, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut e);
    assert_eq!(e.state.objects[&station].zone, Zone::Battlefield);
    assert_eq!(e.state.players[0].mana_pool, Default::default());
    (e, station)
}

#[test]
fn summoning_station_paid_cast_tap_and_exact_pincher() {
    let card = tricerules_cards::registry::global()
        .get(STATION)
        .expect("exact Summoning Station registered");
    assert_eq!(card.name, "Summoning Station");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{7}");
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(face.triggered_abilities.len(), 1);
    assert!(face.triggered_abilities[0].may);
    let (mut e, station) = setup();
    let command = activate_ability_for(&e, station, 0, vec![]);
    let before = serde_json::to_vec(&e.state).unwrap();
    assert!(e.apply_command(20, &command).is_err());
    assert_eq!(serde_json::to_vec(&e.state).unwrap(), before);
    e.apply_command(10, &command).unwrap();
    assert!(
        e.state.objects[&station].tapped,
        "noncreature Station can tap immediately"
    );
    assert!(battlefield_token_oids(&e, 0, PINCHER).is_empty());
    let before = serde_json::to_vec(&e.state).unwrap();
    assert!(e.apply_command(10, &command).is_err());
    assert_eq!(serde_json::to_vec(&e.state).unwrap(), before);
    let batch = resolve_one(&mut e);
    let tokens = battlefield_token_oids(&e, 0, PINCHER);
    assert_eq!(tokens.len(), 1);
    assert_ne!(tokens[0], station);
    let actual = e.characteristics(tokens[0]).unwrap();
    assert_eq!(actual.types, ["Creature", "Pincher"]);
    assert!(actual.colors.is_empty());
    assert!(actual.keywords.is_empty());
    assert_eq!(
        (
            e.effective_power(tokens[0]),
            e.effective_toughness(tokens[0])
        ),
        (Some(2), Some(2))
    );
    let created = token_created_events(&batch);
    assert_eq!(created.len(), 1);
    assert_eq!(
        (created[0].object_id, created[0].controller_player_id),
        (tokens[0], 10)
    );
    let identity = created[0]
        .identity
        .as_ref()
        .expect("public exact token identity");
    assert_eq!(
        (
            identity.name.as_str(),
            identity.pt.as_str(),
            identity.color.as_str()
        ),
        ("Pincher", "2/2", "")
    );
    assert_eq!(identity.types, ["Creature", "Pincher"]);
    assert!(
        identity.is_creature && identity.keywords.is_empty() && identity.ability_texts.is_empty()
    );
    assert!(e.state.stack.is_empty());
    assert!(e.state.pending_triggers.is_empty());
}

fn resolve_one(e: &mut GameEngine) -> RuledEventBatch {
    answer_simultaneous_entry_order_in_engine_order(e);
    answer_trigger_order_in_engine_order(e);
    let count = e.state.players.iter().filter(|p| !p.has_lost).count()
        - e.state.passes_since_stack_change as usize;
    let mut result = RuledEventBatch::default();
    for _ in 0..count {
        let actor = e.state.priority_player_id();
        result = e.apply_command(actor, &pass()).unwrap();
    }
    result
}

fn reject(e: &mut GameEngine, actor: i32, command: &RuledCommand) {
    // Pending branches contain non-string map keys, so JSON cannot encode the full state.
    let before = format!("{:?}", e.state);
    e.apply_command(actor, command)
        .expect_err("illegal input rejects");
    assert_eq!(format!("{:?}", e.state), before);
}

fn priority(e: &mut GameEngine, actor: i32) {
    for _ in 0..3 {
        if e.state.priority_player_id() == actor {
            return;
        }
        let current = e.state.priority_player_id();
        e.apply_command(current, &pass()).unwrap();
    }
    panic!("priority did not reach actor {actor}");
}

fn cast(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    priority(e, 10);
    inject_card_into_hand(e, 0, card);
    give_mana(
        e,
        10,
        ManaGift {
            c: 10,
            w: 4,
            u: 4,
            b: 4,
            r: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, card);
    e.apply_command(10, &cast_spell(slot, targets)).unwrap();
    resolve_one(e);
}

fn tap_station(e: &mut GameEngine, station: u32, actor: i32) {
    priority(e, actor);
    e.apply_command(actor, &activate_ability_for(e, station, 0, vec![]))
        .unwrap();
    resolve_one(e);
    assert!(e.state.objects[&station].tapped);
}

fn optional_untap(e: &mut GameEngine, actor: i32, accept: bool) {
    assert!(
        e.state.pending_resolution.is_none(),
        "option is not chosen when captured"
    );
    let batch = resolve_one(e);
    let choice = find_resolution_choice(&batch).expect("optional untap at resolution");
    assert_eq!(choice.deciding_player_id, actor);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((choice.min, choice.max), (0, 1));
    let accepted = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    reject(e, if actor == 10 { 20 } else { 10 }, &accepted);
    let mut invalid = accepted.clone();
    if let Some(Cmd::SubmitResolutionChoice(answer)) = invalid.cmd.as_mut() {
        answer.selected_branch_index = 1;
    }
    reject(e, actor, &invalid);
    e.apply_command(
        actor,
        &submit_resolution_decision(if accept {
            ResolutionChoiceDecision::SelectBranch
        } else {
            ResolutionChoiceDecision::Decline
        }),
    )
    .unwrap();
    assert!(e.state.pending_resolution.is_none());
}

fn sacrifice_gnomes(e: &mut GameEngine, seat: usize, replaced: bool) -> u32 {
    let actor = e.state.players[seat].id;
    let gnomes = inject_creature_on_battlefield(e, seat, "bottle_gnomes");
    if replaced {
        e.state
            .death_replacement_effects
            .push(ActiveDeathReplacement {
                object_id: gnomes,
                zone_change_generation: e
                    .state
                    .zone_change_generation
                    .get(&gnomes)
                    .copied()
                    .unwrap_or(0),
            });
    }
    priority(e, actor);
    e.apply_command(actor, &activate_ability_for(e, gnomes, 0, vec![]))
        .unwrap();
    gnomes
}

#[test]
fn summoning_station_any_player_artifact_death_optional_resolution() {
    for seat in 0..3 {
        for accept in [false, true] {
            let (mut e, station) = setup();
            tap_station(&mut e, station, 10);
            let gnomes = sacrifice_gnomes(&mut e, seat, false);
            assert_eq!(e.state.objects[&gnomes].zone, Zone::Graveyard);
            assert_eq!(
                e.state.stack.len(),
                2,
                "one untap above actual sacrifice activation"
            );
            assert!(
                e.state.pending_triggers.is_empty(),
                "no upfront targeted trigger choice"
            );
            optional_untap(&mut e, 10, accept);
            assert_eq!(e.state.objects[&station].tapped, !accept);
            assert_eq!(e.state.stack.len(), 1);
            resolve_one(&mut e);
            assert_eq!(e.state.players[seat].life, 23);
            assert!(e.state.stack.is_empty());
        }
    }
}

#[test]
fn summoning_station_counts_each_simultaneous_artifact_not_pincher() {
    let (mut e, station) = setup();
    tap_station(&mut e, station, 10);
    let pincher = battlefield_token_oids(&e, 0, PINCHER)[0];
    let first = inject_creature_on_battlefield(&mut e, 0, "ornithopter");
    let second = inject_creature_on_battlefield(&mut e, 2, "ornithopter");
    cast(&mut e, "wrath_of_god", vec![]);
    assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
    assert!(!e.state.players[0].battlefield.contains(&pincher));
    answer_trigger_order_in_engine_order(&mut e);
    assert_eq!(
        e.state.stack.len(),
        2,
        "two artifacts, never the nonartifact Pincher"
    );
    assert_ne!(e.state.stack[0].id, e.state.stack[1].id);
    optional_untap(&mut e, 10, true);
    assert!(!e.state.objects[&station].tapped);
    tap_station(&mut e, station, 10);
    assert_eq!(battlefield_token_oids(&e, 0, PINCHER).len(), 1);
    assert_eq!(
        e.state.stack.len(),
        1,
        "remaining independent death trigger"
    );
    optional_untap(&mut e, 10, true);
    assert!(!e.state.objects[&station].tapped);
    assert!(e.state.stack.is_empty());
}

#[test]
fn summoning_station_only_actual_artifact_battlefield_to_graveyard_counts() {
    for (spell, destination) in [
        ("boomerang", Zone::Hand),
        ("swords_to_plowshares", Zone::Exile),
    ] {
        let (mut e, station) = setup();
        tap_station(&mut e, station, 10);
        let other = inject_creature_on_battlefield(&mut e, 1, "bottle_gnomes");
        cast(&mut e, spell, target_object(other));
        assert_eq!(e.state.objects[&other].zone, destination);
        assert!(e.state.stack.is_empty() && e.state.pending_triggers.is_empty());
        assert!(e.state.objects[&station].tapped);
    }
    let (mut e, station) = setup();
    tap_station(&mut e, station, 10);
    let gnomes = sacrifice_gnomes(&mut e, 1, true);
    assert_eq!(e.state.objects[&gnomes].zone, Zone::Exile);
    assert_eq!(
        e.state.stack.len(),
        1,
        "replacement excludes the untap trigger"
    );
    resolve_one(&mut e);
    assert!(e.state.stack.is_empty());
    assert!(e.state.objects[&station].tapped);
    let pincher = battlefield_token_oids(&e, 0, PINCHER)[0];
    cast(&mut e, "murder", target_object(pincher));
    assert!(
        e.state.stack.is_empty(),
        "ordinary Pincher death is not artifact death"
    );
    assert!(battlefield_token_oids(&e, 0, PINCHER).is_empty());
    // Logged developer relocation is an off-battlefield fixture, not a legal discard producer.
    let hand_artifact = inject_card_into_hand(&mut e, 1, "bottle_gnomes");
    e.enable_dev_commands();
    e.apply_command(
        20,
        &RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: 20,
                dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                    card_name: "Bottle Gnomes".into(),
                    zone: DevZone::Graveyard as i32,
                    ready: false,
                })),
            })),
        },
    )
    .unwrap();
    assert_eq!(e.state.objects[&hand_artifact].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty() && e.state.pending_triggers.is_empty());
    assert!(e.state.objects[&station].tapped);
}

#[test]
fn summoning_station_artifact_token_sacrifice_triggers() {
    let (mut e, station) = setup();
    tap_station(&mut e, station, 10);
    let port = inject_permanent_on_battlefield(&mut e, 1, "fountainport");
    give_mana(
        &mut e,
        20,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    priority(&mut e, 20);
    e.apply_command(20, &activate_ability_for(&e, port, 3, vec![]))
        .unwrap();
    resolve_one(&mut e);
    let treasure = battlefield_token_oids(&e, 1, "treasure")[0];
    let before = e.state.players[1].mana_pool.green;
    priority(&mut e, 20);
    let mut command = activate_ability_for(&e, treasure, 0, vec![]);
    if let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() {
        ability.mana_option_index = 4;
    }
    e.apply_command(20, &command).unwrap();
    assert_eq!(e.state.players[1].mana_pool.green, before + 1);
    assert_eq!(
        e.state.stack.len(),
        1,
        "artifact token death triggers despite retirement"
    );
    optional_untap(&mut e, 10, true);
    assert!(!e.state.objects[&station].tapped);
}

#[test]
fn summoning_station_source_departure_and_new_generation_are_distinct() {
    let (mut e, station) = setup();
    e.apply_command(10, &activate_ability_for(&e, station, 0, vec![]))
        .unwrap();
    cast(&mut e, "boomerang", target_object(station));
    assert_eq!(e.state.objects[&station].zone, Zone::Hand);
    resolve_one(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, PINCHER).len(),
        1,
        "captured creation survives source departure"
    );
    let (mut e, station) = setup();
    tap_station(&mut e, station, 10);
    sacrifice_gnomes(&mut e, 1, false);
    let stale_activation = activate_ability_for(&e, station, 0, vec![]);
    let generation = e
        .state
        .zone_change_generation
        .get(&station)
        .copied()
        .unwrap_or(0);
    cast(&mut e, "boomerang", target_object(station));
    assert_eq!(
        move_ready_to_battlefield(&mut e, 0, STATION),
        station,
        "logged dev placement, not a legal recast"
    );
    assert!(e.state.zone_change_generation[&station] > generation);
    priority(&mut e, 10);
    reject(&mut e, 10, &stale_activation);
    tap_station(&mut e, station, 10);
    optional_untap(&mut e, 10, true);
    assert!(
        e.state.objects[&station].tapped,
        "old trigger cannot untap returned physical object"
    );
    resolve_one(&mut e);
    assert!(e.state.stack.is_empty());
}

#[test]
fn summoning_station_capture_control_suppression_and_self_departure() {
    for before_capture in [true, false] {
        let (mut e, station) = setup();
        tap_station(&mut e, station, 10);
        let suppression = ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(station),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: e.state.command_index,
        };
        if before_capture {
            e.state.continuous_effects.push(suppression.clone());
        }
        sacrifice_gnomes(&mut e, 1, false);
        assert_eq!(e.state.stack.len(), if before_capture { 1 } else { 2 });
        if !before_capture {
            e.state.continuous_effects.push(suppression);
            // Explicit internal control fixture; no admitted legal card producer claimed.
            e.state.players[0].battlefield.retain(|id| *id != station);
            e.state.players[2].battlefield.push(station);
            let source = e.state.objects.get_mut(&station).unwrap();
            source.base_controller = 30;
            source.controller = 30;
            optional_untap(&mut e, 10, true);
            assert!(!e.state.objects[&station].tapped);
            assert_eq!(e.state.objects[&station].controller, 30);
        }
        resolve_one(&mut e);
        assert!(e.state.stack.is_empty());
    }
    let (mut e, station) = setup();
    tap_station(&mut e, station, 10);
    cast(&mut e, "shatterstorm", vec![]);
    assert_eq!(e.state.objects[&station].zone, Zone::Graveyard);
    assert_eq!(
        e.state.stack.len(),
        1,
        "self departure captures its own artifact death"
    );
    optional_untap(&mut e, 10, true);
    assert_eq!(e.state.objects[&station].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn summoning_station_optional_untap_respects_stun_and_prohibition() {
    for prohibit in [false, true] {
        let (mut e, station) = setup();
        tap_station(&mut e, station, 10);
        // Explicit counter/prohibition fixtures exercise the existing shared untap pipeline.
        e.state
            .objects
            .get_mut(&station)
            .unwrap()
            .set_counter(CounterKind::Stun, 2);
        if prohibit {
            e.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(station),
                kind: ContinuousEffectKind::ProhibitUntap,
                condition: None,
                duration: EffectDuration::Indefinite,
                timestamp: e.state.command_index,
            });
        }
        sacrifice_gnomes(&mut e, 1, false);
        optional_untap(&mut e, 10, false);
        assert_eq!(
            e.state.objects[&station].counter_count(CounterKind::Stun),
            2
        );
        assert!(e.state.objects[&station].tapped);
        resolve_one(&mut e);
        for attempt in 0..3 {
            sacrifice_gnomes(&mut e, 1, false);
            optional_untap(&mut e, 10, true);
            assert_eq!(
                e.state.objects[&station].counter_count(CounterKind::Stun),
                if prohibit {
                    2
                } else {
                    2u32.saturating_sub(attempt + 1)
                }
            );
            assert_eq!(e.state.objects[&station].tapped, prohibit || attempt < 2);
            resolve_one(&mut e);
        }
    }
}
