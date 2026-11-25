//! Registry conformance for the queue-v29 optional attack-tap cohort.
use tricerules_cards::primitives::{
    EffectSubject, ObjectFilter, SpellEffectKind, TargetKind, TriggerCondition,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
}

const CARDS: &[Card] = &[
    Card {
        id: "seasoned_marshal",
        name: "Seasoned Marshal",
        mana: "{2}{W}{W}",
        types: &["Creature", "Human", "Soldier"],
        power: 2,
        toughness: 2,
    },
    Card {
        id: "flanking_troops",
        name: "Flanking Troops",
        mana: "{2}{W}{W}",
        types: &["Creature", "Human", "Soldier"],
        power: 2,
        toughness: 2,
    },
    Card {
        id: "alaborn_cavalier",
        name: "Alaborn Cavalier",
        mana: "{2}{W}{W}",
        types: &["Creature", "Human", "Knight"],
        power: 2,
        toughness: 2,
    },
];

#[test]
fn q29_attack_may_tap_batch_maps_definitions() {
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
        assert_eq!(f.power, Some(card.power));
        assert_eq!(f.toughness, Some(card.toughness));
        assert_eq!(definition.layout, Layout::Normal);
        assert!(f.keywords.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.activated_abilities.is_empty());
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one trigger", card.id);
        };
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0
            },
            "{}",
            card.id
        );
        assert!(ability.may, "{}: printed 'you may'", card.id);
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        let SpellEffectKind::Tap { subject } = effect else {
            panic!("{}: tap", card.id);
        };
        assert_eq!(
            *subject,
            EffectSubject::Chosen(Box::new(ObjectFilter {
                kind: TargetKind::Creature,
                ..ObjectFilter::default()
            })),
            "{}",
            card.id
        );
        let targeting = ability.targeting.as_ref().expect("targeting");
        let [group] = targeting.groups.as_slice() else {
            panic!("{}: one target group", card.id);
        };
        assert_eq!((group.min, group.max), (1, 1), "{}", card.id);
        assert_eq!(group.effect_indices, [0], "{}", card.id);
    }
}

#[test]
fn q29_attack_may_tap_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "seasoned_marshal",
            "Seasoned Marshal",
            "a2b4d02900d499049093efb218ef0967eef977b57675beb49036ba1e1d713511",
        ),
        (
            "flanking_troops",
            "Flanking Troops",
            "a2b4d02900d499049093efb218ef0967eef977b57675beb49036ba1e1d713511",
        ),
        (
            "alaborn_cavalier",
            "Alaborn Cavalier",
            "a2b4d02900d499049093efb218ef0967eef977b57675beb49036ba1e1d713511",
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
