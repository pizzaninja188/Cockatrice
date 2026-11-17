//! Registry conformance for the queue-v29 "opponents' creatures get -1/-0" cohort.
use tricerules_cards::primitives::{
    CreatureScopeController, CreatureScopeFilter, StaticAbilityDef,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
}

const CARDS: &[Card] = &[
    Card {
        id: "cumber_stone",
        name: "Cumber Stone",
        mana: "{3}{U}",
        types: &["Artifact"],
        power: None,
        toughness: None,
    },
    Card {
        id: "dampening_pulse",
        name: "Dampening Pulse",
        mana: "{3}{U}",
        types: &["Enchantment"],
        power: None,
        toughness: None,
    },
    Card {
        id: "haunter_of_nightveil",
        name: "Haunter of Nightveil",
        mana: "{3}{U}{B}",
        types: &["Creature", "Spirit"],
        power: Some(3),
        toughness: Some(4),
    },
];

#[test]
fn q29_opponent_shrink_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, card.types);
        assert_eq!(f.power, card.power);
        assert_eq!(f.toughness, card.toughness);
        assert_eq!(definition.layout, Layout::Normal);
        assert!(f.keywords.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.activated_abilities.is_empty());
        let [ability] = f.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", card.id);
        };
        assert_eq!(
            ability.definition,
            StaticAbilityDef::AnthemPt {
                filter: CreatureScopeFilter {
                    controller: Some(CreatureScopeController::Opponents),
                    ..CreatureScopeFilter::default()
                },
                condition: None,
                delta_power: -1,
                delta_toughness: 0,
            },
            "{}",
            card.id
        );
    }
}

#[test]
fn q29_opponent_shrink_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "cumber_stone",
            "Cumber Stone",
            "67c20425cc1b4069c8d1c1329a4abd7802aa75ffb3c5d22160c15b4161a61db6",
        ),
        (
            "dampening_pulse",
            "Dampening Pulse",
            "67c20425cc1b4069c8d1c1329a4abd7802aa75ffb3c5d22160c15b4161a61db6",
        ),
        (
            "haunter_of_nightveil",
            "Haunter of Nightveil",
            "67c20425cc1b4069c8d1c1329a4abd7802aa75ffb3c5d22160c15b4161a61db6",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
