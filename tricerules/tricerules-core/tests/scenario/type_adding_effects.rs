use crate::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{AttachmentRecipient, Zone};

#[test]
fn dub_adds_knight_without_replacing_printed_types() {
    let decks = Some(vec![
        deck_with("plains", &["dub", "grizzly_bears"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(81_001, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "dub");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );

    let dub_slot = hand_index_for_card(&engine, 0, "dub");
    engine
        .apply_command(0, &cast_spell(dub_slot, target_object(bear)))
        .expect("cast Dub");
    resolve_entire_stack_two_player(&mut engine);

    let characteristics = engine.characteristics(bear).expect("enchanted creature");
    assert!(characteristics.has_type("Creature"));
    assert!(characteristics.has_type("Bear"));
    assert!(characteristics.has_type("Knight"));
    assert_eq!(engine.effective_power(bear), Some(4));
    assert_eq!(engine.effective_toughness(bear), Some(4));
    assert!(engine.effective_has_keyword(bear, Keyword::FirstStrike));

    let dub = battlefield_object_for_card(&engine, 0, "dub");
    assert_eq!(
        engine.state.objects[&dub].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
    engine.state.objects.get_mut(&dub).expect("Dub").zone = Zone::Graveyard;

    let restored = engine
        .characteristics(bear)
        .expect("creature after Dub leaves");
    assert!(restored.has_type("Creature"));
    assert!(restored.has_type("Bear"));
    assert!(!restored.has_type("Knight"));
    assert_eq!(engine.effective_power(bear), Some(2));
    assert_eq!(engine.effective_toughness(bear), Some(2));
    assert!(!engine.effective_has_keyword(bear, Keyword::FirstStrike));
}

#[test]
fn liquimetal_coating_adds_artifact_until_cleanup_and_updates_legality() {
    let decks = Some(vec![
        deck_with("mountain", &["liquimetal_coating"]),
        deck_with("swamp", &["go_for_the_throat", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(81_002, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let coating = relocate_to_battlefield(&mut engine, 0, "liquimetal_coating", false);
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_card_in_hand(&mut engine, 1, "go_for_the_throat");

    engine
        .apply_command(0, &activate_ability(coating, 0, target_object(bear)))
        .expect("activate Liquimetal Coating");
    resolve_entire_stack_two_player(&mut engine);

    let coated = engine.characteristics(bear).expect("coated creature");
    assert!(coated.has_type("Creature"));
    assert!(coated.has_type("Bear"));
    assert!(coated.has_type("Artifact"));

    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &pass()).expect("pass priority");
    let removal_slot = hand_index_for_card(&engine, 1, "go_for_the_throat");
    engine
        .apply_command(1, &cast_spell(removal_slot, target_object(bear)))
        .expect_err("an artifact creature is not legal for Go for the Throat");
    engine
        .apply_command(1, &pass())
        .expect("finish the main-phase priority round");
    engine
        .apply_command(0, &primitive_yield())
        .expect("begin combat to end combat");
    engine
        .apply_command(0, &primitive_yield())
        .expect("end combat to second main");
    engine
        .apply_command(0, &primitive_yield())
        .expect("second main to end step");
    engine
        .apply_command(0, &primitive_yield())
        .expect("end step to cleanup or next upkeep");
    resolve_cleanup_discards_if_any(&mut engine);
    let expired = engine
        .characteristics(bear)
        .expect("creature after cleanup");
    assert!(expired.has_type("Creature"));
    assert!(expired.has_type("Bear"));
    assert!(!expired.has_type("Artifact"));

    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let removal_slot = hand_index_for_card(&engine, 1, "go_for_the_throat");
    engine
        .apply_command(1, &cast_spell(removal_slot, target_object(bear)))
        .expect("Go for the Throat is legal after the type addition expires");
}

#[test]
fn liquimetal_coating_type_addition_does_not_follow_a_zone_change() {
    let decks = Some(vec![
        deck_with("mountain", &["liquimetal_coating"]),
        deck_with("swamp", &["murder", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(81_004, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let coating = relocate_to_battlefield(&mut engine, 0, "liquimetal_coating", false);
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_card_in_hand(&mut engine, 1, "murder");

    engine
        .apply_command(0, &activate_ability(coating, 0, target_object(bear)))
        .expect("activate Liquimetal Coating");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine
        .characteristics(bear)
        .expect("coated bear")
        .has_type("Artifact"));

    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &pass()).expect("pass priority");
    let murder_slot = hand_index_for_card(&engine, 1, "murder");
    engine
        .apply_command(1, &cast_spell(murder_slot, target_object(bear)))
        .expect("cast Murder");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);
    assert!(!engine
        .characteristics(bear)
        .expect("card after zone change")
        .has_type("Artifact"));
}
