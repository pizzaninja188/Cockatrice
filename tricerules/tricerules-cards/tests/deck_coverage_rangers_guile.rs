use tricerules_cards::primitives::{
    EffectSubject, Keyword, SpellEffectKind, TargetController, TargetFilter, TargetGroupDef,
    TargetKind,
};
use tricerules_cards::{CardRegistry, Color, Layout};

#[test]
fn rangers_guile_registers_its_complete_targeted_pump_and_hexproof() {
    let registry = CardRegistry::global();
    assert_eq!(
        registry.id_for_name("Ranger's Guile"),
        Some("rangers_guile")
    );
    let card = registry.get("rangers_guile").expect("Ranger's Guile");
    assert_eq!(card.name, "Ranger's Guile");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "rangers_guile");
    assert_eq!(face.mana_cost.to_string(), "{G}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), [Color::Green]);

    let target = TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::You,
        ..TargetFilter::default()
    };
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 1,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(target.clone())),
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(target)),
                keywords: vec![Keyword::Hexproof],
            },
        ]
    );
    let [TargetGroupDef {
        min: 1,
        max: 1,
        prompt,
        effect_indices,
        ..
    }] = face
        .targeting
        .as_ref()
        .expect("one required target")
        .groups
        .as_slice()
    else {
        panic!("Ranger's Guile has one required target group");
    };
    assert_eq!(prompt, "Choose target creature you control");
    assert_eq!(effect_indices, &[0, 1]);
}
