//! Registry conformance for the queue-v29 "Destroy all Islands" cohort.
use tricerules_cards::primitives::{
    ObjectFilter, PermanentTypeFilter, SpellEffectKind, TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
}

const CARDS: &[Card] = &[
    Card {
        id: "boil",
        name: "Boil",
        mana: "{3}{R}",
        types: &["Instant"],
    },
    Card {
        id: "boiling_seas",
        name: "Boiling Seas",
        mana: "{3}{R}",
        types: &["Sorcery"],
    },
    Card {
        id: "tsunami",
        name: "Tsunami",
        mana: "{3}{G}",
        types: &["Sorcery"],
    },
];

#[test]
fn q29_destroy_islands_batch_maps_definitions() {
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
        assert_eq!(definition.layout, Layout::Normal);
        assert!(f.keywords.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.activated_abilities.is_empty());
        assert!(f.targeting.is_none());
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{}: exactly one spell effect", card.id);
        };
        let SpellEffectKind::DestroyAll {
            kind,
            prevent_regeneration,
        } = effect
        else {
            panic!("{}: destroy all", card.id);
        };
        assert_eq!(
            *kind,
            ObjectFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Land],
                required_subtypes: vec!["Island".to_string()],
                ..ObjectFilter::default()
            },
            "{}",
            card.id
        );
        assert!(!prevent_regeneration, "{}", card.id);
    }
}

#[test]
fn q29_destroy_islands_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "boil",
            "Boil",
            "8c2b6ad7fdf3b9ca249da7b46b4e2856b56926c29df783424d75e793290b5a3e",
        ),
        (
            "boiling_seas",
            "Boiling Seas",
            "8c2b6ad7fdf3b9ca249da7b46b4e2856b56926c29df783424d75e793290b5a3e",
        ),
        (
            "tsunami",
            "Tsunami",
            "8c2b6ad7fdf3b9ca249da7b46b4e2856b56926c29df783424d75e793290b5a3e",
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
