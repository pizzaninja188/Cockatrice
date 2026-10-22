//! Registry and presentation conformance for issue #344.
//!
//! The eight generated Standard identities must register with their complete typed payloads:
//! CR 603.2/121 the controller's second-spell cast counter; CR 603.6a/611.2a the landfall +1/+0
//! self pump; CR 701.6/701.9 the unrestricted counter followed by the loot; CR 701.9/121 the
//! discard-then-draw-two; CR 301.5/701.3/111.10a the Equipment Ally token and self-attachment;
//! CR 614.1c/122/508.1 the conditional Raid entry counter; CR 603.6/508.1/120 the intervening Raid
//! damage trigger; and CR 603.6/701.14/115.1 the Aura fight with its up-to-one target.

use crate::common::draw_discard_program::{
    assert_face_draw_discard_steps, AuthoredDrawDiscardExpectation, AuthoredDrawDiscardOrder,
};
use tricerules_cards::primitives::{
    CastTriggerPlayer, CompiledInstructionKind, EffectSubject, EntersWithCountersAffected,
    GameCondition, PermanentEventFilter, PermanentTypeFilter, PlayerRecipient,
    ProgramTargetBinding, RelativePlayerSet, SpellCastFilter, SpellEffectKind, StackSpellFilter,
    StaticAbilityDef, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{
    AbilityPresentation, Amount, Color, CounterKind, Keyword, ProgramOwnerId, TriggerCondition,
};

#[test]
fn issue_344_registers_the_eight_reviewed_identities() {
    let registry = tricerules_cards::registry::global();
    for (id, name) in [
        ("goblin_boarders", "Goblin Boarders"),
        ("gorehorn_raider", "Gorehorn Raider"),
        ("icecave_crasher", "Icecave Crasher"),
        ("illvoi_operative", "Illvoi Operative"),
        ("kyoshi_battle_fan", "Kyoshi Battle Fan"),
        ("pitiless_fists", "Pitiless Fists"),
        ("refute", "Refute"),
        ("romantic_rendezvous", "Romantic Rendezvous"),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
    }
}

#[test]
fn issue_344_illvoi_operative_counts_the_second_spell() {
    let definition = tricerules_cards::registry::global()
        .get("illvoi_operative")
        .expect("Illvoi Operative");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Creature", "Jellyfish", "Rogue"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    assert_eq!(face.colors(), vec![Color::Blue]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Illvoi Operative must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            filter: SpellCastFilter::default(),
            ordinal: Some(2),
            ordinal_scope: Default::default(),
        }
    );
    assert!(!ability.may);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    );
}

#[test]
fn issue_344_icecave_crasher_keeps_trample_and_landfall() {
    let definition = tricerules_cards::registry::global()
        .get("icecave_crasher")
        .expect("Icecave Crasher");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{G}");
    assert_eq!(face.types, ["Creature", "Beast"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(4)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!(face.keywords, [Keyword::Trample]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Icecave Crasher must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Land),
                ..PermanentEventFilter::default()
            },
            creature_filter: None,
        }
    );
    assert!(!ability.may);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 0,
            scale: None,
            subject: EffectSubject::Source,
        }]
    );
}

#[test]
fn issue_344_refute_counters_then_loots() {
    let registry = tricerules_cards::registry::global();
    let definition = registry.get("refute").expect("Refute");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{U}{U}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert!(face.spell_effect.is_empty() && face.spell_program.is_some());
    let [counter_target] = face.targeting.as_ref().unwrap().groups.as_slice() else {
        panic!("Refute's counter owns exactly one target group");
    };
    assert_eq!((counter_target.min, counter_target.max), (1, 1));
    let owner = ProgramOwnerId::face("refute", face.face_id.clone());
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("Refute's authored program compiles under its face owner");
    assert_eq!(program.owner, owner);
    assert_eq!(
        program.target_bindings,
        [ProgramTargetBinding {
            instruction_address: 0,
            target_group_index: 0,
            role_index: 0,
        }]
    );
    let [counter, ..] = program.body.steps.as_slice() else {
        panic!("Refute preserves its counter before drawing and discarding");
    };
    assert!(matches!(
        &counter.kind,
        CompiledInstructionKind::Perform(effect)
            if **effect == (SpellEffectKind::CounterTargetSpell {
                spell_filter: StackSpellFilter::default(),
                unless_controller_pays: None,
                unless_controller_pays_by_cast_cost: None,
            })
    ));
    assert_face_draw_discard_steps(
        registry,
        "refute",
        &face.face_id,
        AuthoredDrawDiscardExpectation {
            first_instruction: 1,
            trailing_instructions: 0,
            who: PlayerRecipient::Controller,
            draw_count: 1,
            discard_count: 1,
            order: AuthoredDrawDiscardOrder::DrawThenDiscard,
        },
    );
}

#[test]
fn issue_344_romantic_rendezvous_discards_then_draws_two() {
    let registry = tricerules_cards::registry::global();
    let definition = registry
        .get("romantic_rendezvous")
        .expect("Romantic Rendezvous");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::Red]);
    assert!(face.spell_effect.is_empty() && face.spell_program.is_some());
    assert!(face.targeting.is_none());
    assert_face_draw_discard_steps(
        registry,
        "romantic_rendezvous",
        &face.face_id,
        AuthoredDrawDiscardExpectation {
            first_instruction: 0,
            trailing_instructions: 0,
            who: PlayerRecipient::Controller,
            draw_count: 2,
            discard_count: 1,
            order: AuthoredDrawDiscardOrder::DiscardThenDraw,
        },
    );
}

#[test]
fn issue_344_kyoshi_battle_fan_creates_and_attaches_an_ally() {
    let definition = tricerules_cards::registry::global()
        .get("kyoshi_battle_fan")
        .expect("Kyoshi Battle Fan");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(face.types, ["Artifact", "Equipment"]);
    assert_eq!(face.colors(), Vec::<Color>::new());

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Kyoshi Battle Fan must have exactly one triggered ability");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!trigger.may);
    assert!(trigger.targeting.is_none());
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(trigger.effect.is_empty());
    let program = trigger
        .program
        .as_ref()
        .expect("Kyoshi's completed token cohort must feed its typed receipt consumer");
    let expected: tricerules_cards::CardProgram = ron::from_str(
        r#"(body: (steps: [
            (id: "create_tokens", bind: ["created_tokens"], input: None,
                kind: Perform(CreateTokens(token: "ally_w_1_1", count: 1,
                    who: Controller, tapped: false, sacrifice_timing: None))),
            (id: "attach_created_tokens", bind: [], input: Some("created_tokens"),
                kind: Perform(AttachEquipment(
                    equipment: Source, creature: PreviousEffectObjects))),
        ], yields: []), target_bindings: [])"#,
    )
    .expect("the authored Kyoshi receipt Program parses");
    assert_eq!(program, &expected);

    let [modifier] = face.static_abilities.as_slice() else {
        panic!("Kyoshi Battle Fan must keep the equipped-creature modifier");
    };
    assert_eq!(
        modifier.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(
        matches!(
            &modifier.definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 1,
                delta_toughness: 0,
                ..
            }
        ),
        "the equipped creature must keep +1/+0"
    );

    let [equip] = face.activated_abilities.as_slice() else {
        panic!("Kyoshi Battle Fan must keep exactly one Equip activation");
    };
    assert_eq!(
        equip.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        equip.costs,
        [tricerules_cards::AbilityCost::Mana(
            tricerules_cards::ManaCost::parse("{2}").expect("static cost")
        )]
    );
    assert_eq!(
        equip.effect,
        [SpellEffectKind::Equip {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
        }]
    );
}

#[test]
fn issue_344_goblin_boarders_enters_with_a_conditional_counter() {
    let definition = tricerules_cards::registry::global()
        .get("goblin_boarders")
        .expect("Goblin Boarders");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{R}");
    assert_eq!(face.types, ["Creature", "Goblin", "Pirate"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(2)));
    assert_eq!(face.colors(), vec![Color::Red]);
    assert!(face.triggered_abilities.is_empty());
    let [ability] = face.static_abilities.as_slice() else {
        panic!("Goblin Boarders must have exactly one static ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::EntersWithCounters {
            affected: EntersWithCountersAffected::Self_,
            counter: CounterKind::PlusOnePlusOne,
            amount: Amount::Conditional {
                condition: GameCondition::AttackedThisTurn {
                    players: RelativePlayerSet::Controller,
                },
                when_true: 1,
                otherwise: 0,
            },
            cast_cost_condition: None,
        }
    );
}

#[test]
fn issue_344_gorehorn_raider_deals_two_only_after_attacking() {
    let definition = tricerules_cards::registry::global()
        .get("gorehorn_raider")
        .expect("Gorehorn Raider");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{R}");
    assert_eq!(face.types, ["Creature", "Minotaur", "Pirate"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(4)));
    assert_eq!(face.colors(), vec![Color::Red]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Gorehorn Raider must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.intervening_if,
        Some(GameCondition::AttackedThisTurn {
            players: RelativePlayerSet::Controller,
        })
    );
    assert!(!ability.may);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(2),
            target: TargetFilter {
                kind: TargetKind::AnyTarget,
                ..TargetFilter::default()
            },
        }]
    );
    let targeting = ability.targeting.as_ref().expect("damage target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Gorehorn Raider must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(group.prompt, "Choose any target");
}

#[test]
fn issue_344_pitiless_fists_fights_up_to_one() {
    let definition = tricerules_cards::registry::global()
        .get("pitiless_fists")
        .expect("Pitiless Fists");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{G}");
    assert_eq!(face.types, ["Enchantment", "Aura"]);
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::AuraAttach {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
        }]
    );

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Pitiless Fists must have exactly one triggered ability");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!trigger.may);
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::Fight {
            first: EffectSubject::AttachedObject,
            second: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                ..TargetFilter::default()
            })),
        }]
    );
    let targeting = trigger.targeting.as_ref().expect("fight target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Pitiless Fists must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (0, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(
        group.prompt,
        "Choose up to one target creature an opponent controls"
    );

    let [modifier] = face.static_abilities.as_slice() else {
        panic!("Pitiless Fists must keep the enchanted-creature modifier");
    };
    assert_eq!(
        modifier.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert!(
        matches!(
            &modifier.definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 2,
                delta_toughness: 2,
                ..
            }
        ),
        "the enchanted creature must keep +2/+2"
    );
}

#[test]
fn issue_344_fingerprint_rows_match_the_presentation_registry() {
    let registry = tricerules_cards::registry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("goblin_boarders", "goblin_boarders"),
        ("gorehorn_raider", "gorehorn_raider"),
        ("icecave_crasher", "icecave_crasher"),
        ("illvoi_operative", "illvoi_operative"),
        ("kyoshi_battle_fan", "kyoshi_battle_fan"),
        ("pitiless_fists", "pitiless_fists"),
        ("refute", "refute"),
        ("romantic_rendezvous", "romantic_rendezvous"),
    ] {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.oracle_text_sha256.len(), 64);
        let row = fingerprints
            .lines()
            .find(|line| {
                line.starts_with(&format!("{id}\t")) && line.contains(&format!("\t{face_id}\t"))
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}/{face_id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}/{face_id}: {row}"
        );
    }
}
