//! Registry conformance for the reviewed `{W}, {T}: Tap target creature.` family.
//!
//! Every included card is a normal-layout creature whose only printed ability is
//! one `{W}, {T}` activated tap of any target creature. Card names, costs, types
//! and bodies are printed data; the shared scenario executes every card. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "gideons_lawkeeper",
        "Gideon's Lawkeeper",
        "{W}",
        &["Creature", "Human", "Soldier"],
        Some((1, 1)),
        "dfb9a149635a3a02bf06ebace5497ce743ed657100c05648ab0f2992022c195d",
    ),
    (
        "goldmeadow_harrier",
        "Goldmeadow Harrier",
        "{W}",
        &["Creature", "Kithkin", "Soldier"],
        Some((1, 1)),
        "dfb9a149635a3a02bf06ebace5497ce743ed657100c05648ab0f2992022c195d",
    ),
    (
        "benalish_trapper",
        "Benalish Trapper",
        "{1}{W}",
        &["Creature", "Human", "Soldier"],
        Some((1, 2)),
        "dfb9a149635a3a02bf06ebace5497ce743ed657100c05648ab0f2992022c195d",
    ),
    (
        "blinding_mage",
        "Blinding Mage",
        "{1}{W}",
        &["Creature", "Human", "Wizard"],
        Some((1, 2)),
        "dfb9a149635a3a02bf06ebace5497ce743ed657100c05648ab0f2992022c195d",
    ),
    (
        "master_decoy",
        "Master Decoy",
        "{1}{W}",
        &["Creature", "Human", "Soldier"],
        Some((1, 2)),
        "dfb9a149635a3a02bf06ebace5497ce743ed657100c05648ab0f2992022c195d",
    ),
    (
        "akroan_mastiff",
        "Akroan Mastiff",
        "{3}{W}",
        &["Creature", "Dog"],
        Some((2, 2)),
        "dfb9a149635a3a02bf06ebace5497ce743ed657100c05648ab0f2992022c195d",
    ),
    (
        "loxodon_mystic",
        "Loxodon Mystic",
        "{3}{W}{W}",
        &["Creature", "Elephant", "Cleric"],
        Some((3, 3)),
        "dfb9a149635a3a02bf06ebace5497ce743ed657100c05648ab0f2992022c195d",
    ),
    (
        "nebelgast_beguiler",
        "Nebelgast Beguiler",
        "{4}{W}",
        &["Creature", "Spirit"],
        Some((2, 5)),
        "dfb9a149635a3a02bf06ebace5497ce743ed657100c05648ab0f2992022c195d",
    ),
    (
        "ostiary_thrull",
        "Ostiary Thrull",
        "{3}{B}",
        &["Creature", "Thrull"],
        Some((2, 2)),
        "dfb9a149635a3a02bf06ebace5497ce743ed657100c05648ab0f2992022c195d",
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
fn tap_target_activated_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );

        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        let [AbilityCost::Mana(mana_cost), AbilityCost::Tap] = ability.costs.as_slice() else {
            panic!("{id}: {{W}} plus tap costs");
        };
        assert_eq!(mana_cost.to_string(), "{W}", "{id} activation mana");
        let [SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(filter),
        }] = ability.effect.as_slice()
        else {
            panic!("{id}: one chosen tap effect");
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{id}");
        assert!(filter.permanent_types.is_empty(), "{id}");
        assert_eq!(
            filter.controller,
            TargetController::Any,
            "{id}: any player's creature"
        );
    }
}

#[test]
fn tap_target_activated_batch_fingerprints_match_the_pinned_oracle_text() {
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
