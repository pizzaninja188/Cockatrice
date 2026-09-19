//! Issue #413 — the reminder-text Cycling cohort's real generated identities.
//!
//! CR 702.29a defines Cycling as a hand-only activated ability paid with mana and discarding the
//! card, followed by a draw. The reminder printing appends the fixed explanation as part of the
//! same Oracle line. CR 115.1 / 701.8 govern the union destruction, CR 120 the mass damage,
//! CR 208 / 608.2b the toughness recheck, and CR 122.1d / 701.26 the tap plus stun counters.

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

fn issue_413_hand_ability(engine: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

fn issue_413_engine(seed: u64, specials: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", specials),
        deck_with("mountain", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("issue #413 engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn issue_413_reminder_cycling_discards_and_draws_at_the_printed_cost() {
    for (offset, card_id) in [
        "airship_crash",
        "fuel_the_flames",
        "gallant_strike",
        "stall_out",
    ]
    .iter()
    .enumerate()
    {
        let mut engine = issue_413_engine(413_001 + offset as u64, &[card_id]);
        ensure_in_hand(&mut engine, 0, card_id);
        let source = engine.state.players[0].hand[hand_index_for_card(&engine, 0, card_id)];
        let drawn = inject_library_card(&mut engine, 0, "grizzly_bears");
        engine.state.players[0].library.retain(|oid| *oid != drawn);
        engine.state.players[0].library.push_front(drawn);

        let command = issue_413_hand_ability(&engine, source);
        engine
            .apply_command(0, &command)
            .expect_err("the printed {2} cost must be payable");
        assert_eq!(engine.state.objects[&source].zone, Zone::Hand);

        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        engine
            .apply_command(0, &command)
            .expect("activate reminder-form cycling");
        assert_eq!(
            engine.state.objects[&source].zone,
            Zone::Graveyard,
            "{card_id} discards itself as a cost"
        );
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&drawn].zone,
            Zone::Hand,
            "{card_id} draws exactly one card"
        );
    }
}

#[test]
fn issue_413_airship_crash_destroys_artifact_enchantment_or_flying_creature() {
    for (offset, (card_id, is_creature)) in [
        ("short_sword", false),
        ("anthem_of_champions", false),
        ("air_elemental", true),
    ]
    .iter()
    .enumerate()
    {
        let mut engine = issue_413_engine(413_010 + offset as u64, &["airship_crash"]);
        let target = if *is_creature {
            inject_creature_on_battlefield(&mut engine, 1, card_id)
        } else {
            inject_permanent_on_battlefield(&mut engine, 1, card_id)
        };
        ensure_in_hand(&mut engine, 0, "airship_crash");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                g: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "airship_crash");
        engine
            .apply_command(0, &cast_spell(slot, target_object(target)))
            .unwrap_or_else(|error| panic!("{card_id} is a legal union target: {error}"));
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&target].zone,
            Zone::Graveyard,
            "{card_id} must be destroyed"
        );
    }

    // A creature without flying is not inside the union.
    let mut engine = issue_413_engine(413_019, &["airship_crash"]);
    let grounded = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "airship_crash");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "airship_crash");
    engine
        .apply_command(0, &cast_spell(slot, target_object(grounded)))
        .expect_err("a grounded creature is not a legal union target");
    assert_eq!(engine.state.objects[&grounded].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
}

#[test]
fn issue_413_fuel_the_flames_deals_two_to_every_creature() {
    let mut engine = issue_413_engine(413_020, &["fuel_the_flames"]);
    let small = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    let big = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 3, 3);
    ensure_in_hand(&mut engine, 0, "fuel_the_flames");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "fuel_the_flames");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast the untargeted sweep");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&small].zone,
        Zone::Graveyard,
        "the 2/2 dies from the two damage"
    );
    assert_eq!(
        engine.state.objects[&big].zone,
        Zone::Battlefield,
        "the 3/3 survives"
    );
    assert_eq!(engine.state.objects[&big].damage, 2);
}

#[test]
fn issue_413_gallant_strike_destroys_only_toughness_four_or_greater() {
    let mut engine = issue_413_engine(413_030, &["gallant_strike"]);
    let small = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 5, 3);
    ensure_in_hand(&mut engine, 0, "gallant_strike");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "gallant_strike");
    engine
        .apply_command(0, &cast_spell(slot, target_object(small)))
        .expect_err("toughness 3 is not a legal target");
    assert_eq!(engine.state.objects[&small].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].mana_pool.white, 1);

    let big = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 1, 6);
    engine
        .apply_command(0, &cast_spell(slot, target_object(big)))
        .expect("toughness 6 is a legal target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&big].zone, Zone::Graveyard);
}

#[test]
fn issue_413_stall_out_taps_and_stuns_one_creature_or_vehicle_target() {
    let mut engine = issue_413_engine(413_040, &["stall_out", "stall_out"]);
    let creature = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 4, 4);
    let vehicle = inject_permanent_on_battlefield(&mut engine, 1, "cultivators_caravan");
    let sword = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    ensure_in_hand(&mut engine, 0, "stall_out");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "stall_out");

    engine
        .apply_command(0, &cast_spell(slot, target_object(sword)))
        .expect_err("a plain artifact is not a creature or Vehicle");
    engine
        .apply_command(0, &cast_spell(slot, target_object(creature)))
        .expect("target the creature");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&creature].tapped);
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Stun),
        3
    );
    assert!(!engine.state.objects[&vehicle].tapped);

    ensure_in_hand(&mut engine, 0, "stall_out");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "stall_out");
    engine
        .apply_command(0, &cast_spell(slot, target_object(vehicle)))
        .expect("target the Vehicle");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&vehicle].tapped);
    assert_eq!(
        engine.state.objects[&vehicle].counter_count(CounterKind::Stun),
        3
    );

    // CR 122.1d: the untap step removes one stun counter instead of untapping.
    engine.state.turn_step = TurnStep::EndStep;
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;
    let _ = engine.initial_response_batch();
    engine
        .apply_command(0, &primitive_yield())
        .expect("roll into the opponent's untap step");
    for target in [creature, vehicle] {
        assert!(
            engine.state.objects[&target].tapped,
            "stun counters replace the untap"
        );
        assert_eq!(
            engine.state.objects[&target].counter_count(CounterKind::Stun),
            2,
            "exactly one stun counter is removed"
        );
    }
}
