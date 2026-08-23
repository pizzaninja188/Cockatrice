use super::{EffectCx, EffectOutcome};
use crate::engine::events::ev_log;
use crate::engine::{rv1, EngineError};
use crate::state::{
    ParkedStackResolution, PendingResolution, PendingResolutionBranch,
    PendingResolutionBranchStage, PendingResolutionPresentation, ResolutionContinuation, StackItem,
    StagedTrigger, StagedTriggerGroup, TriggerContext,
};
use tricerules_cards::primitives::{
    PlayerRecipient, ResolutionBranchDef, ResolutionBranchRequirement, ResolutionBranchSelection,
    ResolutionCost, SpellEffectKind, TriggerCondition, TriggeredAbilityDef,
};

pub(super) fn choose_resolution_branch(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChooseResolutionBranch {
        chooser,
        optional,
        selection,
        branches,
    } = effect
    else {
        unreachable!();
    };
    let recipients = super::player_recipients(cx, chooser);
    let [deciding_player] = recipients.as_slice() else {
        return Err(EngineError::Illegal(
            "resolution choice requires exactly one deciding player",
        ));
    };
    let legal = branches
        .iter()
        .enumerate()
        .filter(|(_, branch)| {
            resolution_branch_is_live(cx.engine, cx.top, *deciding_player, branch)
        })
        .collect::<Vec<_>>();
    if selection == ResolutionBranchSelection::FirstApplicable {
        let Some((branch_index, branch)) = legal.first() else {
            return Err(EngineError::Illegal(
                "automatic resolution branch has no applicable fallback",
            ));
        };
        cx.events.push(ev_log(format!(
            "P{} resolves: {}.",
            deciding_player, branch.label
        )));
        return Ok(EffectOutcome::RestartResolutionBranch(Some(*branch_index)));
    }
    match (optional, legal.as_slice()) {
        (false, []) => {
            cx.events.push(ev_log(format!(
                "P{} has no legal resolution branch.",
                deciding_player
            )));
            Ok(EffectOutcome::RestartResolutionBranch(None))
        }
        (false, [(branch_index, branch)]) => {
            cx.events.push(ev_log(format!(
                "P{} chooses: {}.",
                deciding_player, branch.label
            )));
            Ok(EffectOutcome::RestartResolutionBranch(Some(*branch_index)))
        }
        (true, []) => {
            cx.events.push(ev_log(format!(
                "P{} declines the optional resolution choice.",
                deciding_player
            )));
            Ok(EffectOutcome::RestartResolutionBranch(None))
        }
        _ => park_resolution_branches_for(cx, chooser, optional, branches),
    }
}

pub(in crate::engine) fn resolution_branch_is_live(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    deciding_player: i32,
    branch: &ResolutionBranchDef,
) -> bool {
    let requirement_met = match &branch.requirement {
        ResolutionBranchRequirement::Always => true,
        ResolutionBranchRequirement::EffectsApplicable => {
            branch.effects.iter().all(|effect| match effect {
                SpellEffectKind::PutCounters { subject, .. } => {
                    super::pump_counters::can_put_counters(engine, top, &[], subject)
                }
                _ => true,
            })
        }
        ResolutionBranchRequirement::GameCondition(condition) => engine.condition_holds(
            condition,
            crate::engine::ConditionContext::for_stack_item(top),
        ),
    };
    requirement_met
        && (matches!(branch.cost, ResolutionCost::None | ResolutionCost::Mana(_))
            || !engine
                .resolution_cost_candidates(deciding_player, &branch.cost)
                .is_empty())
}

pub(super) fn park_resolution_branches(
    cx: &mut EffectCx<'_>,
    optional: bool,
    branches: Vec<tricerules_cards::primitives::ResolutionBranchDef>,
) -> Result<EffectOutcome, EngineError> {
    park_resolution_branches_for(
        cx,
        tricerules_cards::primitives::PlayerRecipient::Controller,
        optional,
        branches,
    )
}

fn park_resolution_branches_for(
    cx: &mut EffectCx<'_>,
    chooser: PlayerRecipient,
    optional: bool,
    branches: Vec<tricerules_cards::primitives::ResolutionBranchDef>,
) -> Result<EffectOutcome, EngineError> {
    let recipients = super::player_recipients(cx, chooser);
    let [deciding_player] = recipients.as_slice() else {
        return Err(EngineError::Illegal(
            "resolution choice requires exactly one deciding player",
        ));
    };
    let deciding_player = *deciding_player;
    let options = branches
        .iter()
        .enumerate()
        .filter(|(_, branch)| resolution_branch_is_live(cx.engine, cx.top, deciding_player, branch))
        .map(|(index, branch)| {
            let (kind, cost_text) = match &branch.cost {
                ResolutionCost::None => (rv1::ResolutionBranchCostKind::Unspecified, String::new()),
                ResolutionCost::Mana(cost) => {
                    (rv1::ResolutionBranchCostKind::Mana, cost.to_string())
                }
                ResolutionCost::DiscardCard { .. } => (
                    rv1::ResolutionBranchCostKind::DiscardCard,
                    "discard a matching card".into(),
                ),
                ResolutionCost::SacrificePermanent { .. } => (
                    rv1::ResolutionBranchCostKind::SacrificePermanent,
                    "sacrifice a matching permanent".into(),
                ),
            };
            rv1::ResolutionBranchOption {
                branch_index: index as u32,
                label: branch.label.clone(),
                cost_kind: kind as i32,
                cost_text,
                selectable: true,
                search_zones: Vec::new(),
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
                deciding_player_id: deciding_player,
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
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
            },
        )),
    });
    cx.events.push(ev_log(prompt.clone()));
    cx.engine.state.pending_resolution = Some(PendingResolution {
        deciding_player,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: Vec::new(),
            min: u32::from(!optional),
            max: 1,
            ordered: false,
            prompt,
            choice_kind: rv1::ChoiceKind::ResolutionBranch,
            unique_names: false,
        },
        continuation: ResolutionContinuation::AuthoredBranch {
            stack: ParkedStackResolution::new(cx.top.clone()),
            branch: PendingResolutionBranch {
                optional,
                chooser,
                branches,
                stage: PendingResolutionBranchStage::Selecting,
            },
        },
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
                    triggers_only_once: false,
                },
                ability_text,
                trigger_context: TriggerContext::default(),
                may: false,
            }],
        });
    Ok(EffectOutcome::Continue)
}
