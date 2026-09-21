//! Registry conformance for the issue #294 reviewed direct-RON pair.
//!
//! Whoosh! (`855353e2-b686-482a-b987-c6ae456f088c`) and Into the Roil
//! (`c2898bbd-82a4-4d26-b6ee-169b0ebe71b4`) were promoted after complete-definition review
//! against the pinned Scryfall snapshot. Their normalized Oracle text is identical, so both
//! pinned fingerprints are the same. Scryfall exact-name records and `rulings_uri` were fetched
//! 2026-09-20; both returned the kicker addendum and Into the Roil's 2020-09-25 ruling that an
//! illegal sole target prevents the kicked draw.

use tricerules_cards::primitives::{
    Amount, CastCostConditionalAmount, CastCostOptionDef, CastCostReceiptCondition, EffectSubject,
    ManaCostChoiceKind, PermanentTypeFilter, PlayerRecipient, SpellEffectKind, TargetFilter,
    TargetGroupDef, TargetKind, TargetingDef,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, ChoiceId, Color, Layout};

const KICKER_FINGERPRINT: &str = "5208c66a815c11c01911fafbfcfcf17083f66d8863963c892844b7e2167d6c1f";

fn single_group(targeting: &TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

#[test]
fn issue_294_direct_ron_registers_both_reviewed_handwritten_cards() {
    let registry = CardRegistry::global();
    let kicker = ChoiceId::new("kicker").expect("stable kicker identity");

    for (id, name, face_id) in [
        ("whoosh!", "Whoosh!", "whoosh"),
        ("into_the_roil", "Into the Roil", "into_the_roil"),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), face_id);
        assert_eq!(face.mana_cost.to_string(), "{1}{U}");
        assert_eq!(face.types, ["Instant"]);
        assert_eq!(face.colors(), vec![Color::Blue]);
        assert!(face.keywords.is_empty());
        assert!(face.triggered_abilities.is_empty());
        assert!(face.activated_abilities.is_empty());
        assert!(face.static_abilities.is_empty());
        assert!(face.custom_effect.is_none());
        assert!(face.modal_spell.is_none());

        // Oracle line 1: Kicker {1}{U} (printed reminder on the same line).
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("{name} has exactly one kicker group");
        };
        assert_eq!(group.group_id, kicker);
        assert_eq!((group.min, group.max), (0, 1));
        assert_eq!(
            group.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let [CastCostOptionDef::Mana {
            option_id,
            presentation,
            kind,
            cost,
        }] = group.options.as_slice()
        else {
            panic!("{name} has exactly one printed mana kicker option");
        };
        assert_eq!(option_id, &kicker);
        assert_eq!(*presentation, AbilityPresentation::OracleLines(vec![1]));
        assert_eq!(*kind, ManaCostChoiceKind::Kicker);
        assert_eq!(cost.to_string(), "{1}{U}");

        // Oracle line 2, in printed order: return the shared sole target, then the receipt-gated
        // draw. `ConditionalCastCost` cannot own Draw, so the count is an Amount::CastCost.
        assert_eq!(
            face.spell_effect,
            [
                SpellEffectKind::ReturnToOwnersHand {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        excluded_permanent_types: vec![PermanentTypeFilter::Land],
                        ..TargetFilter::default()
                    })),
                },
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::CastCost(CastCostConditionalAmount {
                        condition: CastCostReceiptCondition {
                            group_id: kicker.clone(),
                            option_id: kicker.clone(),
                            expected_selected: true,
                        },
                        if_selected: 1,
                        otherwise: 0,
                    }),
                },
            ],
        );

        let targeting = face.targeting.as_ref().expect("targeting");
        let group = single_group(targeting);
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.prompt, "Choose target nonland permanent");
        assert_eq!(group.effect_indices, [0]);
    }
}

#[test]
fn issue_294_direct_ron_fingerprints_match_the_pinned_oracle_text() {
    for (id, name, face_id) in [
        ("whoosh!", "Whoosh!", "whoosh"),
        ("into_the_roil", "Into the Roil", "into_the_roil"),
    ] {
        let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], KICKER_FINGERPRINT, "fingerprint drift for {id}");
    }
}
