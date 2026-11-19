//! Registry conformance for the queue-v37 04-self-keyword-activated cohort.
//! Printed identity, costs, typed keyword shape and pinned fingerprints.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    cost: &'static str,
    keywords: &'static [Keyword],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "cabaretti_initiate",
        name: "Cabaretti Initiate",
        mana_cost: "{G}",
        types: &["Creature", "Raccoon", "Citizen"],
        power: Some(1),
        toughness: Some(2),
        cost: "{2}{R/W}",
        keywords: &[Keyword::DoubleStrike],
        fingerprint: "03db3390464a7da1f6e02fd1d4e2fd4b98736fa15e7328f546565db6a921dcde",
    },
    Card {
        id: "fallaji_chaindancer",
        name: "Fallaji Chaindancer",
        mana_cost: "{3}{R}",
        types: &["Creature", "Human", "Soldier"],
        power: Some(2),
        toughness: Some(4),
        cost: "{2}",
        keywords: &[Keyword::DoubleStrike],
        fingerprint: "d5d380be7f8e7d305fe661c2e39f3cdff8d7d6b6545080502d8e35c679ff203c",
    },
    Card {
        id: "giant_crab",
        name: "Giant Crab",
        mana_cost: "{4}{U}",
        types: &["Creature", "Crab"],
        power: Some(3),
        toughness: Some(3),
        cost: "{U}",
        keywords: &[Keyword::Shroud],
        fingerprint: "a687dd263dff820956182770048dac81d01187fcf8b8ee2190337ad84525f7da",
    },
    Card {
        id: "gruul_nodorog",
        name: "Gruul Nodorog",
        mana_cost: "{4}{G}{G}",
        types: &["Creature", "Beast"],
        power: Some(4),
        toughness: Some(4),
        cost: "{R}",
        keywords: &[Keyword::Menace],
        fingerprint: "8d8cb023a46e27aabdfb87e5d45a486dc2076c2468db039c81b8b2aaa721a950",
    },
    Card {
        id: "horror_of_the_dim",
        name: "Horror of the Dim",
        mana_cost: "{4}{B}",
        types: &["Creature", "Horror"],
        power: Some(3),
        toughness: Some(4),
        cost: "{U}",
        keywords: &[Keyword::Hexproof],
        fingerprint: "d77712247f70ea1df113cf2ad62e00827d67cf3d10d39ba1256d5e8098ce4955",
    },
    Card {
        id: "insatiable_souleater",
        name: "Insatiable Souleater",
        mana_cost: "{4}",
        types: &["Artifact", "Creature", "Phyrexian", "Beast"],
        power: Some(5),
        toughness: Some(1),
        cost: "{G/P}",
        keywords: &[Keyword::Trample],
        fingerprint: "be1f0b2b60c0ad3756357d7934b8f4fe8b301e39a24728b767472428e92b0daf",
    },
    Card {
        id: "moorland_inquisitor",
        name: "Moorland Inquisitor",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Soldier"],
        power: Some(2),
        toughness: Some(2),
        cost: "{2}{W}",
        keywords: &[Keyword::FirstStrike],
        fingerprint: "93dc3163a64e592d0fadec032391d028ba1e0ade3f09f83f7a27e2e7e547eb26",
    },
    Card {
        id: "pestilent_wolf",
        name: "Pestilent Wolf",
        mana_cost: "{1}{G}",
        types: &["Creature", "Wolf"],
        power: Some(2),
        toughness: Some(2),
        cost: "{2}{G}",
        keywords: &[Keyword::Deathtouch],
        fingerprint: "a120662372636fd7f4a0f2d909de360c37252f2348f091d221a1423fde5a127f",
    },
    Card {
        id: "prakhata_pillar-bug",
        name: "Prakhata Pillar-Bug",
        mana_cost: "{3}",
        types: &["Artifact", "Creature", "Insect"],
        power: Some(2),
        toughness: Some(3),
        cost: "{B}",
        keywords: &[Keyword::Lifelink],
        fingerprint: "50542b5f8b60f282b9886b59f170f2d630cfa0a890b2f8f26fbc5c7bf3a9d2ea",
    },
    Card {
        id: "stonefare_crocodile",
        name: "Stonefare Crocodile",
        mana_cost: "{2}{G}",
        types: &["Creature", "Crocodile"],
        power: Some(3),
        toughness: Some(2),
        cost: "{2}{B}",
        keywords: &[Keyword::Lifelink],
        fingerprint: "d51931d999f1eb52c978190060d8f3eabb0c143f0cc280a3b718c50fe8a1e63f",
    },
];

#[test]
fn q37_self_keyword_activated_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types
        );
        assert_eq!((f.power, f.toughness), (card.power, card.toughness));
        assert!(f.keywords.is_empty(), "{}: no printed keyword", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(mana)] if mana.to_string() == card.cost
            ),
            "{}: printed activation cost",
            card.id
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::GrantKeywords { subject, keywords }]
                    if *subject == EffectSubject::Source && keywords == card.keywords
            ),
            "{}: source keyword grant",
            card.id
        );
        assert!(ability.targeting.is_none(), "{}", card.id);
    }
}

#[test]
fn q37_self_keyword_activated_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
