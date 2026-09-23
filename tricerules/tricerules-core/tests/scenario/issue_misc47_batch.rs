//! Batch 49: three pinned Standard spells with a shared uncounterable contract.
//!
//! Exact pinned Scryfall records and official Murders at Karlov Manor rulings were reviewed for
//! Long Goodbye, Slice from the Shadows, and Suspicious Detonation. The shared ruling is that a
//! target's Ward trigger may be paid, but declining Ward does not counter these spells. Governed
//! rules: CR 107.3, 115, 120.3, 202.3, 601.2f, 613.4, 701.6, 701.8, 702.21, and 704.5f.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast_and_resolve(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_misc47_long_goodbye_enforces_types_mana_value_and_uncounterable_ward() {
    // A one-mana Ward creature has mana value below three. Declining Ward cannot counter this spell.
    let mut ward = engine(947_001);
    let ouphe = inject_creature_on_battlefield(&mut ward, 1, "toadstool_admirer");
    let characteristics = ward
        .characteristics(ouphe)
        .expect("Ward creature characteristics");
    assert!(characteristics.is_creature());
    assert_eq!(characteristics.mana_value, 1);
    assert_eq!(ward.state.objects[&ouphe].zone, Zone::Battlefield);
    let plain = inject_creature_on_battlefield(&mut ward, 1, "llanowar_elves");
    inject_card_into_hand(&mut ward, 0, "long_goodbye");
    let slot = hand_index_for_card(&ward, 0, "long_goodbye");
    semantic::accepted(&mut ward, 0, &cast_spell(slot, target_object(plain)));
    resolve_entire_stack_two_player(&mut ward);
    assert_eq!(ward.state.objects[&plain].zone, Zone::Graveyard);
    assert_eq!(ward.state.objects[&ouphe].zone, Zone::Battlefield);
    inject_card_into_hand(&mut ward, 0, "long_goodbye");
    let slot = hand_index_for_card(&ward, 0, "long_goodbye");
    semantic::accepted(&mut ward, 0, &cast_spell(slot, target_object(ouphe)));
    assert_eq!(ward.state.stack.len(), 2, "Ward trigger above Long Goodbye");
    pass_both_players(&mut ward);
    assert!(
        ward.state.pending_resolution.is_some(),
        "Ward payment is offered"
    );
    ward.apply_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline Ward");
    assert_eq!(
        ward.state.stack.len(),
        1,
        "Long Goodbye remains on the stack"
    );
    pass_both_players(&mut ward);
    assert_eq!(ward.state.objects[&ouphe].zone, Zone::Graveyard);

    // A legal creature is accepted, and a mana-value-three planeswalker reaches the other type branch.
    let mut creature = engine(947_002);
    let bear = inject_creature_on_battlefield(&mut creature, 1, "grizzly_bears");
    cast_and_resolve(&mut creature, "long_goodbye", target_object(bear));
    assert_eq!(creature.state.objects[&bear].zone, Zone::Graveyard);

    let mut planeswalker = engine(947_003);
    let jace = inject_permanent_on_battlefield(&mut planeswalker, 1, "jace_beleren");
    cast_and_resolve(&mut planeswalker, "long_goodbye", target_object(jace));
    assert_eq!(planeswalker.state.objects[&jace].zone, Zone::Graveyard);

    let mut too_large = engine(947_004);
    let giant = inject_creature_on_battlefield(&mut too_large, 1, "hill_giant");
    assert_eq!(too_large.characteristics(giant).unwrap().mana_value, 4);
    inject_card_into_hand(&mut too_large, 0, "long_goodbye");
    let slot = hand_index_for_card(&too_large, 0, "long_goodbye");
    assert!(
        too_large
            .apply_command(0, &cast_spell(slot, target_object(giant)))
            .is_err(),
        "mana value four is not a legal target"
    );
}

#[test]
fn issue_misc47_slice_uses_cast_x_for_both_stats_and_zero_toughness() {
    let mut surviving = engine(947_010);
    let giant = inject_creature_with_stats(&mut surviving, 1, "hill_giant", 3, 3);
    inject_card_into_hand(&mut surviving, 0, "slice_from_the_shadows");
    let slot = hand_index_for_card(&surviving, 0, "slice_from_the_shadows");
    semantic::accepted(
        &mut surviving,
        0,
        &cast_spell_x(slot, target_object(giant), 2),
    );
    resolve_entire_stack_two_player(&mut surviving);
    let remaining = surviving.characteristics(giant).expect("living target");
    assert_eq!((remaining.power, remaining.toughness), (Some(1), Some(1)));
    assert_eq!(surviving.state.objects[&giant].zone, Zone::Battlefield);

    let mut zero_toughness = engine(947_011);
    let giant = inject_creature_with_stats(&mut zero_toughness, 1, "hill_giant", 3, 3);
    inject_card_into_hand(&mut zero_toughness, 0, "slice_from_the_shadows");
    let slot = hand_index_for_card(&zero_toughness, 0, "slice_from_the_shadows");
    semantic::accepted(
        &mut zero_toughness,
        0,
        &cast_spell_x(slot, target_object(giant), 3),
    );
    resolve_entire_stack_two_player(&mut zero_toughness);
    assert_eq!(zero_toughness.state.objects[&giant].zone, Zone::Graveyard);

    let mut uncounterable = engine(947_012);
    let giant = inject_creature_with_stats(&mut uncounterable, 1, "hill_giant", 3, 3);
    inject_card_into_hand(&mut uncounterable, 0, "slice_from_the_shadows");
    let slice_slot = hand_index_for_card(&uncounterable, 0, "slice_from_the_shadows");
    semantic::accepted(
        &mut uncounterable,
        0,
        &cast_spell_x(slice_slot, target_object(giant), 2),
    );
    let slice_spell = uncounterable.state.stack.last().expect("Slice on stack").id;
    inject_card_into_hand(&mut uncounterable, 1, "counterspell");
    let counterspell_slot = hand_index_for_card(&uncounterable, 1, "counterspell");
    uncounterable
        .apply_command(0, &pass())
        .expect("pass priority to opponent");
    semantic::accepted(
        &mut uncounterable,
        1,
        &cast_spell(counterspell_slot, target_object(slice_spell)),
    );
    resolve_entire_stack_two_player(&mut uncounterable);
    assert_eq!(uncounterable.state.objects[&giant].zone, Zone::Battlefield);
    let remaining = uncounterable
        .characteristics(giant)
        .expect("uncounterable Slice resolved");
    assert_eq!((remaining.power, remaining.toughness), (Some(1), Some(1)));
}

#[test]
fn issue_misc47_suspicious_detonation_uses_only_controller_artifact_sacrifices_this_turn() {
    // No qualifying sacrifice pays the printed generic cost of four.
    let mut no_sacrifice = engine(947_020);
    let giant = inject_creature_on_battlefield(&mut no_sacrifice, 1, "hill_giant");
    inject_card_into_hand(&mut no_sacrifice, 0, "suspicious_detonation");
    let slot = hand_index_for_card(&no_sacrifice, 0, "suspicious_detonation");
    semantic::accepted(
        &mut no_sacrifice,
        0,
        &cast_spell(slot, target_object(giant)),
    );
    assert_eq!(no_sacrifice.state.players[0].mana_pool.colorless, 5);
    assert_eq!(no_sacrifice.state.players[0].mana_pool.red, 8);
    resolve_entire_stack_two_player(&mut no_sacrifice);
    assert_eq!(no_sacrifice.state.objects[&giant].zone, Zone::Graveyard);

    // Sacrificing a Clue as its activated ability's cost records a controlled artifact sacrifice.
    let mut own_artifact = engine(947_021);
    let clue = inject_permanent_on_battlefield(&mut own_artifact, 0, "clue");
    apply_ability(&mut own_artifact, 0, clue, 0, vec![]).expect("sacrifice own Clue");
    resolve_entire_stack_two_player(&mut own_artifact);
    assert!(own_artifact
        .state
        .turn_history
        .current
        .permanents_sacrificed
        .iter()
        .any(|fact| fact.player == 0 && fact.types.iter().any(|kind| kind == "Artifact")));
    let giant = inject_creature_on_battlefield(&mut own_artifact, 1, "hill_giant");
    inject_card_into_hand(&mut own_artifact, 0, "suspicious_detonation");
    let slot = hand_index_for_card(&own_artifact, 0, "suspicious_detonation");
    semantic::accepted(
        &mut own_artifact,
        0,
        &cast_spell(slot, target_object(giant)),
    );
    assert_eq!(own_artifact.state.players[0].mana_pool.colorless, 6);
    assert_eq!(own_artifact.state.players[0].mana_pool.red, 8);
    resolve_entire_stack_two_player(&mut own_artifact);
    assert_eq!(own_artifact.state.objects[&giant].zone, Zone::Graveyard);

    // An opponent's artifact sacrifice does not reduce the caster's cost.
    let mut opponent_artifact = engine(947_022);
    let clue = inject_permanent_on_battlefield(&mut opponent_artifact, 1, "clue");
    opponent_artifact
        .apply_command(0, &pass())
        .expect("pass priority to opponent");
    apply_ability(&mut opponent_artifact, 1, clue, 0, vec![])
        .expect("opponent sacrifices their Clue");
    resolve_entire_stack_two_player(&mut opponent_artifact);
    let giant = inject_creature_on_battlefield(&mut opponent_artifact, 1, "hill_giant");
    inject_card_into_hand(&mut opponent_artifact, 0, "suspicious_detonation");
    let slot = hand_index_for_card(&opponent_artifact, 0, "suspicious_detonation");
    semantic::accepted(
        &mut opponent_artifact,
        0,
        &cast_spell(slot, target_object(giant)),
    );
    assert_eq!(opponent_artifact.state.players[0].mana_pool.colorless, 5);
    resolve_entire_stack_two_player(&mut opponent_artifact);
    assert_eq!(
        opponent_artifact.state.objects[&giant].zone,
        Zone::Graveyard
    );
}
