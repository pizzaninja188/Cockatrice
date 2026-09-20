//! Registry conformance for the issue #452 reviewed direct-RON pair.
//!
//! Stand Up for Yourself and Oracle's Restoration were promoted from the #451 trial drafts after
//! complete-definition review against the pinned Scryfall snapshot. Scryfall exact-name records
//! and `rulings_uri` were fetched 2026-09-19; Oracle's Restoration carries the 2026-03-20 ruling
//! that an illegal target on resolution means none of its effects happen.

use tricerules_cards::primitives::{
    Amount, EffectSubject, SpellEffectKind, TargetFilter, TargetKind,
};
use tricerules_cards::{CardRegistry, Color, Layout};

fn single_group(
    targeting: &tricerules_cards::primitives::TargetingDef,
) -> &tricerules_cards::primitives::TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

#[test]
fn issue_452_direct_ron_registers_both_reviewed_handwritten_cards() {
    let registry = CardRegistry::global();

    let stand_up = registry
        .get("stand_up_for_yourself")
        .expect("Stand Up for Yourself");
    assert_eq!(stand_up.name, "Stand Up for Yourself");
    assert_eq!(
        registry.id_for_name("Stand Up for Yourself"),
        Some("stand_up_for_yourself")
    );
    assert_eq!(stand_up.layout, Layout::Normal);
    assert_eq!(stand_up.face_count(), 1);
    let face = stand_up.primary_face();
    assert_eq!(face.face_id.as_str(), "stand_up_for_yourself");
    assert_eq!(face.mana_cost.to_string(), "{2}{W}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::White]);
    assert!(face.keywords.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    assert!(face.custom_effect.is_none());
    assert!(face.modal_spell.is_none());
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                power: Some(tricerules_cards::primitives::PowerComparison::AtLeast(3)),
                ..TargetFilter::default()
            })),
        }]
    );
    let targeting = face.targeting.as_ref().expect("Stand Up targeting");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(
        group.prompt,
        "Choose target creature with power 3 or greater"
    );
    assert_eq!(group.effect_indices, [0]);

    let restoration = registry
        .get("oracles_restoration")
        .expect("Oracle's Restoration");
    assert_eq!(restoration.name, "Oracle's Restoration");
    assert_eq!(
        registry.id_for_name("Oracle's Restoration"),
        Some("oracles_restoration")
    );
    assert_eq!(restoration.layout, Layout::Normal);
    assert_eq!(restoration.face_count(), 1);
    let face = restoration.primary_face();
    assert_eq!(face.face_id.as_str(), "oracle_s_restoration");
    assert_eq!(face.mana_cost.to_string(), "{G}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::Green]);
    assert!(face.keywords.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    assert!(face.custom_effect.is_none());
    assert!(face.modal_spell.is_none());
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 1,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    controller: tricerules_cards::primitives::TargetController::You,
                    ..TargetFilter::default()
                })),
            },
            SpellEffectKind::Draw {
                who: tricerules_cards::primitives::PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            },
        ],
        "printed order is pump, draw, life"
    );
    let targeting = face
        .targeting
        .as_ref()
        .expect("Oracle's Restoration targeting");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature you control");
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_452_direct_ron_fingerprints_match_the_pinned_oracle_text() {
    for (id, name, fingerprint) in [
        (
            "stand_up_for_yourself",
            "Stand Up for Yourself",
            "55080a318383389b5ca55fcf9889591d8244ef50d00363301c959a5e56a5b6bc",
        ),
        (
            "oracles_restoration",
            "Oracle's Restoration",
            "dc61eae9859db06efb2d4982164cc628010557fd82ac35e643cf1fae51738ded",
        ),
    ] {
        let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        assert!(
            row.contains(name),
            "fingerprint row names the reviewed card: {row}"
        );
        assert!(
            row.ends_with(fingerprint),
            "fingerprint drift for {id}: {row}"
        );
    }
}
