#![allow(unused_imports)]
//! Registry conformance for the queue-v34 prevention shields cohort.
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
        id: "hold_at_bay",
        name: "Hold at Bay",
        mana_cost: "{1}{W}",
        face_id: "hold_at_bay",
        fingerprint: "bebd062480b9a2d90af48d4839ab0318ecf3bac44e1b661c9f573964e1922929",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "mending_hands",
        name: "Mending Hands",
        mana_cost: "{W}",
        face_id: "mending_hands",
        fingerprint: "fa75102d53e79d0c068cb9ee04b5b8ad27fcb99c71a285080e769d84d3674204",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "shieldmates_blessing",
        name: "Shieldmate's Blessing",
        mana_cost: "{W}",
        face_id: "shieldmates_blessing",
        fingerprint: "23d59c65bf1db8b6e9392f2c9181d7526ce35f679d995e0b63698457dc700501",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q34_prevention_shields_batch_maps_definitions() {
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
            "hold_at_bay" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(
                    matches!(f.spell_effect.as_slice(), [SpellEffectKind::PreventNextDamage { amount: 7, target }] if target.kind == TargetKind::AnyTarget)
                );
            }
            "mending_hands" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(
                    matches!(f.spell_effect.as_slice(), [SpellEffectKind::PreventNextDamage { amount: 4, target }] if target.kind == TargetKind::AnyTarget)
                );
            }
            "shieldmates_blessing" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(
                    matches!(f.spell_effect.as_slice(), [SpellEffectKind::PreventNextDamage { amount: 3, target }] if target.kind == TargetKind::AnyTarget)
                );
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q34_prevention_shields_batch_fingerprints_match_the_pinned_oracle_text() {
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
