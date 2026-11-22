#![allow(unused_imports)]
//! Registry conformance for the queue-v34 targeted life gain cohort.
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
        id: "heroes_reunion",
        name: "Heroes' Reunion",
        mana_cost: "{G}{W}",
        face_id: "heroes_reunion",
        fingerprint: "f2b4f003191be90b0e3c100f61b0c5125c0695c7f0e7b33ba56416a96cb90c94",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "natural_spring",
        name: "Natural Spring",
        mana_cost: "{3}{G}{G}",
        face_id: "natural_spring",
        fingerprint: "3e3a116675cba98e18dbb38c653614fd8de206bcf6068c1d2b28397e438bd4a9",
        types: &["Sorcery"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "soothing_balm",
        name: "Soothing Balm",
        mana_cost: "{1}{W}",
        face_id: "soothing_balm",
        fingerprint: "b2500aeb87fdeb7b667e2423012b96af798394953ef72496d2b601d089c44b0f",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q34_targeted_lifegain_batch_maps_definitions() {
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
            "heroes_reunion" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(
                    matches!(f.spell_effect.as_slice(), [SpellEffectKind::TargetPlayerGainsLife { amount: Amount::Fixed(7), target }] if target.kind == TargetKind::AnyPlayer)
                );
            }
            "natural_spring" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(
                    matches!(f.spell_effect.as_slice(), [SpellEffectKind::TargetPlayerGainsLife { amount: Amount::Fixed(8), target }] if target.kind == TargetKind::AnyPlayer)
                );
            }
            "soothing_balm" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(
                    matches!(f.spell_effect.as_slice(), [SpellEffectKind::TargetPlayerGainsLife { amount: Amount::Fixed(5), target }] if target.kind == TargetKind::AnyPlayer)
                );
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q34_targeted_lifegain_batch_fingerprints_match_the_pinned_oracle_text() {
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
