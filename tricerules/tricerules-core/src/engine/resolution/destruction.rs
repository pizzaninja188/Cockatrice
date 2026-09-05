//! Shared CR 701.8 destruction execution for Murder, Wrath of God, and Turn to Slag.
//! Selection, snapshot timing, result collection, and simultaneous trigger groups belong to
//! callers. This is deliberately separate from sacrifice and lethal-damage SBA execution.
use super::*;
use crate::state::CardResultEntry;

/// Facts captured at the caller's existing evaluation boundary. In particular, attachment
/// destruction freezes indestructible for the whole cohort before moving its first member.
pub(super) struct DestroySnapshot {
    pub object_id: ObjectId,
    pub name: String,
    pub indestructible: bool,
    pub owner: Option<PlayerId>,
    pub source: Option<TriggerSourceSnapshot>,
    pub was_creature: bool,
}

/// Preserve the existing subject/cohort messages and log-before/log-after movement ordering.
#[derive(Clone, Copy)]
pub(super) enum DestroyLogStyle {
    Subject,
    Cohort,
}

pub(super) enum DestroyOutcome {
    Indestructible,
    Regenerated {
        trigger_events: Vec<GameEvent>,
    },
    Destroyed {
        receipt: Option<CardResultEntry>,
        trigger_events: Vec<GameEvent>,
    },
}

/// Attempt one already-selected destruction. No trigger collection, priority changes, or SBAs
/// occur here: the caller must publish the returned trigger inputs at its instruction boundary.
pub(super) fn attempt_destroy(
    engine: &mut GameEngine,
    snapshot: DestroySnapshot,
    prevent_regeneration: bool,
    spell_label: &str,
    log_style: DestroyLogStyle,
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<DestroyOutcome, EngineError> {
    let DestroySnapshot {
        object_id,
        name,
        indestructible,
        owner,
        source,
        was_creature,
    } = snapshot;
    if indestructible {
        events.push(ev_log(match log_style {
            DestroyLogStyle::Subject => {
                format!("{spell_label} has no effect: {name} is indestructible.")
            }
            DestroyLogStyle::Cohort => {
                format!("{name} is indestructible and survives {spell_label}.")
            }
        }));
        return Ok(DestroyOutcome::Indestructible);
    }
    // CR 701.19c: a prohibition bypasses shields without consuming one or tapping the object.
    if !prevent_regeneration {
        let (regenerated, tap_event) = consume_regen_shield(engine, object_id, events);
        if regenerated {
            events.push(ev_log(format!("{name} regenerates.")));
            return Ok(DestroyOutcome::Regenerated {
                trigger_events: tap_event.into_iter().collect(),
            });
        }
    }
    let log = ev_log(format!("{spell_label} destroys {name}"));
    if matches!(log_style, DestroyLogStyle::Subject) {
        events.push(log.clone());
    }
    let died = destroy_permanent(&mut engine.state, engine.registry, object_id)?;
    if matches!(log_style, DestroyLogStyle::Cohort) {
        events.push(log);
    }
    let receipt = owner.map(|owner_id| {
        // Successful destruction counts even when a replacement changes the destination.
        // Indestructible and regeneration never reach this point.
        let receipt = payment::card_result_entry(
            &engine.state,
            engine.registry,
            CardResultAction::Destroy,
            owner_id,
            object_id,
        );
        events.push(permanent_moved_event(
            &engine.state,
            object_id,
            owner_id,
            rv1::permanent_moved::Destination::Graveyard,
        ));
        receipt
    });
    let trigger_events = source.map_or_else(Vec::new, |source| {
        leaves_and_dies_events(source, was_creature, died)
    });
    Ok(DestroyOutcome::Destroyed {
        receipt,
        trigger_events,
    })
}
