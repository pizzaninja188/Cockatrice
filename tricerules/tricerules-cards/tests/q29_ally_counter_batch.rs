//! Registry conformance for the queue-v29 Ally entry-counter cohort.
use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, CreatureEventFilter, EffectSubject, PermanentEventFilter,
    PermanentTypeFilter, SpellEffectKind, TriggerCondition,
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
        id: "hada_freeblade",
        name: "Hada Freeblade",
        mana: "{W}",
        types: &["Creature", "Human", "Soldier", "Ally"],
        power: 0,
        toughness: 1,
    },
    Card {
        id: "oran-rief_survivalist",
        name: "Oran-Rief Survivalist",
        mana: "{1}{G}",
        types: &["Creature", "Human", "Warrior", "Ally"],
        power: 1,
        toughness: 1,
    },
    Card {
        id: "nimana_sell-sword",
        name: "Nimana Sell-Sword",
        mana: "{3}{B}",
        types: &["Creature", "Human", "Warrior", "Ally"],
        power: 2,
        toughness: 2,
    },
];

#[test]
fn q29_ally_counter_batch_maps_definitions() {
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
            TriggerCondition::WheneverPermanentEntersBattlefield {
                controller: CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    permanent_type: Some(PermanentTypeFilter::Creature),
                    ..PermanentEventFilter::default()
                },
                creature_filter: Some(CreatureEventFilter {
                    required_subtypes: vec!["Ally".to_string()],
                    ..CreatureEventFilter::default()
                }),
            },
            "{}",
            card.id
        );
        assert!(ability.may, "{}: printed 'you may'", card.id);
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
fn q29_ally_counter_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "hada_freeblade",
            "Hada Freeblade",
            "a9c5d17b844d99902eaa9bc361069553ad66f2e9e20f9cea4f3c6eb4bc07de9d",
        ),
        (
            "oran-rief_survivalist",
            "Oran-Rief Survivalist",
            "a9c5d17b844d99902eaa9bc361069553ad66f2e9e20f9cea4f3c6eb4bc07de9d",
        ),
        (
            "nimana_sell-sword",
            "Nimana Sell-Sword",
            "a9c5d17b844d99902eaa9bc361069553ad66f2e9e20f9cea4f3c6eb4bc07de9d",
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
