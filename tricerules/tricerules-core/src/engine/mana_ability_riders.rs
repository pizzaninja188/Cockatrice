//! Typed instructions that follow a mana ability's production (CR 605.1a).
//!
//! A mana ability has no stack object (CR 605.3b), so the stack effect dispatcher cannot run its
//! trailing instructions. The card model admits a closed [`ManaAbilityRider`] vocabulary instead,
//! and this module resolves every rider, painland damage included, immediately after the mana is
//! added. The unresolved remainder travels as one [`ManaAbilityResolution`]: whichever rider must
//! wait for a player choice (a damage prevention choice, a CR 616.1 counter-replacement order)
//! parks it in that choice's continuation, and the resume runs the remaining riders in printed
//! order before restoring any mana-payment decision the choice interrupted.

use super::continuous::CounterPlacementOrigin;
use super::damage::{DamageSpec, ManaAbilityDamageOutcome};
use super::events::{ev_log, object_display_name};
use super::resolution::{permanent_moved_event, sacrifice_permanent};
use super::*;
use crate::state::{CounterPlacementCompletion, ManaAbilityResolution};
use tricerules_card_model::primitives::ManaAbilityRider;

/// True when the ability has a rider beyond the painland damage pipeline. Such an activation
/// changes its source after production, so it neither joins the floating-mana undo list nor
/// participates in attack-payment receipt replay.
pub(super) fn has_source_riders(ability: &ActivatedAbilityDef) -> bool {
    ability
        .mana_ability_riders()
        .iter()
        .any(|rider| !matches!(rider, ManaAbilityRider::DamageController { .. }))
}

/// Outcome of running a mana ability's riders.
pub(super) enum ManaAbilityProgress {
    /// Every rider resolved. Painland damage results feed the attack-payment receipt.
    Complete {
        damage: Option<ManaAbilityDamageOutcome>,
    },
    /// A rider parked a player choice that now owns the rest of the ability.
    Parked,
}

impl GameEngine {
    /// The unstacked remainder of a mana ability whose production has just been added.
    /// `damage` is the painland damage captured before costs were paid, and
    /// `source_zone_change_generation` the source incarnation that paid them.
    pub(super) fn begin_mana_ability_resolution(
        player: PlayerId,
        source: ObjectId,
        source_zone_change_generation: u64,
        source_label: &str,
        ability: &ActivatedAbilityDef,
        damage: Option<DamageSpec>,
    ) -> ManaAbilityResolution {
        ManaAbilityResolution {
            player,
            source_object_id: source,
            source_zone_change_generation,
            source_label: source_label.to_owned(),
            remaining_riders: ability.mana_ability_riders().to_vec(),
            damage,
            suspended_payment: None,
        }
    }

    /// Resolve the remaining riders in printed order. When none parks, the interrupted
    /// mana-payment decision (if any) is published again.
    pub(super) fn continue_mana_ability_resolution(
        &mut self,
        mut resolution: ManaAbilityResolution,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<ManaAbilityProgress, EngineError> {
        let mut damage_results = None;
        while !resolution.remaining_riders.is_empty() {
            let rider = resolution.remaining_riders.remove(0);
            if let ManaAbilityRider::DamageController { .. } = rider {
                // The damage source identity was captured before costs were paid, so the
                // damage still happens after a cost moved the source.
                let Some(damage) = resolution.damage.take() else {
                    return Err(EngineError::ProgramInvariant(
                        "a painland damage rider lost its pre-cost damage capture",
                    ));
                };
                match self.resolve_mana_ability_damage(damage, &resolution, out)? {
                    Some(results) => damage_results = Some(results),
                    None => return Ok(ManaAbilityProgress::Parked),
                }
                continue;
            }
            if !self.mana_rider_source_is_current(
                resolution.source_object_id,
                resolution.source_zone_change_generation,
            ) {
                // CR 400.7: a source sacrificed by an earlier rider, or by a cost replacement,
                // is a new object; later riders have nothing to act on.
                resolution.remaining_riders.clear();
                break;
            }
            match rider {
                ManaAbilityRider::DamageController { .. } => unreachable!("handled above"),
                ManaAbilityRider::PutSourceCounters { counter, count } => {
                    match self.put_mana_rider_counters(resolution, counter, count, out)? {
                        Some(next) => resolution = next,
                        None => return Ok(ManaAbilityProgress::Parked),
                    }
                }
                ManaAbilityRider::SacrificeSourceWithoutCounters { counter } => {
                    let remaining =
                        self.state.objects[&resolution.source_object_id].counter_count(counter);
                    if remaining == 0 {
                        self.sacrifice_mana_rider_source(
                            resolution.player,
                            resolution.source_object_id,
                            out,
                        )?;
                    }
                }
            }
        }
        self.restore_mana_ability_payment(resolution.suspended_payment.take(), out)?;
        Ok(ManaAbilityProgress::Complete {
            damage: damage_results,
        })
    }

    fn mana_rider_source_is_current(&self, source: ObjectId, source_generation: u64) -> bool {
        self.state
            .objects
            .get(&source)
            .is_some_and(|object| object.zone == Zone::Battlefield)
            && self
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0)
                == source_generation
    }

    /// CR 122.6 / 616.1: placement uses the ordinary counter-replacement planner. When two or
    /// more replacements apply, the affected object's controller orders them: the parked choice
    /// takes ownership of `resolution` (already advanced past this rider) and this returns
    /// `None`.
    fn put_mana_rider_counters(
        &mut self,
        mut resolution: ManaAbilityResolution,
        counter: CounterKind,
        count: u32,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<ManaAbilityResolution>, EngineError> {
        let Some(proposed) = self.proposed_object_counter_placement(
            resolution.player,
            resolution.source_object_id,
            [(counter, count)],
            CounterPlacementOrigin::Effect,
        ) else {
            return Ok(Some(resolution));
        };
        let (batch, candidates) = self.prepare_counter_placement_batch(vec![proposed])?;
        if let Some(candidates) = candidates {
            self.suspend_mana_ability_resolution_payment(&mut resolution);
            self.park_counter_placement_choice(
                batch,
                candidates,
                None,
                CounterPlacementCompletion::ManaAbility(Box::new(resolution)),
                out,
            );
            return Ok(None);
        }
        let completion = CounterPlacementCompletion::ManaAbility(Box::new(resolution));
        self.commit_counter_placement_batch(&batch.events, &completion, None, out)?;
        let CounterPlacementCompletion::ManaAbility(resolution) = completion else {
            unreachable!("constructed above");
        };
        Ok(Some(*resolution))
    }

    /// Move a mana-payment decision this player owns out of `pending_resolution`, so a rider's
    /// choice can own player input. It is restored once the ability's last rider finishes.
    pub(super) fn suspend_mana_ability_resolution_payment(
        &mut self,
        resolution: &mut ManaAbilityResolution,
    ) {
        if resolution.suspended_payment.is_none()
            && self
                .state
                .pending_resolution
                .as_ref()
                .is_some_and(|pending| {
                    pending.deciding_player == resolution.player
                        && pending.continuation.mana_window_undo_start().is_some()
                })
        {
            resolution.suspended_payment = self.state.pending_resolution.take().map(Box::new);
        }
    }

    fn restore_mana_ability_payment(
        &mut self,
        suspended: Option<Box<PendingResolution>>,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut suspended) = suspended else {
            return Ok(());
        };
        // The parent's answer was suspended while a nested rider choice owned input. Resuming it
        // publishes a new offer, even when its visible payload is byte-for-byte identical.
        suspended.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
        suspended.resolution_prompt_semantics = None;
        self.state.pending_resolution = Some(*suspended);
        if !self.refresh_suspended_ward_payment(out)? {
            out.push(
                self.resolution_payment_choice_event()
                    .expect("suspended resolution payment has a prompt"),
            );
        }
        Ok(())
    }

    /// Resume a counter-rider placement whose CR 616.1 order choice was just answered: commit
    /// the chosen placement, then run the riders that follow it.
    pub(in crate::engine) fn finish_mana_ability_counter_choice(
        &mut self,
        events: &[super::replacement::ProposedCounterPlacement],
        completion: &CounterPlacementCompletion,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let CounterPlacementCompletion::ManaAbility(resolution) = completion else {
            return Err(EngineError::Illegal(
                "mana-ability counter choice lost its continuation",
            ));
        };
        self.commit_counter_placement_batch(events, completion, None, out)?;
        self.continue_mana_ability_resolution((**resolution).clone(), out)?;
        Ok(())
    }

    /// CR 701.21a: the ability's controller sacrifices the source only while still controlling
    /// it. Departure and sacrifice observers see one zone event, as for a resolving Sacrifice.
    fn sacrifice_mana_rider_source(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(snapshot) = self.trigger_source_snapshot(source) else {
            return Ok(());
        };
        if snapshot.controller != player {
            return Ok(());
        }
        let was_creature = self
            .characteristics(source)
            .is_some_and(|characteristics| characteristics.is_creature());
        let owner = self.state.objects[&source].owner;
        let name = object_display_name(&self.state, self.registry, source);
        let zone_snapshot = self.snapshot_zone_event();
        let died = sacrifice_permanent(&mut self.state, self.registry, source)?;
        out.push(permanent_moved_event(
            &self.state,
            source,
            owner,
            rv1::permanent_moved::Destination::Graveyard,
        ));
        out.push(ev_log(format!("P{player} sacrifices {name}")));
        let committed = sacrifice_events(snapshot, was_creature, player, died);
        self.fire_zone_triggers(zone_snapshot, committed, out);
        Ok(())
    }
}
