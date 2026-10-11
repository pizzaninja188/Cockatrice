use tricerules_cards::primitives::AbilityCost;
use tricerules_cards::primitives::{EntersTappedAffected, ManaSpendingEffect, StaticAbilityDef};
use tricerules_cards::SpellEffectKind;

#[test]
fn arcane_signet_and_command_tower_use_only_the_commanders_declared_identity() {
    let registry = tricerules_cards::registry::global();
    for (id, name, cost, card_type) in [
        ("arcane_signet", "Arcane Signet", "{2}", "Artifact"),
        ("command_tower", "Command Tower", "", "Land"),
    ] {
        let card = registry.get(id).expect("supported Commander mana card");
        assert_eq!(card.name, name);
        assert_eq!(card.primary_face().mana_cost.to_string(), cost);
        assert!(card
            .primary_face()
            .types
            .iter()
            .any(|card_type_name| card_type_name == card_type));
        assert!(
            card.color_identity().is_empty(),
            "the dynamic output wording does not itself add colored mana symbols"
        );

        let [ability] = card.primary_face().activated_abilities.as_slice() else {
            panic!("{name} has one printed mana ability");
        };
        assert_eq!(ability.costs, [AbilityCost::Tap]);
        assert!(ability.targeting.is_none());
        assert!(ability.is_mana_ability());
        assert!(ability.mana_options().unwrap().is_empty());
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::ProduceMana {
                options,
                commander_color_identity: true,
                restriction: None,
                conditional: None,
            }] if options.is_empty()
        ));
    }
}

#[test]
fn path_of_ancestry_maps_as_a_complete_handwritten_commander_mana_definition() {
    let card = tricerules_cards::registry::global()
        .get("path_of_ancestry")
        .expect("Path of Ancestry is a registered card");
    assert_eq!(card.name, "Path of Ancestry");
    assert_eq!(card.primary_face().mana_cost.to_string(), "");
    assert_eq!(card.primary_face().types, ["Land"]);
    assert!(card.color_identity().is_empty());

    let [enters_tapped] = card.primary_face().static_abilities.as_slice() else {
        panic!("Path of Ancestry has its enters-tapped static ability");
    };
    assert!(matches!(
        enters_tapped.definition,
        StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: None,
            ..
        }
    ));

    let [ability] = card.primary_face().activated_abilities.as_slice() else {
        panic!("Path of Ancestry has one printed mana ability");
    };
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    assert!(ability.targeting.is_none());
    assert!(ability.is_mana_ability());
    let [SpellEffectKind::ProduceMana {
        options,
        commander_color_identity,
        restriction: Some(restriction),
        conditional: None,
    }] = ability.effect.as_slice()
    else {
        panic!("Path produces its commander-identity mana with a spending rider");
    };
    assert!(options.is_empty());
    assert!(commander_color_identity);
    assert_eq!(
        restriction.restriction_id.as_str(),
        "commander_creature_scry"
    );
    assert!(restriction.unrestricted);
    assert_eq!(
        restriction.spending_effects,
        [ManaSpendingEffect::Scry1IfSpellSharesCreatureTypeWithCommander]
    );
}
