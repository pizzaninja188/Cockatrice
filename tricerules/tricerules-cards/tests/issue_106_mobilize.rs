use tricerules_cards::{CardRegistry, SpellEffectKind};

#[test]
fn issue_106_mobilize_cards_are_complete_registry_definitions() {
    let registry = CardRegistry::global();

    for (card_id, name) in [
        ("reigning_victor", "Reigning Victor"),
        ("nightblade_brigade", "Nightblade Brigade"),
        ("dragonback_lancer", "Dragonback Lancer"),
        ("shock_brigade", "Shock Brigade"),
    ] {
        let card = registry
            .get(card_id)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(card.name, name);
        assert_eq!(card.partial, None);
        assert!(card
            .primary_face()
            .triggered_abilities
            .iter()
            .any(|ability| {
                matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::CreateAttackingTokens {
                        token,
                        sacrifice_at_next_end_step: true,
                        ..
                    }] if token == "warrior_r_1_1"
                )
            }));
    }

    let warrior = registry
        .get("warrior_r_1_1")
        .expect("red 1/1 Warrior token");
    assert!(registry.is_token("warrior_r_1_1"));
    assert_eq!(warrior.name, "Warrior");
    assert_eq!(warrior.primary_face().power, Some(1));
    assert_eq!(warrior.primary_face().toughness, Some(1));
}
