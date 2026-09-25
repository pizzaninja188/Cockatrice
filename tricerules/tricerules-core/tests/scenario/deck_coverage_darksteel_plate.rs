//! Exact deck-corpus coverage for Darksteel Plate.
//!
//! Scryfall Oracle and rulings checked 2026-09-25; no rulings were returned. CR 702.6a governs
//! Equip targeting and timing. CR 702.12a-b governs the Plate's and equipped creature's
//! indestructibility, including survival of destroy effects.

use super::helpers::*;
use tricerules_cards::Keyword;

fn darksteel_plate_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "darksteel_plate",
                "grizzly_bears",
                "disenchant",
                "wrath_of_god",
            ],
        ),
        deck_with("plains", &["savannah_lions"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn deck_coverage_darksteel_plate_and_equipped_creature_survive_destroy_effects() {
    let mut engine = darksteel_plate_engine(20_260_927);
    relocate_to_hand(&mut engine, 0, "darksteel_plate");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let plate_slot = hand_index_for_card(&engine, 0, "darksteel_plate");
    semantic::accepted(&mut engine, 0, &cast_spell(plate_slot, vec![]));
    resolve_entire_stack_two_player(&mut engine);

    let plate = battlefield_object_for_card(&engine, 0, "darksteel_plate");
    let equipped = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let opponent_creature = relocate_to_battlefield(&mut engine, 1, "savannah_lions", false);
    assert!(engine.effective_has_keyword(plate, Keyword::Indestructible));
    assert!(!engine.effective_has_keyword(equipped, Keyword::Indestructible));

    assert!(apply_ability(&mut engine, 0, plate, 0, target_object(equipped)).is_err());
    assert_eq!(engine.state.objects[&plate].attached_to, None);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let unfunded_target = apply_ability(&mut engine, 0, plate, 0, target_object(opponent_creature));
    assert!(
        unfunded_target.is_err(),
        "Equip can target only your creature"
    );
    assert_eq!(engine.state.objects[&plate].attached_to, None);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);

    let equip = activate_ability_for(&engine, plate, 0, target_object(equipped));
    semantic::accepted(&mut engine, 0, &equip);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&plate].attached_to,
        Some(AttachmentRecipient::Object(equipped))
    );
    assert!(engine.effective_has_keyword(equipped, Keyword::Indestructible));

    relocate_to_hand(&mut engine, 0, "disenchant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    let disenchant_slot = hand_index_for_card(&engine, 0, "disenchant");
    semantic::accepted(
        &mut engine,
        0,
        &cast_spell(disenchant_slot, target_object(plate)),
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&plate].zone,
        tricerules_core::Zone::Battlefield,
        "the indestructible Equipment survives a destroy effect"
    );

    relocate_to_hand(&mut engine, 0, "wrath_of_god");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 4,
            ..Default::default()
        },
    );
    let wrath_slot = hand_index_for_card(&engine, 0, "wrath_of_god");
    semantic::accepted(&mut engine, 0, &cast_spell(wrath_slot, vec![]));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&equipped].zone,
        tricerules_core::Zone::Battlefield,
        "the equipped creature's granted indestructible lets it survive a mass destroy effect"
    );
    assert_eq!(
        engine.state.objects[&opponent_creature].zone,
        tricerules_core::Zone::Graveyard,
        "Wrath still destroys a creature without indestructible"
    );
    assert_eq!(
        engine.state.objects[&plate].zone,
        tricerules_core::Zone::Battlefield
    );
}
