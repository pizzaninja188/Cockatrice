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

use crate::common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    ActivationLimit, CounterKind, DelayedTokenSacrificeTiming, EffectSubject, ManaAmount,
    PlayerRecipient, ResolutionBranchRequirement, ResolutionCost, SpellEffectKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, ActivationTiming, Amount, CardFace,
    InstructionKind, Keyword, ManaCost,
};

fn face(id: &str) -> &'static CardFace {
    tricerules_cards::registry::global()
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
    let registry = tricerules_cards::registry::global();
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
            commander_color_identity: false,
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
fn issue_459_all_four_fixed_endure_owners_use_the_ordered_program_contract() {
    for (id, line, count, counter_branch, token_branch, token_id, branch_presentation) in [
        (
            "kin-tree_nurturer",
            2,
            1,
            "put_a_1_1_counter_on_kin_tree_nurturer",
            "create_a_1_1_white_spirit_creature_token",
            "spirit_w_1_1",
            AbilityPresentation::Fallback,
        ),
        (
            "sandskitter_outrider",
            2,
            2,
            "put_two_1_1_counters_on_sandskitter_outrider",
            "create_a_2_2_white_spirit_creature_token",
            "spirit_w_2_2",
            AbilityPresentation::Fallback,
        ),
        (
            "dusyut_earthcarver",
            2,
            3,
            "put_three_1_1_counters_on_dusyut_earthcarver",
            "create_a_3_3_white_spirit_creature_token",
            "spirit_w_3_3",
            AbilityPresentation::Fallback,
        ),
        (
            "fortress_kin-guard",
            1,
            1,
            "put_a_1_1_counter_on_source",
            "create_a_1_1_white_spirit_creature_token",
            "spirit_w_1_1",
            AbilityPresentation::OracleLines(vec![1]),
        ),
    ] {
        let face = face(id);
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must retain its one Endure entry ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![line]),
            "{id}"
        );
        assert_eq!(
            ability.trigger,
            tricerules_cards::TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(
            ability.effect.is_empty(),
            "{id} must move to its Program body"
        );
        let program = ability
            .program
            .as_ref()
            .unwrap_or_else(|| panic!("{id} must use the ordered Endure program"));
        assert!(program.target_bindings.is_empty(), "{id}");
        assert!(program.body.yields.is_empty(), "{id}");
        let [choose] = program.body.steps.as_slice() else {
            panic!("{id} must have one top-level choice");
        };
        assert!(choose.bind.is_empty() && choose.input.is_none(), "{id}");
        let InstructionKind::Choose {
            choice,
            chooser,
            optional,
            branches,
            otherwise,
        } = &choose.kind
        else {
            panic!("{id} must use Program Choose");
        };
        assert_eq!(choice.as_str(), "endure", "{id}");
        assert_eq!(*chooser, PlayerRecipient::SourceController, "{id}");
        assert!(!optional, "{id}");
        assert!(
            otherwise.steps.is_empty() && otherwise.yields.is_empty(),
            "{id}"
        );
        let [counters, spirit] = branches.as_slice() else {
            panic!("{id} must retain exactly its counter and token branches");
        };
        assert_eq!(counters.branch_id.as_str(), counter_branch, "{id}");
        assert_eq!(spirit.branch_id.as_str(), token_branch, "{id}");
        assert_eq!(counters.presentation, branch_presentation, "{id}");
        assert_eq!(spirit.presentation, branch_presentation, "{id}");
        assert_eq!(counters.cost, ResolutionCost::None, "{id}");
        assert_eq!(spirit.cost, ResolutionCost::None, "{id}");
        assert_eq!(
            counters.requirement,
            ResolutionBranchRequirement::EffectsApplicable,
            "{id}"
        );
        assert_eq!(
            spirit.requirement,
            ResolutionBranchRequirement::Always,
            "{id}"
        );
        let [counter_step] = counters.body.steps.as_slice() else {
            panic!("{id} counter branch must remain one leaf");
        };
        assert!(
            counter_step.bind.is_empty() && counter_step.input.is_none(),
            "{id}"
        );
        assert!(counters.body.yields.is_empty(), "{id}");
        let InstructionKind::Perform(effect) = &counter_step.kind else {
            panic!("{id} counter branch must be a resultless PutCounters leaf");
        };
        assert_eq!(
            effect.as_ref(),
            &SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(count),
                subject: EffectSubject::Source,
            },
            "{id}"
        );
        let [token_step] = spirit.body.steps.as_slice() else {
            panic!("{id} token branch must remain one leaf");
        };
        assert!(
            token_step.bind.is_empty() && token_step.input.is_none(),
            "{id}"
        );
        assert!(spirit.body.yields.is_empty(), "{id}");
        let InstructionKind::Perform(effect) = &token_step.kind else {
            panic!("{id} token branch must be a resultless CreateTokens leaf");
        };
        assert_eq!(
            effect.as_ref(),
            &SpellEffectKind::CreateTokens {
                token: token_id.into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::SourceController,
                tapped: false,
                sacrifice_timing: None,
            },
            "{id}"
        );
    }
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
            commander_color_identity: false,
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
