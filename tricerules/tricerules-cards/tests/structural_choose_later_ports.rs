use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, CompiledChooseBranch, CompiledInstructionKind, CompiledProgram,
    PlayerRecipient, ResolutionCost, SpellEffectKind, TriggerCondition, TriggeredAbilityDef,
};
use tricerules_cards::{
    AbilityDefinitionId, AbilityPresentation, CardFace, CardRegistry, ManaCost, ProgramOwnerId,
};

fn program_for_ability<'a>(
    registry: &'a CardRegistry,
    card_id: &str,
    face: &CardFace,
    ability: &TriggeredAbilityDef,
) -> &'a CompiledProgram {
    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: card_id.to_owned(),
        face_id: face.face_id.clone(),
        ability_path: vec![ability.ability_id.clone()],
    });
    registry
        .compiled_program_for_owner(&owner)
        .unwrap_or_else(|| panic!("missing compiled program for {owner:?}"))
}

fn choose_branches<'a>(
    program: &'a CompiledProgram,
    choice_id: &str,
    optional: bool,
    branch_ids: &[&str],
) -> &'a [CompiledChooseBranch] {
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.slots.is_empty());
    assert!(program.body.yields.is_empty());
    let [instruction] = program.body.steps.as_slice() else {
        panic!("the owner has one top-level Choose instruction");
    };
    let CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional: actual_optional,
        branches,
        otherwise,
    } = &instruction.kind
    else {
        panic!("the owner instruction is a compiled Choose");
    };
    assert_eq!(choice.as_str(), choice_id);
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert_eq!(*actual_optional, optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    assert_eq!(branches.len(), branch_ids.len());
    for (branch, expected_id) in branches.iter().zip(branch_ids) {
        assert_eq!(branch.branch_id.as_str(), *expected_id);
        assert!(matches!(
            &branch.requirement,
            tricerules_cards::primitives::CompiledChooseRequirement::Always
        ));
        assert!(branch.body.yields.is_empty());
    }
    branches
}

fn assert_single_perform(branch: &CompiledChooseBranch, expected: SpellEffectKind) {
    let [instruction] = branch.body.steps.as_slice() else {
        panic!("each selected branch has one leaf operation");
    };
    let CompiledInstructionKind::Perform(effect) = &instruction.kind else {
        panic!("each branch leaf is a Perform instruction");
    };
    assert_eq!(**effect, expected);
}

#[test]
fn ant_mans_army_registry_has_mandatory_food_or_treasure_choose() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("ant-mans_army")
        .expect("Ant-Man's Army")
        .primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Ant-Man's Army keeps one ETB owner");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(ability.effect.is_empty());
    let program = program_for_ability(registry, "ant-mans_army", face, ability);
    let branches = choose_branches(
        program,
        "token_choice",
        false,
        &["create_food", "create_treasure"],
    );
    assert_eq!(branches[0].cost, ResolutionCost::None);
    assert_eq!(branches[0].presentation, AbilityPresentation::Fallback);
    assert_eq!(branches[0].fallback_label(), "Choice (create_food)");
    assert_single_perform(
        &branches[0],
        SpellEffectKind::CreateTokens {
            token: "food".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        },
    );
    assert_eq!(branches[1].cost, ResolutionCost::None);
    assert_eq!(branches[1].presentation, AbilityPresentation::Fallback);
    assert_eq!(branches[1].fallback_label(), "Choice (create_treasure)");
    assert_single_perform(
        &branches[1],
        SpellEffectKind::CreateTokens {
            token: "treasure".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        },
    );
}

#[test]
fn consecrated_sphinx_registry_keeps_the_optional_free_draw_two_branch() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("consecrated_sphinx")
        .expect("Consecrated Sphinx")
        .primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Consecrated Sphinx keeps one draw trigger");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        &ability.trigger,
        TriggerCondition::WheneverPlayerDrawsCard {
            drawer: CastTriggerPlayer::Opponent
        }
    ));
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(ability.effect.is_empty());
    let branches = choose_branches(
        program_for_ability(registry, "consecrated_sphinx", face, ability),
        "sphinx_draw",
        true,
        &["draw_two"],
    );
    assert_eq!(branches[0].cost, ResolutionCost::None);
    assert_eq!(
        branches[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_single_perform(
        &branches[0],
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(2),
        },
    );
}

#[test]
fn mentor_of_the_meek_registry_keeps_its_event_filter_and_optional_paid_draw() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("mentor_of_the_meek")
        .expect("Mentor of the Meek")
        .primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Mentor of the Meek keeps one entry trigger");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        &ability.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield { .. }
    ));
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(ability.effect.is_empty());
    let branches = choose_branches(
        program_for_ability(registry, "mentor_of_the_meek", face, ability),
        "mentor_pay_draw",
        true,
        &["pay_1"],
    );
    assert!(matches!(
        &branches[0].cost,
        ResolutionCost::Mana(cost) if cost == &ManaCost::parse("{1}").unwrap()
    ));
    assert_eq!(branches[0].presentation, AbilityPresentation::Fallback);
    assert_eq!(branches[0].fallback_label(), "Pay {1}");
    assert_single_perform(
        &branches[0],
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        },
    );
}

#[test]
fn minds_eye_registry_keeps_the_optional_paid_draw_and_oracle_presentation() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("minds_eye")
        .expect("Mind's Eye")
        .primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Mind's Eye keeps one opponent-draw trigger");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        &ability.trigger,
        TriggerCondition::WheneverPlayerDrawsCard {
            drawer: CastTriggerPlayer::Opponent
        }
    ));
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(ability.effect.is_empty());
    let branches = choose_branches(
        program_for_ability(registry, "minds_eye", face, ability),
        "minds_eye_pay_draw",
        true,
        &["pay_1"],
    );
    assert!(matches!(
        &branches[0].cost,
        ResolutionCost::Mana(cost) if cost == &ManaCost::parse("{1}").unwrap()
    ));
    assert_eq!(
        branches[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_single_perform(
        &branches[0],
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        },
    );
}

#[test]
fn solemn_simulacrum_registry_programs_only_the_death_draw_owner() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("solemn_simulacrum")
        .expect("Solemn Simulacrum")
        .primary_face();
    let [entry, death] = face.triggered_abilities.as_slice() else {
        panic!("Solemn Simulacrum keeps both triggered owners");
    };
    assert_eq!(entry.ability_id.as_str(), "triggered_01");
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(entry.program.is_none());
    assert!(matches!(
        entry.effect.as_slice(),
        [SpellEffectKind::ChooseResolutionBranch { branches, .. }]
            if branches.len() == 1 && branches[0].branch_id.as_str() == "search_for_a_basic_land"
    ));

    assert_eq!(death.ability_id.as_str(), "triggered_02");
    assert_eq!(death.trigger, TriggerCondition::WhenSelfDies);
    assert_eq!(
        death.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(death.effect.is_empty());
    let branches = choose_branches(
        program_for_ability(registry, "solemn_simulacrum", face, death),
        "solemn_death_draw",
        true,
        &["draw_a_card"],
    );
    assert_eq!(branches[0].cost, ResolutionCost::None);
    assert_eq!(
        branches[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_single_perform(
        &branches[0],
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        },
    );
}
