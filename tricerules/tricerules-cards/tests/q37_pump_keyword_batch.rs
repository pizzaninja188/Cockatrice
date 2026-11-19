//! Registry conformance for the queue-v37 06-pump-keyword cohort.
//! Printed identity, costs, typed keyword shape and pinned fingerprints.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::{Keyword, Layout};

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
        id: "aerial_maneuver",
        name: "Aerial Maneuver",
        mana_cost: "{1}{W}",
        types: &["Instant"],
        power: None,
        toughness: None,
        cost: "{1}{W}",
        keywords: &[Keyword::Flying, Keyword::FirstStrike],
        fingerprint: "bb9a948dd89f192ca5f344c0f435dfb4b5b6c8b34c36318e1afaf35f1f420199",
    },
    Card {
        id: "angelic_blessing",
        name: "Angelic Blessing",
        mana_cost: "{2}{W}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        cost: "{2}{W}",
        keywords: &[Keyword::Flying],
        fingerprint: "5218dc03d6f00ed9809095c3a558858902089ffe0eae73f840b8c297c8f47483",
    },
    Card {
        id: "coat_with_venom",
        name: "Coat with Venom",
        mana_cost: "{B}",
        types: &["Instant"],
        power: None,
        toughness: None,
        cost: "{B}",
        keywords: &[Keyword::Deathtouch],
        fingerprint: "3a9bc19d26ef3ca7977e18d236df8be5b360d9f524090eef93490aec347a625e",
    },
    Card {
        id: "fervent_strike",
        name: "Fervent Strike",
        mana_cost: "{R}",
        types: &["Instant"],
        power: None,
        toughness: None,
        cost: "{R}",
        keywords: &[Keyword::FirstStrike, Keyword::Haste],
        fingerprint: "736fd8986c9ecf9a361a41157b127f06b352ae1981986615f555de40fc535c5b",
    },
    Card {
        id: "gift_of_strength",
        name: "Gift of Strength",
        mana_cost: "{1}{G}",
        types: &["Instant"],
        power: None,
        toughness: None,
        cost: "{1}{G}",
        keywords: &[Keyword::Reach],
        fingerprint: "3493b3e860ad5fcac475c547a4c5cba9bcdd567ad3380a8c7ab61c9639c94b98",
    },
];

#[test]
fn q37_pump_keyword_batch_maps_definitions() {
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
        assert_eq!(card.cost, card.mana_cost, "printed mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types
        );
        assert_eq!((f.power, f.toughness), (card.power, card.toughness));
        assert!(f.keywords.is_empty(), "{}: no printed keyword", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::PumpTarget { .. }, SpellEffectKind::GrantKeywords { subject, keywords }]
                    if matches!(subject, EffectSubject::Chosen(filter)
                        if filter.kind == TargetKind::Creature)
                        && keywords == card.keywords
            ),
            "{}: pump then the printed keywords",
            card.id
        );
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1, "{}", card.id);
        assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
        assert_eq!(targeting.groups[0].effect_indices, [0, 1]);
    }
}

#[test]
fn q37_pump_keyword_batch_fingerprints_match_the_pinned_oracle_text() {
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
