#![allow(unused_imports)]
//! Registry conformance for the queue-v35 reach pump spells cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CombatRestrictionScope, EffectSubject, EntersTappedAffected,
    SpellEffectKind, StaticAbilityDef, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    face_id: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    keywords: &'static [Keyword],
}

const CARDS: &[Card] = &[
    Card {
        id: "shape_the_sands",
        name: "Shape the Sands",
        mana_cost: "{G}",
        face_id: "shape_the_sands",
        fingerprint: "ff125f92f6dde169222279742ed8adb44012c19e2228242e7ddc42cc21760a76",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "silk_net",
        name: "Silk Net",
        mana_cost: "{G}",
        face_id: "silk_net",
        fingerprint: "06417a69b1c6c558ec6405482b832795dcb8e4f09a28edac2d7e63ed76319c43",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "snare_the_skies",
        name: "Snare the Skies",
        mana_cost: "{G}",
        face_id: "snare_the_skies",
        fingerprint: "06417a69b1c6c558ec6405482b832795dcb8e4f09a28edac2d7e63ed76319c43",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q35_reach_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.face_id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types.to_vec()
        );
        match card.body {
            Some((power, toughness)) => {
                assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)))
            }
            None => assert_eq!((f.power, f.toughness), (None, None)),
        }
        assert_eq!(f.keywords, card.keywords);
        match card.id {
            "shape_the_sands" => {
                assert_eq!(f.spell_effect.len(), 2);
                assert!(
                    matches!(f.spell_effect.as_slice(), [SpellEffectKind::PumpTarget { power: 0, toughness: 5, subject: EffectSubject::Chosen(_), .. }, SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }] if keywords.as_slice() == [Keyword::Reach])
                );
                let groups = &f.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0, 1]);
            }
            "silk_net" => {
                assert_eq!(f.spell_effect.len(), 2);
                assert!(
                    matches!(f.spell_effect.as_slice(), [SpellEffectKind::PumpTarget { power: 1, toughness: 1, subject: EffectSubject::Chosen(_), .. }, SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }] if keywords.as_slice() == [Keyword::Reach])
                );
                let groups = &f.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0, 1]);
            }
            "snare_the_skies" => {
                assert_eq!(f.spell_effect.len(), 2);
                assert!(
                    matches!(f.spell_effect.as_slice(), [SpellEffectKind::PumpTarget { power: 1, toughness: 1, subject: EffectSubject::Chosen(_), .. }, SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }] if keywords.as_slice() == [Keyword::Reach])
                );
                let groups = &f.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0, 1]);
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q35_reach_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.face_id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
