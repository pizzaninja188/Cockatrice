#![allow(unused_imports)]
//! Registry conformance for the queue-v33 graveyard self return cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, CastTriggerPlayer, Color, CounterKind,
    CreatureScopeController, EffectSubject, RelativePlayerSet, SpellEffectKind, StaticAbilityDef,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Keyword, Layout};

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
        id: "clay_revenant",
        name: "Clay Revenant",
        mana_cost: "{1}",
        face_id: "clay_revenant",
        fingerprint: "58b67f729a1a2aeb8ead38506b06881bd12d5fe4d75370c4160e8be17af311bb",
        types: &["Artifact", "Creature", "Golem"],
        body: Some((1, 2)),
        keywords: &[],
    },
    Card {
        id: "durable_coilbug",
        name: "Durable Coilbug",
        mana_cost: "{1}{B}",
        face_id: "durable_coilbug",
        fingerprint: "2df86ff39583c2ab1c467bc28c5dbfdc98c205e19b457824305516e41bc4db3e",
        types: &["Creature", "Insect"],
        body: Some((2, 2)),
        keywords: &[],
    },
    Card {
        id: "jungle_creeper",
        name: "Jungle Creeper",
        mana_cost: "{1}{B}{G}",
        face_id: "jungle_creeper",
        fingerprint: "151859990a49056fc22ec4649cc613c4970a847cfec1b9e9db6ee045f5bd4506",
        types: &["Creature", "Elemental"],
        body: Some((3, 3)),
        keywords: &[],
    },
    Card {
        id: "sanitarium_skeleton",
        name: "Sanitarium Skeleton",
        mana_cost: "{B}",
        face_id: "sanitarium_skeleton",
        fingerprint: "1931e3950fbc6149950f7a41bf8f2cb4c889ed101cf211c314a75d7a5837ab73",
        types: &["Creature", "Skeleton"],
        body: Some((1, 2)),
        keywords: &[],
    },
];

#[test]
fn q33_graveyard_self_return_batch_maps_definitions() {
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
            "clay_revenant" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard);
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{2}{B}")
                );
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::ReturnToOwnersHand {
                        subject: EffectSubject::Source
                    }]
                ));
                assert_eq!(f.static_abilities.len(), 1);
                assert!(matches!(
                    f.static_abilities[0].definition,
                    StaticAbilityDef::EntersTapped { .. }
                ));
            }
            "durable_coilbug" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard);
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{4}{B}")
                );
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::ReturnToOwnersHand {
                        subject: EffectSubject::Source
                    }]
                ));
            }
            "jungle_creeper" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard);
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{3}{B}{G}")
                );
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::ReturnToOwnersHand {
                        subject: EffectSubject::Source
                    }]
                ));
            }
            "sanitarium_skeleton" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard);
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{2}{B}")
                );
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::ReturnToOwnersHand {
                        subject: EffectSubject::Source
                    }]
                ));
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q33_graveyard_self_return_batch_fingerprints_match_the_pinned_oracle_text() {
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
