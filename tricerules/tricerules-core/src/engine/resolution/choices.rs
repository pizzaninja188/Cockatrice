use super::{EffectCx, EffectOutcome};
use crate::engine::events::ev_log;
use crate::engine::{rv1, EngineError};
use crate::state::{
    PendingResolution, PendingResolutionBranch, StagedTrigger, StagedTriggerGroup, TriggerContext,
};
use tricerules_cards::primitives::{
    ResolutionCost, SearchDestination, SpellEffectKind, TriggerCondition, TriggeredAbilityDef,
};

pub(super) fn choose_resolution_branch(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChooseResolutionBranch { optional, branches } = effect else {
        unreachable!();
    };
    park_resolution_branches(cx, optional, branches)
}

pub(super) fn park_resolution_branches(
    cx: &mut EffectCx<'_>,
    optional: bool,
    branches: Vec<tricerules_cards::primitives::ResolutionBranchDef>,
) -> Result<EffectOutcome, EngineError> {
    let options = branches
        .iter()
        .enumerate()
        .map(|(index, branch)| {
            let (kind, cost_text, selectable) = match &branch.cost {
                ResolutionCost::None => (
                    rv1::ResolutionBranchCostKind::Unspecified,
                    String::new(),
                    true,
                ),
                ResolutionCost::Mana(cost) => {
                    (rv1::ResolutionBranchCostKind::Mana, cost.to_string(), true)
                }
                ResolutionCost::DiscardCard { .. } => (
                    rv1::ResolutionBranchCostKind::DiscardCard,
                    "discard a matching card".into(),
                    !cx.engine
                        .resolution_cost_candidates(cx.controller, &branch.cost)
                        .is_empty(),
                ),
                ResolutionCost::SacrificePermanent { .. } => (
                    rv1::ResolutionBranchCostKind::SacrificePermanent,
                    "sacrifice a matching permanent".into(),
                    !cx.engine
                        .resolution_cost_candidates(cx.controller, &branch.cost)
                        .is_empty(),
                ),
            };
            rv1::ResolutionBranchOption {
                branch_index: index as u32,
                label: branch.label.clone(),
                cost_kind: kind as i32,
                cost_text,
                selectable,
            }
        })
        .collect();
    let prompt = if optional {
        "Choose a resolution option, or decline."
    } else {
        "Choose a resolution option."
    }
    .to_string();
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: cx.controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                candidate_object_ids: Vec::new(),
                candidate_card_ids: Vec::new(),
                min: u32::from(!optional),
                max: 1,
                ordered: false,
                candidate_names: Vec::new(),
                candidate_server_card_ids: Vec::new(),
                candidate_selectable: Vec::new(),
                unique_names: false,
                generic_mana_cost: 0,
                payment_currently_legal: false,
                resolution_branches: options,
                mana_cost: String::new(),
            },
        )),
    });
    cx.events.push(ev_log(prompt.clone()));
    cx.engine.state.pending_resolution = Some(PendingResolution {
        item: cx.top.clone(),
        custom_key: "__resolution_branch".into(),
        step: 0,
        scratch: Vec::new(),
        deciding_player: cx.controller,
        candidates: Vec::new(),
        min: u32::from(!optional),
        max: 1,
        ordered: false,
        prompt,
        choice_kind: rv1::ChoiceKind::ResolutionBranch,
        unique_names: false,
        mana_payment: None,
        resolution_branch: Some(PendingResolutionBranch {
            optional,
            branches,
            selected_branch: None,
        }),
        discard: None,
        copy_source_object_id: 0,
        search_destination: SearchDestination::Hand,
        search_shuffle: false,
        search_reveal: false,
        resume_effect_index: None,
    });
    Ok(EffectOutcome::Suspended)
}

pub(super) fn create_reflexive_trigger(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreateReflexiveTrigger { ability } = effect else {
        unreachable!();
    };
    let source_id = cx
        .top
        .source_permanent_id
        .ok_or(EngineError::Illegal("reflexive trigger source missing"))?;
    let object_id = cx.engine.state.next_object_id;
    cx.engine.state.next_object_id += 1;
    let card_name = cx
        .engine
        .registry
        .get(&cx.top.card_id)
        .map(|definition| definition.name.clone())
        .unwrap_or_else(|| cx.top.card_id.clone());
    let ability_text = ability.text.clone();
    cx.engine
        .state
        .staged_trigger_groups
        .push_back(StagedTriggerGroup {
            triggers: vec![StagedTrigger {
                object_id,
                source_permanent_id: source_id,
                source_face_index: cx.top.face_index,
                source_zone_change: cx.top.source_zone_change,
                source_face_change: cx.top.source_face_change,
                card_id: cx.top.card_id.clone(),
                card_name,
                controller: cx.controller,
                ability_index: 0,
                ability: TriggeredAbilityDef {
                    // This definition is staged directly; the condition is never scanned.
                    trigger: TriggerCondition::WhenSelfEntersBattlefield,
                    effect: ability.effect,
                    modal: None,
                    targeting: ability.targeting,
                    text: ability_text.clone(),
                    may: false,
                    intervening_if: None,
                },
                ability_text,
                trigger_context: TriggerContext::default(),
                may: false,
            }],
        });
    Ok(EffectOutcome::Continue)
}
