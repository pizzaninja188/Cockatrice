//! Registry conformance for the reviewed First strike cohort.
//!
//! Every included card is a normal-layout creature whose only rules text is First strike.
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
        "coiled_tinviper",
        "Coiled Tinviper",
        "{3}",
        &["Artifact", "Creature", "Snake"],
        &[],
        Some((2, 1)),
        "364e5ef085b4ee40fb1f02b931e1cb2b1a564e0f8dda261d2ed64f67b42088f3",
    ),
    (
        "head_of_security",
        "Head of Security",
        "{1}{W}",
        &["Creature", "Andorian", "Soldier"],
        &[],
        Some((2, 2)),
        "234f08fafad7406df534c3c500bcd865154dfeaf8a31a7fda94dcb441041e7e7",
    ),
    (
        "ramirez_depietro",
        "Ramirez DePietro",
        "{3}{U}{B}{B}",
        &["Creature", "Human", "Pirate"],
        &["Legendary"],
        Some((4, 3)),
        "364e5ef085b4ee40fb1f02b931e1cb2b1a564e0f8dda261d2ed64f67b42088f3",
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
fn first_strike_batch_maps_definitions() {
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
        assert_eq!(f.keywords, [Keyword::FirstStrike], "{id} keywords");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(!f.must_attack_if_able, "{id}");
    }
}

#[test]
fn first_strike_batch_fingerprints_match_the_pinned_oracle_text() {
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
