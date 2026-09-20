//! Issue #459 registry and presentation conformance for the Devotee tri-color mana, fixed Endure,
//! and fixed Mobilize generator families.
//!
//! Abzan Devotee prints `{1}: Add {W}, {B}, or {G}. Activate only once each turn.` plus the
//! shipped graveyard activation; Fortress Kin-Guard prints `When this creature enters, it
//! endures 1. (...)`, and Dalkovan Packbeasts prints `Vigilance` plus `Mobilize 3 (...)`, all in
//! the pinned Scryfall snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`. Exact-name records and
//! `rulings_uri` responses were fetched 2026-09-20 with zero failures. The Fortress Kin-Guard
//! ruling confirms the resolution-time counter-or-Spirit choice and that a cannot-receive-counter
//! creature (for example one no longer on the battlefield) just creates the Spirit; the Dalkovan
//! Packbeasts ruling confirms the Warrior tokens enter attacking without having been declared as
//! attackers, so attack triggers do not fire for them. The expectations below are the reviewed
//! printed Oracle behavior and the shipped typed vocabulary, not a copy of generator output.
//! CR 602.5b (activation restrictions), CR 701.63 (Endure, the keyword-action number in the
//! 2026-09-25 official rules), CR 702.181 (Mobilize), CR 508.4 (put onto the battlefield
//! attacking is not "attacked"), CR 111.1 (tokens), and CR 122.1 (+1/+1 counters) govern the
//! asserted shapes.

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    ActivationLimit, CounterKind, DelayedTokenSacrificeTiming, EffectSubject, ManaAmount,
    PlayerRecipient, ResolutionBranchRequirement, ResolutionBranchSelection, ResolutionCost,
    SpellEffectKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, ActivationTiming, Amount, CardFace,
    CardRegistry, Keyword, ManaCost,
};

fn face(id: &str) -> &'static CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .primary_face()
}

fn mana(w: u32, u: u32, b: u32, r: u32, g: u32) -> ManaAmount {
    ManaAmount {
        w,
        u,
        b,
        r,
        g,
        c: 0,
    }
}

#[test]
fn issue_459_registers_the_three_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana_cost, types, keywords, power_toughness) in [
        (
            "abzan_devotee",
            "Abzan Devotee",
            "abzan_devotee",
            "{1}{B}",
            &["Creature", "Dog", "Cleric"][..],
            &[][..],
            Some((2, 1)),
        ),
        (
            "fortress_kin-guard",
            "Fortress Kin-Guard",
            "fortress_kin_guard",
            "{1}{W}",
            &["Creature", "Dog", "Soldier"][..],
            &[][..],
            Some((1, 2)),
        ),
        (
            "dalkovan_packbeasts",
            "Dalkovan Packbeasts",
            "dalkovan_packbeasts",
            "{2}{W}",
            &["Creature", "Ox"][..],
            &[Keyword::Vigilance][..],
            Some((0, 4)),
        ),
    ] {
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
fn issue_459_abzan_devotee_owns_the_tri_color_mana_and_graveyard_abilities_in_order() {
    let face = face("abzan_devotee");
    assert!(face.spell_effect.is_empty() && face.triggered_abilities.is_empty());
    let [mana_ability, graveyard_ability] = face.activated_abilities.as_slice() else {
        panic!("Abzan Devotee must own exactly the two reviewed activated abilities");
    };
    assert_eq!(mana_ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        mana_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(mana_ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        mana_ability.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{1}").expect("reviewed ability cost")
        )]
    );
    assert_eq!(
        mana_ability.effect,
        [SpellEffectKind::ProduceMana {
            options: vec![
                mana(1, 0, 0, 0, 0),
                mana(0, 0, 1, 0, 0),
                mana(0, 0, 0, 0, 1)
            ],
            restriction: None,
            conditional: None,
        }]
    );
    assert_eq!(
        mana_ability.activation_limit,
        Some(ActivationLimit::PerTurn { max_activations: 1 })
    );
    assert_eq!(mana_ability.timing, ActivationTiming::Normal);
    assert!(mana_ability.targeting.is_none());
    assert!(mana_ability.conditions.is_empty());

    assert_eq!(graveyard_ability.ability_id.as_str(), "activated_02");
    assert_eq!(
        graveyard_ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(graveyard_ability.source_zone, AbilitySourceZone::Graveyard);
    assert_eq!(
        graveyard_ability.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{2}{B}").expect("reviewed ability cost")
        )]
    );
    assert_eq!(
        graveyard_ability.effect,
        [SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Source,
        }]
    );
    assert_eq!(graveyard_ability.activation_limit, None);
    assert_eq!(graveyard_ability.timing, ActivationTiming::Normal);
    assert!(graveyard_ability.targeting.is_none());
    assert!(graveyard_ability.conditions.is_empty());
}

#[test]
fn issue_459_fortress_kin_guard_endures_one_with_the_exact_branch_shape() {
    let face = face("fortress_kin-guard");
    assert!(
        face.spell_effect.is_empty()
            && face.activated_abilities.is_empty()
            && face.static_abilities.is_empty()
    );
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Fortress Kin-Guard must own exactly one triggered ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.trigger,
        tricerules_cards::TriggerCondition::WhenSelfEntersBattlefield
    );
    assert!(!ability.may && ability.intervening_if.is_none() && !ability.triggers_only_once);
    assert!(ability.targeting.is_none());
    let [SpellEffectKind::ChooseResolutionBranch {
        chooser,
        optional,
        selection,
        branches,
        otherwise,
    }] = ability.effect.as_slice()
    else {
        panic!("Fortress Kin-Guard must emit exactly one resolution-branch instruction");
    };
    assert_eq!(*chooser, PlayerRecipient::SourceController);
    assert!(!*optional);
    assert_eq!(*selection, ResolutionBranchSelection::PlayerChoice);
    assert!(otherwise.is_empty());
    let [counters, spirit] = branches.as_slice() else {
        panic!("endure must offer exactly the counter and Spirit branches");
    };
    assert_eq!(counters.branch_id.as_str(), "put_a_1_1_counter_on_source");
    assert_eq!(
        counters.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(counters.cost, ResolutionCost::None);
    assert_eq!(
        counters.requirement,
        ResolutionBranchRequirement::EffectsApplicable
    );
    assert_eq!(
        counters.effects,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    );
    assert_eq!(
        spirit.branch_id.as_str(),
        "create_a_1_1_white_spirit_creature_token"
    );
    assert_eq!(
        spirit.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(spirit.cost, ResolutionCost::None);
    assert_eq!(spirit.requirement, ResolutionBranchRequirement::Always);
    assert_eq!(
        spirit.effects,
        [SpellEffectKind::CreateTokens {
            token: "spirit_w_1_1".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::SourceController,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
}

#[test]
fn issue_459_dalkovan_packbeasts_carries_vigilance_and_the_fixed_mobilize_three_trigger() {
    let face = face("dalkovan_packbeasts");
    assert_eq!(face.keywords, [Keyword::Vigilance]);
    assert!(
        face.spell_effect.is_empty()
            && face.activated_abilities.is_empty()
            && face.static_abilities.is_empty()
    );
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Dalkovan Packbeasts must own exactly one triggered ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.trigger,
        tricerules_cards::TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert!(!ability.may && ability.intervening_if.is_none() && !ability.triggers_only_once);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::CreateAttackingTokens {
            token: "warrior_r_1_1".into(),
            count: Amount::Fixed(3),
            sacrifice_timing: Some(DelayedTokenSacrificeTiming::NextEndStep),
        }]
    );
}

#[test]
fn issue_459_handwritten_devotee_anchors_keep_the_recipe_shapes() {
    for (id, options) in [
        (
            "mardu_devotee",
            [
                mana(0, 0, 0, 1, 0),
                mana(1, 0, 0, 0, 0),
                mana(0, 0, 1, 0, 0),
            ],
        ),
        (
            "jeskai_devotee",
            [
                mana(0, 1, 0, 0, 0),
                mana(0, 0, 0, 1, 0),
                mana(1, 0, 0, 0, 0),
            ],
        ),
        (
            "temur_devotee",
            [
                mana(0, 0, 0, 0, 1),
                mana(0, 1, 0, 0, 0),
                mana(0, 0, 0, 1, 0),
            ],
        ),
        (
            "sultai_devotee",
            [
                mana(0, 0, 1, 0, 0),
                mana(0, 0, 0, 0, 1),
                mana(0, 1, 0, 0, 0),
            ],
        ),
    ] {
        let face = face(id);
        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id} must own exactly one activated ability");
        };
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield, "{id}");
        assert_eq!(
            ability.costs,
            [AbilityCost::Mana(
                ManaCost::parse("{1}").expect("reviewed ability cost")
            )],
            "{id}"
        );
        let [SpellEffectKind::ProduceMana {
            options: actual,
            restriction: None,
            conditional: None,
        }] = ability.effect.as_slice()
        else {
            panic!("{id} must print the mana effect");
        };
        assert_eq!(actual.as_slice(), options.as_slice(), "{id}");
        assert_eq!(
            ability.activation_limit,
            Some(ActivationLimit::PerTurn { max_activations: 1 }),
            "{id}"
        );
    }
}

#[test]
fn issue_459_handwritten_endure_anchors_keep_the_recipe_shapes() {
    for (id, count, token) in [
        ("kin-tree_nurturer", 1, "spirit_w_1_1"),
        ("sandskitter_outrider", 2, "spirit_w_2_2"),
        ("dusyut_earthcarver", 3, "spirit_w_3_3"),
    ] {
        let face = face(id);
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must own exactly one triggered ability");
        };
        assert_eq!(
            ability.trigger,
            tricerules_cards::TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        let [SpellEffectKind::ChooseResolutionBranch {
            chooser,
            optional,
            branches,
            ..
        }] = ability.effect.as_slice()
        else {
            panic!("{id} must emit one resolution-branch instruction");
        };
        assert_eq!(*chooser, PlayerRecipient::SourceController, "{id}");
        assert!(!*optional, "{id}");
        let [counters, spirit] = branches.as_slice() else {
            panic!("{id} must offer the counter and Spirit branches");
        };
        assert_eq!(
            counters.requirement,
            ResolutionBranchRequirement::EffectsApplicable,
            "{id}"
        );
        assert_eq!(
            counters.effects,
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(count),
                subject: EffectSubject::Source,
            }],
            "{id}"
        );
        assert_eq!(
            spirit.requirement,
            ResolutionBranchRequirement::Always,
            "{id}"
        );
        assert_eq!(
            spirit.effects,
            [SpellEffectKind::CreateTokens {
                token: token.into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::SourceController,
                tapped: false,
                sacrifice_timing: None,
            }],
            "{id}"
        );
    }
}

#[test]
fn issue_459_handwritten_mobilize_anchors_keep_the_recipe_shapes() {
    let expected = [SpellEffectKind::CreateAttackingTokens {
        token: "warrior_r_1_1".into(),
        count: Amount::Fixed(1),
        sacrifice_timing: Some(DelayedTokenSacrificeTiming::NextEndStep),
    }];
    for id in [
        "reigning_victor",
        "dragonback_lancer",
        "shock_brigade",
        "nightblade_brigade",
    ] {
        let face = face(id);
        let ability = face
            .triggered_abilities
            .iter()
            .find(|ability| {
                ability.trigger
                    == tricerules_cards::TriggerCondition::WheneverSelfAttacks {
                        minimum_other_attackers: 0,
                    }
            })
            .unwrap_or_else(|| panic!("{id} must keep its mobilize attack trigger"));
        assert_eq!(ability.effect, expected, "{id}");
    }
}
