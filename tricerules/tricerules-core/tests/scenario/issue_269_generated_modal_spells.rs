//! Issue #269 — the second exact generated modal-spell cohort.
//!
//! Oracle and rulings were checked 2026-09-13. CR 115.8, 601.2b-d, 608.2b-c, and 700.2
//! govern cast-time mode and target choices, target revalidation, and printed resolution order.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{state::PlayerState, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChoiceKind, DevCommand, DevMoveCard, DevZone, RuledCommand,
    TargetRef,
};

fn modal_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn modal_engine_with_third_player(seed: u64) -> GameEngine {
    let mut engine = modal_engine(seed);
    // Session admission is still M2; append a fixture-only third player to exercise the generic
    // relative-player resolver, matching the in-engine mass-effect multiplayer coverage.
    engine.state.players.push(PlayerState::new(2, 20));
    engine
}

fn fund_modal_spell(engine: &mut GameEngine) {
    give_mana(
        engine,
        0,
        ManaGift {
            w: 4,
            u: 4,
            r: 4,
            g: 4,
            b: 4,
            c: 4,
        },
    );
}

fn prepare_spell(engine: &mut GameEngine, card_id: &str) -> usize {
    inject_card_into_hand(engine, 0, card_id);
    fund_modal_spell(engine);
    hand_index_for_card(engine, 0, card_id)
}

fn dev_move_card(engine: &mut GameEngine, target_player_id: i32, card_name: &str, zone: DevZone) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("dev move");
}

fn target_in_group(object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        ..Default::default()
    }
}

#[test]
fn overwhelming_surge_supports_both_modes_with_local_targets_and_revalidation() {
    let mut engine = modal_engine(269_001);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let slot = prepare_spell(&mut engine, "overwhelming_surge");

    assert!(
        engine
            .apply_command(
                0,
                &cast_modal_spell(
                    slot,
                    vec![(0, target_object(artifact)), (1, target_object(creature))],
                ),
            )
            .is_err(),
        "each selected mode must enforce its own target filter"
    );

    engine
        .apply_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(0, target_object(creature)), (1, target_object(artifact))],
            ),
        )
        .expect("choose both reviewed Overwhelming Surge modes");

    engine.enable_dev_commands();
    let cast_generation = engine
        .state
        .zone_change_generation
        .get(&artifact)
        .copied()
        .unwrap_or(0);
    dev_move_card(&mut engine, 1, "Short Sword", DevZone::Hand);
    let hand_generation = engine.state.zone_change_generation[&artifact];
    assert!(hand_generation > cast_generation);
    dev_move_card(&mut engine, 1, "Short Sword", DevZone::Battlefield);
    let returned_generation = engine.state.zone_change_generation[&artifact];
    assert!(returned_generation > hand_generation);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&artifact],
        returned_generation,
        "the same physical object returned as a new generation must not be destroyed by the stale target"
    );

    let mut first_only = modal_engine(269_007);
    let first_creature = inject_creature_on_battlefield(&mut first_only, 1, "grizzly_bears");
    let first_slot = prepare_spell(&mut first_only, "overwhelming_surge");
    first_only
        .apply_command(
            0,
            &cast_modal_spell(first_slot, vec![(0, target_object(first_creature))]),
        )
        .expect("Choose one or both permits selecting only the first mode");
    resolve_entire_stack_two_player(&mut first_only);
    assert_eq!(
        first_only.state.objects[&first_creature].zone,
        Zone::Graveyard
    );

    let mut second_only = modal_engine(269_008);
    let second_artifact = inject_permanent_on_battlefield(&mut second_only, 1, "short_sword");
    let second_slot = prepare_spell(&mut second_only, "overwhelming_surge");
    second_only
        .apply_command(
            0,
            &cast_modal_spell(second_slot, vec![(1, target_object(second_artifact))]),
        )
        .expect("Choose one or both permits selecting only the second mode");
    resolve_entire_stack_two_player(&mut second_only);
    assert_eq!(
        second_only.state.objects[&second_artifact].zone,
        Zone::Graveyard
    );
}

#[test]
fn issue_449_iroh_targeted_mode_preserves_source_and_creature_only_damage() {
    use super::helpers::semantic;
    use tricerules_proto::ruled::v1::ruled_event::Ev;

    let mut engine = modal_engine(449_001);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let target = inject_creature_with_stats(&mut engine, 1, "hill_giant", 5, 5);
    let other = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let slot = prepare_spell(&mut engine, "irohs_demonstration");
    let source = engine.state.players[0].hand[slot];
    let generation = semantic::generation(&engine, source);
    for illegal in [target_object(artifact), target_player(1)] {
        assert!(engine
            .apply_command(0, &cast_modal_spell(slot, vec![(1, illegal)]))
            .is_err());
    }
    let cast = semantic::accepted(
        &mut engine,
        0,
        &cast_modal_spell(slot, vec![(1, target_object(target))]),
    );
    assert!(cast.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackPushed(pushed)) if pushed.object_id == source && pushed.card_id == "irohs_demonstration")));
    semantic::complete(&mut engine, 8, |_| None).require_exercised();
    semantic::assert_object(
        &engine,
        source,
        "irohs_demonstration",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].damage, 4);
    for untouched in [own, other, artifact] {
        assert_eq!(engine.state.objects[&untouched].damage, 0);
        assert_eq!(engine.state.objects[&untouched].zone, Zone::Battlefield);
    }
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn irohs_demonstration_damage_is_limited_to_opponent_creatures() {
    let mut engine = modal_engine(269_002);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let slot = prepare_spell(&mut engine, "irohs_demonstration");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, vec![])]))
        .expect("choose the untargeted opponent-creature mode");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&own].damage, 0);
    assert_eq!(engine.state.objects[&opposing].damage, 1);
}

#[test]
fn irohs_demonstration_damages_all_and_only_opponent_creatures_in_multiplayer() {
    let mut engine = modal_engine_with_third_player(269_009);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let first_opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second_opponent = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let slot = prepare_spell(&mut engine, "irohs_demonstration");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, vec![])]))
        .expect("choose the untargeted opponent-creature mode");
    resolve_entire_stack_three_player(&mut engine);

    assert_eq!(engine.state.objects[&own].damage, 0);
    assert_eq!(engine.state.objects[&first_opponent].damage, 1);
    assert_eq!(engine.state.objects[&second_opponent].damage, 1);
}

#[test]
fn giantfall_and_valorous_stance_keep_exact_source_and_toughness_filters() {
    let mut engine = modal_engine(269_003);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.objects.get_mut(&opposing).unwrap().toughness = Some(4);
    let giantfall = prepare_spell(&mut engine, "giantfall");
    engine
        .apply_command(
            0,
            &cast_modal_spell(
                giantfall,
                vec![(
                    0,
                    vec![target_object(source)[0], target_in_group(opposing, 1)],
                )],
            ),
        )
        .expect("choose source and opposing creature for Giantfall");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].damage, 0);
    assert_eq!(engine.state.objects[&opposing].damage, 2);

    let mut stance = modal_engine(269_004);
    let small = inject_creature_on_battlefield(&mut stance, 1, "grizzly_bears");
    let large = inject_creature_on_battlefield(&mut stance, 1, "grizzly_bears");
    stance.state.objects.get_mut(&large).unwrap().toughness = Some(4);
    let stance_slot = prepare_spell(&mut stance, "valorous_stance");
    assert!(
        stance
            .apply_command(
                0,
                &cast_modal_spell(stance_slot, vec![(1, target_object(small))])
            )
            .is_err(),
        "toughness four or greater must not use the power or default filter"
    );
    stance
        .apply_command(
            0,
            &cast_modal_spell(stance_slot, vec![(1, target_object(large))]),
        )
        .expect("target the four-toughness creature");
    resolve_entire_stack_two_player(&mut stance);
    assert_eq!(stance.state.objects[&large].zone, Zone::Graveyard);
    assert_eq!(stance.state.objects[&small].zone, Zone::Battlefield);
}

#[test]
fn origin_of_metalbending_applies_both_ordered_effects_to_one_target() {
    let mut engine = modal_engine(269_005);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let slot = prepare_spell(&mut engine, "origin_of_metalbending");
    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_object(creature))]),
        )
        .expect("choose Origin's counter-and-indestructible mode");
    let resolved = resolve_top_stack(&mut engine);

    let log_index = |needle: &str| {
        resolved
            .events
            .iter()
            .position(|event| {
                matches!(
                    &event.ev,
                    Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                        if log.text.contains(needle)
                )
            })
            .unwrap_or_else(|| panic!("missing resolution log containing {needle:?}"))
    };
    assert!(
        log_index("puts 1 +1/+1 counter") < log_index("grants Indestructible"),
        "Origin must emit its counter effect before its keyword effect"
    );

    assert_eq!(
        engine.state.objects[&creature]
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert!(engine.effective_has_keyword(creature, Keyword::Indestructible));
}

fn resolve_entire_stack_three_player(engine: &mut GameEngine) {
    loop {
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("pass priority");
        if engine.state.stack.is_empty() {
            return;
        }
    }
}

#[test]
fn seekers_folly_uses_a_private_opponent_hand_choice() {
    let mut engine = modal_engine(269_006);
    let first = inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    let second = inject_card_into_hand(&mut engine, 1, "island");
    let slot = prepare_spell(&mut engine, "seekers_folly");
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_player(1))]))
        .expect("target opponent for Seeker's Folly");
    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("private discard choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!((choice.min, choice.max), (2, 2));
    assert!(choice.public_reveal.is_none());
    assert!(choice.candidate_object_ids.contains(&first));
    assert!(choice.candidate_object_ids.contains(&second));

    engine
        .apply_command(1, &submit_resolution_choice(vec![first, second]))
        .expect("opponent chooses two cards to discard");
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}
