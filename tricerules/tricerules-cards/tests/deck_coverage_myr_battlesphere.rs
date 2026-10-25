use tricerules_cards::primitives::*;
use tricerules_cards::{AbilityPresentation, CardRegistry};

fn myr_program_source() -> String {
    include_str!("../data/myr_battlesphere.ron").replace("\r\n", "\n")
}

fn myr_program_source_with_resultless_interlude() -> String {
    myr_program_source().replace(
        "        (id: \"attack_myrs\", input: Some(\"myrs\"),",
        "        (id: \"untap_myrs\", input: Some(\"myrs\"), kind: Perform(UntapChosenPermanents)),\n        (id: \"attack_myrs\", input: Some(\"myrs\"),",
    )
}

fn myr_registry_error(source: &str) -> String {
    let token = include_str!("../data/tokens/myr_token_c_1_1.ron");
    CardRegistry::from_chunks_and_tokens(&[source], &[token])
        .unwrap_err()
        .to_string()
}

const MYR_INPUT_PROFILE_ERROR: &str = "MyrBattlesphereAttack input result must be the optional Controller choice of controlled untapped Myr permanents with cardinality 0..=u32::MAX";

#[test]
fn myr_attack_program_accepts_its_named_selection_across_a_resultless_instruction() {
    let source = myr_program_source_with_resultless_interlude();
    let token = include_str!("../data/tokens/myr_token_c_1_1.ron");
    let registry = CardRegistry::from_chunks_and_tokens(&[&source], &[token])
        .expect("typed Myr selection input should compile across an intervening instruction");
    let attack = &registry
        .get("myr_battlesphere")
        .unwrap()
        .primary_face()
        .triggered_abilities[1];
    assert!(attack.effect.is_empty());
    let program = attack.program.as_ref().unwrap();
    assert_eq!(program.body.steps.len(), 3);
    assert_eq!(program.body.steps[2].input.as_deref(), Some("myrs"));
}

#[test]
fn myr_attack_program_rejects_missing_or_incompatible_selection_inputs() {
    let valid = myr_program_source();
    let missing_input = valid.replace(
        "(id: \"attack_myrs\", input: Some(\"myrs\"),",
        "(id: \"attack_myrs\",",
    );
    assert!(myr_registry_error(&missing_input)
        .contains("instruction requires an explicitly bound input result"));

    let wrong_filter = valid.replace(
        "controller: You, required_subtypes: [\"Myr\"]",
        "controller: Any, required_subtypes: [\"Myr\"]",
    );
    assert!(myr_registry_error(&wrong_filter).contains(MYR_INPUT_PROFILE_ERROR));

    let wrong_chooser = valid.replace(
        "kind: Perform(ChoosePermanents(",
        "kind: Perform(ChoosePermanents(\n        chooser: AffectedPlayer,",
    );
    assert_ne!(
        wrong_chooser, valid,
        "the invalid chooser fixture changes source"
    );
    assert!(myr_registry_error(&wrong_chooser).contains(
        "ChoosePermanents chooser supports only the resolving spell controller in spell_program"
    ));

    let wrong_cardinality = valid.replace("max: 4294967295,", "max: 4,");
    assert!(myr_registry_error(&wrong_cardinality).contains(MYR_INPUT_PROFILE_ERROR));

    let wrong_type = valid.replace(
        "(id: \"choose_myrs\", bind: [\"myrs\"], kind: Perform(ChoosePermanents(\n        filter: (kind: AnyPermanent, controller: You, required_subtypes: [\"Myr\"], tapped: Some(false)),\n        min: 0,\n        max: 4294967295,\n        )))",
        "(id: \"choose_myrs\", bind: [\"myrs\"], kind: Perform(SelectHandCards(\n        count: 1, ordered: false, prompt: \"Choose a card\",\n      )))",
    );
    assert_ne!(
        wrong_type, valid,
        "the wrong-result-type fixture changes source"
    );
    assert!(myr_registry_error(&wrong_type)
        .contains("input result 'myrs' has the wrong type for this instruction"));
}

#[test]
fn myr_attack_program_rejects_nonattack_and_nested_nonattack_contexts() {
    let valid = myr_program_source();
    let token = include_str!("../data/tokens/myr_token_c_1_1.ron");
    let wrong_trigger = valid.replace(
        "WheneverSelfAttacks(minimum_other_attackers: 0)",
        "AtBeginningOfUpkeep(player: Controller)",
    );
    let error = CardRegistry::from_chunks_and_tokens(&[&wrong_trigger], &[token]).unwrap_err();
    assert!(error
        .to_string()
        .contains("MyrBattlesphereAttack requires WheneverSelfAttacks"));

    let nested = valid.replace(
        "(id: \"attack_myrs\", input: Some(\"myrs\"), kind: Perform(MyrBattlesphereAttack))",
        r#"(id: "attack_branch", kind: Choose(
          choice: "attack_branch",
          chooser: Controller,
          optional: false,
          branches: [(
            branch_id: "attack_myrs",
            presentation: Fallback,
            cost: None,
            requirement: Always,
            body: (steps: [
              (id: "attack_myrs", input: Some("myrs"), kind: Perform(MyrBattlesphereAttack)),
            ], yields: []),
          )],
          otherwise: (steps: [], yields: []),
        ))"#,
    );
    let nested_wrong_trigger = nested.replace(
        "WheneverSelfAttacks(minimum_other_attackers: 0)",
        "AtBeginningOfUpkeep(player: Controller)",
    );
    let error =
        CardRegistry::from_chunks_and_tokens(&[&nested_wrong_trigger], &[token]).unwrap_err();
    assert!(error
        .to_string()
        .contains("MyrBattlesphereAttack requires WheneverSelfAttacks"));
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
    assert_eq!(attack_program.body.steps.len(), 2);
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
            if matches!(effect.as_ref(), SpellEffectKind::MyrBattlesphereAttack)
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
fn myr_attack_rejects_legacy_flat_consumer_without_program_input() {
    let legacy = [
        SpellEffectKind::ChoosePermanents {
            chooser: PlayerRecipient::Controller,
            filter: TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::You,
                tapped: Some(false),
                required_subtypes: vec!["Myr".into()],
                ..TargetFilter::default()
            },
            min: 0,
            max: u32::MAX,
            constraints: Vec::new(),
        },
        SpellEffectKind::MyrBattlesphereAttack,
    ];
    assert_eq!(
        SpellEffectKind::validate_list(&legacy).unwrap_err(),
        "MyrBattlesphereAttack requires an explicitly bound Program input result"
    );
}

#[test]
fn myr_attack_consumer_rejects_reflexive_trigger_context() {
    let card = include_str!("../data/myr_battlesphere.ron").replace("\r\n", "\n");
    let token = include_str!("../data/tokens/myr_token_c_1_1.ron");
    let wrong = card.replace(
        "  )],\n)",
        "  ), (\n    ability_id: \"triggered_03\",\n    presentation: Fallback,\n    trigger: WheneverSelfAttacks(minimum_other_attackers: 0),\n    effect: [CreateReflexiveTrigger(ability: (ability_id: \"reflexive_01\", presentation: Fallback, effect: [MyrBattlesphereAttack]))],\n  )],\n)",
    );
    let error = CardRegistry::from_chunks_and_tokens(&[&wrong], &[token]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("reflexive triggered abilities cannot reference attack context"),
        "{error}"
    );
}
