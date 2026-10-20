use tricerules_cards::primitives::{
    Amount, CardResultAction, Cardinality, CompiledChooseRequirement, CompiledInstructionKind,
    CountExpression, PlayerRecipient, ResolutionCost, ResultType, SpellEffectKind, TargetKind,
    TriggerCondition,
};
use tricerules_cards::{
    AbilityDefinitionId, AbilityPresentation, EffectContext, Keyword, Layout, ManaCost,
    ProgramOwnerId,
};

#[test]
fn combustible_gearhulk_registers_its_exact_identity_and_etb_choice() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("combustible_gearhulk")
        .expect("Combustible Gearhulk needs a complete definition");
    assert_eq!(card.name, "Combustible Gearhulk");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "combustible_gearhulk");
    assert_eq!(face.mana_cost, ManaCost::parse("{4}{R}{R}").unwrap());
    assert_eq!(face.types, ["Artifact", "Creature", "Construct"]);
    assert_eq!((face.power, face.toughness), (Some(6), Some(6)));
    assert!(face.keywords.contains(&Keyword::FirstStrike));

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Combustible Gearhulk has exactly one triggered ability");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        trigger.presentation,
        AbilityPresentation::OracleLines(ref lines) if lines == &[2]
    ));
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(
        trigger.effect.is_empty(),
        "the trigger is owned by its Program"
    );
    assert!(
        trigger.program.is_some(),
        "the trigger declares its Program"
    );

    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "combustible_gearhulk".to_owned(),
        face_id: face.face_id.clone(),
        ability_path: vec![trigger.ability_id.clone()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact triggered ability owns a compiled Program");
    assert_eq!(program.owner, owner);
    assert_eq!(program.context, EffectContext::Ability);
    assert!(program.body.yields.is_empty());

    let [choice] = program.body.steps.as_slice() else {
        panic!("the triggered Program begins with its one choice");
    };
    assert!(choice.input.is_none());
    assert!(choice.outputs.is_empty());
    let CompiledInstructionKind::Choose {
        choice: choice_id,
        chooser,
        optional,
        branches,
        otherwise,
    } = &choice.kind
    else {
        panic!("the triggered Program asks its target opponent to choose a branch");
    };
    assert_eq!(choice_id.as_str(), "draw_or_mill");
    assert_eq!(
        *chooser,
        PlayerRecipient::TargetedPlayer {
            group_index: 0,
            kind: TargetKind::OpponentPlayer,
        }
    );
    assert!(*optional);
    assert!(otherwise.yields.is_empty());

    let [draw_branch] = branches.as_slice() else {
        panic!("the optional choice offers exactly the draw branch");
    };
    assert_eq!(draw_branch.branch_id.as_str(), "draw_three");
    assert_eq!(
        draw_branch.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(draw_branch.requirement, CompiledChooseRequirement::Always);
    assert_eq!(draw_branch.cost, ResolutionCost::None);
    assert!(draw_branch.body.yields.is_empty());
    let [draw] = draw_branch.body.steps.as_slice() else {
        panic!("the draw branch contains one draw instruction");
    };
    let CompiledInstructionKind::Perform(draw_effect) = &draw.kind else {
        panic!("the draw branch resolves a Draw effect");
    };
    assert_eq!(
        **draw_effect,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(3),
        }
    );

    let [mill, damage] = otherwise.steps.as_slice() else {
        panic!("declining the draw mills three cards and damages the same target");
    };
    assert!(mill.input.is_none());
    let CompiledInstructionKind::Perform(mill_effect) = &mill.kind else {
        panic!("the decline branch performs a Mill effect");
    };
    assert_eq!(
        **mill_effect,
        SpellEffectKind::Mill {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(3),
        }
    );
    let [milled_slot] = mill.outputs.as_slice() else {
        panic!("the Mill instruction binds its typed card-result cohort");
    };
    let mill_result = program
        .slots
        .iter()
        .find(|slot| slot.id == *milled_slot)
        .expect("the Mill result is declared in the Program slot table");
    assert_eq!(mill_result.name, "milled");
    assert!(matches!(
        mill_result.result_type,
        ResultType::CardResultCohort {
            action: CardResultAction::Mill,
            who: PlayerRecipient::Controller,
            cardinality: Cardinality {
                min: 0,
                max: Some(3),
            },
        }
    ));
    assert_eq!(damage.input, Some(*milled_slot));
    let CompiledInstructionKind::Perform(damage_effect) = &damage.kind else {
        panic!("the damage instruction consumes the bound Mill result");
    };
    assert_eq!(
        **damage_effect,
        SpellEffectKind::DamagePlayer {
            who: PlayerRecipient::TargetedPlayer {
                group_index: 0,
                kind: TargetKind::OpponentPlayer,
            },
            amount: Amount::Count(CountExpression::PreviousMillManaValueSum),
        }
    );

    assert_eq!(program.target_bindings.len(), 1);
    let binding = &program.target_bindings[0];
    assert_eq!(binding.instruction_address, damage.instruction_address);
    assert_eq!(binding.target_group_index, 0);
    assert_eq!(binding.role_index, 0);

    let targeting = trigger
        .targeting
        .as_ref()
        .expect("the triggered ability retains its opponent target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Combustible Gearhulk has exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert!(group.effect_indices.is_empty());
}
