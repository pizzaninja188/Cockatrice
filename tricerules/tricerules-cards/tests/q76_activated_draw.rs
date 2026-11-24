//! Registry conformance for the queue-v76 03-activated-draw cohort.
//!
//! Every card has one normal-layout activated ability that draws a fixed number of cards for
//! its printed cost. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{AbilityCost, Amount, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    activation_mana: Option<&'static str>,
    taps: bool,
    draw: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "archivist",
        name: "Archivist",
        mana_cost: "{2}{U}{U}",
        types: &["Creature", "Human", "Wizard"],
        body: Some((1, 1)),
        activation_mana: None,
        taps: true,
        draw: 1,
        fingerprint: "c4a1e992c8d254ce2a4320de1f257e1a51a566c60b42bcb64f58e5f3b75fc1eb",
    },
    Card {
        id: "scepter_of_insight",
        name: "Scepter of Insight",
        mana_cost: "{1}{U}{U}",
        types: &["Artifact"],
        body: None,
        activation_mana: Some("{3}{U}"),
        taps: true,
        draw: 1,
        fingerprint: "89f5d05b2ce9ff89e4c5f1a5979f448b89095be10500b4442f0ab9eb7b2e6290",
    },
    Card {
        id: "treasure_trove",
        name: "Treasure Trove",
        mana_cost: "{2}{U}{U}",
        types: &["Enchantment"],
        body: None,
        activation_mana: Some("{2}{U}{U}"),
        taps: false,
        draw: 1,
        fingerprint: "ae330749bc5284e5d2248e9d5bf3e04d47fa6140addcd88dbc4a174728c9d589",
    },
    Card {
        id: "third_path_savant",
        name: "Third Path Savant",
        mana_cost: "{2}{U}",
        types: &["Creature", "Human", "Wizard"],
        body: Some((2, 3)),
        activation_mana: Some("{7}"),
        taps: false,
        draw: 2,
        fingerprint: "ac33713a0d4afa4fc40822094646a9d92a5970f95aed94ddee82b57c280c0f88",
    },
    Card {
        id: "tower_of_fortunes",
        name: "Tower of Fortunes",
        mana_cost: "{4}",
        types: &["Artifact"],
        body: None,
        activation_mana: Some("{8}"),
        taps: true,
        draw: 4,
        fingerprint: "aed17cbd6e7cb5512a188499720947afb34b5ca8c671ae8d855f89450a239dc0",
    },
    Card {
        id: "benalish_heralds",
        name: "Benalish Heralds",
        mana_cost: "{3}{W}",
        types: &["Creature", "Human", "Soldier"],
        body: Some((2, 4)),
        activation_mana: Some("{3}{U}"),
        taps: true,
        draw: 1,
        fingerprint: "89f5d05b2ce9ff89e4c5f1a5979f448b89095be10500b4442f0ab9eb7b2e6290",
    },
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn q76_activated_draw_map_definitions() {
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
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (card.body.map(|b| b.0), card.body.map(|b| b.1)),
            "{} body",
            card.id
        );
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            card.id
        );
        assert_eq!(
            ability.effect.as_slice(),
            [SpellEffectKind::Draw {
                who: Default::default(),
                count: Amount::Fixed(card.draw),
            }],
            "{}",
            card.id
        );
        match card.activation_mana {
            None => assert_eq!(ability.costs.as_slice(), [AbilityCost::Tap], "{}", card.id),
            Some(mana) => {
                assert_eq!(
                    ability.costs.len(),
                    if card.taps { 2 } else { 1 },
                    "{}",
                    card.id
                );
                assert!(
                    matches!(&ability.costs[0], AbilityCost::Mana(cost) if cost.to_string() == mana),
                    "{} mana cost",
                    card.id
                );
                if card.taps {
                    assert_eq!(ability.costs[1], AbilityCost::Tap, "{}", card.id);
                }
            }
        }
    }
}

#[test]
fn q76_activated_draw_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
