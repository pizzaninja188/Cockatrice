//! Issue #413 — registry identities for the reminder-text Cycling batch.
//!
//! Oracle text verified against the pinned Scryfall `oracle_cards` bulk
//! (`27bf3214-1271-490b-bdfe-c0be6c23d02e`, compressed SHA-256
//! `9611b5d93b20478a0ee46bae8b20a9eb39ee980f0ef4f5f6f6aaa8f7ab010ab2`).
//! CR 702.29a defines Cycling as a hand-only activated ability with mana and discard costs
//! followed by a draw; CR 701.8 governs destruction, CR 120 damage, CR 208 toughness, and
//! CR 122.1d / 701.26 stun counters and tapping.

use tricerules_cards::primitives::{
    Amount, EffectSubject, PlayerRecipient, PowerComparison, RelativePlayerSet, TargetFilter,
    TargetKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, CardRegistry, CounterKind, Keyword,
    ManaCost, PermanentTypeFilter, SpellEffectKind,
};

fn issue_413_cycling_face(card_id: &str) -> tricerules_cards::CardFace {
    let registry = CardRegistry::global();
    let face = registry
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} must be registered"))
        .primary_face()
        .clone();
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("{card_id} must have exactly one Cycling ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2]),
        "{card_id} maps its hand ability to the printed Cycling line"
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Hand);
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}").expect("printed Cycling cost")),
            AbilityCost::DiscardSelf,
        ],
        "{card_id} pays the printed Cycling {{2}} and discards itself"
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }],
        "{card_id} draws exactly one card on resolution"
    );
    assert!(ability.targeting.is_none());
    assert!(
        face.keywords.is_empty(),
        "{card_id} prints no separate keyword ability"
    );
    face
}

#[test]
fn issue_413_airship_crash_is_the_three_branch_union_destroy() {
    let face = issue_413_cycling_face("airship_crash");
    assert_eq!(face.name, "Airship Crash");
    assert_eq!(
        face.mana_cost,
        ManaCost::parse("{2}{G}").expect("printed cost")
    );
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                any_of: Some(vec![
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        permanent_types: vec![PermanentTypeFilter::Artifact],
                        ..TargetFilter::default()
                    },
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        permanent_types: vec![PermanentTypeFilter::Enchantment],
                        ..TargetFilter::default()
                    },
                    TargetFilter {
                        kind: TargetKind::Creature,
                        required_keywords: vec![Keyword::Flying],
                        ..TargetFilter::default()
                    },
                ]),
                ..TargetFilter::default()
            })),
        }]
    );
}

#[test]
fn issue_413_fuel_the_flames_is_the_untargeted_two_damage_sweep() {
    let face = issue_413_cycling_face("fuel_the_flames");
    assert_eq!(face.name, "Fuel the Flames");
    assert_eq!(
        face.mana_cost,
        ManaCost::parse("{2}{R}").expect("printed cost")
    );
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::DamageAll {
            amount: Amount::Fixed(2),
            players: RelativePlayerSet::All,
            kind: TargetFilter::default_creature(),
        }]
    );
    assert!(face.targeting.is_none());
}

#[test]
fn issue_413_gallant_strike_is_the_toughness_four_destroy() {
    let face = issue_413_cycling_face("gallant_strike");
    assert_eq!(face.name, "Gallant Strike");
    assert_eq!(
        face.mana_cost,
        ManaCost::parse("{1}{W}").expect("printed cost")
    );
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                toughness: Some(PowerComparison::AtLeast(4)),
                ..TargetFilter::default()
            })),
        }]
    );
}

#[test]
fn issue_413_stall_out_taps_and_puts_three_stun_counters_on_one_target() {
    let face = issue_413_cycling_face("stall_out");
    assert_eq!(face.name, "Stall Out");
    assert_eq!(
        face.mana_cost,
        ManaCost::parse("{1}{U}").expect("printed cost")
    );
    assert_eq!(face.types, ["Sorcery"]);

    let target = TargetFilter {
        any_of: Some(vec![
            TargetFilter::default_creature(),
            TargetFilter {
                kind: TargetKind::AnyPermanent,
                required_subtypes: vec!["Vehicle".into()],
                ..TargetFilter::default()
            },
        ]),
        ..TargetFilter::default()
    };
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::Tap {
                subject: EffectSubject::Chosen(Box::new(target.clone())),
            },
            SpellEffectKind::PutCounters {
                counter: CounterKind::Stun,
                count: Amount::Fixed(3),
                subject: EffectSubject::Chosen(Box::new(target)),
            },
        ]
    );

    let targeting = face.targeting.expect("Stall Out targets one object");
    let [group] = targeting.groups.as_slice() else {
        panic!("Stall Out must author exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature or Vehicle");
    assert_eq!(group.effect_indices, [0, 1]);
}
