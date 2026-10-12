use tricerules_cards::primitives::{
    CardTypeFilter, SpellCastOrigin, SpellKeyword, StaticAbilityDef,
};
use tricerules_cards::{CardRegistry, Keyword, ManaCost};

#[test]
fn wildsear_registers_its_exact_card_identity_and_hand_enchantment_cascade_grant() {
    let card = tricerules_cards::registry::global()
        .get("wildsear,_scouring_maw")
        .expect("complete Wildsear definition");

    assert_eq!(card.name, "Wildsear, Scouring Maw");
    assert_eq!(card.faces_iter().count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost, ManaCost::parse("{3}{R}{G}").unwrap());
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Elemental", "Wolf"]);
    assert_eq!((face.power, face.toughness), (Some(6), Some(6)));
    assert_eq!(face.keywords, [Keyword::Trample]);
    assert_eq!(face.static_abilities.len(), 1);
    assert!(matches!(
        &face.static_abilities[0].definition,
        StaticAbilityDef::GrantSpellKeyword {
            filter,
            keyword: SpellKeyword::Cascade,
        } if filter.card_type == Some(CardTypeFilter::Enchantment)
            && filter.origin == Some(SpellCastOrigin::Hand)
    ));
}

#[test]
fn wildsear_spell_keyword_filter_and_cascade_keyword_round_trip() {
    let parsed: StaticAbilityDef = ron::from_str(
        r#"GrantSpellKeyword(
            filter: (card_type: Some(Enchantment), origin: Some(Hand)),
            keyword: Cascade,
        )"#,
    )
    .expect("typed Wildsear filter");
    assert_eq!(
        ron::from_str::<StaticAbilityDef>(&ron::to_string(&parsed).unwrap()).unwrap(),
        parsed
    );
    assert_eq!(
        ron::from_str::<SpellKeyword>("Cascade").unwrap(),
        SpellKeyword::Cascade
    );

    let invalid = r#"(
        id: "invalid_wildsear_filter",
        name: "Invalid Wildsear Filter",
        face_id: "invalid_wildsear_filter",
        types: ["Creature"],
        static_abilities: [(
            ability_id: "bad_filter",
            presentation: Fallback,
            definition: GrantSpellKeyword(
                filter: (min_mana_value: Some(4), max_mana_value: Some(2)),
                keyword: Cascade,
            ),
        )],
    )"#;
    assert!(CardRegistry::from_chunks_and_tokens(&[invalid], &[]).is_err());
}
