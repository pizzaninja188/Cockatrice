//! Registry conformance for the reviewed Cycling {2} creature cohort.
//!
//! Every included card is a normal-layout creature with the hand-only Cycling
//! {2} ability and no other rules text.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "barkhide_mauler",
        "Barkhide Mauler",
        "{4}{G}",
        &["Creature", "Beast"],
        &[],
        Some((4, 4)),
        "321ed39cf03451f3c53f5bcec5c32012eacfa42bcf08c7915091de380d43663c",
    ),
    (
        "ridge_rannet",
        "Ridge Rannet",
        "{5}{R}{R}",
        &["Creature", "Beast"],
        &[],
        Some((6, 4)),
        "321ed39cf03451f3c53f5bcec5c32012eacfa42bcf08c7915091de380d43663c",
    ),
    (
        "sandbar_merfolk",
        "Sandbar Merfolk",
        "{U}",
        &["Creature", "Merfolk"],
        &[],
        Some((1, 1)),
        "321ed39cf03451f3c53f5bcec5c32012eacfa42bcf08c7915091de380d43663c",
    ),
    (
        "sandbar_serpent",
        "Sandbar Serpent",
        "{4}{U}",
        &["Creature", "Serpent"],
        &[],
        Some((3, 4)),
        "321ed39cf03451f3c53f5bcec5c32012eacfa42bcf08c7915091de380d43663c",
    ),
    (
        "yoked_plowbeast",
        "Yoked Plowbeast",
        "{5}{W}{W}",
        &["Creature", "Beast"],
        &[],
        Some((5, 5)),
        "321ed39cf03451f3c53f5bcec5c32012eacfa42bcf08c7915091de380d43663c",
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
fn cycling_two_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert_eq!(
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let [cycling] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(cycling.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            cycling.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(cycling.source_zone, AbilitySourceZone::Hand, "{id}");
        assert!(
            matches!(
                cycling.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"
            ),
            "{id}: Cycling {{2}}"
        );
        assert!(
            matches!(
                cycling.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: tricerules_cards::Amount::Fixed(1),
                    ..
                }]
            ),
            "{id}: draw one"
        );
        assert!(cycling.targeting.is_none(), "{id}: no target");
    }
}

#[test]
fn cycling_two_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
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
