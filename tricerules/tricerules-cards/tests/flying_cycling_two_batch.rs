//! Registry conformance for the reviewed Flying / Cycling {2} creature cohort.
//!
//! Every included card is a normal-layout creature with Flying and the hand-only
//! Cycling {2} ability.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Keyword, Layout};

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
        "angel_of_the_god-pharaoh",
        "Angel of the God-Pharaoh",
        "{4}{W}{W}",
        &["Creature", "Angel"],
        &[],
        Some((4, 4)),
        "7e1195670aebaac553524b57ddd94348a0380ae099363a50cbde541d289985bd",
    ),
    (
        "keeneye_aven",
        "Keeneye Aven",
        "{3}{U}",
        &["Creature", "Bird", "Soldier"],
        &[],
        Some((2, 3)),
        "7e1195670aebaac553524b57ddd94348a0380ae099363a50cbde541d289985bd",
    ),
    (
        "pendrell_drake",
        "Pendrell Drake",
        "{3}{U}",
        &["Creature", "Drake"],
        &[],
        Some((2, 3)),
        "7e1195670aebaac553524b57ddd94348a0380ae099363a50cbde541d289985bd",
    ),
    (
        "primoc_escapee",
        "Primoc Escapee",
        "{6}{U}",
        &["Creature", "Bird", "Beast"],
        &[],
        Some((4, 4)),
        "7e1195670aebaac553524b57ddd94348a0380ae099363a50cbde541d289985bd",
    ),
    (
        "shimmerscale_drake",
        "Shimmerscale Drake",
        "{4}{U}",
        &["Creature", "Drake"],
        &[],
        Some((3, 4)),
        "7e1195670aebaac553524b57ddd94348a0380ae099363a50cbde541d289985bd",
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
fn flying_cycling_two_batch_maps_definitions() {
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
        assert_eq!(f.keywords, [Keyword::Flying], "{id} keywords");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let [cycling] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(cycling.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            cycling.presentation,
            AbilityPresentation::OracleLines(vec![2]),
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
fn flying_cycling_two_batch_fingerprints_match_the_pinned_oracle_text() {
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
