//! Registry conformance for the queue-v80 03-kicker-spells cohort.
//!
//! Each card is an ordinary instant or sorcery whose only additional cost is one Kicker option and
//! whose printed effect changes through the cast-cost receipt. Fingerprints are checked after the
//! presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, CastCostConditionalAmount, CastCostGroupDef, CastCostOptionDef,
    CastCostReceiptCondition, EffectSubject, ManaCostChoiceKind, ObjectCastCostKind, ObjectFilter,
    PermanentTypeFilter, PlayerRecipient, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, ChoiceId, ChoicePresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    fingerprint: &'static str,
}

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "blink_of_an_eye",
            name: "Blink of an Eye",
            mana_cost: "{1}{U}",
            types: &["Instant"],
            fingerprint: "5208c66a815c11c01911fafbfcfcf17083f66d8863963c892844b7e2167d6c1f",
        },
        Card {
            id: "eject_the_warp_core",
            name: "Eject the Warp Core",
            mana_cost: "{1}{B}",
            types: &["Instant"],
            fingerprint: "e2beb14aa514b84cc7a6bc799c626fe7b6a8b1f054c29a0dd41da4046872f702",
        },
        Card {
            id: "final_flourish",
            name: "Final Flourish",
            mana_cost: "{1}{B}",
            types: &["Instant"],
            fingerprint: "e2beb14aa514b84cc7a6bc799c626fe7b6a8b1f054c29a0dd41da4046872f702",
        },
        Card {
            id: "roil_eruption",
            name: "Roil Eruption",
            mana_cost: "{1}{R}",
            types: &["Sorcery"],
            fingerprint: "3c3c11960dbd87921ca862b51b2e165e01e12325146fdb5ea1f0c0da53aba71b",
        },
    ]
}

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn kicker_group(f: &tricerules_cards::CardFace) -> &CastCostGroupDef {
    let [group] = f.cast_cost_groups.as_slice() else {
        panic!("one kicker group");
    };
    group
}

fn kicker_mana(group: &CastCostGroupDef, cost: &str) -> ChoiceId {
    let [CastCostOptionDef::Mana {
        option_id,
        presentation,
        kind,
        cost: actual,
    }] = group.options.as_slice()
    else {
        panic!("one mana kicker option");
    };
    assert_eq!(*presentation, ChoicePresentation::OracleLines(vec![1]));
    assert_eq!(*kind, ManaCostChoiceKind::Kicker);
    assert_eq!(actual.to_string(), cost);
    option_id.clone()
}

#[test]
fn q80_kicker_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in cases() {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);

        let group = kicker_group(f);
        assert_eq!(group.group_id.as_str(), "kicker", "{}", card.id);
        assert_eq!((group.min, group.max), (0, 1), "{}", card.id);
        assert_eq!(
            group.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            card.id
        );

        match card.id {
            "blink_of_an_eye" => {
                let option = kicker_mana(group, "{1}{U}");
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [
                        SpellEffectKind::ReturnToOwnersHand {
                            subject: EffectSubject::Chosen(filter),
                        },
                        SpellEffectKind::Draw {
                            who: PlayerRecipient::Controller,
                            count: Amount::CastCost(CastCostConditionalAmount {
                                condition:
                                    CastCostReceiptCondition {
                                        group_id,
                                        option_id,
                                        expected_selected: true,
                                    },
                                if_selected: 1,
                                otherwise: 0,
                            }),
                        },
                    ] if filter.kind == TargetKind::AnyPermanent
                        && filter.excluded_permanent_types == vec![PermanentTypeFilter::Land]
                        && group_id == &option
                        && option_id == &option
                ));
                let groups = &f.targeting.as_ref().expect("targeting").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
            }
            "roil_eruption" => {
                let option = kicker_mana(group, "{5}");
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [SpellEffectKind::DamageTarget {
                        amount: Amount::CastCost(CastCostConditionalAmount {
                            condition:
                                CastCostReceiptCondition {
                                    group_id,
                                    option_id,
                                    expected_selected: true,
                                },
                            if_selected: 5,
                            otherwise: 3,
                        }),
                        target: ObjectFilter {
                            kind: TargetKind::AnyTarget,
                            ..
                        },
                    }] if group_id == &option && option_id == &option
                ));
                assert!(
                    f.targeting.is_none(),
                    "{} uses the implicit any-target",
                    card.id
                );
            }
            "final_flourish" | "eject_the_warp_core" => {
                let [CastCostOptionDef::SacrificePermanent {
                    option_id: sacrifice_option,
                    presentation,
                    kind,
                    filter,
                    ..
                }] = group.options.as_slice()
                else {
                    panic!("{}: one sacrifice kicker option", card.id);
                };
                assert_eq!(*presentation, ChoicePresentation::OracleLines(vec![1]));
                assert_eq!(*kind, ObjectCastCostKind::Kicker);
                assert!(matches!(
                    filter.any_of.as_deref(),
                    Some([
                        ObjectFilter {
                            kind: TargetKind::AnyPermanent,
                            controller: TargetController::You,
                            permanent_types,
                            ..
                        },
                        ObjectFilter {
                            kind: TargetKind::Creature,
                            controller: TargetController::You,
                            ..
                        },
                    ]) if permanent_types == &vec![PermanentTypeFilter::Artifact]
                ));
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [
                        SpellEffectKind::PumpTarget {
                            power: -2,
                            toughness: -2,
                            subject: EffectSubject::Chosen(filter),
                            ..
                        },
                        SpellEffectKind::ConditionalCastCost {
                            condition:
                                CastCostReceiptCondition {
                                    group_id,
                                    option_id: condition_option,
                                    expected_selected: true,
                                },
                            effect,
                        },
                    ] if filter.kind == TargetKind::Creature
                        && group_id.as_str() == "kicker"
                        && condition_option == sacrifice_option
                        && matches!(
                            effect.as_ref(),
                            SpellEffectKind::PumpTarget {
                                power: -4,
                                toughness: -4,
                                subject: EffectSubject::Chosen(filter),
                                ..
                            } if filter.kind == TargetKind::Creature
                        )
                ));
                let groups = &f.targeting.as_ref().expect("targeting").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0, 1]);
            }
            other => panic!("unexpected card {other}"),
        }
    }
}

#[test]
fn q80_kicker_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in cases() {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
