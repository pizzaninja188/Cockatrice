use tricerules_cards::primitives::{
    EffectContext, LibraryBottomOrder, SpellEffectKind, ZoneCardFilter,
};
use tricerules_cards::{AbilityPresentation, CardRegistry};

#[test]
fn issue_228_bounds_and_unfiltered_selection_are_validated() {
    for (count, min, max, valid) in [
        (3, 2, 2, true),
        (3, 0, 1, true),
        (0, 0, 1, false),
        (2, 2, 1, false),
        (2, 0, 3, false),
        (2, 0, 0, false),
    ] {
        let mut effect = SpellEffectKind::LookChooseToHand {
            count,
            filter: None,
            min,
            max,
            reveal: false,
            bottom_order: LibraryBottomOrder::Chosen,
        };
        assert_eq!(effect.validate(EffectContext::Spell).is_ok(), valid);
        if let SpellEffectKind::LookChooseToHand { filter, .. } = &mut effect {
            *filter = Some(ZoneCardFilter::default());
        }
        assert!(
            effect.validate(EffectContext::Spell).is_err(),
            "empty filters still fail closed"
        );
    }
}

#[test]
fn issue_228_cards_and_existing_revealed_consumers_are_complete() {
    let registry = CardRegistry::global();
    for (id, name, mana) in [
        ("sleight_of_hand", "Sleight of Hand", "{U}"),
        ("flow_state", "Flow State", "{1}{U}"),
    ] {
        let card = registry.get(id).unwrap();
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(card.primary_face().mana_cost.to_string(), mana);
        assert_eq!(card.primary_face().types, ["Sorcery"]);
    }
    let [SpellEffectKind::ChooseResolutionBranch { branches, .. }] = registry
        .get("flow_state")
        .unwrap()
        .primary_face()
        .spell_effect
        .as_slice()
    else {
        panic!("automatic conditional choice")
    };
    assert_eq!(branches.len(), 2);
    for branch in branches {
        assert_eq!(
            branch.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
    }
    for id in [
        "commune_with_nature",
        "brightwood_tracker",
        "living_phone",
        "star_charter",
    ] {
        let face = registry.get(id).unwrap().primary_face();
        let effects = if id == "commune_with_nature" {
            &face.spell_effect
        } else if id == "brightwood_tracker" {
            &face.activated_abilities[0].effect
        } else {
            &face.triggered_abilities[0].effect
        };
        assert!(matches!(
            effects.as_slice(),
            [SpellEffectKind::LookChooseToHand {
                min: 0,
                max: 1,
                reveal: true,
                filter: Some(_),
                ..
            }]
        ));
    }
}
