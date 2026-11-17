//! Registry conformance for the queue-v29 "another creature you control dies" counter cohort.
use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, EffectSubject, PermanentEventFilter, PermanentTypeFilter,
    SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{CounterKind, Layout};

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
        id: "unruly_mob",
        name: "Unruly Mob",
        mana: "{1}{W}",
        types: &["Creature", "Human"],
        power: 1,
        toughness: 1,
    },
    Card {
        id: "rot_shambler",
        name: "Rot Shambler",
        mana: "{1}{G}",
        types: &["Creature", "Fungus"],
        power: 1,
        toughness: 1,
    },
    Card {
        id: "gavony_unhallowed",
        name: "Gavony Unhallowed",
        mana: "{3}{B}",
        types: &["Creature", "Zombie"],
        power: 2,
        toughness: 4,
    },
];

#[test]
fn q29_dies_another_counter_batch_maps_definitions() {
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
            TriggerCondition::WheneverCreatureDies {
                controller: CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    permanent_type: Some(PermanentTypeFilter::Creature),
                    exclude_source: true,
                    ..PermanentEventFilter::default()
                },
            },
            "{}",
            card.id
        );
        assert!(!ability.may, "{}", card.id);
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        let SpellEffectKind::PutCounters {
            counter,
            count,
            subject,
        } = effect
        else {
            panic!("{}: put counters", card.id);
        };
        assert_eq!(*counter, CounterKind::PlusOnePlusOne);
        assert_eq!(*count, Amount::Fixed(1));
        assert_eq!(*subject, EffectSubject::Source);
    }
}

#[test]
fn q29_dies_another_counter_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "unruly_mob",
            "Unruly Mob",
            "1322afedf15141a5301cb1033d247eed69ebe78fc6875ac9ae8a05a5b74754fd",
        ),
        (
            "rot_shambler",
            "Rot Shambler",
            "1322afedf15141a5301cb1033d247eed69ebe78fc6875ac9ae8a05a5b74754fd",
        ),
        (
            "gavony_unhallowed",
            "Gavony Unhallowed",
            "1322afedf15141a5301cb1033d247eed69ebe78fc6875ac9ae8a05a5b74754fd",
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
