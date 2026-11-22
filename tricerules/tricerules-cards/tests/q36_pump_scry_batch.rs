//! Registry conformance for the queue-v36 pump-and-scry cohort.
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
        id: "battlewise_valor",
        name: "Battlewise Valor",
        mana_cost: "{1}{W}",
        fingerprint: "fd72671243db5a1144776dd07882597800290b93a2e2e85b91a9e0117f43d950",
        types: &["Instant"],
        power: 2,
        toughness: 2,
        scry: 1,
    },
    Card {
        id: "ferocious_charge",
        name: "Ferocious Charge",
        mana_cost: "{2}{G}",
        fingerprint: "7e15b6969d4b2b73e640512e39322a62babc5388cc95a89e4c7e5185f08267a8",
        types: &["Instant"],
        power: 4,
        toughness: 4,
        scry: 2,
    },
    Card {
        id: "inordinate_rage",
        name: "Inordinate Rage",
        mana_cost: "{1}{R}",
        fingerprint: "a48f3b7ef040bb7465c6c697a31254af2605a88df51a7631119beed8198071d8",
        types: &["Instant"],
        power: 3,
        toughness: 2,
        scry: 1,
    },
    Card {
        id: "stand_firm",
        name: "Stand Firm",
        mana_cost: "{W}",
        fingerprint: "5ba391a19ed6fc5b66bb46e9ec0efb0bbf429a7ec1884cc1d99245b94497b967",
        types: &["Instant"],
        power: 1,
        toughness: 1,
        scry: 2,
    },
    Card {
        id: "titans_strength",
        name: "Titan's Strength",
        mana_cost: "{R}",
        fingerprint: "c53fa5411307ae73ab711ae425f4564ade48d28a4ee030429c254aed1fcc6f9f",
        types: &["Instant"],
        power: 3,
        toughness: 1,
        scry: 1,
    },
];

#[test]
fn q36_pump_scry_batch_maps_definitions() {
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
            "{}: printed pump on a chosen creature",
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
        assert!(
            groups[0].prompt.is_empty(),
            "{}: default target prompt",
            card.id
        );
    }
}

#[test]
fn q36_pump_scry_batch_fingerprints_match_the_pinned_oracle_text() {
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
