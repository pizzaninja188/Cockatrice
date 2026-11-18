#![allow(unused_imports)]
//! Registry conformance for the queue-v33 another-creature entry life gain cohort.
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
        id: "healer_of_the_pride",
        name: "Healer of the Pride",
        mana_cost: "{3}{W}",
        face_id: "healer_of_the_pride",
        fingerprint: "382637162d6e916cf0acb7b52aa34ddcb92040cf6ec56e75eaecde5060c2d4bc",
        types: &["Creature", "Cat", "Cleric"],
        body: Some((2, 3)),
        keywords: &[],
    },
    Card {
        id: "impassioned_orator",
        name: "Impassioned Orator",
        mana_cost: "{1}{W}",
        face_id: "impassioned_orator",
        fingerprint: "19a9122fce2cb9c19dbadc41abeaa495841a48eeedee416938bf6e76bd5d5510",
        types: &["Creature", "Human", "Cleric"],
        body: Some((2, 2)),
        keywords: &[],
    },
    Card {
        id: "social_climber",
        name: "Social Climber",
        mana_cost: "{2}{G}",
        face_id: "social_climber",
        fingerprint: "8058c0bd18ba52718e3d933b9568ec7a8815abcf7961e71f727ee75e5c753250",
        types: &["Creature", "Human", "Druid"],
        body: Some((3, 2)),
        keywords: &[],
    },
];

#[test]
fn q33_soul_warden_batch_maps_definitions() {
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
            "healer_of_the_pride" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WheneverPermanentEntersBattlefield {
                        controller: CastTriggerPlayer::Controller,
                        ..
                    }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::GainLife {
                        amount: Amount::Fixed(2),
                        ..
                    }]
                ));
            }
            "impassioned_orator" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WheneverPermanentEntersBattlefield {
                        controller: CastTriggerPlayer::Controller,
                        ..
                    }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::GainLife {
                        amount: Amount::Fixed(1),
                        ..
                    }]
                ));
            }
            "social_climber" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WheneverPermanentEntersBattlefield {
                        controller: CastTriggerPlayer::Controller,
                        ..
                    }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::GainLife {
                        amount: Amount::Fixed(1),
                        ..
                    }]
                ));
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q33_soul_warden_batch_fingerprints_match_the_pinned_oracle_text() {
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
