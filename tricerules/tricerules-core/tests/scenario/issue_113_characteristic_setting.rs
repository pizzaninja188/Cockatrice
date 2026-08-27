//! Issue #113: CR 612 / 613 characteristic setting and visible ability-removal state.

use crate::helpers::*;
use tricerules_cards::{Color, Keyword};
use tricerules_core::Zone;

fn cast_aura(engine: &mut GameEngine, card_id: &str, target: u32) -> u32 {
    ensure_card_in_hand(engine, 0, card_id);
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast characteristic-setting Aura");
    resolve_entire_stack_two_player(engine);
    battlefield_object_for_card(engine, 0, card_id)
}

#[test]
fn witness_protection_sets_characteristics_and_publishes_ability_removal() {
    let decks = Some(vec![
        deck_with("island", &["witness_protection", "zetalpa,_primal_dawn"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(113_001, &[0, 1], 20, decks, true)
        .expect("new game with Witness Protection");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 0, "zetalpa,_primal_dawn", false);
    let aura = cast_aura(&mut engine, "witness_protection", target);

    let protected = engine.characteristics(target).expect("protected creature");
    assert_eq!(protected.names, vec!["Legitimate Businessperson"]);
    assert_eq!(protected.types, vec!["Creature", "Citizen"]);
    assert_eq!(protected.supertypes, vec!["Legendary"]);
    assert_eq!(protected.colors, vec![Color::White, Color::Green]);
    assert!(protected.keywords.is_empty());
    assert_eq!((protected.power, protected.toughness), (Some(1), Some(1)));
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, target),
        vec!["Loses all abilities"]
    );
    assert_eq!(
        zone_view_effective_display_name(&mut engine, 0, target).as_deref(),
        Some("Legitimate Businessperson")
    );
    engine.state.objects.get_mut(&aura).expect("Aura").zone = Zone::Graveyard;
    let restored = engine.characteristics(target).expect("restored creature");
    assert_eq!(restored.names, vec!["Zetalpa, Primal Dawn"]);
    assert!(restored.has_type("Dinosaur"));
    assert_eq!(restored.colors, vec![Color::White]);
    assert!(restored.has_keyword(Keyword::Flying));
    assert_eq!((restored.power, restored.toughness), (Some(4), Some(8)));
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, target).is_empty());
    assert_eq!(
        zone_view_effective_display_name(&mut engine, 0, target).as_deref(),
        Some("Zetalpa, Primal Dawn")
    );
}

#[test]
fn witness_and_unable_to_scream_follow_layer_timestamp_order() {
    fn protected_in_order(first: &str, second: &str, seed: u64) -> (GameEngine, u32) {
        let decks = Some(vec![
            deck_with("island", &[first, second, "grizzly_bears"]),
            deck_with("swamp", &[]),
        ]);
        let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
        advance_to_main1_from_game_start(&mut engine);
        let target = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        cast_aura(&mut engine, first, target);
        cast_aura(&mut engine, second, target);
        (engine, target)
    }

    let (mut witness_then_unable, target) =
        protected_in_order("witness_protection", "unable_to_scream", 113_002);
    let later_unable = witness_then_unable
        .characteristics(target)
        .expect("both Auras");
    assert_eq!(
        later_unable.types,
        vec!["Creature", "Citizen", "Artifact", "Toy"]
    );
    assert_eq!(
        (later_unable.power, later_unable.toughness),
        (Some(0), Some(2))
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut witness_then_unable, 0, target),
        vec!["Loses all abilities"]
    );

    let (mut unable_then_witness, target) =
        protected_in_order("unable_to_scream", "witness_protection", 113_003);
    let later_witness = unable_then_witness
        .characteristics(target)
        .expect("both Auras");
    assert_eq!(later_witness.types, vec!["Creature", "Citizen"]);
    assert_eq!(
        (later_witness.power, later_witness.toughness),
        (Some(1), Some(1))
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut unable_then_witness, 0, target),
        vec!["Loses all abilities"]
    );
}

#[test]
fn ability_gained_after_removal_is_retained_and_annotated() {
    let decks = Some(vec![
        deck_with("island", &["witness_protection", "flight", "grizzly_bears"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(113_004, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    cast_aura(&mut engine, "witness_protection", target);
    cast_aura(&mut engine, "flight", target);

    assert!(engine.effective_has_keyword(target, Keyword::Flying));
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, target),
        vec!["Loses all abilities", "Flying"]
    );
}

#[test]
fn witness_protection_rejects_a_noncreature_target_without_partial_changes() {
    let decks = Some(vec![
        deck_with("island", &["witness_protection"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(113_005, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "witness_protection");
    let land = relocate_to_battlefield(&mut engine, 0, "island", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "witness_protection");
    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect_err("a noncreature is not a legal Aura target");

    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert!(engine.state.players[0]
        .hand
        .iter()
        .any(|oid| engine.state.objects[oid].card_id == "witness_protection"));
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, land).is_empty());
}

#[test]
fn clone_excludes_witness_protections_later_layer_changes() {
    let decks = Some(vec![
        deck_with("island", &["witness_protection", "clone"]),
        deck_with("plains", &["zetalpa,_primal_dawn"]),
    ]);
    let mut engine = GameEngine::new(113_006, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 1, "zetalpa,_primal_dawn", false);
    cast_aura(&mut engine, "witness_protection", source);

    ensure_card_in_hand(&mut engine, 0, "clone");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let clone_slot = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_command(0, &cast_spell(clone_slot, vec![]))
        .expect("cast Clone");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy the protected creature");

    let clone = battlefield_object_for_card(&engine, 0, "clone");
    let copied = engine.characteristics(clone).expect("copied creature");
    assert_eq!(copied.names, vec!["Zetalpa, Primal Dawn"]);
    assert!(copied.has_type("Dinosaur"));
    assert!(!copied.has_type("Citizen"));
    assert_eq!(copied.colors, vec![Color::White]);
    assert!(copied.has_keyword(Keyword::Flying));
    assert_eq!((copied.power, copied.toughness), (Some(4), Some(8)));
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, clone).is_empty());
}

#[test]
fn witness_protection_names_drive_the_legend_rule() {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                "witness_protection",
                "witness_protection",
                "isamaru,_hound_of_konda",
                "zetalpa,_primal_dawn",
            ],
        ),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(113_007, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let first = relocate_to_battlefield(&mut engine, 0, "isamaru,_hound_of_konda", false);
    let second = relocate_to_battlefield(&mut engine, 0, "zetalpa,_primal_dawn", false);
    cast_aura(&mut engine, "witness_protection", first);

    ensure_card_in_hand(&mut engine, 0, "witness_protection");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "witness_protection");
    engine
        .apply_command(0, &cast_spell(slot, target_object(second)))
        .expect("cast second Witness Protection");
    engine.apply_command(0, &pass()).expect("caster pass");
    let batch = engine.apply_command(1, &pass()).expect("opponent pass");

    let choice = find_resolution_choice(&batch).expect("renamed legends require a keep choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LegendKeep);
    assert_eq!(choice.candidate_object_ids, vec![first, second]);
    assert_eq!(
        choice.candidate_names,
        vec!["Legitimate Businessperson", "Legitimate Businessperson"]
    );
}
