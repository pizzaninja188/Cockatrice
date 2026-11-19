//! Registry conformance for the queue-v37 08-landfall-keyword cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{
    EffectSubject, PermanentTypeFilter, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    cost: &'static str,
    keywords: &'static [Keyword],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "belligerent_whiptail",
        name: "Belligerent Whiptail",
        mana_cost: "{3}{R}",
        types: &["Creature", "Wurm"],
        power: Some(4),
        toughness: Some(2),
        cost: "",
        keywords: &[Keyword::FirstStrike],
        fingerprint: "a91083030af339844cb81874e9ea09b3e7a53b80bbfb909c1342b14a03e05add",
    },
    Card {
        id: "fledgling_griffin",
        name: "Fledgling Griffin",
        mana_cost: "{1}{W}",
        types: &["Creature", "Griffin"],
        power: Some(2),
        toughness: Some(2),
        cost: "",
        keywords: &[Keyword::Flying],
        fingerprint: "3ac23039f54f70d71f90c2e8a9dc63ac82a30b1b20d272e5ae0d05bd6ce6f9c6",
    },
    Card {
        id: "geyser_glider",
        name: "Geyser Glider",
        mana_cost: "{3}{R}{R}",
        types: &["Creature", "Elemental", "Beast"],
        power: Some(4),
        toughness: Some(4),
        cost: "",
        keywords: &[Keyword::Flying],
        fingerprint: "3ac23039f54f70d71f90c2e8a9dc63ac82a30b1b20d272e5ae0d05bd6ce6f9c6",
    },
    Card {
        id: "snapping_creeper",
        name: "Snapping Creeper",
        mana_cost: "{2}{G}",
        types: &["Creature", "Plant"],
        power: Some(2),
        toughness: Some(3),
        cost: "",
        keywords: &[Keyword::Vigilance],
        fingerprint: "44b0e75512cb33fce4fc3f18c1f2c4522abc3d92e44d75b1d21a9319c3752b99",
    },
];

#[test]
fn q37_landfall_keyword_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(card.cost, "", "no activation cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types
        );
        assert_eq!((f.power, f.toughness), (card.power, card.toughness));
        assert!(f.keywords.is_empty(), "{}: no printed keyword", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one triggered ability", card.id);
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(
            matches!(
                &trigger.trigger,
                TriggerCondition::WheneverPermanentEntersBattlefield { filter, .. }
                    if filter.permanent_type == Some(PermanentTypeFilter::Land)
            ),
            "{}: landfall trigger",
            card.id
        );
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::GrantKeywords { subject, keywords }]
                    if *subject == EffectSubject::Source && keywords == card.keywords
            ),
            "{}: source keyword grant",
            card.id
        );
        assert!(trigger.targeting.is_none(), "{}", card.id);
    }
}

#[test]
fn q37_landfall_keyword_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
