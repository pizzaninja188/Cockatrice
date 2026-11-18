//! Registry conformance for the queue-v31 heroic counter cohort.
use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, EffectSubject, Keyword, SpellEffectKind, TargetingSourceFilter,
    TriggerCondition,
};
use tricerules_cards::{CounterKind, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    counters: u32,
    flying: bool,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "lagonna-band_trailblazer",
        name: "Lagonna-Band Trailblazer",
        mana: "{W}",
        types: &["Creature", "Centaur", "Scout"],
        body: (0, 4),
        counters: 1,
        flying: false,
        fingerprint: "09ea82153d60fd2b88cc2bc0b6a2257e72380e73c703e064d28688790fa1e31e",
    },
    Card {
        id: "akroan_skyguard",
        name: "Akroan Skyguard",
        mana: "{1}{W}",
        types: &["Creature", "Human", "Soldier"],
        body: (1, 1),
        counters: 1,
        flying: true,
        fingerprint: "5383c9e6100e24685f4fa6888565819c320844599b7c391b8f4c10fe83b80041",
    },
    Card {
        id: "wingsteed_rider",
        name: "Wingsteed Rider",
        mana: "{1}{W}{W}",
        types: &["Creature", "Human", "Knight"],
        body: (2, 2),
        counters: 1,
        flying: true,
        fingerprint: "5383c9e6100e24685f4fa6888565819c320844599b7c391b8f4c10fe83b80041",
    },
    Card {
        id: "pheres-band_thunderhoof",
        name: "Pheres-Band Thunderhoof",
        mana: "{4}{G}",
        types: &["Creature", "Centaur", "Warrior"],
        body: (3, 4),
        counters: 2,
        flying: false,
        fingerprint: "3d53642de72e45bc9f69bc990a4ea14ef85bb3d372fb33556771766e513cc37e",
    },
    Card {
        id: "setessan_oathsworn",
        name: "Setessan Oathsworn",
        mana: "{1}{G}{G}",
        types: &["Creature", "Satyr", "Warrior"],
        body: (1, 1),
        counters: 2,
        flying: false,
        fingerprint: "3d53642de72e45bc9f69bc990a4ea14ef85bb3d372fb33556771766e513cc37e",
    },
    Card {
        id: "staunch-hearted_warrior",
        name: "Staunch-Hearted Warrior",
        mana: "{3}{G}",
        types: &["Creature", "Human", "Warrior"],
        body: (2, 2),
        counters: 2,
        flying: false,
        fingerprint: "3d53642de72e45bc9f69bc990a4ea14ef85bb3d372fb33556771766e513cc37e",
    },
];

#[test]
fn q31_heroic_counters_batch_maps_definitions() {
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
        assert_eq!(f.types, card.types, "{}", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.body.0), Some(card.body.1))
        );
        assert_eq!(definition.layout, Layout::Normal);
        assert_eq!(
            f.keywords.contains(&Keyword::Flying),
            card.flying,
            "{}",
            card.id
        );
        assert!(f.activated_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{}: one trigger", card.id);
        };
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WheneverSelfBecomesTarget {
                source: TargetingSourceFilter::SpellCast,
                source_controller: CastTriggerPlayer::Controller,
            },
            "{}",
            card.id
        );
        let [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(count),
            subject: EffectSubject::Source,
        }] = triggered.effect.as_slice()
        else {
            panic!("{}: self +1/+1 counters", card.id);
        };
        assert_eq!(*count, card.counters, "{}", card.id);
    }
}

#[test]
fn q31_heroic_counters_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
