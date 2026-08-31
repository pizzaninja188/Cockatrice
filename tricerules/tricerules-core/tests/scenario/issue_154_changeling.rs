use crate::helpers::*;
use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{CardRegistry, CharacteristicDefiningAbility, Keyword};
use tricerules_core::state::CastCostObjectReceipt;
use tricerules_proto::ruled::v1::{
    cast_cost_group_selection::SelectedObject, CastCostGroupSelection,
};

const CHANGELINGS: [&str; 7] = [
    "chitinous_graspling",
    "firdoch_core",
    "changeling_wayfinder",
    "rooftop_percher",
    "gangly_stompling",
    "feisty_spikeling",
    "mischievous_sneakling",
];

#[test]
fn issue_154_authors_the_seven_changeling_cards() {
    let registry = CardRegistry::global();
    for card_id in CHANGELINGS {
        let definition = registry
            .get(card_id)
            .unwrap_or_else(|| panic!("missing {card_id}"));
        assert_eq!(
            definition.primary_face().characteristic_defining_abilities,
            [CharacteristicDefiningAbility::Changeling],
            "{card_id} must author Changeling as a CDA",
        );
    }

    let graspling = registry.get("chitinous_graspling").unwrap().primary_face();
    assert!(graspling.keywords.contains(&Keyword::Reach));
    let wayfinder = registry.get("changeling_wayfinder").unwrap().primary_face();
    assert_eq!(wayfinder.triggered_abilities.len(), 1);
    let percher = registry.get("rooftop_percher").unwrap().primary_face();
    assert!(percher.keywords.contains(&Keyword::Flying));
    assert_eq!(percher.triggered_abilities.len(), 1);
    let stompling = registry.get("gangly_stompling").unwrap().primary_face();
    assert!(stompling.keywords.contains(&Keyword::Trample));
    let spikeling = registry.get("feisty_spikeling").unwrap().primary_face();
    assert!(matches!(
        spikeling.static_abilities.as_slice(),
        [ability] if matches!(
            ability.definition,
            StaticAbilityDef::ConditionalSelfModifier { .. }
        )
    ));
    let sneakling = registry
        .get("mischievous_sneakling")
        .unwrap()
        .primary_face();
    assert!(sneakling.keywords.contains(&Keyword::Flash));

    let core = registry.get("firdoch_core").unwrap();
    let core_face = core.primary_face();
    assert!(core_face.is_artifact);
    assert!(!core_face.is_creature);
    assert!(core_face.has_subtype("Elf"));
    assert_eq!(core_face.activated_abilities.len(), 1);
}

#[test]
fn issue_154_changeling_satisfies_a_live_goblin_lord() {
    let decks = Some(vec![
        deck_with("mountain", &["goblin_chieftain", "chitinous_graspling"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(154_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "chitinous_graspling");
    ensure_card_in_hand(&mut engine, 0, "goblin_chieftain");
    let changeling = put_creature_on_battlefield(&mut engine, 0, "chitinous_graspling");
    engine.state.players[0].mana_pool.red = 2;
    engine.state.players[0].mana_pool.colorless = 1;
    let chieftain_slot = hand_index_for_card(&engine, 0, "goblin_chieftain");
    engine
        .apply_command(0, &cast_spell(chieftain_slot, vec![]))
        .expect("cast Goblin Chieftain");
    resolve_entire_stack_two_player(&mut engine);

    let characteristics = engine.characteristics(changeling).expect("changeling");
    assert!(characteristics.has_type("Goblin"));
    assert!(characteristics.has_type("Dragon"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(4), Some(5))
    );
    assert!(characteristics.keywords.contains(&Keyword::Haste));
}

#[test]
fn issue_154_changeling_can_be_beheld_as_a_dragon_from_hand() {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &["caustic_exhale", "chitinous_graspling", "grizzly_bears"],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(154_002, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "caustic_exhale");
    ensure_card_in_hand(&mut engine, 0, "chitinous_graspling");
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine.state.players[0].mana_pool.black = 1;
    let spell_slot = hand_index_for_card(&engine, 0, "caustic_exhale");
    let changeling_slot = hand_index_for_card(&engine, 0, "chitinous_graspling");

    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                spell_slot,
                target_object(target),
                vec![CastCostGroupSelection {
                    group_index: 0,
                    option_index: 0,
                    selected_object: Some(SelectedObject::HandIndex(changeling_slot as u32)),
                    expected_zone_change_generation: 0,
                }],
            ),
        )
        .expect("behold the Changeling as a Dragon");
    assert!(matches!(
        engine.state.stack.last().unwrap().cast_cost_receipts[0].object,
        Some(CastCostObjectReceipt::RevealedHand { ref card_id, .. })
            if card_id == "chitinous_graspling"
    ));
}
