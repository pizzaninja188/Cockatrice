//! Registry conformance for the queue-v30 filtered creature exile cohort.
use tricerules_cards::primitives::{
    EffectSubject, ObjectFilter, PowerComparison, SpellEffectKind, TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: Option<PowerComparison>,
    tapped: Option<bool>,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "reaver_ambush",
        types: &["Instant"],
        name: "Reaver Ambush",
        mana: "{2}{B}",
        power: Some(PowerComparison::AtMost(3)),
        tapped: None,
        fingerprint: "5e8d3c0744a94842da197794551a02e4097943dcb3c2273aa028efb92a71afc8",
    },
    Card {
        id: "grotesque_demise",
        types: &["Instant"],
        name: "Grotesque Demise",
        mana: "{2}{B}",
        power: Some(PowerComparison::AtMost(3)),
        tapped: None,
        fingerprint: "5e8d3c0744a94842da197794551a02e4097943dcb3c2273aa028efb92a71afc8",
    },
    Card {
        id: "bring_to_trial",
        types: &["Sorcery"],
        name: "Bring to Trial",
        mana: "{2}{W}",
        power: Some(PowerComparison::AtLeast(4)),
        tapped: None,
        fingerprint: "c6d99d748ee464fc1b7c98c8a6929cc4a6613335dce8d80e5e533c34eb9834bb",
    },
    Card {
        id: "blade_banish",
        types: &["Instant"],
        name: "Blade Banish",
        mana: "{3}{W}",
        power: Some(PowerComparison::AtLeast(4)),
        tapped: None,
        fingerprint: "c6d99d748ee464fc1b7c98c8a6929cc4a6613335dce8d80e5e533c34eb9834bb",
    },
    Card {
        id: "expel",
        types: &["Instant"],
        name: "Expel",
        mana: "{2}{W}",
        power: None,
        tapped: Some(true),
        fingerprint: "56f8aac2ac2e54a50991aa9c5476746144ec52a7e804dca6b0d40a9d41e644b2",
    },
    Card {
        id: "excoriate",
        types: &["Sorcery"],
        name: "Excoriate",
        mana: "{3}{W}",
        power: None,
        tapped: Some(true),
        fingerprint: "56f8aac2ac2e54a50991aa9c5476746144ec52a7e804dca6b0d40a9d41e644b2",
    },
];

#[test]
fn q30_exile_creature_filtered_batch_maps_definitions() {
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
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        let SpellEffectKind::Exile { subject, .. } = effect else {
            panic!("{}: exile effect", card.id);
        };
        let EffectSubject::Chosen(target) = subject else {
            panic!("{}: chosen target", card.id);
        };
        assert_eq!(
            **target,
            ObjectFilter {
                kind: TargetKind::Creature,
                power: card.power,
                tapped: card.tapped,
                ..ObjectFilter::default()
            },
            "{}",
            card.id
        );
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1, "{}", card.id);
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{}",
            card.id
        );
    }
}

#[test]
fn q30_exile_creature_filtered_batch_fingerprints_match_the_pinned_oracle_text() {
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
