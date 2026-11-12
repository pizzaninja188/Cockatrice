//! Registry conformance for the reviewed Aggressive Urge pump-and-draw family.
//!
//! Every included card is a normal-layout instant that pumps one chosen creature and then
//! has its controller draw one card. Card names, costs and printed pump values are data;
//! the shared scenario executes every card. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::Layout;

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
        "aggressive_urge",
        "Aggressive Urge",
        "{1}{G}",
        1,
        1,
        "2d44b4564b9763bde5b83c7848ff9405a9fa529f12d92873f50218b30af182e7",
    ),
    (
        "defiant_strike",
        "Defiant Strike",
        "{W}",
        1,
        0,
        "43ef2cd8e32257dd3f74241adb56d584af4606de859c8dc81126ae3894206072",
    ),
    (
        "sudden_strength",
        "Sudden Strength",
        "{3}{G}",
        3,
        3,
        "393dd97eb91102cc04f2fe117740c6b19e9eeeaf3f39bfb1266e01c9a447425c",
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
fn aggressive_urge_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, power, toughness, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Instant"], "{id} type");
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(f.power, None, "{id}");
        assert_eq!(f.toughness, None, "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(
            f.targeting.is_none(),
            "{id}: the pump uses the implicit target"
        );
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power: printed_power,
                    toughness: printed_toughness,
                    ..
                }, SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                    ..
                }] if *printed_power == power && *printed_toughness == toughness
            ),
            "{id}: exact pump then one controller draw"
        );
    }
}

#[test]
fn aggressive_urge_batch_fingerprints_match_the_pinned_oracle_text() {
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
