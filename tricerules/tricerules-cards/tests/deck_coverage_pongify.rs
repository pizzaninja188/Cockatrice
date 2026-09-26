use tricerules_cards::primitives::{EffectSubject, PlayerRecipient, SpellEffectKind, TargetKind};
use tricerules_cards::{Amount, CardRegistry, Color, Layout};

#[test]
fn pongify_has_exact_identity_target_and_ape_token_contract() {
    let registry = CardRegistry::global();
    assert_eq!(registry.id_for_name("Pongify"), Some("pongify"));
    let card = registry.get("pongify").expect("Pongify");
    assert_eq!(card.name, "Pongify");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "pongify");
    assert_eq!(face.mana_cost.to_string(), "{U}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), [Color::Blue]);
    assert_eq!(face.spell_effect.len(), 2);
    match &face.spell_effect[0] {
        SpellEffectKind::DestroyPreventingRegeneration {
            subject: EffectSubject::Chosen(filter),
        } => assert_eq!(filter.kind, TargetKind::Creature),
        other => panic!("expected destroy of a chosen creature, got {other:?}"),
    }
    match &face.spell_effect[1] {
        SpellEffectKind::CreateTokens {
            token, who, count, ..
        } => {
            assert_eq!(token, "ape_g_3_3");
            assert_eq!(*count, Amount::Fixed(1));
            assert_eq!(
                *who,
                PlayerRecipient::ControllerOfTargetGroup { group_index: 0 }
            );
        }
        other => panic!("expected one Ape for the target's controller, got {other:?}"),
    }
    let group = &face
        .targeting
        .as_ref()
        .expect("Pongify targets a creature")
        .groups[0];
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(group.prompt, "Choose target creature");

    assert!(registry.is_token("ape_g_3_3"));
    let ape = registry.get("ape_g_3_3").expect("Ape token").primary_face();
    assert_eq!(ape.face_id.as_str(), "ape");
    assert_eq!(ape.types, ["Creature", "Ape"]);
    assert_eq!(ape.colors(), [Color::Green]);
    assert_eq!(ape.power, Some(3));
    assert_eq!(ape.toughness, Some(3));
}
