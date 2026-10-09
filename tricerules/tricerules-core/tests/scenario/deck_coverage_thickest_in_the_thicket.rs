//! Actual-card coverage for Thickest in the Thicket.
//!
//! Oracle and both card-specific rulings are from the official Bloomburrow release notes.
//! CR 107.1b, 208.5, 603.2, 608.2b/h, and 613.4c govern signed power, trigger timing, target
//! identity, resolution-time information, and derived power.

use super::helpers::*;
use tricerules_cards::primitives::{
    Amount, CountExpression, EffectSubject, GameCondition, PlayerRecipient, SpellEffectKind,
    TargetKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CounterKind, Layout, ManaCost};
use tricerules_core::state::CopiableValues;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChooseTriggerTarget, DevCommand, DevMoveCard, DevZone,
    RuledCommand,
};

const THICKEST: &str = "thickest_in_the_thicket";

fn four_player_engine(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2, 3],
        20,
        Some(vec![deck; 4]),
        true,
    )
    .expect("new four-player game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_thickest(engine: &mut GameEngine, player: i32) -> u32 {
    let player_index = engine.state.player_idx(player).expect("casting player");
    let object = inject_card_into_hand(engine, player_index, THICKEST);
    grant_pool(engine, player_index);
    let slot = hand_index_for_card(engine, player_index, THICKEST);
    engine
        .apply_command(player, &cast_spell(slot, Vec::new()))
        .expect("cast Thickest in the Thicket");
    object
}

fn choose_entry_target(engine: &mut GameEngine, target: u32) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: target_object(target),
                    ..Default::default()
                })),
            },
        )
        .expect("choose the mandatory creature target");
}

fn dev_move_owned_card(engine: &mut GameEngine, owner: i32, card_name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move the requested card");
}

fn advance_to_end_step(engine: &mut GameEngine) {
    assert_eq!(engine.state.active_player_id(), 0);
    for _ in 0..12 {
        if engine.state.turn_step == TurnStep::EndStep {
            return;
        }
        pass_priority_round(engine);
    }
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
}

fn set_test_creature_power(engine: &mut GameEngine, object: u32, power: i32) {
    let permanent = engine.state.objects.get_mut(&object).expect("creature");
    permanent.power = None;
    permanent.toughness = None;
    let mut face = tricerules_cards::registry::global()
        .get("grizzly_bears")
        .expect("Grizzly Bears definition")
        .primary_face()
        .clone();
    face.power = Some(0);
    face.toughness = Some(5);
    permanent.copiable_values = Some(CopiableValues {
        source_card_id: "grizzly_bears".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Test high-toughness creature".into(),
    });
    permanent.counters.remove(&CounterKind::PlusOnePlusOne);
    permanent.counters.remove(&CounterKind::MinusOneMinusOne);
    if power > 0 {
        permanent
            .counters
            .insert(CounterKind::PlusOnePlusOne, power as u32);
    } else if power < 0 {
        permanent
            .counters
            .insert(CounterKind::MinusOneMinusOne, power.unsigned_abs());
    }
    assert_eq!(
        engine
            .characteristics(object)
            .expect("test creature characteristics")
            .power,
        Some(power.max(0) as u32),
    );
}

fn set_test_creature_power_undefined(engine: &mut GameEngine, object: u32) {
    let permanent = engine.state.objects.get_mut(&object).expect("creature");
    permanent.power = None;
    permanent.toughness = None;
    let mut face = tricerules_cards::registry::global()
        .get("grizzly_bears")
        .expect("Grizzly Bears definition")
        .primary_face()
        .clone();
    face.power = None;
    face.toughness = Some(5);
    permanent.copiable_values = Some(CopiableValues {
        source_card_id: "grizzly_bears".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Test creature without a power value".into(),
    });
    permanent.counters.remove(&CounterKind::PlusOnePlusOne);
    permanent.counters.remove(&CounterKind::MinusOneMinusOne);
    assert_eq!(
        engine
            .characteristics(object)
            .expect("test creature characteristics")
            .power,
        Some(0),
        "CR 208.5 makes a creature without a power value power 0"
    );
}

#[test]
fn thickest_has_its_complete_single_face_and_exact_trigger_definitions() {
    let card = tricerules_cards::registry::global()
        .get(THICKEST)
        .expect("Thickest in the Thicket is registered");
    assert_eq!(card.name, "Thickest in the Thicket");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost, ManaCost::parse("{3}{G}{G}").unwrap());
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.triggered_abilities.len(), 2);

    let entry = &face.triggered_abilities[0];
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        entry.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    ));
    assert!(entry.intervening_if.is_none());
    let group = &entry
        .targeting
        .as_ref()
        .expect("mandatory creature target")
        .groups[0];
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert!(matches!(
        entry.effect.as_slice(),
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Count(CountExpression::ChosenTargetPower {
                group_index: 0,
                target_index: 0,
            }),
            subject: EffectSubject::Chosen(filter),
        }] if filter.kind == TargetKind::Creature
    ));

    let end_step = &face.triggered_abilities[1];
    assert_eq!(
        end_step.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        end_step.trigger,
        TriggerCondition::AtBeginningOfEndStep {
            player: tricerules_cards::primitives::CastTriggerPlayer::Controller
        }
    ));
    assert!(end_step.intervening_if.is_none());
    assert!(matches!(
        end_step.effect.as_slice(),
        [SpellEffectKind::Conditional {
            condition: GameCondition::ControlsCreatureTiedForGreatestPower,
            effect,
        }] if matches!(
            effect.as_ref(),
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(2),
            }
        )
    ));
}

#[test]
fn entry_trigger_reads_current_target_power_and_resolves_after_its_source_leaves() {
    let mut engine = four_player_engine(20_261_008);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let source = cast_thickest(&mut engine, 0);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    choose_entry_target(&mut engine, target);

    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target creature")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 2);
    assert_eq!(engine.effective_power(target), Some(4));
    dev_move_owned_card(
        &mut engine,
        0,
        "Thickest in the Thicket",
        DevZone::Graveyard,
    );
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        6,
        "the ability reads power 4 at resolution and adds four counters to the two already present"
    );
    engine.state.objects.get_mut(&target).unwrap().power = Some(10);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        6,
        "later power changes do not recalculate a resolved counter amount"
    );
}

#[test]
fn entry_trigger_places_no_counters_for_a_negative_target_power() {
    let mut engine = four_player_engine(20_261_009);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    set_test_creature_power(&mut engine, target, -1);
    cast_thickest(&mut engine, 0);
    pass_priority_round(&mut engine);
    choose_entry_target(&mut engine, target);
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(target), Some(0));
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "CR 107.1b uses zero when a counter count would be negative"
    );
}

#[test]
fn entry_trigger_fizzles_when_its_target_leaves_and_returns() {
    let mut engine = four_player_engine(20_261_010);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_thickest(&mut engine, 0);
    pass_priority_round(&mut engine);
    choose_entry_target(&mut engine, target);
    // Direct scenario fixture insertion bypasses normal object-generation initialization.
    let selected_generation = *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_insert(0);

    dev_move_owned_card(&mut engine, 1, "Grizzly Bears", DevZone::Graveyard);
    dev_move_owned_card(&mut engine, 1, "Grizzly Bears", DevZone::Battlefield);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert!(engine.state.zone_change_generation[&target] > selected_generation);
    pass_priority_round(&mut engine);

    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "the returned object generation is not the target selected by the trigger"
    );
    assert!(engine.state.stack.is_empty());
}

fn end_step_draws_for_powers(seed: u64, powers: &[(usize, i32)]) -> bool {
    let mut engine = four_player_engine(seed);
    inject_permanent_on_battlefield(&mut engine, 0, THICKEST);
    for &(player, power) in powers {
        let creature = inject_creature_on_battlefield(&mut engine, player, "grizzly_bears");
        set_test_creature_power(&mut engine, creature, power);
    }
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
    assert_eq!(engine.state.stack.len(), 1, "the end-step trigger occurs");
    let library_before = engine.state.players[0].library.len();
    pass_priority_round(&mut engine);
    library_before - engine.state.players[0].library.len() == 2
}

#[test]
fn end_step_trigger_is_unconditional_and_checks_the_maximum_at_resolution() {
    let mut engine = four_player_engine(20_261_011);
    inject_permanent_on_battlefield(&mut engine, 0, THICKEST);
    let controller_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    set_test_creature_power(&mut engine, controller_creature, 2);
    set_test_creature_power(&mut engine, opponent_creature, 5);
    advance_to_end_step(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "another player's greater power does not suppress the trigger"
    );

    set_test_creature_power(&mut engine, controller_creature, 5);
    let library_before = engine.state.players[0].library.len();
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 2,
        "the resolving condition sees the newly tied greatest power"
    );
}

#[test]
fn end_step_does_not_draw_when_an_opponent_becomes_the_unique_power_leader() {
    let mut engine = four_player_engine(20_261_012);
    inject_permanent_on_battlefield(&mut engine, 0, THICKEST);
    let controller_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 3, "grizzly_bears");
    set_test_creature_power(&mut engine, controller_creature, 5);
    set_test_creature_power(&mut engine, opponent_creature, 3);
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);

    set_test_creature_power(&mut engine, opponent_creature, 6);
    let library_before = engine.state.players[0].library.len();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].library.len(), library_before);
}

#[test]
fn greatest_power_check_compares_signed_values_across_four_players() {
    assert!(end_step_draws_for_powers(
        20_261_013,
        &[(0, -1), (1, -2), (2, -3)],
    ));
    assert!(!end_step_draws_for_powers(
        20_261_014,
        &[(0, -2), (1, -1), (2, -3)],
    ));
    assert!(end_step_draws_for_powers(
        20_261_015,
        &[(0, -2), (1, -2), (2, -3)],
    ));
}

#[test]
fn greatest_power_check_is_false_with_no_battlefield_creatures() {
    assert!(!end_step_draws_for_powers(20_261_016, &[]));
}

#[test]
fn greatest_power_check_treats_undefined_creature_power_as_zero() {
    let mut engine = four_player_engine(20_261_017);
    inject_permanent_on_battlefield(&mut engine, 0, THICKEST);
    let controller_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    set_test_creature_power_undefined(&mut engine, controller_creature);
    set_test_creature_power(&mut engine, opponent_creature, -1);

    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "the end-step trigger occurs");
    let library_before = engine.state.players[0].library.len();
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 2,
        "CR 208.5 gives an undefined creature power of zero for the greatest-power comparison"
    );
}
