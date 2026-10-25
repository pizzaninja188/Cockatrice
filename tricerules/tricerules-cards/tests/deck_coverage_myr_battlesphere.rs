use tricerules_cards::primitives::*;
use tricerules_cards::{AbilityPresentation, CardRegistry};

fn myr_program_source() -> String {
    include_str!("../data/myr_battlesphere.ron").replace("\r\n", "\n")
}

fn myr_registry_error(source: &str) -> String {
    let token = include_str!("../data/tokens/myr_token_c_1_1.ron");
    CardRegistry::from_chunks_and_tokens(&[source], &[token])
        .unwrap_err()
        .to_string()
}

#[test]
fn myr_attack_program_binds_the_generic_tap_count_to_both_followups() {
    let source = myr_program_source();
    let token = include_str!("../data/tokens/myr_token_c_1_1.ron");
    let registry = CardRegistry::from_chunks_and_tokens(&[&source], &[token])
        .expect("typed Myr tap result should feed both generic follow-up effects");
    let attack = &registry
        .get("myr_battlesphere")
        .unwrap()
        .primary_face()
        .triggered_abilities[1];
    assert!(attack.effect.is_empty());
    let program = attack.program.as_ref().unwrap();
    assert_eq!(program.body.steps.len(), 4);
    assert_eq!(program.body.steps[1].input.as_deref(), Some("myrs"));
    assert_eq!(program.body.steps[1].bind, ["tapped_count"]);
    assert_eq!(program.body.steps[2].input.as_deref(), Some("tapped_count"));
    assert_eq!(program.body.steps[3].input.as_deref(), Some("tapped_count"));
}

#[test]
fn myr_tap_program_requires_its_bound_selection_input() {
    let valid = myr_program_source();
    let missing_input = valid.replace(
        "(id: \"tap_myrs\", bind: [\"tapped_count\"], input: Some(\"myrs\"),",
        "(id: \"tap_myrs\", bind: [\"tapped_count\"],",
    );
    assert!(myr_registry_error(&missing_input)
        .contains("instruction requires an explicitly bound input result"));

    let wrong_type = valid.replace(
        "(id: \"tap_myrs\", bind: [\"tapped_count\"], input: Some(\"myrs\"),",
        "(id: \"tap_myrs\", bind: [\"tapped_count\"], input: Some(\"tapped_count\"),",
    );
    assert!(myr_registry_error(&wrong_type).contains("input result 'tapped_count' is not in scope"));
}

#[test]
fn generic_attacked_recipient_damage_requires_an_attack_trigger() {
    let valid = myr_program_source();
    let token = include_str!("../data/tokens/myr_token_c_1_1.ron");
    let wrong_trigger = valid.replace(
        "WheneverSelfAttacks(minimum_other_attackers: 0)",
        "AtBeginningOfUpkeep(player: Controller)",
    );
    let error = CardRegistry::from_chunks_and_tokens(&[&wrong_trigger], &[token]).unwrap_err();
    assert!(error
        .to_string()
        .contains("DamageAttackedPlayerOrPlaneswalker requires WheneverSelfAttacks"));
}

#[test]
fn myr_battlesphere_complete_definition_and_rules_named_token_are_exact() {
    let registry = tricerules_cards::registry::global();
    let card = registry.get("myr_battlesphere").unwrap();
    assert_eq!(
        registry.id_for_name("Myr Battlesphere"),
        Some("myr_battlesphere")
    );
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "myr_battlesphere");
    assert_eq!(face.mana_cost.to_string(), "{7}");
    assert_eq!(face.types, ["Artifact", "Creature", "Myr", "Construct"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(7)));
    assert!(
        face.spell_effect.is_empty()
            && face.activated_abilities.is_empty()
            && face.static_abilities.is_empty()
    );
    assert_eq!(face.triggered_abilities.len(), 2);
    let entry = &face.triggered_abilities[0];
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(
        matches!(entry.effect.as_slice(), [SpellEffectKind::CreateTokens { token, count: Amount::Fixed(4), .. }] if token == "myr_token_c_1_1")
    );
    let attack = &face.triggered_abilities[1];
    assert_eq!(
        attack.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        attack.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert!(!attack.may && attack.targeting.is_none());
    assert!(attack.effect.is_empty());
    let attack_program = attack
        .program
        .as_ref()
        .expect("attack uses a typed Program");
    assert_eq!(attack_program.body.steps.len(), 4);
    assert_eq!(attack_program.body.steps[0].bind, ["myrs"]);
    assert_eq!(attack_program.body.steps[1].input.as_deref(), Some("myrs"));
    assert!(matches!(
        &attack_program.body.steps[0].kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::ChoosePermanents {
                chooser: PlayerRecipient::Controller,
                filter,
                min: 0,
                max: u32::MAX,
                constraints,
            } if filter.kind == TargetKind::AnyPermanent
                && filter.controller == TargetController::You
                && filter.tapped == Some(false)
                && filter.required_subtypes == ["Myr"]
                && constraints.is_empty())
    ));
    assert!(matches!(
        &attack_program.body.steps[1].kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::TapChosenPermanents)
    ));
    assert_eq!(attack_program.body.steps[1].bind, ["tapped_count"]);
    assert!(matches!(
        &attack_program.body.steps[2].kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::PumpTarget {
                subject: EffectSubject::Source,
                scale: Some(PtScale {
                    basis: PtScaleBasis::Amount(Amount::Count(CountExpression::ProgramInputCount)),
                    power_per_unit: 1,
                    toughness_per_unit: 0,
                }),
                ..
            })
    ));
    assert!(matches!(
        &attack_program.body.steps[3].kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::DamageAttackedPlayerOrPlaneswalker {
                amount: Amount::Count(CountExpression::ProgramInputCount),
            })
    ));
    let token = registry.get("myr_token_c_1_1").unwrap();
    assert!(registry.is_token("myr_token_c_1_1"));
    assert_eq!(token.name, "Myr Token");
    assert_eq!(token.primary_face().types, ["Artifact", "Creature", "Myr"]);
    assert_eq!(
        (token.primary_face().power, token.primary_face().toughness),
        (Some(1), Some(1))
    );
}

#[test]
fn attacked_recipient_damage_rejects_reflexive_trigger_context() {
    let card = include_str!("../data/myr_battlesphere.ron").replace("\r\n", "\n");
    let token = include_str!("../data/tokens/myr_token_c_1_1.ron");
    let wrong = card.replace(
        "  )],\n)",
        "  ), (\n    ability_id: \"triggered_03\",\n    presentation: Fallback,\n    trigger: WheneverSelfAttacks(minimum_other_attackers: 0),\n    effect: [CreateReflexiveTrigger(ability: (ability_id: \"reflexive_01\", presentation: Fallback, effect: [DamageAttackedPlayerOrPlaneswalker(amount: 1)]))],\n  )],\n)",
    );
    let error = CardRegistry::from_chunks_and_tokens(&[&wrong], &[token]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("reflexive triggered abilities cannot reference attack context"),
        "{error}"
    );
}
