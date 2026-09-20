//! Issue #460 registry and presentation conformance for the fixed negative pump, ETB team pump,
//! activated each-opponent damage, and second-spell cost-reduction generator families.
//!
//! Fleeting Distraction prints `Target creature gets -1/-0 until end of turn.` then `Draw a card.`;
//! Overkill prints `Target creature gets -0/-9999 until end of turn.`; Malamet War Scribe prints
//! `When this creature enters, creatures you control get +2/+1 until end of turn.`; Panicked
//! Altisaur prints `Reach` then `{T}: This creature deals 2 damage to each opponent.`; and Uthros
//! Psionicist prints `The second spell you cast each turn costs {2} less to cast.` in the pinned
//! Scryfall snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`. Exact-name records and `rulings_uri`
//! responses were fetched 2026-09-20 with zero failures. The Fleeting Distraction ruling confirms
//! the spell fizzles with no draw when its only target is illegal; the Uthros Psionicist ruling
//! confirms any spell cast this turn is counted (including itself, one that was countered, or one
//! still on the stack) and that the reduction never changes the spell's mana value. The
//! expectations below are the reviewed printed Oracle behavior and the shipped typed vocabulary,
//! not a copy of generator output. CR 611.2a / 613.4c (until-end-of-turn P/T modification),
//! CR 608.2b (target revalidation), CR 514.2 (end-of-turn duration), CR 118.3 (life loss),
//! CR 601.2f (static cost reductions) and CR 601.2a (spells cast this turn) govern them.

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    CreatureScopeController, CreatureScopeFilter, EffectSubject, GameCondition, PlayerRecipient,
    RelativePlayerSet, SpellCastFilter, SpellEffectKind, StaticAbilityDef, TargetFilter,
    TargetKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, ActivationTiming, Amount, CardFace,
    CardRegistry, Keyword, ManaCost, TriggerCondition,
};

fn face(id: &str) -> &'static CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .primary_face()
}

fn creatures_you_control() -> CreatureScopeFilter {
    CreatureScopeFilter {
        controller: Some(CreatureScopeController::YouControl),
        ..CreatureScopeFilter::default()
    }
}

struct ReviewedIdentity {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    keywords: &'static [Keyword],
    power_toughness: Option<(u32, u32)>,
}

const REVIEWED_IDENTITIES: [ReviewedIdentity; 5] = [
    ReviewedIdentity {
        id: "fleeting_distraction",
        name: "Fleeting Distraction",
        face_id: "fleeting_distraction",
        mana_cost: "{U}",
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
    },
    ReviewedIdentity {
        id: "overkill",
        name: "Overkill",
        face_id: "overkill",
        mana_cost: "{2}{B}",
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
    },
    ReviewedIdentity {
        id: "malamet_war_scribe",
        name: "Malamet War Scribe",
        face_id: "malamet_war_scribe",
        mana_cost: "{3}{W}{W}",
        types: &["Creature", "Cat", "Warrior"],
        keywords: &[],
        power_toughness: Some((4, 3)),
    },
    ReviewedIdentity {
        id: "panicked_altisaur",
        name: "Panicked Altisaur",
        face_id: "panicked_altisaur",
        mana_cost: "{4}{R}",
        types: &["Creature", "Dinosaur"],
        keywords: &[Keyword::Reach],
        power_toughness: Some((4, 5)),
    },
    ReviewedIdentity {
        id: "uthros_psionicist",
        name: "Uthros Psionicist",
        face_id: "uthros_psionicist",
        mana_cost: "{2}{U}",
        types: &["Creature", "Jellyfish", "Scientist"],
        keywords: &[],
        power_toughness: Some((2, 4)),
    },
];

#[test]
fn issue_460_registers_the_five_reviewed_identities() {
    let registry = CardRegistry::global();
    for identity in REVIEWED_IDENTITIES {
        let ReviewedIdentity {
            id,
            name,
            face_id,
            mana_cost,
            types,
            keywords,
            power_toughness,
        } = identity;
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        FaceExpectation {
            id,
            name,
            face_id,
            mana_cost,
            types,
            keywords,
            power_toughness,
        }
        .check();
    }
}

#[test]
fn issue_460_fleeting_distraction_and_overkill_own_the_reviewed_negative_pumps() {
    for (id, power, toughness, draws) in [
        ("fleeting_distraction", -1, 0, true),
        ("overkill", 0, -9999, false),
    ] {
        let face = face(id);
        assert!(
            face.static_abilities.is_empty()
                && face.triggered_abilities.is_empty()
                && face.activated_abilities.is_empty(),
            "{id} is a plain spell"
        );
        let mut expected = vec![SpellEffectKind::PumpTarget {
            power,
            toughness,
            scale: None,
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                ..TargetFilter::default()
            })),
        }];
        if draws {
            expected.push(SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            });
        }
        assert_eq!(face.spell_effect, expected, "{id} printed effects in order");
        let targeting = face
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{id} must author its single mandatory target group"));
        let [group] = targeting.groups.as_slice() else {
            panic!("{id} must author exactly one target group");
        };
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(group.prompt, "Choose target creature", "{id}");
        assert_eq!(group.effect_indices, vec![0], "{id}: only the pump targets");
        assert!(group.distinct_from.is_empty(), "{id}");
        assert!(!group.same_graveyard, "{id}");
        assert!(group.cast_cost_expansion.is_none(), "{id}");
    }
}

#[test]
fn issue_460_malamet_war_scribe_pumps_the_team_on_entry() {
    let face = face("malamet_war_scribe");
    assert!(
        face.spell_effect.is_empty()
            && face.static_abilities.is_empty()
            && face.activated_abilities.is_empty()
    );
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Malamet War Scribe must own exactly one triggered ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may && ability.intervening_if.is_none() && !ability.triggers_only_once);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpAll {
            filter: creatures_you_control(),
            power: 2,
            toughness: 1,
        }]
    );
}

#[test]
fn issue_460_panicked_altisaur_taps_to_damage_each_opponent() {
    let face = face("panicked_altisaur");
    assert!(
        face.spell_effect.is_empty()
            && face.static_abilities.is_empty()
            && face.triggered_abilities.is_empty()
    );
    assert_eq!(face.keywords, [Keyword::Reach]);
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Panicked Altisaur must own exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert!(ability.activation_limit.is_none());
    assert!(ability.conditions.is_empty());
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(2),
            who: PlayerRecipient::EachOpponent,
        }]
    );
}

#[test]
fn issue_460_uthros_psionicist_reduces_only_the_second_spell() {
    let face = face("uthros_psionicist");
    assert!(
        face.spell_effect.is_empty()
            && face.triggered_abilities.is_empty()
            && face.activated_abilities.is_empty()
    );
    let [ability] = face.static_abilities.as_slice() else {
        panic!("Uthros Psionicist must own exactly one static ability");
    };
    assert_eq!(ability.ability_id.as_str(), "static_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::SpellGenericReduction {
            casters: RelativePlayerSet::Controller,
            spell_type: None,
            amount: Amount::Fixed(2),
            condition: Some(GameCondition::SpellsCastThisTurn {
                players: RelativePlayerSet::Controller,
                filter: SpellCastFilter::default(),
                min: Some(1),
                max: Some(1),
            }),
        }
    );
}

#[test]
fn issue_460_handwritten_anchors_keep_the_recipe_shapes() {
    let registry = CardRegistry::global();
    for (id, name) in [
        ("inspiring_captain", "Inspiring Captain"),
        ("vindictive_warden", "Vindictive Warden"),
        ("highspire_bell-ringer", "Highspire Bell-Ringer"),
    ] {
        assert_eq!(
            registry.id_for_name(name),
            Some(id),
            "{id} stays unambiguous"
        );
    }

    let captain = face("inspiring_captain");
    let [pump] = captain.triggered_abilities.as_slice() else {
        panic!("Inspiring Captain must keep one triggered ability");
    };
    assert_eq!(
        pump.effect,
        [SpellEffectKind::PumpAll {
            filter: creatures_you_control(),
            power: 1,
            toughness: 1,
        }]
    );

    let warden = face("vindictive_warden");
    let damage = warden
        .activated_abilities
        .iter()
        .find(|ability| {
            ability.costs
                == [AbilityCost::Mana(
                    ManaCost::parse("{3}").expect("reviewed ability cost"),
                )]
        })
        .unwrap_or_else(|| panic!("Vindictive Warden must keep its {{3}} ability"));
    assert_eq!(
        damage.effect,
        [SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(1),
            who: PlayerRecipient::EachOpponent,
        }]
    );

    let bell_ringer = face("highspire_bell-ringer");
    let [reduction] = bell_ringer.static_abilities.as_slice() else {
        panic!("Highspire Bell-Ringer must keep one static ability");
    };
    assert_eq!(
        reduction.definition,
        StaticAbilityDef::SpellGenericReduction {
            casters: RelativePlayerSet::Controller,
            spell_type: None,
            amount: Amount::Fixed(1),
            condition: Some(GameCondition::SpellsCastThisTurn {
                players: RelativePlayerSet::Controller,
                filter: SpellCastFilter::default(),
                min: Some(1),
                max: Some(1),
            }),
        }
    );
}
