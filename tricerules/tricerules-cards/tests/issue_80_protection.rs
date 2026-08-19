use tricerules_cards::primitives::{
    Color, ProtectionCardType, ProtectionGrant, ProtectionQuality, SpellEffectKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn issue_80_calibration_cards_are_registered() {
    let registry = CardRegistry::global();

    for card_id in [
        "feat_of_resistance",
        "apostles_blessing",
        "beloved_chaplain",
        "white_knight",
        "black_knight",
    ] {
        let definition = registry
            .get(card_id)
            .unwrap_or_else(|| panic!("issue #80 calibration card {card_id} is not registered"));
        assert!(definition.partial.is_none());
    }
}

#[test]
fn protection_cards_author_fixed_and_chosen_qualities() {
    let registry = CardRegistry::global();
    let feat = registry.get("feat_of_resistance").expect("Feat");
    assert!(matches!(
        &feat.primary_face().spell_effect[1],
        SpellEffectKind::GrantProtection {
            protection: ProtectionGrant::Choose(qualities),
            ..
        } if qualities == &vec![
            ProtectionQuality::Color(Color::White),
            ProtectionQuality::Color(Color::Blue),
            ProtectionQuality::Color(Color::Black),
            ProtectionQuality::Color(Color::Red),
            ProtectionQuality::Color(Color::Green),
        ]
    ));

    let blessing = registry.get("apostles_blessing").expect("Blessing");
    assert!(matches!(
        &blessing.primary_face().spell_effect[0],
        SpellEffectKind::GrantProtection {
            protection: ProtectionGrant::Choose(qualities),
            ..
        } if qualities.first() == Some(&ProtectionQuality::CardType(ProtectionCardType::Artifact))
            && qualities.len() == 6
    ));

    assert_eq!(
        registry
            .get("beloved_chaplain")
            .expect("Chaplain")
            .primary_face()
            .protections,
        vec![ProtectionQuality::CardType(ProtectionCardType::Creature)]
    );
    assert_eq!(
        registry
            .get("white_knight")
            .expect("White Knight")
            .primary_face()
            .protections,
        vec![ProtectionQuality::Color(Color::Black)]
    );
    assert_eq!(
        registry
            .get("black_knight")
            .expect("Black Knight")
            .primary_face()
            .protections,
        vec![ProtectionQuality::Color(Color::White)]
    );
}
