//! Actual Summoning Station, exact nonartifact Pincher and artifact-death untap evidence.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, CounterKind, EffectDuration};
use tricerules_core::state::{ActiveDeathReplacement, AffectedScope, ContinuousEffect};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChoiceKind, DevCommand, DevMoveCard, DevZone,
    ResolutionChoiceDecision, RuledEventBatch,
};

const STATION: &str = "summoning_station";
const PINCHER: &str = "pincher_c_2_2";

fn setup_with_players(player_ids: &[i32], seed: u64) -> (GameEngine, u32) {
    let deck = deck_with("mountain", &[]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        player_ids,
        20,
        Some((0..player_ids.len()).map(|_| deck.clone()).collect()),
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
    e.apply_scenario_command(10, &cast_spell(slot, vec![]))
        .unwrap();
    pass_priority_round(&mut e);
    assert_eq!(e.state.objects[&station].zone, Zone::Battlefield);
    assert_eq!(e.state.players[0].mana_pool, Default::default());
    (e, station)
}

fn setup() -> (GameEngine, u32) {
    setup_with_players(&[10, 20, 30], 26_100_701)
}

fn stack_resolved_count(batch: &RuledEventBatch, stack_id: u32) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackResolved(resolved))
                    if resolved.object_id == stack_id
            )
        })
        .count()
}

fn select_branch_index(index: u32) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() {
        answer.selected_branch_index = index;
    }
    command
}

fn advance_to_player_main1(e: &mut GameEngine, player: i32) {
    for _ in 0..40 {
        if e.state.turn_step == tricerules_core::TurnStep::Main1
            && e.state.active_player_id() == player
        {
            return;
        }
        let (actor, command) = match e.state.cleanup_discard_player {
            Some(player) => {
                let player_index = e.state.player_idx(player).expect("cleanup player");
                let excess = e.state.players[player_index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (e.state.priority_player_id(), pass()),
        };
        e.apply_scenario_command(actor, &command)
            .expect("pass priority toward the requested player's main phase");
    }
    panic!("game did not reach player {player}'s main phase");
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
    assert!(e.apply_scenario_command(20, &command).is_err());
    assert_eq!(serde_json::to_vec(&e.state).unwrap(), before);
    e.apply_scenario_command(10, &command).unwrap();
    assert!(
        e.state.objects[&station].tapped,
        "noncreature Station can tap immediately"
    );
    assert!(battlefield_token_oids(&e, 0, PINCHER).is_empty());
    let before = serde_json::to_vec(&e.state).unwrap();
    assert!(e.apply_scenario_command(10, &command).is_err());
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
        result = e.apply_scenario_command(actor, &pass()).unwrap();
    }
    result
}

fn reject(e: &mut GameEngine, actor: i32, command: &RuledCommand) {
    // Pending branches contain non-string map keys, so JSON cannot encode the full state.
    let before = format!("{:?}", e.state);
    e.apply_scenario_command(actor, command)
        .expect_err("illegal input rejects");
    assert_eq!(format!("{:?}", e.state), before);
}

fn priority(e: &mut GameEngine, actor: i32) {
    for _ in 0..3 {
        if e.state.priority_player_id() == actor {
            return;
        }
        let current = e.state.priority_player_id();
        e.apply_scenario_command(current, &pass()).unwrap();
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
    e.apply_scenario_command(10, &cast_spell(slot, targets))
        .unwrap();
    resolve_one(e);
}

fn tap_station(e: &mut GameEngine, station: u32, actor: i32) {
    priority(e, actor);
    e.apply_scenario_command(actor, &activate_ability_for(e, station, 0, vec![]))
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
    e.apply_scenario_command(
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
    e.apply_scenario_command(actor, &activate_ability_for(e, gnomes, 0, vec![]))
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
    e.apply_scenario_command(
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
    e.apply_scenario_command(20, &activate_ability_for(&e, port, 3, vec![]))
        .unwrap();
    resolve_one(&mut e);
    let treasure = battlefield_token_oids(&e, 1, "treasure")[0];
    let before = e.state.players[1].mana_pool.green;
    priority(&mut e, 20);
    let mut command = activate_ability_for(&e, treasure, 0, vec![]);
    if let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() {
        ability.mana_option_index = 4;
    }
    e.apply_scenario_command(20, &command).unwrap();
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
    e.apply_scenario_command(10, &activate_ability_for(&e, station, 0, vec![]))
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

#[test]
fn summoning_station_confiscate_controller_concession_preserves_begun_untap() {
    for accept in [false, true] {
        run_confiscated_station_departure(&[10, 20, 30], 26_100_704, 2, accept, 30, 0, 10);
    }
}

#[test]
fn summoning_station_four_player_departure_keeps_selector_and_delegate_distinct() {
    run_confiscated_station_departure(&[10, 20, 30, 40], 26_100_705, 3, true, 30, 2, 40);
}

#[test]
fn summoning_station_same_player_delegate_gets_a_fresh_prompt_id() {
    run_confiscated_station_departure(&[10, 20, 30], 26_100_706, 2, true, 30, 1, 30);
}

#[test]
fn summoning_station_source_owner_departure_keeps_live_controller_choice() {
    let (mut e, station) = setup_with_players(&[10, 20, 30], 26_100_707);
    advance_to_player_main1(&mut e, 20);
    inject_card_into_hand(&mut e, 1, "confiscate");
    give_mana(
        &mut e,
        20,
        ManaGift {
            u: 6,
            ..Default::default()
        },
    );
    priority(&mut e, 20);
    let confiscate_slot = hand_index_for_card(&e, 1, "confiscate");
    e.apply_scenario_command(20, &cast_spell(confiscate_slot, target_object(station)))
        .expect("P20 controls the Station with Confiscate");
    resolve_one(&mut e);
    tap_station(&mut e, station, 20);
    sacrifice_gnomes(&mut e, 2, false);
    let _initial_offer = resolve_one(&mut e);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the stolen Station's controller has begun resolving its trigger");
    assert_eq!(pending.deciding_player, 20);
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("the Station trigger uses an optional-trigger continuation");
    };
    let original = stack.item.clone();
    assert_eq!(original.controller, 20);
    assert_eq!(original.source_owner, Some(10));
    assert_eq!(original.source_permanent_id, Some(station));

    e.apply_scenario_command(10, &concede())
        .expect("the physical source owner may leave during the live controller's choice");
    assert!(e.state.players[0].has_lost);
    assert!(!e.state.players[1].has_lost && !e.state.players[2].has_lost);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the controller's begun choice remains pending");
    assert_eq!(pending.deciding_player, 20);
    assert_eq!((pending.presentation.min, pending.presentation.max), (0, 1));
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("source-owner departure does not replace the continuation");
    };
    assert_eq!(stack.item.id, original.id);
    assert_eq!(stack.item.controller, 20);
    assert_eq!(stack.item.source_owner, Some(10));
    assert_eq!(stack.item.source_permanent_id, Some(station));

    let declined = e
        .apply_scenario_command(
            20,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("the still-live original controller may decline after source-owner departure");
    assert_eq!(stack_resolved_count(&declined, original.id), 1);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn summoning_station_departed_delegate_is_replaced_and_root_completes_once() {
    let (mut e, station) = setup_with_players(&[10, 20, 30, 40], 26_100_708);
    advance_to_player_main1(&mut e, 20);
    inject_card_into_hand(&mut e, 1, "confiscate");
    give_mana(
        &mut e,
        20,
        ManaGift {
            u: 6,
            ..Default::default()
        },
    );
    priority(&mut e, 20);
    let confiscate_slot = hand_index_for_card(&e, 1, "confiscate");
    e.apply_scenario_command(20, &cast_spell(confiscate_slot, target_object(station)))
        .expect("P20 controls the Station with Confiscate");
    resolve_one(&mut e);
    tap_station(&mut e, station, 20);
    sacrifice_gnomes(&mut e, 3, false);
    let _ = resolve_one(&mut e);
    let original = match &e
        .state
        .pending_resolution
        .as_ref()
        .expect("the Station trigger begins resolving")
        .continuation
    {
        ResolutionContinuation::OptionalTriggeredAbility { stack, .. } => stack.item.clone(),
        _ => panic!("the Station trigger is optional"),
    };
    let station_generation = e.state.zone_change_generation[&station];

    e.apply_scenario_command(20, &concede())
        .expect("P20 leaves while its Station trigger is pending");
    let selector = e
        .state
        .pending_resolution
        .as_ref()
        .expect("P30 receives the selector stage")
        .deciding_player;
    assert_eq!(selector, 30);
    e.apply_scenario_command(selector, &select_branch_index(2))
        .expect("P30 chooses P40 as the first delegate");
    let delegated = e
        .state
        .pending_resolution
        .as_ref()
        .expect("P40 receives the Apply/Decline stage");
    assert_eq!(delegated.deciding_player, 40);
    let stale_delegate_prompt_id = delegated.resolution_prompt_id.get();

    e.apply_scenario_command(40, &concede())
        .expect("the selected delegate may leave before answering");
    assert!(e.state.players[3].has_lost);
    let selector_prompt = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the remaining selector receives a refreshed delegate offer");
    assert_eq!(selector_prompt.deciding_player, 30);
    assert_eq!(
        (
            selector_prompt.presentation.min,
            selector_prompt.presentation.max
        ),
        (1, 1)
    );
    assert!(matches!(
        &selector_prompt.continuation,
        ResolutionContinuation::OptionalTriggeredAbility { .. }
    ));
    let selector_prompt_id = selector_prompt.resolution_prompt_id.get();

    let before_stale_departed_delegate = format!("{:?}", e.state);
    let mut stale_answer = select_branch_index(1);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = stale_answer.cmd.as_mut() {
        answer.resolution_prompt_id = stale_delegate_prompt_id;
    }
    e.apply_scenario_command(40, &stale_answer)
        .expect_err("a departed delegate cannot answer its former prompt");
    assert_eq!(format!("{:?}", e.state), before_stale_departed_delegate);

    e.apply_scenario_command(30, &select_branch_index(1))
        .expect("the sole remaining eligible player may delegate to themself");
    let self_delegate = e
        .state
        .pending_resolution
        .as_ref()
        .expect("P30 receives the same trigger's Apply/Decline stage");
    assert_eq!(self_delegate.deciding_player, 30);
    assert_ne!(self_delegate.resolution_prompt_id.get(), selector_prompt_id);
    let applied = e
        .apply_scenario_command(
            30,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("the surviving delegate applies the original untap");
    assert_eq!(stack_resolved_count(&applied, original.id), 1);
    assert!(e.state.pending_resolution.is_none());
    assert!(!e.state.objects[&station].tapped);
    assert_eq!(e.state.zone_change_generation[&station], station_generation);
}

fn run_confiscated_station_departure(
    player_ids: &[i32],
    seed: u64,
    artifact_seat: usize,
    accept: bool,
    selector: i32,
    delegate_index: u32,
    delegate: i32,
) {
    let (mut e, station) = setup_with_players(player_ids, seed);
    advance_to_player_main1(&mut e, 20);
    inject_card_into_hand(&mut e, 1, "confiscate");
    give_mana(
        &mut e,
        20,
        ManaGift {
            u: 6,
            ..Default::default()
        },
    );
    priority(&mut e, 20);
    let confiscate_slot = hand_index_for_card(&e, 1, "confiscate");
    e.apply_scenario_command(20, &cast_spell(confiscate_slot, target_object(station)))
        .expect("cast Confiscate on the owned Station");
    resolve_one(&mut e);
    assert_eq!(e.state.objects[&station].owner, 10);
    assert_eq!(e.state.objects[&station].controller, 20);

    tap_station(&mut e, station, 20);
    let sacrificed_artifact = sacrifice_gnomes(&mut e, artifact_seat, false);
    assert_eq!(e.state.objects[&sacrificed_artifact].zone, Zone::Graveyard);
    assert_eq!(
        e.state.stack.len(),
        2,
        "Station trigger is above the sacrifice"
    );
    let initial_offer = resolve_one(&mut e);
    let choice = find_resolution_choice(&initial_offer).expect("Station's optional untap offer");
    assert_eq!(choice.deciding_player_id, 20);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the trigger has begun resolving");
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("the begun trigger remains an optional-trigger continuation");
    };
    let original = stack.item.clone();
    let station_generation = e.state.zone_change_generation[&station];
    assert_eq!(original.controller, 20);
    assert_eq!(original.source_permanent_id, Some(station));
    assert_eq!(original.source_zone_change, station_generation);

    let departure = e
        .apply_scenario_command(20, &concede())
        .expect("the Confiscate controller may concede during Station's untap choice");
    assert!(e.state.players[1].has_lost);
    assert!(!e.state.players[0].has_lost && !e.state.players[2].has_lost);
    assert_eq!(
        e.state.objects[&station].controller, 10,
        "Station returns to its surviving owner after Confiscate leaves"
    );
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the begun Station untap choice remains pending for delegation");
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("the original optional-trigger continuation is retained");
    };
    assert_eq!(pending.deciding_player, selector);
    assert_eq!(stack.item.id, original.id);
    assert_eq!(stack.item.controller, original.controller);
    assert_eq!(stack.item.source_owner, original.source_owner);
    assert_eq!(stack.item.source_permanent_id, original.source_permanent_id);
    assert_eq!(stack.item.source_zone_change, original.source_zone_change);
    assert_eq!(stack.item.targets, original.targets);
    assert_eq!(e.state.zone_change_generation[&station], station_generation);
    assert!(find_resolution_choice(&departure).is_some());

    let selector_prompt_id = pending.resolution_prompt_id.get();
    e.apply_scenario_command(selector, &select_branch_index(delegate_index))
        .expect("the next live player selects a delegate");
    let delegated_prompt = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the selected delegate receives Apply/Decline");
    assert_eq!(delegated_prompt.deciding_player, delegate);
    assert_ne!(
        (
            delegated_prompt.deciding_player,
            delegated_prompt.resolution_prompt_id.get()
        ),
        (selector, selector_prompt_id),
        "the new actor and prompt ID together identify the delegate's answer stage"
    );
    assert!(matches!(
        &delegated_prompt.continuation,
        ResolutionContinuation::OptionalTriggeredAbility { .. }
    ));
    assert_eq!(
        (
            delegated_prompt.presentation.min,
            delegated_prompt.presentation.max
        ),
        (0, 1)
    );
    let delegated_prompt_id = delegated_prompt.resolution_prompt_id.get();

    let mut malformed_answer = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = malformed_answer.cmd.as_mut() {
        answer.chosen_object_ids.push(station);
    }
    let before_malformed_answer = format!("{:?}", e.state);
    e.apply_scenario_command(delegate, &malformed_answer)
        .expect_err("a delegated optional answer cannot include object selections");
    assert_eq!(format!("{:?}", e.state), before_malformed_answer);

    if selector == delegate {
        assert_ne!(delegated_prompt_id, selector_prompt_id);
        let before_stale_prompt = format!("{:?}", e.state);
        let mut stale_stage_answer = select_branch_index(0);
        if let Some(Cmd::SubmitResolutionChoice(answer)) = stale_stage_answer.cmd.as_mut() {
            answer.resolution_prompt_id = selector_prompt_id;
        }
        e.apply_scenario_command(delegate, &stale_stage_answer)
            .expect_err("the prior selector-stage prompt cannot answer the delegate stage");
        assert_eq!(format!("{:?}", e.state), before_stale_prompt);
    } else {
        let before_old_actor = format!("{:?}", e.state);
        let mut old_actor_answer = select_branch_index(0);
        if let Some(Cmd::SubmitResolutionChoice(answer)) = old_actor_answer.cmd.as_mut() {
            answer.resolution_prompt_id = delegated_prompt_id;
        }
        e.apply_scenario_command(selector, &old_actor_answer)
            .expect_err("the prior selector cannot answer the delegate's current prompt");
        assert_eq!(format!("{:?}", e.state), before_old_actor);
    }

    let answer = if accept {
        submit_resolution_decision(ResolutionChoiceDecision::SelectBranch)
    } else {
        submit_resolution_decision(ResolutionChoiceDecision::Decline)
    };
    let completed = e
        .apply_scenario_command(delegate, &answer)
        .expect("the delegate answers the original optional untap");
    assert_eq!(stack_resolved_count(&completed, original.id), 1);
    assert_eq!(e.state.objects[&station].tapped, !accept);
    assert_eq!(e.state.zone_change_generation[&station], station_generation);
    assert!(e.state.pending_resolution.is_none());

    let before_replay = format!("{:?}", e.state);
    let mut stale_delegate_answer = submit_resolution_decision(if accept {
        ResolutionChoiceDecision::SelectBranch
    } else {
        ResolutionChoiceDecision::Decline
    });
    if let Some(Cmd::SubmitResolutionChoice(answer)) = stale_delegate_answer.cmd.as_mut() {
        answer.resolution_prompt_id = delegated_prompt_id;
    }
    e.apply_scenario_command(delegate, &stale_delegate_answer)
        .expect_err("a completed root cannot be answered a second time");
    assert_eq!(format!("{:?}", e.state), before_replay);
}
