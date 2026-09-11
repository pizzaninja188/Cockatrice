use tricerules_cards::primitives::{PlayerRecipient, SearchDestination};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, Amount, CardRegistry, Keyword,
    SpellEffectKind,
};

#[test]
fn issue_249_registers_the_seven_standard_cards_with_exact_cycling_abilities() {
    let registry = CardRegistry::global();

    for (id, oracle_line) in [("lightshield_parry", 2), ("migrating_ketradon", 3)] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face();
        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id} must have one Cycling ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(ability.source_zone, AbilitySourceZone::Hand);
        assert!(matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"
        ));
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }]
        );
    }

    for (id, oracle_line, subtype) in [
        ("balamb_t-rexaur", 3, "Forest"),
        ("bedhead_beastie", 2, "Mountain"),
        ("hill_gigas", 2, "Mountain"),
        ("saber-tooth_moose-lion", 2, "Forest"),
        ("soaring_sandwing", 3, "Plains"),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face();
        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id} must have one typecycling ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(ability.source_zone, AbilitySourceZone::Hand);
        assert!(matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"
        ));
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::SearchLibrary {
                filter: Some(filter),
                destination: SearchDestination::Hand,
                shuffle: true,
                reveal: true,
                ..
            }] if filter.card_type.is_none() && filter.required_subtypes == [subtype]
        ));
    }

    assert_eq!(
        registry
            .get("hill_gigas")
            .expect("Hill Gigas")
            .primary_face()
            .keywords,
        [Keyword::Trample, Keyword::Haste]
    );
    assert!(registry
        .get("lightshield_parry")
        .expect("Lightshield Parry")
        .primary_face()
        .spell_effect
        .iter()
        .any(|effect| matches!(
            effect,
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                ..
            }
        )));
}
