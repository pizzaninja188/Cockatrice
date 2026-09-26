//! Actual-card coverage for Nevinyrral's Disk.
//!
//! Scryfall Oracle and rulings checked 2026-09-26. CR 603.6d covers its enters-tapped static
//! effect during entry; CR 602.2a-b covers the activation cost; CR 113.7a covers the activated
//! ability existing independently on the stack; CR 701.8 covers destruction; and CR 702.12b
//! covers indestructible permanents surviving it.

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{GameEngine, TurnStep, Zone};

const DISK: &str = "nevinyrrals_disk";

fn disk_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.priority_player_id(), 0);
    engine
}

#[test]
fn nevinyrrals_disk_enters_tapped_rejects_tapped_activation_then_sweeps_matching_permanents() {
    let mut engine = disk_engine(20_260_927);
    let disk_in_hand = inject_card_into_hand(&mut engine, 0, DISK);
    grant_pool(&mut engine, 0);

    let disk_slot = hand_index_for_card(&engine, 0, DISK);
    let cast_disk = cast_spell(disk_slot, vec![]);
    semantic::accepted(&mut engine, 0, &cast_disk);
    resolve_entire_stack_two_player(&mut engine);

    let disk = battlefield_object_for_card(&engine, 0, DISK);
    assert_eq!(disk, disk_in_hand);
    assert_eq!(engine.state.objects[&disk].zone, Zone::Battlefield);
    assert!(
        engine.state.objects[&disk].tapped,
        "the artifact's entry replacement sets it tapped"
    );
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(engine.state.priority_player_id(), 0);
    assert_eq!(zone_view_ability_flags(&mut engine, 0, disk), vec![false]);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let mana_before_rejected_activation = engine.state.players[0].mana_pool;
    engine
        .apply_command(0, &activate_ability_for(&engine, disk, 0, vec![]))
        .expect_err("a tapped Disk cannot pay its tap activation cost");
    assert_eq!(
        engine.state.players[0].mana_pool, mana_before_rejected_activation,
        "an illegal activation pays no mana"
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.objects[&disk].tapped);

    // Reach a real untap step before the legal activation; this keeps the activation and its tap
    // cost on the engine command path rather than changing the source's state as test setup.
    end_active_turn(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 1);
    end_active_turn(&mut engine, 1);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert!(!engine.state.objects[&disk].tapped);

    let own_artifact = inject_permanent_on_battlefield(&mut engine, 0, "bonesplitter");
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing_enchantment = inject_permanent_on_battlefield(&mut engine, 1, "bad_moon");
    let indestructible_artifact_creature =
        inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    let own_land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let opposing_planeswalker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&opposing_planeswalker)
        .expect("Jace on battlefield")
        .set_counter(CounterKind::Loyalty, 3);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let activation = activate_ability_for(&engine, disk, 0, vec![]);
    semantic::accepted(&mut engine, 0, &activation);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(engine.state.priority_player_id(), 0);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the Disk ability is on the stack"
    );
    assert_eq!(engine.state.objects[&disk].zone, Zone::Battlefield);
    assert!(
        engine.state.objects[&disk].tapped,
        "the tap activation cost taps the Disk"
    );

    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.stack.is_empty());
    for object_id in [disk, own_artifact, own_creature, opposing_enchantment] {
        assert_eq!(
            engine.state.objects[&object_id].zone,
            Zone::Graveyard,
            "matching artifact, creature, enchantment, and the source are destroyed"
        );
    }
    assert_eq!(
        engine.state.objects[&indestructible_artifact_creature].zone,
        Zone::Battlefield,
        "an indestructible artifact creature matches the union once and survives"
    );
    assert_eq!(
        engine.state.objects[&own_land].zone,
        Zone::Battlefield,
        "a pure land is outside the filter"
    );
    assert_eq!(
        engine.state.objects[&opposing_planeswalker].zone,
        Zone::Battlefield,
        "a planeswalker is outside the filter"
    );
}
