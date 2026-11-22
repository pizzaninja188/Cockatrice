//! Registry conformance for the reviewed controller-wide anthem family.
//!
//! Every included card is a normal-layout enchantment with one static layer-7c anthem for
//! creatures its controller controls. Card names, costs and printed pump values are data;
//! the shared scenario executes every card. Each fingerprint is SHA-256 of the exact pinned
//! Oracle line.

use tricerules_cards::primitives::{
    CreatureScopeController, CreatureScopeFilter, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "collective_blessing",
        "Collective Blessing",
        "{3}{G}{G}{W}",
        3,
        3,
        "07327dbb97e45f4041fcd51379135cf3b6db3726c682cfd1f9bb2c586d0e4815",
    ),
    (
        "fire_nations_conquest",
        "Fire Nation's Conquest",
        "{2}{R}",
        1,
        0,
        "d4b8d6949eda43dabc7656575a36114b2ce9dac5e61afe945dfccde2122e13d9",
    ),
    (
        "gaeas_anthem",
        "Gaea's Anthem",
        "{1}{G}{G}",
        1,
        1,
        "ce0bb963a8a090d56153734aa724b38c056130c01caae4d3824e308561fc2ca4",
    ),
    (
        "power_boost",
        "Power Boost",
        "{2}{R}",
        1,
        0,
        "d4b8d6949eda43dabc7656575a36114b2ce9dac5e61afe945dfccde2122e13d9",
    ),
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
fn gaeas_anthem_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, delta_power, delta_toughness, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Enchantment"], "{id} type");
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(f.power, None, "{id}");
        assert_eq!(f.toughness, None, "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(
            ability.definition,
            StaticAbilityDef::AnthemPt {
                filter: CreatureScopeFilter {
                    controller: Some(CreatureScopeController::YouControl),
                    ..CreatureScopeFilter::default()
                },
                condition: None,
                delta_power,
                delta_toughness,
            },
            "{id}: exact controller-only anthem filter and pump"
        );
    }
}

#[test]
fn gaeas_anthem_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
