//! Registry conformance for the reviewed activated flying-grant cohort.
//!
//! Every included card is a normal-layout creature with one mana-only activated
//! ability granting itself flying until end of turn. Each fingerprint is SHA-256
//! of the exact pinned Oracle lines.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, CardRegistry, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    activation: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "cobalt_golem",
        name: "Cobalt Golem",
        mana_cost: "{4}",
        types: &["Artifact", "Creature", "Golem"],
        power: 2,
        toughness: 3,
        activation: "{1}{U}",
        fingerprint: "aad318bbc0bc70c466306de4af7074e02460d8fc410bd81991f34e5486595b70",
    },
    Card {
        id: "kor_sky_climber",
        name: "Kor Sky Climber",
        mana_cost: "{2}{W}",
        types: &["Creature", "Kor", "Soldier", "Ally"],
        power: 3,
        toughness: 2,
        activation: "{1}{W}",
        fingerprint: "cdfeda181545153da80ce6c9777896261e9ef481deb01d61bfa4867b08467e8b",
    },
    Card {
        id: "leaping_master",
        name: "Leaping Master",
        mana_cost: "{1}{R}",
        types: &["Creature", "Human", "Monk"],
        power: 2,
        toughness: 1,
        activation: "{2}{W}",
        fingerprint: "21c0387e67efa66c5435324ba14b1b2d8181d71b6c5bb1d1f1606dd733874d0c",
    },
    Card {
        id: "roofstalker_wight",
        name: "Roofstalker Wight",
        mana_cost: "{1}{B}",
        types: &["Creature", "Zombie"],
        power: 2,
        toughness: 1,
        activation: "{1}{U}",
        fingerprint: "aad318bbc0bc70c466306de4af7074e02460d8fc410bd81991f34e5486595b70",
    },
    Card {
        id: "steeple_creeper",
        name: "Steeple Creeper",
        mana_cost: "{2}{G}",
        types: &["Creature", "Frog", "Snake"],
        power: 4,
        toughness: 2,
        activation: "{3}{U}",
        fingerprint: "3bd5beaa54f3368de39e236778a7bfffaf6ccf70ece1ff8bc04387e153e4582c",
    },
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn flying_grant_activated_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
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
        assert_eq!(
            f.mana_cost.to_string(),
            card.mana_cost,
            "{} mana cost",
            card.id
        );
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert!(f.keywords.is_empty(), "{} no printed keyword", card.id);
        assert!(
            f.static_abilities.is_empty(),
            "{} no static ability",
            card.id
        );
        assert!(f.triggered_abilities.is_empty(), "{} no trigger", card.id);
        assert!(f.spell_effect.is_empty(), "{} no spell effect", card.id);

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} presentation",
            card.id
        );
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost)] if cost.to_string() == card.activation
            ),
            "{}: printed mana-only cost",
            card.id
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::GrantKeywords { subject, keywords, .. }]
                    if *subject == tricerules_cards::primitives::EffectSubject::Source
                        && keywords.as_slice() == [Keyword::Flying]
            ),
            "{}: source-only flying grant",
            card.id
        );
        assert!(ability.targeting.is_none(), "{} targetless", card.id);
    }
}

#[test]
fn flying_grant_activated_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
