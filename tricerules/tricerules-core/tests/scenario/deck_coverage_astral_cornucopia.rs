//! Actual-card coverage for Astral Cornucopia's X-based charge counters and scaling mana ability.
//!
//! The exact SOC printing and rulings endpoint were checked 2026-09-26. The 2014-02-01 ruling
//! confirms X=1 costs three mana and enters with one Charge counter, X=2 costs six and enters
//! with two, and the last ability is a mana ability that uses no stack. CR 107.3m and 122.6 govern
//! its entry counters; CR 605.1a, 605.2, and 605.3b govern its mana ability.

use super::helpers::*;
use tricerules_cards::primitives::{EntersWithCountersAffected, StaticAbilityDef};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, Amount, CardRegistry, CounterKind, Layout,
    ManaAmount, SpellEffectKind,
};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn mana_pool(engine: &GameEngine) -> ManaPool {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn cornucopia_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_cornucopia_x(engine: &mut GameEngine, x: u32, generic_mana_cost: u32) -> u32 {
    let source = inject_card_into_hand(engine, 0, "astral_cornucopia");
    let generation = semantic::generation(engine, source);
    give_mana(
        engine,
        0,
        ManaGift {
            c: generic_mana_cost,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(engine, 0, "astral_cornucopia");
    semantic::accepted(engine, 0, &cast_spell_x(hand_index, vec![], x));

    let stack_item = engine.state.stack.last().expect("Cornucopia on stack");
    assert_eq!(stack_item.card_id, "astral_cornucopia");
    assert_eq!(stack_item.chosen_x, x);
    assert_eq!(mana_pool(engine), (0, 0, 0, 0, 0, 0));

    semantic::complete(engine, 8, |_| None).require_exercised();
    semantic::assert_object(
        engine,
        source,
        "astral_cornucopia",
        0,
        0,
        Zone::Battlefield,
        generation + 2,
    );
    source
}

fn activated_ability_info(
    engine: &mut GameEngine,
    source: u32,
) -> tricerules_proto::ruled::v1::AbilityInfo {
    let batch = engine.initial_response_batch();
    batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == source)
                .and_then(|object| object.activated_abilities.first())
                .cloned(),
            _ => None,
        })
        .expect("Cornucopia publishes its mana ability")
}

fn activate_with_mana_option(engine: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn astral_cornucopia_registers_exact_identity_and_typed_abilities() {
    let card = CardRegistry::global()
        .get("astral_cornucopia")
        .expect("reviewed Astral Cornucopia definition");
    assert_eq!(card.id, "astral_cornucopia");
    assert_eq!(card.name, "Astral Cornucopia");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "astral_cornucopia");
    assert_eq!(face.name, "Astral Cornucopia");
    assert_eq!(face.mana_cost.to_string(), "{X}{X}{X}");
    assert_eq!(face.types, vec!["Artifact".to_string()]);
    assert!(face.colors().is_empty(), "the face is colorless");
    assert!(face.power.is_none());
    assert!(face.toughness.is_none());
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(face.activated_abilities.len(), 1);

    let entry = &face.static_abilities[0];
    assert_eq!(entry.ability_id.as_str(), "static_01");
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    match &entry.definition {
        StaticAbilityDef::EntersWithCounters {
            affected: EntersWithCountersAffected::Self_,
            counter,
            amount,
            cast_cost_condition,
            ..
        } => {
            assert_eq!(*counter, CounterKind::Charge);
            assert_eq!(amount, &Amount::X);
            assert!(cast_cost_condition.is_none());
        }
        other => panic!("unexpected entry replacement: {other:?}"),
    }

    let ability = &face.activated_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(ability.costs, vec![AbilityCost::Tap]);
    match ability.effect.as_slice() {
        [SpellEffectKind::ProduceManaPerSourceCounter { counter, options }] => {
            assert_eq!(*counter, CounterKind::Charge);
            assert_eq!(
                options.as_slice(),
                &[
                    ManaAmount {
                        w: 1,
                        ..Default::default()
                    },
                    ManaAmount {
                        u: 1,
                        ..Default::default()
                    },
                    ManaAmount {
                        b: 1,
                        ..Default::default()
                    },
                    ManaAmount {
                        r: 1,
                        ..Default::default()
                    },
                    ManaAmount {
                        g: 1,
                        ..Default::default()
                    },
                ]
            );
        }
        other => panic!("unexpected mana ability effect: {other:?}"),
    }
}

#[test]
fn astral_cornucopia_casts_x_zero_one_two_and_enters_with_chosen_charge() {
    for (seed, x, generic_mana_cost, expected_charge) in
        [(342_001, 0, 0, 0), (342_002, 1, 3, 1), (342_003, 2, 6, 2)]
    {
        let mut engine = cornucopia_engine(seed);
        let source = cast_cornucopia_x(&mut engine, x, generic_mana_cost);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            expected_charge,
            "X={x} enters with {expected_charge} Charge counters"
        );
    }
}

#[test]
fn astral_cornucopia_outputs_two_mana_in_each_chosen_color_without_stack() {
    let choices = [
        (0, (2, 0, 0, 0, 0, 0)),
        (1, (0, 2, 0, 0, 0, 0)),
        (2, (0, 0, 2, 0, 0, 0)),
        (3, (0, 0, 0, 2, 0, 0)),
        (4, (0, 0, 0, 0, 2, 0)),
    ];

    for (index, (option, expected_pool)) in choices.into_iter().enumerate() {
        let mut engine = cornucopia_engine(342_010 + index as u64);
        let source = cast_cornucopia_x(&mut engine, 2, 6);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            2
        );
        let published = activated_ability_info(&mut engine, source);
        assert_eq!(published.mana_produced, "WW/UU/BB/RR/GG");
        assert!(published.mana_option_labels.is_empty());

        let command = activate_with_mana_option(&engine, source, option);
        semantic::accepted(&mut engine, 0, &command);

        assert_eq!(mana_pool(&engine), expected_pool, "color option {option}");
        assert!(engine.state.objects[&source].tapped);
        assert!(engine.state.stack.is_empty(), "mana ability uses no stack");
        assert!(engine.state.pending_triggers.is_empty());
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn astral_cornucopia_output_uses_live_charge_counter_count() {
    let mut engine = cornucopia_engine(342_020);
    let source = cast_cornucopia_x(&mut engine, 2, 6);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        2
    );

    // Seed a later game state in which another effect has changed this permanent's counter count.
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("Cornucopia permanent")
        .set_counter(CounterKind::Charge, 3);
    let published = activated_ability_info(&mut engine, source);
    assert_eq!(published.mana_produced, "WWW/UUU/BBB/RRR/GGG");
    assert!(published.mana_option_labels.is_empty());
    let command = activate_with_mana_option(&engine, source, 0);
    semantic::accepted(&mut engine, 0, &command);

    assert_eq!(mana_pool(&engine), (3, 0, 0, 0, 0, 0));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        3
    );
    assert!(engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn astral_cornucopia_with_zero_counters_taps_for_no_mana_without_stack() {
    let mut engine = cornucopia_engine(342_021);
    let source = cast_cornucopia_x(&mut engine, 0, 0);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    let published = activated_ability_info(&mut engine, source);
    assert!(published.mana_produced.is_empty());
    assert_eq!(published.mana_option_labels, ["W", "U", "B", "R", "G"]);
    let command_index_before = engine.state.command_index;

    let command = activate_with_mana_option(&engine, source, 4);
    semantic::accepted(&mut engine, 0, &command);

    assert_eq!(engine.state.command_index, command_index_before + 1);
    assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 0));
    assert!(engine.state.objects[&source].tapped, "the tap cost is paid");
    assert!(engine.state.stack.is_empty(), "mana ability uses no stack");
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn astral_cornucopia_rejects_invalid_mana_option_without_mutation() {
    let mut engine = cornucopia_engine(342_022);
    let source = cast_cornucopia_x(&mut engine, 2, 6);
    let command_index_before = engine.state.command_index;
    let invalid = activate_with_mana_option(&engine, source, 5);

    let error = engine
        .apply_command(0, &invalid)
        .expect_err("only the five color options are available");
    assert!(matches!(
        error,
        tricerules_core::EngineError::Illegal("invalid mana option")
    ));

    assert_eq!(engine.state.command_index, command_index_before);
    assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 0));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
