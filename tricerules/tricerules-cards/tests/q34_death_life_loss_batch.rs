#![allow(unused_imports)]
//! Registry conformance for the queue-v34 dies, each player loses life cohort.
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
        id: "caustic_hound",
        name: "Caustic Hound",
        mana_cost: "{5}{B}",
        face_id: "caustic_hound",
        fingerprint: "bbdde31565574c6f051058bc4c1bd099eacf110efe29ff59798701966b5d143a",
        types: &["Creature", "Phyrexian", "Dog"],
        body: Some((4, 4)),
        keywords: &[],
    },
    Card {
        id: "deathcurse_ogre",
        name: "Deathcurse Ogre",
        mana_cost: "{5}{B}",
        face_id: "deathcurse_ogre",
        fingerprint: "e6cf5640a0b7f72a806551f42c393969f23d9040c7c2ea6599186de1dcff75cc",
        types: &["Creature", "Ogre", "Warrior"],
        body: Some((3, 3)),
        keywords: &[],
    },
    Card {
        id: "soulcage_fiend",
        name: "Soulcage Fiend",
        mana_cost: "{1}{B}{B}",
        face_id: "soulcage_fiend",
        fingerprint: "e6cf5640a0b7f72a806551f42c393969f23d9040c7c2ea6599186de1dcff75cc",
        types: &["Creature", "Demon"],
        body: Some((3, 2)),
        keywords: &[],
    },
];

#[test]
fn q34_death_life_loss_batch_maps_definitions() {
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
            "caustic_hound" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.trigger, TriggerCondition::WhenSelfDies));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::LoseLife {
                        amount: LifeAmount::Fixed(4),
                        who: PlayerRecipient::EachPlayer
                    }]
                ));
            }
            "deathcurse_ogre" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.trigger, TriggerCondition::WhenSelfDies));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::LoseLife {
                        amount: LifeAmount::Fixed(3),
                        who: PlayerRecipient::EachPlayer
                    }]
                ));
            }
            "soulcage_fiend" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.trigger, TriggerCondition::WhenSelfDies));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::LoseLife {
                        amount: LifeAmount::Fixed(3),
                        who: PlayerRecipient::EachPlayer
                    }]
                ));
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q34_death_life_loss_batch_fingerprints_match_the_pinned_oracle_text() {
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
