//! Registry conformance for the queue-v30 enters-bounce cohort.
use tricerules_cards::primitives::{
    EffectSubject, ObjectFilter, SpellEffectKind, TargetKind, TriggerCondition,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    may: bool,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "man-o-war",
        name: "Man-o'-War",
        mana: "{2}{U}",
        types: &["Creature", "Jellyfish"],
        body: (2, 2),
        may: false,
        fingerprint: "e1146e30720eb9cf46118a58b6cc2c27a3d41cd1a17442d880848190b84f827a",
    },
    Card {
        id: "aether_adept",
        name: "Aether Adept",
        mana: "{1}{U}{U}",
        types: &["Creature", "Human", "Wizard"],
        body: (2, 2),
        may: false,
        fingerprint: "e1146e30720eb9cf46118a58b6cc2c27a3d41cd1a17442d880848190b84f827a",
    },
    Card {
        id: "voidwielder",
        name: "Voidwielder",
        mana: "{4}{U}",
        types: &["Creature", "Human", "Wizard"],
        body: (1, 4),
        may: true,
        fingerprint: "3cb86f86c9e6fc25525f8a19cc32dede6076a5548c97fafa95d462854466efaf",
    },
    Card {
        id: "separatist_voidmage",
        name: "Separatist Voidmage",
        mana: "{3}{U}",
        types: &["Creature", "Human", "Wizard"],
        body: (2, 2),
        may: true,
        fingerprint: "3cb86f86c9e6fc25525f8a19cc32dede6076a5548c97fafa95d462854466efaf",
    },
];

#[test]
fn q30_etb_bounce_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"), "{}", card.id);
        assert_eq!(f.name, card.name, "{}", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{}", card.id);
        assert_eq!(f.types, card.types, "{}", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.body.0), Some(card.body.1)),
            "{}",
            card.id
        );
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one trigger", card.id);
        };
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            card.id
        );
        assert_eq!(triggered.may, card.may, "{}", card.id);
        assert!(triggered.targeting.is_some(), "{}", card.id);
        let [SpellEffectKind::ReturnToOwnersHand { subject }] = triggered.effect.as_slice() else {
            panic!("{}: bounce effect", card.id);
        };
        let EffectSubject::Chosen(target) = subject else {
            panic!("{}: chosen target", card.id);
        };
        assert_eq!(
            **target,
            ObjectFilter {
                kind: TargetKind::Creature,
                ..ObjectFilter::default()
            },
            "{}",
            card.id
        );
    }
}

#[test]
fn q30_etb_bounce_batch_fingerprints_match_the_pinned_oracle_text() {
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
