#![allow(unused_imports)]
//! Registry conformance for the queue-v35 target can't-block activations cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CombatRestrictionScope, EffectSubject, EntersTappedAffected,
    SpellEffectKind, StaticAbilityDef, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    face_id: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    keywords: &'static [Keyword],
}

const CARDS: &[Card] = &[
    Card {
        id: "chainwhip_cyclops",
        name: "Chainwhip Cyclops",
        mana_cost: "{4}{R}",
        face_id: "chainwhip_cyclops",
        fingerprint: "13f5bda421b50b45f45d63901cf4226858eb2f625c356dd0ded31b08f249fd88",
        types: &["Creature", "Cyclops", "Warrior"],
        body: Some((4, 4)),
        keywords: &[],
    },
    Card {
        id: "lambholt_harrier",
        name: "Lambholt Harrier",
        mana_cost: "{1}{R}",
        face_id: "lambholt_harrier",
        fingerprint: "13f5bda421b50b45f45d63901cf4226858eb2f625c356dd0ded31b08f249fd88",
        types: &["Creature", "Wolf"],
        body: Some((2, 2)),
        keywords: &[],
    },
    Card {
        id: "sower_of_chaos",
        name: "Sower of Chaos",
        mana_cost: "{3}{R}",
        face_id: "sower_of_chaos",
        fingerprint: "bba968e30c077774cd1523da7e445b5395a0fc045d3f70cbbed06b22c62e983d",
        types: &["Creature", "Devil"],
        body: Some((4, 3)),
        keywords: &[],
    },
];

#[test]
fn q35_cant_block_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.face_id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types.to_vec()
        );
        match card.body {
            Some((power, toughness)) => {
                assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)))
            }
            None => assert_eq!((f.power, f.toughness), (None, None)),
        }
        assert_eq!(f.keywords, card.keywords);
        match card.id {
            "chainwhip_cyclops" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{3}{R}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::ApplyCombatRestriction { scope: CombatRestrictionScope::Chosen(_), restriction }] if restriction.cant_block)
                );
                let groups = &ability.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
            }
            "lambholt_harrier" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{3}{R}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::ApplyCombatRestriction { scope: CombatRestrictionScope::Chosen(_), restriction }] if restriction.cant_block)
                );
                let groups = &ability.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
            }
            "sower_of_chaos" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{2}{R}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::ApplyCombatRestriction { scope: CombatRestrictionScope::Chosen(_), restriction }] if restriction.cant_block)
                );
                let groups = &ability.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q35_cant_block_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{}", card.id);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.face_id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
