//! Registry conformance for the reviewed haste and must-attack creature cohort.
//!
//! Every included card is a normal-layout creature whose only rules text is Haste plus the CR 508.1d must-attack requirement.
//! Each included card is executed by the shared actual-card scenarios.
use tricerules_cards::{Keyword, Layout};

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
        "ashen_monstrosity",
        "Ashen Monstrosity",
        "{5}{R}{R}",
        &["Creature", "Spirit"],
        &[],
        Some((7, 4)),
        "651ea43e2cc31e762692125c867f99a58133854606abe95c19e815da1b3e363f",
    ),
    (
        "flameborn_hellion",
        "Flameborn Hellion",
        "{5}{R}",
        &["Creature", "Hellion"],
        &[],
        Some((5, 4)),
        "651ea43e2cc31e762692125c867f99a58133854606abe95c19e815da1b3e363f",
    ),
    (
        "valley_dasher",
        "Valley Dasher",
        "{1}{R}",
        &["Creature", "Human", "Berserker"],
        &[],
        Some((2, 2)),
        "651ea43e2cc31e762692125c867f99a58133854606abe95c19e815da1b3e363f",
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
fn attack_haste_batch_maps_definitions() {
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
        assert!(f.must_attack_if_able, "{id}: must attack");
        assert_eq!(f.keywords, [Keyword::Haste], "{id}: haste");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
    }
}

#[test]
fn attack_haste_batch_fingerprints_match_the_pinned_oracle_text() {
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
