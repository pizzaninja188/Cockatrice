#![allow(unused_imports)]
//! Registry conformance for the queue-v34 during-your-turn self pump cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CombatRole, EffectSubject, GameCondition, LifeAmount, PlayerRecipient,
    PowerComparison, RelativePlayerSet, SpellEffectKind, StaticAbilityDef, TargetKind,
    TriggerCondition,
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
        id: "hardy_veteran",
        name: "Hardy Veteran",
        mana_cost: "{1}{G}",
        face_id: "hardy_veteran",
        fingerprint: "884920f7a29cab34eaeb145b4da6a2ddae0b73a86099ce72fede8a327d6c4150",
        types: &["Creature", "Human", "Warrior"],
        body: Some((2, 2)),
        keywords: &[],
    },
    Card {
        id: "sporeback_wolf",
        name: "Sporeback Wolf",
        mana_cost: "{1}{G}",
        face_id: "sporeback_wolf",
        fingerprint: "884920f7a29cab34eaeb145b4da6a2ddae0b73a86099ce72fede8a327d6c4150",
        types: &["Creature", "Wolf"],
        body: Some((2, 2)),
        keywords: &[],
    },
    Card {
        id: "wildwood_geist",
        name: "Wildwood Geist",
        mana_cost: "{4}{G}",
        face_id: "wildwood_geist",
        fingerprint: "48784778c2032ffe28ff838160bd1582f46dd096b6e14dd41fd13e502c550d0d",
        types: &["Creature", "Spirit"],
        body: Some((3, 3)),
        keywords: &[],
    },
];

#[test]
fn q34_during_turn_pump_batch_maps_definitions() {
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
            "hardy_veteran" => {
                assert_eq!(f.static_abilities.len(), 1);
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    &f.static_abilities[0].definition,
                    StaticAbilityDef::ConditionalSelfModifier {
                        condition: GameCondition::ActivePlayer {
                            players: RelativePlayerSet::Controller
                        },
                        delta_power: 0,
                        delta_toughness: 2,
                        ..
                    }
                ));
            }
            "sporeback_wolf" => {
                assert_eq!(f.static_abilities.len(), 1);
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    &f.static_abilities[0].definition,
                    StaticAbilityDef::ConditionalSelfModifier {
                        condition: GameCondition::ActivePlayer {
                            players: RelativePlayerSet::Controller
                        },
                        delta_power: 0,
                        delta_toughness: 2,
                        ..
                    }
                ));
            }
            "wildwood_geist" => {
                assert_eq!(f.static_abilities.len(), 1);
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    &f.static_abilities[0].definition,
                    StaticAbilityDef::ConditionalSelfModifier {
                        condition: GameCondition::ActivePlayer {
                            players: RelativePlayerSet::Controller
                        },
                        delta_power: 2,
                        delta_toughness: 2,
                        ..
                    }
                ));
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q34_during_turn_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
