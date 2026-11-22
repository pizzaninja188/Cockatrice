#![allow(unused_imports)]
//! Registry conformance for the queue-v33 creature mana abilities cohort.
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
        id: "agent_of_stromgald",
        name: "Agent of Stromgald",
        mana_cost: "{R}",
        face_id: "agent_of_stromgald",
        fingerprint: "a2a1e3e94707cc50578a1de7b68253dbfabb15cde0a57fa225714bc9c821812a",
        types: &["Creature", "Human", "Knight"],
        body: Some((1, 1)),
        keywords: &[],
    },
    Card {
        id: "bog_initiate",
        name: "Bog Initiate",
        mana_cost: "{1}{B}",
        face_id: "bog_initiate",
        fingerprint: "bae243df006e19f1a45bae89df9dbd1160445f93f6be3f12dfaf9aa79d830dd8",
        types: &["Creature", "Human", "Wizard"],
        body: Some((1, 1)),
        keywords: &[],
    },
    Card {
        id: "princess_lucrezia",
        name: "Princess Lucrezia",
        mana_cost: "{3}{U}{U}{B}",
        face_id: "princess_lucrezia",
        fingerprint: "8ae83cb569a9767af532f8e24af5b92d5c063f57ec5d535b52b8f4a890c504ed",
        types: &["Creature", "Human", "Wizard"],
        body: Some((5, 4)),
        keywords: &[],
    },
    Card {
        id: "riven_turnbull",
        name: "Riven Turnbull",
        mana_cost: "{5}{U}{B}",
        face_id: "riven_turnbull",
        fingerprint: "e3446c7d22984e1351857f3f0ab45968e28560005151f755d78cd03cd7234fdd",
        types: &["Creature", "Human", "Advisor"],
        body: Some((5, 7)),
        keywords: &[],
    },
];

#[test]
fn q33_creature_mana_abilities_batch_maps_definitions() {
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
            "agent_of_stromgald" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.costs.as_slice(), [AbilityCost::Mana(_)]));
                assert!(ability.is_mana_ability());
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 1 && options[0].b == 1)
                );
            }
            "bog_initiate" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.costs.as_slice(), [AbilityCost::Mana(_)]));
                assert!(ability.is_mana_ability());
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 1 && options[0].b == 1)
                );
            }
            "princess_lucrezia" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.costs.as_slice(), [AbilityCost::Tap]));
                assert!(ability.is_mana_ability());
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 1 && options[0].u == 1)
                );
            }
            "riven_turnbull" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.costs.as_slice(), [AbilityCost::Tap]));
                assert!(ability.is_mana_ability());
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 1 && options[0].b == 1)
                );
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q33_creature_mana_abilities_batch_fingerprints_match_the_pinned_oracle_text() {
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
