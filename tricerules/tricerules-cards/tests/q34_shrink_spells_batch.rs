#![allow(unused_imports)]
//! Registry conformance for the queue-v34 +P/-T shrink spells cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CombatRole, EffectSubject, GameCondition, LifeAmount, PlayerRecipient,
    PowerComparison, RelativePlayerSet, SpellEffectKind, StaticAbilityDef, TargetKind,
    TriggerCondition,
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
        id: "auger_spree",
        name: "Auger Spree",
        mana_cost: "{1}{B}{R}",
        face_id: "auger_spree",
        fingerprint: "c8159337ea684d9050a352740bf408f2277a2cd2237579b023c6dd43f6a88ff5",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "flowstone_infusion",
        name: "Flowstone Infusion",
        mana_cost: "{R}",
        face_id: "flowstone_infusion",
        fingerprint: "cc3ba20365e450728ae46db493860a947250384ecb8302e5bedc1b10884a8d52",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "lash_of_malice",
        name: "Lash of Malice",
        mana_cost: "{B}",
        face_id: "lash_of_malice",
        fingerprint: "cc3ba20365e450728ae46db493860a947250384ecb8302e5bedc1b10884a8d52",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q34_shrink_spells_batch_maps_definitions() {
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
            "auger_spree" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 4,
                        toughness: -4,
                        ..
                    }]
                ));
            }
            "flowstone_infusion" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 2,
                        toughness: -2,
                        ..
                    }]
                ));
            }
            "lash_of_malice" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 2,
                        toughness: -2,
                        ..
                    }]
                ));
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q34_shrink_spells_batch_fingerprints_match_the_pinned_oracle_text() {
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
