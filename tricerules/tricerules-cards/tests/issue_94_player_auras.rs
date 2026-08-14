use tricerules_cards::primitives::{SpellEffectKind, TargetFilter, TargetKind};
use tricerules_cards::CardRegistry;

#[test]
fn curses_have_exact_attachment_data_and_track_only_deferred_rewards() {
    let registry = CardRegistry::global();
    for (id, name, mana_cost, deferred_reward) in [
        (
            "curse_of_opulence",
            "Curse of Opulence",
            "{R}",
            "Gold token",
        ),
        (
            "curse_of_disturbance",
            "Curse of Disturbance",
            "{2}{B}",
            "Zombie reward",
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
        let partial = definition.partial.as_deref().expect("reward is partial");
        assert!(!partial.contains("#63"));
        assert!(partial.contains("#86"));
        assert!(partial.contains(deferred_reward));
    }
}
