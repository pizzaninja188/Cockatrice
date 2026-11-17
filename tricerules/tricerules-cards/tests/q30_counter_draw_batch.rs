//! Registry conformance for the queue-v30 counter-and-draw cohort.
use tricerules_cards::primitives::{Amount, CardTypeFilter, SpellEffectKind, StackSpellFilter};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    creature_only: bool,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "dismiss",
        name: "Dismiss",
        mana: "{2}{U}{U}",
        creature_only: false,
        fingerprint: "2ad2c7849c896e1c17860fed445c0339f7b6bc5bdff4f78e3ba65c6eab563204",
    },
    Card {
        id: "contradict",
        name: "Contradict",
        mana: "{3}{U}{U}",
        creature_only: false,
        fingerprint: "2ad2c7849c896e1c17860fed445c0339f7b6bc5bdff4f78e3ba65c6eab563204",
    },
    Card {
        id: "bone_to_ash",
        name: "Bone to Ash",
        mana: "{2}{U}{U}",
        creature_only: true,
        fingerprint: "adc99d32b6dbc7cefd3010315e5b96d6744060b6b676e803e9fc8d277915ba66",
    },
    Card {
        id: "exclude",
        name: "Exclude",
        mana: "{2}{U}",
        creature_only: true,
        fingerprint: "adc99d32b6dbc7cefd3010315e5b96d6744060b6b676e803e9fc8d277915ba66",
    },
];

#[test]
fn q30_counter_draw_batch_maps_definitions() {
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
        assert_eq!(f.types, ["Instant"], "{}", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.protections.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);
        let [counter, draw] = f.spell_effect.as_slice() else {
            panic!("{}: exactly two effects", card.id);
        };
        let SpellEffectKind::CounterTargetSpell { spell_filter, .. } = counter else {
            panic!("{}: counter first", card.id);
        };
        let expected = if card.creature_only {
            StackSpellFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..StackSpellFilter::default()
            }
        } else {
            StackSpellFilter::default()
        };
        assert_eq!(*spell_filter, expected, "{}", card.id);
        let SpellEffectKind::Draw { count, .. } = draw else {
            panic!("{}: draw second", card.id);
        };
        assert_eq!(*count, Amount::Fixed(1), "{}", card.id);
    }
}

#[test]
fn q30_counter_draw_batch_fingerprints_match_the_pinned_oracle_text() {
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
