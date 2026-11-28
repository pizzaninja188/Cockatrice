//! Typed instructions that follow a mana ability's production (CR 605.1a).
//!
//! A mana ability has no stack object (CR 605.3b), so the stack effect dispatcher cannot run its
//! trailing instructions. The card model admits a closed [`ManaAbilityRider`] vocabulary instead,
//! and this module resolves each non-damage rider immediately after the mana is added. Painland
//! damage keeps its prepared damage pipeline in `damage.rs`.

use super::continuous::CounterPlacementOrigin;
use super::events::{ev_log, object_display_name};
use super::resolution::{permanent_moved_event, sacrifice_permanent};
use super::*;
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

impl GameEngine {
    /// Resolve the source riders of a mana ability whose production has just been added.
    /// `source_generation` is the source incarnation that paid the costs: a rider never follows
    /// the source to a new object (CR 400.7).
    pub(super) fn resolve_mana_ability_source_riders(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        source_generation: u64,
        source_label: &str,
        ability: &ActivatedAbilityDef,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        for rider in ability.mana_ability_riders() {
            if !self.mana_rider_source_is_current(source, source_generation) {
                // CR 400.7: a source sacrificed by an earlier rider, or by a cost replacement,
                // is a new object; later riders have nothing to act on.
                break;
            }
            match rider {
                ManaAbilityRider::DamageController { .. } => {}
                ManaAbilityRider::PutSourceCounters { counter, count } => {
                    self.put_mana_rider_counters(player, source, counter, count, source_label, out)?
                }
                ManaAbilityRider::SacrificeSourceWithoutCounters { counter } => {
                    let remaining = self.state.objects[&source].counter_count(counter);
                    if remaining == 0 {
                        self.sacrifice_mana_rider_source(player, source, out)?;
                    }
                }
            }
        }
        Ok(())
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

    /// CR 122.6 / 616.1: placement uses the ordinary counter-replacement planner. A mana ability
    /// cannot suspend for a replacement-order choice here, so a board that would require one
    /// rejects the whole activation through the complete-command rollback.
    fn put_mana_rider_counters(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        counter: CounterKind,
        count: u32,
        source_label: &str,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(proposed) = self.proposed_object_counter_placement(
            player,
            source,
            [(counter, count)],
            CounterPlacementOrigin::Effect,
        ) else {
            return Ok(());
        };
        let (batch, candidates) = self.prepare_counter_placement_batch(vec![proposed])?;
        if candidates.is_some() {
            return Err(EngineError::ProgramInvariant(
                "a mana ability counter rider cannot suspend for a replacement-order choice",
            ));
        }
        self.commit_counter_placement_batch(
            &batch.events,
            &crate::state::CounterPlacementCompletion::StackEffect {
                spell_label: source_label.to_owned(),
            },
            None,
            out,
        )?;
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
