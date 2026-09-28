use tricerules_cards::primitives::AbilityCost;
use tricerules_cards::{CardRegistry, SpellEffectKind};

#[test]
fn arcane_signet_and_command_tower_use_only_the_commanders_declared_identity() {
    let registry = CardRegistry::global();
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
