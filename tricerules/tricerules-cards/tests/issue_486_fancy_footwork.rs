use tricerules_cards::{CardRegistry, Layout, SpellEffectKind};

#[test]
fn issue_486_fancy_footwork_identity() {
    let card = CardRegistry::global().get("fancy_footwork").expect("card");
    assert_eq!(card.layout, Layout::Normal);
    let face = card.primary_face();
    assert_eq!(face.name, "Fancy Footwork");
    assert_eq!(face.mana_cost.to_string(), "{2}{W}");
    assert_eq!(face.types, &["Instant", "Lesson"]);
    assert!(matches!(
        face.spell_effect.as_slice(),
        [
            SpellEffectKind::Untap { .. },
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                ..
            }
        ]
    ));
    let group = &face.targeting.as_ref().expect("target group").groups[0];
    assert_eq!((group.min, group.max), (1, 2));
    assert_eq!(group.effect_indices, vec![0, 1]);
}
