//! Registry conformance for the queue-v37 07-grant-keyword cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

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
        id: "battle-rage_blessing",
        name: "Battle-Rage Blessing",
        mana_cost: "{1}{B}",
        types: &["Instant"],
        power: None,
        toughness: None,
        cost: "{1}{B}",
        keywords: &[Keyword::Deathtouch, Keyword::Indestructible],
        fingerprint: "db2a20d2c23c4e23fbb744661572a6082eb4d22c6e4ba340bd779f36b429ceec",
    },
    Card {
        id: "jump",
        name: "Jump",
        mana_cost: "{U}",
        types: &["Instant"],
        power: None,
        toughness: None,
        cost: "{U}",
        keywords: &[Keyword::Flying],
        fingerprint: "c48522980e2af999adbc793d489037e75d3d4527d516fd661a2fe171dbdcb6ba",
    },
    Card {
        id: "serpents_gift",
        name: "Serpent's Gift",
        mana_cost: "{2}{G}",
        types: &["Instant"],
        power: None,
        toughness: None,
        cost: "{2}{G}",
        keywords: &[Keyword::Deathtouch],
        fingerprint: "145e556d94b3f5de17ce214142044245d2eada57cb4cdf770a77233c4792a131",
    },
    Card {
        id: "unnatural_speed",
        name: "Unnatural Speed",
        mana_cost: "{R}",
        types: &["Instant", "Arcane"],
        power: None,
        toughness: None,
        cost: "{R}",
        keywords: &[Keyword::Haste],
        fingerprint: "715ad6bc67229f0f257405d09285d6490b4749025f5551c91c42cdb9957e06d7",
    },
    Card {
        id: "withstand_death",
        name: "Withstand Death",
        mana_cost: "{G}",
        types: &["Instant"],
        power: None,
        toughness: None,
        cost: "{G}",
        keywords: &[Keyword::Indestructible],
        fingerprint: "02c219d7eb74c962707b1e000f29d41955ef3a4931258b06bfe4b0768bc10d7a",
    },
];

#[test]
fn q37_grant_keyword_batch_maps_definitions() {
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
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::GrantKeywords { subject, keywords }]
                    if matches!(subject, EffectSubject::Chosen(filter)
                        if filter.kind == TargetKind::Creature)
                        && keywords == card.keywords
            ),
            "{}: targeted keyword grant",
            card.id
        );
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
        assert_eq!(targeting.groups[0].effect_indices, [0]);
    }
}

#[test]
fn q37_grant_keyword_batch_fingerprints_match_the_pinned_oracle_text() {
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
