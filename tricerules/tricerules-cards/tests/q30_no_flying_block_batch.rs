//! Registry conformance for the queue-v30 "creatures without flying can't block" cohort.
use tricerules_cards::primitives::{
    CombatRestriction, CombatRestrictionScope, Keyword, SpellEffectKind, TargetFilter, TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "magmatic_chasm",
        name: "Magmatic Chasm",
        mana: "{1}{R}",
        types: &["Sorcery"],
        fingerprint: "31bd5d5c5bb69be1400ecc3ae2e9bffe0df22c1d6f085bd2c151910237c23e7a",
    },
    Card {
        id: "seismic_stomp",
        name: "Seismic Stomp",
        mana: "{1}{R}",
        types: &["Sorcery"],
        fingerprint: "31bd5d5c5bb69be1400ecc3ae2e9bffe0df22c1d6f085bd2c151910237c23e7a",
    },
    Card {
        id: "falter",
        name: "Falter",
        mana: "{1}{R}",
        types: &["Instant"],
        fingerprint: "31bd5d5c5bb69be1400ecc3ae2e9bffe0df22c1d6f085bd2c151910237c23e7a",
    },
];

#[test]
fn q30_no_flying_block_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id, "{}", card.id);
        assert_eq!(f.name, card.name, "{}", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{}", card.id);
        assert_eq!(f.types, card.types, "{}", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        let SpellEffectKind::ApplyCombatRestriction { scope, restriction } = effect else {
            panic!("{}: combat restriction", card.id);
        };
        let CombatRestrictionScope::Matching(filter) = scope else {
            panic!("{}: matching scope", card.id);
        };
        assert_eq!(
            *filter,
            TargetFilter {
                kind: TargetKind::Creature,
                excluded_keywords: vec![Keyword::Flying],
                ..TargetFilter::default()
            },
            "{}",
            card.id
        );
        assert_eq!(
            *restriction,
            CombatRestriction {
                cant_block: true,
                ..CombatRestriction::default()
            },
            "{}",
            card.id
        );
    }
}

#[test]
fn q30_no_flying_block_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
