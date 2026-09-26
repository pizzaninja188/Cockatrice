use tricerules_cards::primitives::{AbilityCost, ManaAmount, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, CardRegistry, ManaCost};

#[test]
fn fire_lit_thicket_registers_both_mana_abilities_and_all_three_outputs() {
    let registry = CardRegistry::global();
    let card = registry
        .get("fire-lit_thicket")
        .expect("Fire-Lit Thicket registry definition");

    assert_eq!(card.name, "Fire-Lit Thicket");
    assert_eq!(
        registry.id_for_name("Fire-Lit Thicket"),
        Some("fire-lit_thicket")
    );
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "fire_lit_thicket");
    assert_eq!(face.mana_cost.to_string(), "");
    assert_eq!(face.types, ["Land"]);
    assert!(face.colors().is_empty());
    assert!(face.spell_effect.is_empty());
    assert_eq!(face.activated_abilities.len(), 2);

    let [colorless_ability, hybrid_ability] = face.activated_abilities.as_slice() else {
        panic!("Fire-Lit Thicket has two activated abilities");
    };
    assert_eq!(colorless_ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        colorless_ability.source_zone,
        AbilitySourceZone::Battlefield
    );
    assert_eq!(
        colorless_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(colorless_ability.costs, [AbilityCost::Tap]);
    assert!(matches!(
        colorless_ability.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }]
            if options.as_slice() == [ManaAmount { c: 1, ..ManaAmount::default() }]
    ));

    assert_eq!(hybrid_ability.ability_id.as_str(), "activated_02");
    assert_eq!(hybrid_ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        hybrid_ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        hybrid_ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap]
            if *cost == ManaCost::parse("{R/G}").expect("printed hybrid cost")
    ));
    let expected_outputs = [
        ManaAmount {
            r: 2,
            ..ManaAmount::default()
        },
        ManaAmount {
            r: 1,
            g: 1,
            ..ManaAmount::default()
        },
        ManaAmount {
            g: 2,
            ..ManaAmount::default()
        },
    ];
    assert!(matches!(
        hybrid_ability.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }]
            if options.as_slice() == expected_outputs
    ));
}
