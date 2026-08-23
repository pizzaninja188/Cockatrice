use tricerules_cards::primitives::{
    PlayerRecipient, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{AdditionalCost, Amount, CardRegistry};

#[test]
fn issue_53_cards_have_exact_additional_costs_and_effects() {
    let registry = CardRegistry::global();

    for (id, name, mana, instant) in [
        (
            "thrill_of_possibility",
            "Thrill of Possibility",
            "{1}{R}",
            true,
        ),
        ("tormenting_voice", "Tormenting Voice", "{1}{R}", false),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {name}"));
        assert!(definition.partial.is_none());
        let face = definition.primary_face();
        assert_eq!(face.mana_cost.to_string(), mana);
        assert_eq!(face.is_instant, instant);
        assert_eq!(face.additional_costs, [AdditionalCost::DiscardCard]);
        assert!(matches!(
            face.spell_effect.as_slice(),
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(2)
            }]
        ));
    }

    for (id, name, mana, instant, destroys) in [
        ("bone_splinters", "Bone Splinters", "{B}", false, true),
        ("village_rites", "Village Rites", "{B}", true, false),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {name}"));
        assert!(definition.partial.is_none());
        let face = definition.primary_face();
        assert_eq!(face.mana_cost.to_string(), mana);
        assert_eq!(face.is_instant, instant);
        let AdditionalCost::SacrificePermanent { filter } = &face.additional_costs[0] else {
            panic!("{name} must sacrifice a creature");
        };
        assert_eq!(filter.kind, TargetKind::Creature);
        assert_eq!(filter.controller, TargetController::You);
        assert_eq!(
            face.spell_effect
                .iter()
                .any(|effect| matches!(effect, SpellEffectKind::Destroy { .. })),
            destroys
        );
    }
}
