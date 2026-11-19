//! Registry conformance for the queue-v36 pump-and-fight cohort.
use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetController};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    power: i32,
    toughness: i32,
}

const CARDS: &[Card] = &[
    Card {
        id: "epic_confrontation",
        name: "Epic Confrontation",
        mana_cost: "{1}{G}",
        fingerprint: "46f53f7d69e589f984d6dab23fe3f4c97801c22467ccaa307c3067b40b0dbb12",
        types: &["Sorcery"],
        power: 1,
        toughness: 2,
    },
    Card {
        id: "ruthless_predation",
        name: "Ruthless Predation",
        mana_cost: "{1}{G}",
        fingerprint: "46f53f7d69e589f984d6dab23fe3f4c97801c22467ccaa307c3067b40b0dbb12",
        types: &["Sorcery"],
        power: 1,
        toughness: 2,
    },
    Card {
        id: "savage_smash",
        name: "Savage Smash",
        mana_cost: "{1}{R}{G}",
        fingerprint: "40489b4ec9ed4ee20582c86b7a2f442f23ca8315ba86d95673a0c58ddbaa2e96",
        types: &["Sorcery"],
        power: 2,
        toughness: 2,
    },
    Card {
        id: "swift_kick",
        name: "Swift Kick",
        mana_cost: "{3}{R}",
        fingerprint: "63abce1449f54aca8b522f2ee77b8541493a6afe17f404a15af6f754870a4fa6",
        types: &["Instant"],
        power: 1,
        toughness: 0,
    },
];

#[test]
fn q36_pump_fight_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types.to_vec()
        );
        assert_eq!((f.power, f.toughness), (None, None), "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert_eq!(f.spell_effect.len(), 2, "{}", card.id);
        assert!(
            matches!(
                &f.spell_effect[0],
                SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject: EffectSubject::Chosen(filter),
                    ..
                } if *power == card.power
                    && *toughness == card.toughness
                    && filter.controller == TargetController::You
            ),
            "{}: printed pump to a creature you control",
            card.id
        );
        assert!(
            matches!(
                &f.spell_effect[1],
                SpellEffectKind::Fight { first, second }
                    if matches!(first, EffectSubject::Chosen(filter)
                        if filter.controller == TargetController::You)
                        && matches!(second, EffectSubject::Chosen(filter)
                            if filter.controller == TargetController::NotYou)
            ),
            "{}: fight between the two chosen creatures",
            card.id
        );
        let groups = &f.targeting.as_ref().expect("two target groups").groups;
        assert_eq!(groups.len(), 2, "{}", card.id);
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{}", card.id);
        assert_eq!(groups[0].effect_indices, [0, 1], "{}", card.id);
        assert_eq!(groups[0].distinct_from, [1], "{}", card.id);
        assert_eq!((groups[1].min, groups[1].max), (1, 1), "{}", card.id);
        assert_eq!(groups[1].effect_indices, [1], "{}", card.id);
        assert_eq!(groups[1].distinct_from, [0], "{}", card.id);
        assert_eq!(
            groups[1].prompt, "Choose target creature you don't control",
            "{}",
            card.id
        );
    }
}

#[test]
fn q36_pump_fight_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{}", card.id);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
