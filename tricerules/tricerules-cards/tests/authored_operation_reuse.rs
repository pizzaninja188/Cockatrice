//! Exact authored owners for the reusable Scry, search, and Proliferate operations.

use tricerules_cards::primitives::{
    Amount, CardSearchZone, CardTypeFilter, Cardinality, CastCostOptionDef,
    CompiledChooseRequirement, CompiledInstruction, CompiledInstructionKind,
    CompiledProgramCondition, CompiledProgramResultCondition, DiscardQuantity,
    DiscardResultOutcome, PlayerRecipient, ResultType, SearchDestination, SearchSelectionBounds,
    SearchZoneSelection, SpellEffectKind, TargetController, TargetFilter, TargetKind,
    ZoneCardFilter,
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
