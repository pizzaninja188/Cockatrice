//! Registry conformance for the queue-v29 keyword plus becomes-target sacrifice cohort.
use tricerules_cards::primitives::{
    CastTriggerPlayer, EffectSubject, SpellEffectKind, TargetingSourceFilter, TriggerCondition,
};
use tricerules_cards::{Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keyword: Keyword,
}

const CARDS: &[Card] = &[
    Card {
        id: "gossamer_phantasm",
        name: "Gossamer Phantasm",
        mana: "{1}{U}",
        types: &["Creature", "Illusion"],
        power: 2,
        toughness: 1,
        keyword: Keyword::Flying,
    },
    Card {
        id: "illusionary_servant",
        name: "Illusionary Servant",
        mana: "{1}{U}{U}",
        types: &["Creature", "Illusion"],
        power: 3,
        toughness: 4,
        keyword: Keyword::Flying,
    },
    Card {
        id: "phantasmal_dragon",
        name: "Phantasmal Dragon",
        mana: "{2}{U}{U}",
        types: &["Creature", "Dragon", "Illusion"],
        power: 5,
        toughness: 5,
        keyword: Keyword::Flying,
    },
    Card {
        id: "skulking_ghost",
        name: "Skulking Ghost",
        mana: "{1}{B}",
        types: &["Creature", "Spirit"],
        power: 2,
        toughness: 1,
        keyword: Keyword::Flying,
    },
    Card {
        id: "phantasmal_abomination",
        name: "Phantasmal Abomination",
        mana: "{1}{U}{U}",
        types: &["Creature", "Illusion"],
        power: 5,
        toughness: 5,
        keyword: Keyword::Defender,
    },
    Card {
        id: "phantasmal_dreadmaw",
        name: "Phantasmal Dreadmaw",
        mana: "{2}{U}{U}",
        types: &["Creature", "Dinosaur", "Illusion"],
        power: 6,
        toughness: 6,
        keyword: Keyword::Trample,
    },
];

#[test]
fn q29_target_sacrifice_batch_maps_definitions() {
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
        assert_eq!(f.keywords, [card.keyword], "{}", card.id);
        assert!(f.static_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.activated_abilities.is_empty());
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one trigger", card.id);
        };
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverSelfBecomesTarget {
                source: TargetingSourceFilter::SpellOrAbility,
                source_controller: CastTriggerPlayer::AnyPlayer,
            },
            "{}",
            card.id
        );
        assert!(!ability.may, "{}", card.id);
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        assert_eq!(
            *effect,
            SpellEffectKind::Sacrifice {
                subject: EffectSubject::Source
            },
            "{}",
            card.id
        );
    }
}

#[test]
fn q29_target_sacrifice_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "illusionary_servant",
            "Illusionary Servant",
            "b18ee5e92c6856e7950bb1aeeb92e5f84c54822007979a955188bf7f4d30acf0",
        ),
        (
            "gossamer_phantasm",
            "Gossamer Phantasm",
            "b18ee5e92c6856e7950bb1aeeb92e5f84c54822007979a955188bf7f4d30acf0",
        ),
        (
            "phantasmal_dragon",
            "Phantasmal Dragon",
            "b18ee5e92c6856e7950bb1aeeb92e5f84c54822007979a955188bf7f4d30acf0",
        ),
        (
            "skulking_ghost",
            "Skulking Ghost",
            "b18ee5e92c6856e7950bb1aeeb92e5f84c54822007979a955188bf7f4d30acf0",
        ),
        (
            "phantasmal_abomination",
            "Phantasmal Abomination",
            "87bf73e8f87f9f3567adacab3b1f8a5500247c4c5f1bcf1ce56cbb92ad3a21ac",
        ),
        (
            "phantasmal_dreadmaw",
            "Phantasmal Dreadmaw",
            "ad7c08c677fbfc0dc376b1bcfa9ca9072fa58b0fbb8b5c1f2aa6f3bcf71c7fc1",
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
