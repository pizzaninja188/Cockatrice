//! Internal execution form shared by flat effect lists and compiled Program leaves.

use super::*;

/// A snapshot of the selected target incarnations and their owner-local roles.
/// Non-target group operands may contribute receipts without making the leaf targeted.
#[derive(Default)]
pub(super) struct ResolvedTargets {
    pub(super) has_bindings: bool,
    pub(super) targets: Vec<ObjectId>,
    pub(super) targets_by_role: Vec<Vec<ObjectId>>,
    pub(super) target_receipts: Vec<StackTarget>,
    pub(super) role_group_indices: Vec<u32>,
}

impl ResolvedTargets {
    pub(super) fn new(
        has_bindings: bool,
        targets: Vec<ObjectId>,
        target_receipts: Vec<StackTarget>,
        role_group_indices: Vec<u32>,
    ) -> Self {
        let targets_by_role = role_group_indices
            .iter()
            .map(|group_index| {
                target_receipts
                    .iter()
                    .filter(|target| target.group_index == *group_index)
                    .map(|target| target.object_id)
                    .collect()
            })
            .collect();
        Self {
            has_bindings,
            targets,
            targets_by_role,
            target_receipts,
            role_group_indices,
        }
    }

    pub(super) fn has_no_bound_targets(&self) -> bool {
        self.has_bindings && self.targets.is_empty()
    }
}

/// Both drivers lower a leaf into this form after their own cursor/input preparation.
/// Skip outputs, typed slot publication, retry and rollback remain with the owning driver.
pub(super) struct ResolvedInstruction<'a> {
    pub(super) effect: SpellEffectKind,
    pub(super) targets: &'a ResolvedTargets,
    pub(super) index: u32,
    pub(super) input: &'a dyn ResolutionFacts,
    pub(super) previous_result: &'a OperationResults,
    pub(super) program_state: Option<ProgramRunState>,
    pub(super) affected_player: PlayerId,
}

impl ResolvedInstruction<'_> {
    pub(super) fn execute(
        self,
        engine: &mut GameEngine,
        top: &StackItem,
        spell_label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(EffectOutcome, OperationResults), EngineError> {
        let mut result = OperationResults::default();
        let mut cx = EffectCx {
            engine,
            events,
            top,
            spell_label,
            controller: top.controller,
            affected_player: self.affected_player,
            targets: &self.targets.targets,
            targets_by_role: &self.targets.targets_by_role,
            target_receipts: &self.targets.target_receipts,
            target_role_group_indices: &self.targets.role_group_indices,
            previous_effect_result: self.input,
            resume_previous_result: Some(self.previous_result),
            effect_result: &mut result,
            effect_index: self.index,
            program_state: self.program_state,
        };
        let outcome = execute_leaf(&mut cx, self.effect)?;
        Ok((outcome, result))
    }
}

impl GameEngine {
    /// The driver has already selected the exact retry/next cursor and published its results.
    pub(super) fn finish_resolution_instruction_boundary(
        &mut self,
        observer_stack: ParkedStackResolution,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        if self.drain_immediate_observer_actions(Some(observer_stack), events)? {
            return Ok(true);
        }
        self.refresh_hideaway_look_permissions();
        self.check_millennium_calendar_state_triggers();
        Ok(false)
    }
}
