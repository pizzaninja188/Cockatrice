use tricerules_cards::primitives::{
    Amount, PlayerRecipient, SpellEffectKind, TargetFilter, TargetKind, TriggerCondition,
};
use tricerules_cards::CardRegistry;

#[test]
fn curses_have_exact_attachment_data_and_complete_rewards() {
    let registry = CardRegistry::global();
    for (id, name, mana_cost, token) in [
        ("curse_of_opulence", "Curse of Opulence", "{R}", "gold"),
        (
            "curse_of_disturbance",
            "Curse of Disturbance",
            "{2}{B}",
            "zombie_b_2_2",
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("{name} must be registered"));
        let face = definition.primary_face();
        assert_eq!(definition.name, name);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, ["Enchantment", "Aura", "Curse"]);
        assert_eq!(
            face.spell_effect,
            [SpellEffectKind::AuraAttach {
                target: TargetFilter {
                    kind: TargetKind::AnyPlayer,
                    ..TargetFilter::default()
                },
            }]
        );
        assert!(definition.partial.is_none());
        assert_eq!(face.triggered_abilities.len(), 1);
        let ability = &face.triggered_abilities[0];
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverAttachedPlayerIsAttacked
        );
        assert_eq!(
            ability.effect,
            [
                SpellEffectKind::CreateTokens {
                    token: token.into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    sacrifice_timing: None,
                },
                SpellEffectKind::CreateTokens {
                    token: token.into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::AttackingOpponentsOfDefendingPlayer,
                    sacrifice_timing: None,
                },
            ]
        );
    }
}
