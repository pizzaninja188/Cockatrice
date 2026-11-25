//! Registry conformance for the queue-v30 filtered creature destruction cohort.
use tricerules_cards::primitives::{
    Color, EffectSubject, Keyword, ObjectFilter, PowerComparison, SpellEffectKind, TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    prevent_regeneration: bool,
    fingerprint: &'static str,
}

fn creature(filter: ObjectFilter) -> ObjectFilter {
    ObjectFilter {
        kind: TargetKind::Creature,
        ..filter
    }
}

const CARDS: &[Card] = &[
    Card {
        id: "wing_snare",
        name: "Wing Snare",
        mana: "{2}{G}",
        prevent_regeneration: false,
        fingerprint: "e53687c51d51cfe53d72d3cfc5cfa12eb4356043e6d22e46a5f484f696cb2bd7",
    },
    Card {
        id: "plummet",
        name: "Plummet",
        mana: "{1}{G}",
        prevent_regeneration: false,
        fingerprint: "e53687c51d51cfe53d72d3cfc5cfa12eb4356043e6d22e46a5f484f696cb2bd7",
    },
    Card {
        id: "vanquish_the_weak",
        name: "Vanquish the Weak",
        mana: "{2}{B}",
        prevent_regeneration: false,
        fingerprint: "90ad31ace2de9b75f2ffc49004d2b9beb0c36212d2ef10e7e0ee22c2b69e1a46",
    },
    Card {
        id: "reave_soul",
        name: "Reave Soul",
        mana: "{1}{B}",
        prevent_regeneration: false,
        fingerprint: "90ad31ace2de9b75f2ffc49004d2b9beb0c36212d2ef10e7e0ee22c2b69e1a46",
    },
    Card {
        id: "sungold_barrage",
        name: "Sungold Barrage",
        mana: "{2}{W}",
        prevent_regeneration: false,
        fingerprint: "ac0fea0f743a9c80a1f5ad22f0c01c67ce9a640821098d9a3d36481814476665",
    },
    Card {
        id: "collar_the_culprit",
        name: "Collar the Culprit",
        mana: "{3}{W}",
        prevent_regeneration: false,
        fingerprint: "ac0fea0f743a9c80a1f5ad22f0c01c67ce9a640821098d9a3d36481814476665",
    },
    Card {
        id: "ghostly_visit",
        name: "Ghostly Visit",
        mana: "{2}{B}",
        prevent_regeneration: false,
        fingerprint: "40617ff36b22cc008888b92fb5328535a50cb9b2de37295992dedfb8d6e62234",
    },
    Card {
        id: "hand_of_death",
        name: "Hand of Death",
        mana: "{2}{B}",
        prevent_regeneration: false,
        fingerprint: "40617ff36b22cc008888b92fb5328535a50cb9b2de37295992dedfb8d6e62234",
    },
    Card {
        id: "flesh_to_dust",
        name: "Flesh to Dust",
        mana: "{3}{B}{B}",
        prevent_regeneration: true,
        fingerprint: "21cd73015b875a9852cf967b2a0b94fe507124f7887056ca38d05a48d35e3368",
    },
    Card {
        id: "terminate",
        name: "Terminate",
        mana: "{B}{R}",
        prevent_regeneration: true,
        fingerprint: "21cd73015b875a9852cf967b2a0b94fe507124f7887056ca38d05a48d35e3368",
    },
];

fn expected_filter(id: &str) -> ObjectFilter {
    match id {
        "wing_snare" | "plummet" => creature(ObjectFilter {
            required_keywords: vec![Keyword::Flying],
            ..ObjectFilter::default()
        }),
        "vanquish_the_weak" | "reave_soul" => creature(ObjectFilter {
            power: Some(PowerComparison::AtMost(3)),
            ..ObjectFilter::default()
        }),
        "sungold_barrage" | "collar_the_culprit" => creature(ObjectFilter {
            toughness: Some(PowerComparison::AtLeast(4)),
            ..ObjectFilter::default()
        }),
        "ghostly_visit" | "hand_of_death" => creature(ObjectFilter {
            not_color: Some(Color::Black),
            ..ObjectFilter::default()
        }),
        _ => ObjectFilter::default_creature(),
    }
}

#[test]
fn q30_destroy_creature_filtered_batch_maps_definitions() {
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
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        let subject = match effect {
            SpellEffectKind::Destroy { subject } if !card.prevent_regeneration => subject,
            SpellEffectKind::DestroyPreventingRegeneration { subject }
                if card.prevent_regeneration =>
            {
                subject
            }
            _ => panic!("{}: wrong destruction primitive", card.id),
        };
        let EffectSubject::Chosen(target) = subject else {
            panic!("{}: chosen target", card.id);
        };
        assert_eq!(**target, expected_filter(card.id), "{}", card.id);
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
fn q30_destroy_creature_filtered_batch_fingerprints_match_the_pinned_oracle_text() {
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
