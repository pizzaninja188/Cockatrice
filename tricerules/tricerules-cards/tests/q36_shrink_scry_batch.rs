//! Registry conformance for the queue-v36 shrink-and-scry cohort.
use tricerules_cards::primitives::{Amount, EffectSubject, SpellEffectKind};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    power: i32,
    toughness: i32,
    scry: u32,
}

const CARDS: &[Card] = &[
    Card {
        id: "chain_to_memory",
        name: "Chain to Memory",
        mana_cost: "{U}",
        fingerprint: "50c8d22a3bf56a556b136690a4ef86b80c6db174886093316d637c3a39548366",
        types: &["Instant"],
        power: -4,
        toughness: 0,
        scry: 2,
    },
    Card {
        id: "cruel_finality",
        name: "Cruel Finality",
        mana_cost: "{2}{B}",
        fingerprint: "b68424d07344d3af4f15d813c0e96d35db74738e97bd789c2287b571c865169e",
        types: &["Instant"],
        power: -2,
        toughness: -2,
        scry: 1,
    },
    Card {
        id: "lose_hope",
        name: "Lose Hope",
        mana_cost: "{B}",
        fingerprint: "fbb3293f69051244a266e309e85b171850992ea99089c6c27ec508911cdfa2b9",
        types: &["Instant"],
        power: -1,
        toughness: -1,
        scry: 2,
    },
    Card {
        id: "lost_in_a_labyrinth",
        name: "Lost in a Labyrinth",
        mana_cost: "{U}",
        fingerprint: "224638271b4cb6b84a6af6df974876af994659e02bb71a7cde365c27bdfc5e1c",
        types: &["Instant"],
        power: -3,
        toughness: 0,
        scry: 1,
    },
];

#[test]
fn q36_shrink_scry_batch_maps_definitions() {
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
                    subject: EffectSubject::Chosen(_),
                    ..
                } if *power == card.power && *toughness == card.toughness
            ),
            "{}: printed shrink on a chosen creature",
            card.id
        );
        assert!(
            matches!(
                &f.spell_effect[1],
                SpellEffectKind::Scry { count: Amount::Fixed(n) } if *n == card.scry
            ),
            "{}: printed scry",
            card.id
        );
        let groups = &f.targeting.as_ref().expect("target group").groups;
        assert_eq!(groups.len(), 1, "{}", card.id);
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{}", card.id);
        assert_eq!(groups[0].effect_indices, [0], "{}", card.id);
    }
}

#[test]
fn q36_shrink_scry_batch_fingerprints_match_the_pinned_oracle_text() {
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
