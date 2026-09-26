use tricerules_cards::primitives::{EffectSubject, PlayerRecipient, SpellEffectKind, TargetKind};
use tricerules_cards::{CardRegistry, Color, Layout};

#[test]
fn beast_within_targets_any_permanent_and_gives_a_beast_to_its_controller() {
    let registry = CardRegistry::global();
    assert_eq!(registry.id_for_name("Beast Within"), Some("beast_within"));
    let card = registry.get("beast_within").expect("Beast Within");
    assert_eq!(card.name, "Beast Within");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "beast_within");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(face.colors(), [Color::Green]);
    assert_eq!(face.spell_effect.len(), 2);
    match &face.spell_effect[0] {
        SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        } => assert_eq!(filter.kind, TargetKind::AnyPermanent),
        other => panic!("expected destroy of the chosen permanent, got {other:?}"),
    }
    match &face.spell_effect[1] {
        SpellEffectKind::CreateTokens { token, who, .. } => {
            assert_eq!(token, "beast_g_3_3");
            assert_eq!(
                *who,
                PlayerRecipient::ControllerOfTargetGroup { group_index: 0 }
            );
        }
        other => panic!("expected the target's controller to create a Beast, got {other:?}"),
    }
    let group = &face
        .targeting
        .as_ref()
        .expect("Beast Within targets a permanent")
        .groups[0];
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(group.prompt, "Choose target permanent");

    assert!(registry.is_token("beast_g_3_3"));
    let token = registry.get("beast_g_3_3").expect("Beast token");
    let token_face = token.primary_face();
    assert_eq!(token_face.types, ["Creature", "Beast"]);
    assert_eq!(token_face.colors(), [Color::Green]);
    assert_eq!(token_face.power, Some(3));
    assert_eq!(token_face.toughness, Some(3));
}
