//! Exact Kodama of the East Tree identity and entry-trigger behavior.

use super::helpers::*;
use tricerules_cards::primitives::{
    CardTypeFilter, CastTriggerPlayer, GameCondition, ManaValueLimit, PermanentEventFilter,
    SpellEffectKind, ZoneCardFilter,
};
use tricerules_cards::{Keyword, Layout};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1;

fn kodama_engine(seed: u64, p0_basic: &str, p0_specials: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with(p0_basic, p0_specials),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_kodama_to_hand_choice(engine: &mut GameEngine) -> v1::ResolutionChoiceRequired {
    answer_trigger_order_in_engine_order(engine);
    let first = engine.state.priority_player_id();
    let second = engine
        .state
        .players
        .iter()
        .map(|player| player.id)
        .find(|player| *player != first)
        .expect("opponent");
    engine
        .apply_command(first, &pass())
        .expect("first player passes for Kodama trigger");
    let batch = engine
        .apply_command(second, &pass())
        .expect("second player passes for Kodama trigger");
    find_resolution_choice(&batch).expect("Kodama private hand choice")
}

fn selectable(choice: &ResolutionChoiceRequired, object_id: u32) -> bool {
    let Some(index) = choice
        .candidate_object_ids
        .iter()
        .position(|candidate| *candidate == object_id)
    else {
        return false;
    };
    choice.candidate_selectable[index]
}

#[test]
fn kodama_registers_its_complete_printed_identity() {
    let card = tricerules_cards::registry::global()
        .get("kodama_of_the_east_tree")
        .expect("complete Kodama of the East Tree definition");
    assert_eq!(card.name, "Kodama of the East Tree");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{G}{G}");
    assert_eq!(face.supertypes, vec!["Legendary".to_string()]);
    assert_eq!(
        face.types,
        vec!["Creature".to_string(), "Spirit".to_string()]
    );
    assert_eq!((face.power, face.toughness), (Some(6), Some(6)));
    assert_eq!(face.keywords, vec![Keyword::Reach, Keyword::Partner]);
    assert_eq!(face.triggered_abilities.len(), 1);
    let ability = &face.triggered_abilities[0];
    assert_eq!(
        ability.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.intervening_if,
        Some(GameCondition::TriggerObjectWasNotPutWithThisAbility)
    );
    match &ability.trigger {
        tricerules_cards::TriggerCondition::WheneverPermanentEntersBattlefield {
            controller,
            filter,
            ..
        } => {
            assert_eq!(*controller, CastTriggerPlayer::Controller);
            assert_eq!(
                *filter,
                PermanentEventFilter {
                    permanent_type: None,
                    exclude_source: true,
                    ..Default::default()
                }
            );
        }
        other => panic!("unexpected Kodama trigger: {other:?}"),
    }
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::PutPermanentFromHand {
            filter: None,
            max_mana_value: Some(ManaValueLimit::TriggerObject),
        }]
    ));
    // Partner's in-game effects are N/A: CR 702.124a applies before the game. The existing
    // setup path accepts commander declarations but does not validate Partner compatibility.
}

#[test]
fn kodama_only_observes_another_permanent_entering_under_its_controllers_control() {
    let mut self_entry = kodama_engine(20_261_011, "forest", &["kodama_of_the_east_tree"]);
    let kodama = move_ready_to_battlefield(&mut self_entry, 0, "kodama_of_the_east_tree");
    assert_eq!(self_entry.state.objects[&kodama].zone, Zone::Battlefield);
    assert!(
        self_entry.state.stack.is_empty(),
        "Kodama does not trigger on itself"
    );
    assert!(self_entry.state.pending_triggers.is_empty());

    let mut opponent_entry = kodama_engine(20_261_012, "forest", &[]);
    let _kodama =
        inject_permanent_on_battlefield(&mut opponent_entry, 0, "kodama_of_the_east_tree");
    inject_card_into_hand(&mut opponent_entry, 1, "grizzly_bears");
    let opponent_permanent = move_ready_to_battlefield(&mut opponent_entry, 1, "grizzly_bears");
    assert_eq!(
        opponent_entry.state.objects[&opponent_permanent].controller,
        opponent_entry.state.players[1].id
    );
    assert!(opponent_entry.state.stack.is_empty());
    assert!(opponent_entry.state.pending_triggers.is_empty());
}

#[test]
fn kodama_bounds_its_private_choice_by_the_entering_permanents_mana_value() {
    let mut engine = kodama_engine(20_261_013, "forest", &["grizzly_bears"]);
    let kodama = inject_permanent_on_battlefield(&mut engine, 0, "kodama_of_the_east_tree");
    let entrant = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    let lower = inject_card_into_hand(&mut engine, 0, "forest");
    let equal = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let higher = inject_card_into_hand(&mut engine, 0, "hill_giant");
    let instant = inject_card_into_hand(&mut engine, 0, "counterspell");

    let choice = resolve_kodama_to_hand_choice(&mut engine);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(choice.deciding_player_id, engine.state.players[0].id);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(selectable(&choice, lower), "lower-MV land is legal");
    assert!(selectable(&choice, equal), "equal-MV permanent is legal");
    assert!(
        !selectable(&choice, higher),
        "higher-MV permanent is illegal"
    );
    assert!(
        !selectable(&choice, instant),
        "nonpermanent card is illegal"
    );

    let revision = engine.state.command_index;
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![higher]))
        .is_err());
    assert_eq!(engine.state.command_index, revision);
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&higher].zone, Zone::Hand);

    let lands_played = engine.state.lands_played_this_turn;
    engine
        .apply_command(0, &submit_resolution_choice(vec![equal]))
        .expect("select the equal-MV permanent");
    assert_eq!(engine.state.objects[&equal].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&equal].controller,
        engine.state.players[0].id
    );
    assert_eq!(engine.state.lands_played_this_turn, lands_played);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
    assert_eq!(engine.state.objects[&kodama].zone, Zone::Battlefield);
}

#[test]
fn kodama_trigger_object_ceiling_composes_with_an_or_card_filter() {
    let mut engine = kodama_engine(20_261_017, "forest", &["grizzly_bears"]);
    let kodama = inject_permanent_on_battlefield(&mut engine, 0, "kodama_of_the_east_tree");
    let _entrant = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let equal = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let higher = inject_card_into_hand(&mut engine, 0, "hill_giant");

    // Exercise the reusable primitive's valid OR filter together with its independent
    // trigger-object bound. Resolution materializes that bound at the filter root.
    let item = engine
        .state
        .stack
        .iter_mut()
        .find(|item| item.source_permanent_id == Some(kodama))
        .expect("Kodama trigger on stack");
    let ability = item
        .triggered_ability
        .as_mut()
        .expect("triggered ability snapshot");
    ability.effect = vec![SpellEffectKind::PutPermanentFromHand {
        filter: Some(ZoneCardFilter {
            any_of: Some(vec![
                ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Creature),
                    ..Default::default()
                },
                ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Enchantment),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        }),
        max_mana_value: Some(ManaValueLimit::TriggerObject),
    }];

    let choice = resolve_kodama_to_hand_choice(&mut engine);
    assert!(
        selectable(&choice, equal),
        "equal-MV creature matches one OR branch"
    );
    assert!(
        !selectable(&choice, higher),
        "the independent trigger-object ceiling still applies through any_of"
    );
}

#[test]
fn kodama_x_cost_trigger_object_has_zero_mana_value_and_may_decline() {
    let mut engine = kodama_engine(20_261_014, "counterspell", &["astral_cornucopia"]);
    let _kodama = inject_permanent_on_battlefield(&mut engine, 0, "kodama_of_the_east_tree");
    let zero = inject_card_into_hand(&mut engine, 0, "ornithopter");
    let positive = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let entrant = move_ready_to_battlefield(&mut engine, 0, "astral_cornucopia");
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);

    let choice = resolve_kodama_to_hand_choice(&mut engine);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(selectable(&choice, zero), "zero-MV permanent is legal");
    assert!(
        !selectable(&choice, positive),
        "X on the entering permanent is zero"
    );
    engine
        .apply_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline Kodama's optional choice");
    assert_eq!(engine.state.objects[&zero].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&positive].zone, Zone::Hand);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn kodama_keeps_fixed_optional_bounds_when_no_permanent_qualifies() {
    let mut engine = kodama_engine(20_261_015, "counterspell", &["grizzly_bears"]);
    let _kodama = inject_permanent_on_battlefield(&mut engine, 0, "kodama_of_the_east_tree");
    let _entrant = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");

    let choice = resolve_kodama_to_hand_choice(&mut engine);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(
        choice.candidate_object_ids.len(),
        choice.candidate_selectable.len()
    );
    assert!(choice
        .candidate_selectable
        .iter()
        .all(|selectable| !selectable));
    engine
        .apply_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline when no permanent in hand qualifies");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn kodama_reentry_generation_does_not_confuse_an_older_ability_use() {
    let mut engine = kodama_engine(
        20_261_016,
        "forest",
        &["kodama_of_the_east_tree", "grizzly_bears"],
    );
    let kodama = move_ready_to_battlefield(&mut engine, 0, "kodama_of_the_east_tree");
    let _entrant = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let original_generation = engine.state.zone_change_generation[&kodama];
    let original_use = engine
        .state
        .stack
        .iter()
        .find(|item| item.source_permanent_id == Some(kodama))
        .and_then(|item| item.trigger_use_key.as_ref())
        .expect("exact Kodama ability use is retained on the stack")
        .clone();
    assert_eq!(original_use.zone_change_generation, original_generation);

    move_kodama_with_dev(&mut engine, v1::DevZone::Graveyard);
    assert_eq!(engine.state.objects[&kodama].zone, Zone::Graveyard);
    move_kodama_with_dev(&mut engine, v1::DevZone::Battlefield);
    let new_generation = engine.state.zone_change_generation[&kodama];
    assert!(new_generation > original_generation);

    let choice = resolve_kodama_to_hand_choice(&mut engine);
    assert_eq!((choice.min, choice.max), (0, 1));
    let land = choice
        .candidate_object_ids
        .iter()
        .copied()
        .find(|object_id| selectable(&choice, *object_id))
        .expect("a permanent remains eligible");
    engine
        .apply_command(0, &submit_resolution_choice(vec![land]))
        .expect("resolve the older Kodama trigger");
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert!(
        engine.state.stack.iter().any(|item| {
            item.source_permanent_id == Some(kodama)
                && item.trigger_use_key.as_ref().is_some_and(|key| {
                    key.zone_change_generation == new_generation && *key != original_use
                })
        }),
        "the returned Kodama sees the entry caused by its older ability generation"
    );
}

fn move_kodama_with_dev(engine: &mut GameEngine, zone: v1::DevZone) {
    use v1::{dev_command, DevCommand, DevMoveCard, RuledCommand};

    let player = engine.state.players[0].id;
    engine.enable_dev_commands();
    engine
        .apply_command(
            player,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Kodama of the East Tree".into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move the same physical Kodama card");
}
