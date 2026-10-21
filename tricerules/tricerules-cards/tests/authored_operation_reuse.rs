//! Exact authored owners for the reusable Scry, search, and Proliferate operations.

use tricerules_cards::primitives::{
    AbilityCost, Amount, CardSearchZone, CardTypeFilter, Cardinality, CastCostOptionDef,
    CastTriggerPlayer, CompiledChooseBranch, CompiledChooseRequirement, CompiledInstruction,
    CompiledInstructionKind, CompiledProgram, CompiledProgramCondition,
    CompiledProgramResultCondition, DiscardQuantity, DiscardResultOutcome, EntersTappedAffected,
    GameCondition, GraveyardAggregate, LibraryBottomOrder, LifeAmount, ManaAmount,
    PermanentTypeFilter, PlayerRecipient, RelativePlayerSet, ResolutionCost, ResultType,
    SearchDestination, SearchSelectionBounds, SearchZoneSelection, SpellEffectKind,
    StaticAbilityDef, TargetController, TargetFilter, TargetKind, TriggerCondition, ZoneCardFilter,
};
use tricerules_cards::{AbilityDefinitionId, EffectContext, ProgramOwnerId};

fn face_program(card_id: &str) -> &'static tricerules_cards::primitives::CompiledProgram {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(card_id)
        .unwrap_or_else(|| panic!("missing {card_id}"));
    let face = card.primary_face();
    assert!(
        face.spell_effect.is_empty(),
        "{card_id} uses its Program body"
    );
    assert!(
        face.spell_program.is_some(),
        "{card_id} has an authored Program"
    );
    let owner = ProgramOwnerId::face(card_id, face.face_id.clone());
    let program = registry
        .compiled_program_for_owner(&owner)
        .unwrap_or_else(|| panic!("{card_id} face has a compiled Program"));
    assert_eq!(
        program.owner, owner,
        "{card_id} Program has the exact face owner"
    );
    program
}

fn ability_program(
    card_id: &str,
    index: usize,
    activated: bool,
) -> &'static tricerules_cards::primitives::CompiledProgram {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(card_id)
        .unwrap_or_else(|| panic!("missing {card_id}"));
    let face = card.primary_face();
    let (effect, program_definition, ability_id) = if activated {
        let ability = &face.activated_abilities[index];
        (
            &ability.effect,
            ability.program.as_ref(),
            &ability.ability_id,
        )
    } else {
        let ability = &face.triggered_abilities[index];
        (
            &ability.effect,
            ability.program.as_ref(),
            &ability.ability_id,
        )
    };
    assert!(effect.is_empty(), "{card_id} ability uses its Program body");
    assert!(
        program_definition.is_some(),
        "{card_id} ability has an authored Program"
    );
    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: card_id.to_owned(),
        face_id: face.face_id.clone(),
        ability_path: vec![ability_id.clone()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .unwrap_or_else(|| panic!("{card_id} exact ability has a compiled Program"));
    assert_eq!(
        program.owner, owner,
        "{card_id} Program has the exact ability owner"
    );
    program
}

fn assert_perform_effect(step: &CompiledInstruction, expected: SpellEffectKind) {
    assert!(step.input.is_none());
    let CompiledInstructionKind::Perform(effect) = &step.kind else {
        panic!("expected a Perform instruction");
    };
    assert_eq!(**effect, expected);
}

fn assert_perform(step: &CompiledInstruction, expected: SpellEffectKind) {
    assert!(step.outputs.is_empty());
    assert_perform_effect(step, expected);
}

fn assert_single_look_choose(steps: &[CompiledInstruction], count: u32, min: u32, max: u32) {
    let [step] = steps else {
        panic!("the block has one direct library-look operation");
    };
    assert_perform(
        step,
        SpellEffectKind::LookChooseToHand {
            count,
            filter: None,
            min,
            max,
            reveal: false,
            bottom_order: LibraryBottomOrder::Chosen,
        },
    );
}

fn assert_single_search(
    program: &tricerules_cards::primitives::CompiledProgram,
    filter: Option<ZoneCardFilter>,
    destination: SearchDestination,
    reveal: bool,
    min: u32,
) {
    let [step] = program.body.steps.as_slice() else {
        panic!("the owner has one direct search operation");
    };
    let CompiledInstructionKind::Perform(effect) = &step.kind else {
        panic!("the search is a Perform instruction");
    };
    let SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        count_by_cast_cost,
        filter: actual_filter,
        selection_constraint,
        slots,
        zones,
        destination: actual_destination,
        conditional_destination,
        shuffle,
        reveal: actual_reveal,
        result_id,
        selection_bounds,
    } = effect.as_ref()
    else {
        panic!("the Program performs SearchLibrary");
    };
    assert_eq!(*who, PlayerRecipient::Controller);
    assert!(!optional);
    assert_eq!(*count, 1);
    assert!(count_by_cast_cost.is_none());
    assert_eq!(actual_filter, &filter);
    assert!(selection_constraint.is_none());
    assert!(slots.is_empty());
    assert_eq!(
        zones,
        &SearchZoneSelection::Fixed(vec![CardSearchZone::Library])
    );
    assert_eq!(*actual_destination, destination);
    assert!(conditional_destination.is_none());
    assert!(*shuffle);
    assert_eq!(*actual_reveal, reveal);
    assert!(result_id.is_none());
    assert_eq!(
        selection_bounds.as_ref(),
        Some(&SearchSelectionBounds { min, max: 1 })
    );
}

fn assert_spell_program(program: &tricerules_cards::primitives::CompiledProgram) {
    assert_eq!(program.context, EffectContext::Spell);
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.slots.is_empty());
}

fn assert_flaring_cinder_optional_discard_program(
    program: &tricerules_cards::primitives::CompiledProgram,
) {
    assert_eq!(program.context, EffectContext::Ability);
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.body.yields.is_empty());
    let [discard, branch] = program.body.steps.as_slice() else {
        panic!("Flaring Cinder discards before its typed result branch");
    };
    assert_perform_effect(
        discard,
        SpellEffectKind::Discard {
            who: PlayerRecipient::Controller,
            quantity: DiscardQuantity::OptionalExact(1),
        },
    );
    let [discard_result] = discard.outputs.as_slice() else {
        panic!("Flaring Cinder's optional discard binds one typed result");
    };
    assert_eq!(
        program.slot(*discard_result).unwrap().result_type,
        ResultType::DiscardResult {
            who: PlayerRecipient::Controller,
            cardinality: Cardinality {
                min: 0,
                max: Some(1),
            },
            optional_payment: true,
        }
    );
    let CompiledInstructionKind::If { condition, yes, no } = &branch.kind else {
        panic!("Flaring Cinder draws only through a result condition");
    };
    assert!(matches!(
        condition.as_ref(),
        CompiledProgramCondition::Result(
            CompiledProgramResultCondition::DiscardOutcome {
                result,
                outcome: DiscardResultOutcome::ChosenToPay,
            }
        ) if result == discard_result
    ));
    assert!(yes.yields.is_empty());
    assert!(no.steps.is_empty() && no.yields.is_empty());
    let [draw] = yes.steps.as_slice() else {
        panic!("a paid discard produces exactly one draw");
    };
    assert_perform(
        draw,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        },
    );
}

#[test]
fn authored_sleight_of_hand_uses_exact_private_library_look_program() {
    let program = face_program("sleight_of_hand");
    assert_spell_program(program);
    assert_single_look_choose(&program.body.steps, 2, 1, 1);
}

#[test]
fn authored_flow_state_uses_resolution_time_graveyard_program_if() {
    let program = face_program("flow_state");
    assert_spell_program(program);
    assert!(program.body.yields.is_empty());
    let [gate] = program.body.steps.as_slice() else {
        panic!("Flow State has one automatic graveyard condition");
    };
    assert!(gate.input.is_none());
    assert!(gate.outputs.is_empty());
    let CompiledInstructionKind::If { condition, yes, no } = &gate.kind else {
        panic!("Flow State uses ProgramIf rather than a player-selected branch");
    };
    let CompiledProgramCondition::Game(GameCondition::AllOf(conditions)) = condition.as_ref()
    else {
        panic!("Flow State checks both graveyard card types at resolution");
    };
    assert_eq!(conditions.len(), 2);
    for (condition, expected_type) in conditions
        .iter()
        .zip([CardTypeFilter::Instant, CardTypeFilter::Sorcery])
    {
        let GameCondition::GraveyardAggregate {
            owners,
            aggregate,
            filter,
            min,
            max,
        } = condition
        else {
            panic!("each Flow State condition counts cards in a graveyard");
        };
        assert_eq!(*owners, RelativePlayerSet::Controller);
        assert_eq!(*aggregate, GraveyardAggregate::CardCount);
        assert_eq!(
            filter.as_ref().and_then(|filter| filter.card_type),
            Some(expected_type)
        );
        assert_eq!(*min, Some(1));
        assert_eq!(*max, None);
    }
    assert!(yes.yields.is_empty());
    assert!(no.yields.is_empty());
    assert_single_look_choose(&yes.steps, 3, 2, 2);
    assert_single_look_choose(&no.steps, 3, 1, 1);
}

#[test]
fn authored_flaring_cinder_enters_with_optional_discard_program() {
    let program = ability_program("flaring_cinder", 0, false);
    assert_flaring_cinder_optional_discard_program(program);
}

#[test]
fn authored_flaring_cinder_cast_trigger_uses_optional_discard_program() {
    let program = ability_program("flaring_cinder", 1, false);
    assert_flaring_cinder_optional_discard_program(program);
}

#[test]
fn authored_osseous_exhale_uses_owned_behold_receipt_program() {
    let registry = tricerules_cards::registry::global();
    let card = registry.get("osseous_exhale").expect("Osseous Exhale");
    let face = card.primary_face();
    let [group] = face.cast_cost_groups.as_slice() else {
        panic!("Osseous Exhale keeps its one optional Behold group");
    };
    assert_eq!(group.group_id.as_str(), "cast_cost_01");
    assert_eq!((group.min, group.max), (0, 1));
    let [CastCostOptionDef::Behold {
        option_id,
        hand_filter,
        permanent_filter,
        ..
    }] = group.options.as_slice()
    else {
        panic!("Osseous Exhale keeps the exact Dragon Behold option");
    };
    assert_eq!(option_id.as_str(), "option_01");
    assert_eq!(hand_filter.required_subtypes, ["Dragon".to_string()]);
    assert_eq!(permanent_filter.kind, TargetKind::AnyPermanent);
    assert_eq!(permanent_filter.controller, TargetController::You);
    assert_eq!(permanent_filter.required_subtypes, ["Dragon".to_string()]);

    let program = face_program("osseous_exhale");
    assert_eq!(program.context, EffectContext::Spell);
    assert!(program.required_inputs.is_empty());
    assert!(program.slots.is_empty());
    let [damage, choose] = program.body.steps.as_slice() else {
        panic!("Osseous Exhale damages before checking its recorded Behold option");
    };
    let expected_target = TargetFilter {
        kind: TargetKind::Creature,
        combat_role: Some(tricerules_cards::primitives::CombatRole::AttackingOrBlocking),
        ..Default::default()
    };
    let CompiledInstructionKind::Perform(effect) = &damage.kind else {
        panic!("Osseous Exhale begins with its targeted damage leaf");
    };
    assert!(matches!(
        effect.as_ref(),
        SpellEffectKind::DamageTarget { amount: Amount::Fixed(5), target }
            if target == &expected_target
    ));
    assert_eq!(program.target_bindings.len(), 1);
    assert_eq!(
        program.target_bindings[0].instruction_address,
        damage.instruction_address
    );
    assert_eq!(program.target_bindings[0].target_group_index, 0);
    assert_eq!(program.target_bindings[0].role_index, 0);

    let CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        panic!("the recorded Behold receipt controls a finite choice");
    };
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(!optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [paid, unpaid] = branches.as_slice() else {
        panic!("the cast-cost receipt has exact selected and not-selected branches");
    };
    assert_eq!(
        paid.cost,
        tricerules_cards::primitives::ResolutionCost::None
    );
    assert_eq!(
        unpaid.cost,
        tricerules_cards::primitives::ResolutionCost::None
    );
    assert!(matches!(
        &paid.requirement,
        CompiledChooseRequirement::CastCostReceipt(condition)
            if condition.group_id.as_str() == "cast_cost_01"
                && condition.option_id.as_str() == "option_01"
                && condition.expected_selected
    ));
    assert!(matches!(
        &unpaid.requirement,
        CompiledChooseRequirement::CastCostReceipt(condition)
            if condition.group_id.as_str() == "cast_cost_01"
                && condition.option_id.as_str() == "option_01"
                && !condition.expected_selected
    ));
    assert!(unpaid.body.steps.is_empty() && unpaid.body.yields.is_empty());
    let [gain_life] = paid.body.steps.as_slice() else {
        panic!("a recorded Behold receipt grants exactly two life");
    };
    assert_perform(
        gain_life,
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(2),
        },
    );
}

#[test]
fn authored_opt_is_scry_then_draw_program() {
    let program = face_program("opt");
    assert_spell_program(program);
    let [scry, draw] = program.body.steps.as_slice() else {
        panic!("Opt preserves Scry 1 before one draw");
    };
    assert_perform(
        scry,
        SpellEffectKind::Scry {
            count: Amount::Fixed(1),
        },
    );
    assert_perform(
        draw,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        },
    );
}

#[test]
fn authored_preordain_is_scry_then_draw_program() {
    let program = face_program("preordain");
    assert_spell_program(program);
    let [scry, draw] = program.body.steps.as_slice() else {
        panic!("Preordain preserves Scry 2 before one draw");
    };
    assert_perform(
        scry,
        SpellEffectKind::Scry {
            count: Amount::Fixed(2),
        },
    );
    assert_perform(
        draw,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        },
    );
}

#[test]
fn authored_decode_transmissions_is_void_if_then_draw_and_life_loss_program() {
    let program = face_program("decode_transmissions");
    assert_spell_program(program);
    assert!(program.body.yields.is_empty());
    let [branch] = program.body.steps.as_slice() else {
        panic!("Decode Transmissions selects one branch before its effects");
    };
    let CompiledInstructionKind::If { condition, yes, no } = &branch.kind else {
        panic!("Decode Transmissions begins with a typed ProgramIf");
    };
    assert!(matches!(
        condition.as_ref(),
        CompiledProgramCondition::Game(GameCondition::Void)
    ));
    assert!(yes.yields.is_empty() && no.yields.is_empty());

    let [void_draw, void_life_loss] = yes.steps.as_slice() else {
        panic!("the Void branch draws before losing life");
    };
    assert_perform(
        void_draw,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(2),
        },
    );
    assert_perform(
        void_life_loss,
        SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(2),
            who: PlayerRecipient::EachOpponent,
        },
    );

    let [ordinary_draw, ordinary_life_loss] = no.steps.as_slice() else {
        panic!("the ordinary branch draws before losing life");
    };
    assert_perform(
        ordinary_draw,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(2),
        },
    );
    assert_perform(
        ordinary_life_loss,
        SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(2),
            who: PlayerRecipient::Controller,
        },
    );
}

#[test]
fn authored_visions_of_villainy_is_draw_then_controller_life_loss_program() {
    let program = face_program("visions_of_villainy");
    assert_spell_program(program);
    assert!(program.body.yields.is_empty());
    let [draw, life_loss] = program.body.steps.as_slice() else {
        panic!("Visions of Villainy draws before its life-loss tail");
    };
    assert_perform(
        draw,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(2),
        },
    );
    assert_perform(
        life_loss,
        SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(2),
            who: PlayerRecipient::Controller,
        },
    );
}

#[test]
fn authored_rune_scarred_demon_is_unrestricted_hand_search_program() {
    let program = ability_program("rune-scarred_demon", 0, false);
    assert_eq!(program.context, EffectContext::Ability);
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.slots.is_empty());
    assert_single_search(program, None, SearchDestination::Hand, false, 1);
}

#[test]
fn authored_mystical_tutor_is_revealed_filtered_top_search_program() {
    let program = face_program("mystical_tutor");
    assert_spell_program(program);
    assert_single_search(
        program,
        Some(ZoneCardFilter {
            card_type: Some(CardTypeFilter::InstantOrSorcery),
            ..Default::default()
        }),
        SearchDestination::TopOfLibrary,
        true,
        0,
    );
}

#[test]
fn authored_rampant_growth_is_tapped_basic_land_search_program() {
    let program = face_program("rampant_growth");
    assert_spell_program(program);
    assert_single_search(
        program,
        Some(ZoneCardFilter {
            card_type: Some(CardTypeFilter::BasicLand),
            ..Default::default()
        }),
        SearchDestination::Battlefield { tapped: true },
        false,
        0,
    );
}

#[test]
fn authored_karns_bastion_keeps_mana_and_program_proliferate_separate() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("karns_bastion")
        .expect("Karn's Bastion")
        .primary_face();
    assert!(face.activated_abilities[0].program.is_none());
    assert!(matches!(
        face.activated_abilities[0].effect.as_slice(),
        [SpellEffectKind::ProduceMana { .. }]
    ));
    assert_eq!(
        face.activated_abilities[1].ability_id.as_str(),
        "activated_02"
    );
    let program = ability_program("karns_bastion", 1, true);
    assert_eq!(program.context, EffectContext::Ability);
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.slots.is_empty());
    let [proliferate] = program.body.steps.as_slice() else {
        panic!("Karn's Bastion's second ability is one Proliferate operation");
    };
    assert_perform(proliferate, SpellEffectKind::Proliferate);
}

#[test]
fn authored_crypt_lurker_uses_optional_creature_sacrifice_or_discard_program() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("crypt_lurker")
        .expect("Crypt Lurker")
        .primary_face();
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Crypt Lurker owns its one enters-the-battlefield trigger");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);

    let program = ability_program("crypt_lurker", 0, false);
    assert_eq!(program.context, EffectContext::Ability);
    assert!(program.body.yields.is_empty());
    let [choose] = program.body.steps.as_slice() else {
        panic!("Crypt Lurker's trigger has one resultless Choose");
    };
    let CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        panic!("Crypt Lurker's trigger uses its compiled Program Choose");
    };
    assert_eq!(choice.as_str(), "crypt_lurker");
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [sacrifice, discard] = branches.as_slice() else {
        panic!("the Program preserves both printed payment branches in order");
    };
    assert_eq!(sacrifice.branch_id.as_str(), "sacrifice_a_creature");
    assert_eq!(
        sacrifice.cost,
        ResolutionCost::SacrificePermanent {
            filter: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..Default::default()
            },
            source_only: false,
        }
    );
    assert_eq!(discard.branch_id.as_str(), "discard_a_creature_card");
    assert_eq!(
        discard.cost,
        ResolutionCost::DiscardCard {
            filter: Some(CardTypeFilter::Creature),
        }
    );
    for (branch, step_id) in [
        (sacrifice, "draw_after_sacrifice"),
        (discard, "draw_after_discard"),
    ] {
        assert!(branch.body.yields.is_empty());
        let [draw] = branch.body.steps.as_slice() else {
            panic!("each paid branch draws once");
        };
        assert_eq!(draw.id.as_str(), step_id);
        assert_perform(
            draw,
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        );
    }
}

fn assert_single_sacrifice_choose<'a>(
    program: &'a CompiledProgram,
    expected_branch_id: &str,
    expected_filter: TargetFilter,
    expected_source_only: bool,
) -> &'a CompiledChooseBranch {
    assert_eq!(program.context, EffectContext::Ability);
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.slots.is_empty());
    assert!(program.body.yields.is_empty());
    let [step] = program.body.steps.as_slice() else {
        panic!("the triggered ability is one Program Choose");
    };
    assert_eq!(step.id, "choose_sacrifice");
    assert!(step.input.is_none());
    assert!(step.outputs.is_empty());
    let CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &step.kind
    else {
        panic!("the triggered ability uses Program Choose");
    };
    assert_eq!(choice.as_str(), "sacrifice");
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("the Program has one optional sacrifice branch");
    };
    assert_eq!(branch.branch_id.as_str(), expected_branch_id);
    assert_eq!(branch.requirement, CompiledChooseRequirement::Always);
    assert!(branch.body.yields.is_empty());
    assert_eq!(
        branch.cost,
        ResolutionCost::SacrificePermanent {
            filter: expected_filter,
            source_only: expected_source_only,
        }
    );
    branch
}

fn assert_single_tap_choose<'a>(
    program: &'a CompiledProgram,
    expected_branch_id: &str,
    expected_filter: TargetFilter,
    expected_exclude_source: bool,
) -> (
    &'a CompiledChooseBranch,
    &'a tricerules_cards::primitives::CompiledBlock,
) {
    assert_eq!(program.context, EffectContext::Ability);
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.slots.is_empty());
    assert!(program.body.yields.is_empty());
    let [step] = program.body.steps.as_slice() else {
        panic!("the triggered ability is one Program Choose");
    };
    assert_eq!(step.id, "choose_tap");
    assert!(step.input.is_none());
    assert!(step.outputs.is_empty());
    let CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &step.kind
    else {
        panic!("the triggered ability uses Program Choose");
    };
    assert_eq!(choice.as_str(), "tap_permanent");
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    let [branch] = branches.as_slice() else {
        panic!("the Program has one optional tap-cost branch");
    };
    assert_eq!(branch.branch_id.as_str(), expected_branch_id);
    assert_eq!(branch.requirement, CompiledChooseRequirement::Always);
    assert_eq!(
        branch.cost,
        ResolutionCost::TapPermanents {
            count: 1,
            filter: expected_filter,
            exclude_source: expected_exclude_source,
        }
    );
    (branch, otherwise)
}

#[test]
fn authored_heated_argument_is_exact_typed_optional_graveyard_exile_program() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("heated_argument")
        .expect("Heated Argument")
        .primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{R}");
    assert_eq!(face.types, ["Instant"]);
    assert!(
        face.spell_effect.is_empty(),
        "Heated Argument's exact owner uses its typed Program body"
    );

    let program = face_program("heated_argument");
    assert_eq!(program.context, EffectContext::Spell);
    assert_eq!(
        program.target_projection,
        tricerules_cards::primitives::ProgramTargetProjection::OwnerLocal
    );
    let [damage_target, choose] = program.body.steps.as_slice() else {
        panic!("Heated Argument damages one creature before its optional cost choice");
    };
    assert_perform(
        damage_target,
        SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(6),
            target: TargetFilter {
                kind: TargetKind::Creature,
                ..Default::default()
            },
        },
    );
    assert_eq!(program.target_bindings.len(), 1);
    assert_eq!(
        program.target_bindings[0].instruction_address,
        damage_target.instruction_address
    );
    assert_eq!(program.target_bindings[0].target_group_index, 0);
    assert_eq!(program.target_bindings[0].role_index, 0);

    let targeting = face
        .targeting
        .as_ref()
        .expect("the exact face owns a target group for DamageTarget");
    let [group] = targeting.groups.as_slice() else {
        panic!("Heated Argument has one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert!(group.effect_indices.is_empty());

    let CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        panic!("the paid rider is an optional typed Program Choose");
    };
    assert_eq!(choice.as_str(), "exile_graveyard_card");
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("Heated Argument has one optional graveyard-exile branch");
    };
    assert_eq!(branch.branch_id.as_str(), "exile_graveyard_card");
    assert_eq!(branch.requirement, CompiledChooseRequirement::Always);
    assert_eq!(
        branch.cost,
        ResolutionCost::ExileGraveyardCard { filter: None }
    );
    let [damage_player] = branch.body.steps.as_slice() else {
        panic!("the paid branch applies one fixed player-damage rider");
    };
    assert_eq!(program.group_inputs.len(), 1);
    assert_eq!(
        (
            program.group_inputs[0].instruction_address,
            program.group_inputs[0].group_index,
        ),
        (damage_player.instruction_address, 0),
        "the branch rider reads the cast's singleton target group"
    );
    assert_perform(
        damage_player,
        SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(2),
            who: PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
        },
    );
}

#[test]
fn authored_fanatic_of_the_harrowing_is_each_player_discard_then_controller_draw_program() {
    let program = ability_program("fanatic_of_the_harrowing", 0, false);
    assert_eq!(program.context, EffectContext::Ability);
    assert!(program.body.yields.is_empty());
    let [discard, draw_check] = program.body.steps.as_slice() else {
        panic!("Fanatic's exact trigger discards before checking its result");
    };
    assert_perform_effect(
        discard,
        SpellEffectKind::Discard {
            who: PlayerRecipient::EachPlayer,
            quantity: DiscardQuantity::Exact(1),
        },
    );
    assert_eq!(discard.outputs.len(), 1);
    let result = program
        .slot(discard.outputs[0])
        .expect("Fanatic binds the complete EachPlayer discard result");
    assert_eq!(
        result.result_type,
        ResultType::DiscardResult {
            who: PlayerRecipient::EachPlayer,
            cardinality: Cardinality { min: 0, max: None },
            optional_payment: false,
        }
    );

    let CompiledInstructionKind::If { condition, yes, no } = &draw_check.kind else {
        panic!("Fanatic checks whether its controller discarded a card");
    };
    let CompiledProgramCondition::Result(CompiledProgramResultCondition::DiscardCount {
        result: checked_result,
        players,
        card_type,
        min,
        max,
    }) = condition.as_ref()
    else {
        panic!("Fanatic's condition uses the typed discard result");
    };
    assert_eq!(*checked_result, discard.outputs[0]);
    assert_eq!(*players, RelativePlayerSet::Controller);
    assert_eq!(*card_type, None);
    assert_eq!(*min, Some(1));
    assert_eq!(*max, None);
    let [draw] = yes.steps.as_slice() else {
        panic!("the controller draws exactly one card after discarding");
    };
    assert_perform(
        draw,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        },
    );
    assert!(no.steps.is_empty() && no.yields.is_empty());
}

#[test]
fn authored_command_bridge_and_gravelgill_tap_triggers_are_exact_programs() {
    let registry = tricerules_cards::registry::global();
    let bridge_face = registry
        .get("command_bridge")
        .expect("Command Bridge")
        .primary_face();
    let [enters_tapped] = bridge_face.static_abilities.as_slice() else {
        panic!("Command Bridge has one enters-tapped replacement ability");
    };
    assert!(matches!(
        enters_tapped.definition,
        StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: None,
            unless_cost: None,
        }
    ));
    let [bridge_mana] = bridge_face.activated_abilities.as_slice() else {
        panic!("Command Bridge has one printed mana ability");
    };
    assert_eq!(bridge_mana.costs, [AbilityCost::Tap]);
    assert!(bridge_mana.is_mana_ability());
    let any_color_options = [
        ManaAmount {
            w: 1,
            ..Default::default()
        },
        ManaAmount {
            u: 1,
            ..Default::default()
        },
        ManaAmount {
            b: 1,
            ..Default::default()
        },
        ManaAmount {
            r: 1,
            ..Default::default()
        },
        ManaAmount {
            g: 1,
            ..Default::default()
        },
    ];
    assert!(matches!(
        bridge_mana.effect.as_slice(),
        [SpellEffectKind::ProduceMana {
            options,
            commander_color_identity: false,
            restriction: None,
            conditional: None,
        }] if options.as_slice() == any_color_options.as_slice()
    ));
    let [bridge_trigger] = bridge_face.triggered_abilities.as_slice() else {
        panic!("Command Bridge keeps its one entry trigger");
    };
    assert_eq!(bridge_trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        bridge_trigger.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    let bridge_program = ability_program("command_bridge", 0, false);
    let bridge_filter = TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller: TargetController::You,
        ..Default::default()
    };
    let (bridge_tap, bridge_otherwise) = assert_single_tap_choose(
        bridge_program,
        "tap_an_untapped_permanent_you_control",
        bridge_filter,
        false,
    );
    assert!(bridge_tap.body.steps.is_empty() && bridge_tap.body.yields.is_empty());
    let [sacrifice_source] = bridge_otherwise.steps.as_slice() else {
        panic!("declining or lacking the tap payment sacrifices Command Bridge");
    };
    assert_perform(
        sacrifice_source,
        SpellEffectKind::Sacrifice {
            subject: tricerules_cards::primitives::EffectSubject::Source,
        },
    );
    assert!(bridge_otherwise.yields.is_empty());

    let gravelgill_face = registry
        .get("gravelgill_scoundrel")
        .expect("Gravelgill Scoundrel")
        .primary_face();
    assert_eq!(gravelgill_face.power, Some(1));
    assert_eq!(gravelgill_face.toughness, Some(3));
    assert_eq!(
        gravelgill_face.keywords.as_slice(),
        &[tricerules_cards::primitives::Keyword::Vigilance]
    );
    let [gravelgill_trigger] = gravelgill_face.triggered_abilities.as_slice() else {
        panic!("Gravelgill Scoundrel keeps its one attack trigger");
    };
    assert_eq!(gravelgill_trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        gravelgill_trigger.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    let gravelgill_program = ability_program("gravelgill_scoundrel", 0, false);
    let creature_filter = TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::You,
        ..Default::default()
    };
    let (gravelgill_tap, gravelgill_otherwise) = assert_single_tap_choose(
        gravelgill_program,
        "tap_another_untapped_creature_you_control",
        creature_filter,
        true,
    );
    let [unblockable] = gravelgill_tap.body.steps.as_slice() else {
        panic!("paying the tap cost makes Gravelgill Scoundrel unblockable");
    };
    assert_perform(
        unblockable,
        SpellEffectKind::ApplyCombatRestriction {
            scope: tricerules_cards::primitives::CombatRestrictionScope::Source,
            restriction: tricerules_cards::primitives::CombatRestriction {
                cant_be_blocked: true,
                ..Default::default()
            },
        },
    );
    assert!(gravelgill_otherwise.steps.is_empty() && gravelgill_otherwise.yields.is_empty());
}

#[test]
fn authored_servant_and_west_wind_sacrifice_triggers_are_exact_programs() {
    let registry = tricerules_cards::registry::global();
    let servant_face = registry
        .get("servant_of_the_stinger")
        .expect("Servant of the Stinger")
        .primary_face();
    assert_eq!(servant_face.power, Some(1));
    assert_eq!(servant_face.toughness, Some(3));
    assert_eq!(
        servant_face.keywords.as_slice(),
        &[tricerules_cards::primitives::Keyword::Deathtouch]
    );
    let [servant] = servant_face.triggered_abilities.as_slice() else {
        panic!("Servant keeps its single combat-damage trigger");
    };
    assert_eq!(servant.ability_id.as_str(), "triggered_01");
    assert_eq!(
        servant.trigger,
        TriggerCondition::WheneverSelfDealsCombatDamageToPlayer
    );
    assert_eq!(
        servant.intervening_if,
        Some(GameCondition::CrimesCommittedThisTurn {
            players: RelativePlayerSet::Controller,
            min: Some(1),
            max: None,
        })
    );
    let servant_program = ability_program("servant_of_the_stinger", 0, false);
    let servant_filter = TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller: TargetController::You,
        ..Default::default()
    };
    let servant_branch = assert_single_sacrifice_choose(
        servant_program,
        "sacrifice_servant_of_the_stinger",
        servant_filter,
        true,
    );
    let [search] = servant_branch.body.steps.as_slice() else {
        panic!("a paid sacrifice searches the library");
    };
    assert_eq!(search.id, "search_library");
    assert!(search.input.is_none() && search.outputs.is_empty());
    let CompiledInstructionKind::Perform(effect) = &search.kind else {
        panic!("Servant performs its library search after payment");
    };
    let SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        count_by_cast_cost,
        filter,
        selection_constraint,
        slots,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
        result_id,
        selection_bounds,
    } = effect.as_ref()
    else {
        panic!("Servant searches for a card");
    };
    assert_eq!(*who, PlayerRecipient::Controller);
    assert!(!optional);
    assert_eq!(*count, 1);
    assert!(count_by_cast_cost.is_none());
    assert!(filter.is_none());
    assert!(selection_constraint.is_none());
    assert!(slots.is_empty());
    assert_eq!(
        zones,
        &SearchZoneSelection::Fixed(vec![CardSearchZone::Library])
    );
    assert_eq!(*destination, SearchDestination::Hand);
    assert!(conditional_destination.is_none());
    assert!(*shuffle);
    assert!(!reveal);
    assert!(result_id.is_none());
    assert_eq!(
        selection_bounds.as_ref(),
        Some(&SearchSelectionBounds { min: 1, max: 1 })
    );

    let avatar_face = registry
        .get("west_wind_avatar")
        .expect("West Wind Avatar")
        .primary_face();
    assert_eq!(avatar_face.power, Some(7));
    assert_eq!(avatar_face.toughness, Some(7));
    assert_eq!(
        avatar_face.keywords.as_slice(),
        &[tricerules_cards::primitives::Keyword::Trample]
    );
    let [entry, attack, disappear] = avatar_face.triggered_abilities.as_slice() else {
        panic!("West Wind keeps its two sacrifice triggers and Disappear trigger");
    };
    assert_eq!(entry.ability_id.as_str(), "triggered_01");
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(entry.intervening_if.is_none());
    assert_eq!(attack.ability_id.as_str(), "triggered_02");
    assert_eq!(
        attack.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert!(attack.intervening_if.is_none());
    assert_eq!(disappear.ability_id.as_str(), "triggered_03");
    assert_eq!(
        disappear.trigger,
        TriggerCondition::AtBeginningOfEndStep {
            player: CastTriggerPlayer::Controller,
        }
    );
    assert_eq!(
        disappear.intervening_if,
        Some(GameCondition::PermanentLeftBattlefieldThisTurn {
            controllers: RelativePlayerSet::Controller,
        })
    );

    let avatar_filter = TargetFilter {
        any_of: Some(vec![
            TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::You,
                token: Some(true),
                ..Default::default()
            },
            TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::You,
                permanent_types: vec![PermanentTypeFilter::Land],
                ..Default::default()
            },
        ]),
        ..Default::default()
    };
    for (index, ability_id) in [(0, "triggered_01"), (1, "triggered_02")] {
        assert_eq!(
            avatar_face.triggered_abilities[index].ability_id.as_str(),
            ability_id
        );
        let program = ability_program("west_wind_avatar", index, false);
        let branch = assert_single_sacrifice_choose(
            program,
            "sacrifice_a_token_or_land",
            avatar_filter.clone(),
            false,
        );
        let [gain_life] = branch.body.steps.as_slice() else {
            panic!("paying the sacrifice cost gains three life");
        };
        assert_eq!(gain_life.id, "gain_life");
        assert_perform(
            gain_life,
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(3),
            },
        );
    }

    let draw_program = ability_program("west_wind_avatar", 2, false);
    assert_eq!(draw_program.context, EffectContext::Ability);
    assert!(draw_program.required_inputs.is_empty());
    assert!(draw_program.target_bindings.is_empty());
    assert!(draw_program.slots.is_empty());
    let [draw] = draw_program.body.steps.as_slice() else {
        panic!("Disappear is one direct draw operation");
    };
    assert_eq!(draw.id, "draw");
    assert_perform(
        draw,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        },
    );
}
