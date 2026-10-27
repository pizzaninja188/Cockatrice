//! Memory Jar's private, occurrence-bound hand cohort.
use super::*;
use crate::state::MemoryJarCohort;

pub(super) fn exile_hands(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let key = cx.top.id;
    if cx.engine.state.memory_jar_cohorts.contains_key(&key) {
        return Err(EngineError::ProgramInvariant(
            "Memory Jar activation already captured its hands",
        ));
    }
    let players = player_recipients(cx, PlayerRecipient::EachPlayer);
    let mut moves = Vec::new();
    for player in players {
        if let Some(index) = cx.engine.state.player_idx(player) {
            moves.extend(
                cx.engine.state.players[index]
                    .hand
                    .iter()
                    .copied()
                    .map(|id| (id, Zone::Exile, None)),
            );
        }
    }
    let receipts = if moves.is_empty() {
        Vec::new()
    } else {
        cx.engine.commit_memory_jar_exile_batch(&moves, cx.events)?
    };
    let mut cards = Vec::with_capacity(receipts.len());
    for receipt in receipts {
        if receipt.origin != Zone::Hand || receipt.destination != Zone::Exile {
            return Err(EngineError::ProgramInvariant(
                "Memory Jar received an unexpected exile receipt",
            ));
        }
        cards.push(TriggerObjectRef {
            object_id: receipt.object_id,
            zone_change_generation: receipt.destination_generation,
            controller_at_event: receipt.owner,
        });
        cx.events.push(permanent_moved_event(
            &cx.engine.state,
            receipt.object_id,
            receipt.owner,
            rv1::permanent_moved::Destination::Exile,
        ));
    }
    cx.engine.state.memory_jar_cohorts.insert(
        key,
        MemoryJarCohort {
            controller: cx.controller,
            cards,
        },
    );
    Ok(EffectOutcome::Continue)
}

pub(super) fn return_hands(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let key = cx
        .top
        .trigger_context
        .memory_jar_cohort
        .ok_or(EngineError::ProgramInvariant(
            "Memory Jar return lost its delayed occurrence",
        ))?;
    let cohort =
        cx.engine
            .state
            .memory_jar_cohorts
            .get(&key)
            .ok_or(EngineError::ProgramInvariant(
                "Memory Jar return lost its hand cohort",
            ))?;
    let cards = cohort
        .cards
        .iter()
        .copied()
        .filter(|card| {
            cx.engine
                .state
                .objects
                .get(&card.object_id)
                .is_some_and(|object| {
                    object.zone == Zone::Exile
                        && object.owner == card.controller_at_event
                        && cx
                            .engine
                            .state
                            .zone_change_generation
                            .get(&card.object_id)
                            .copied()
                            == Some(card.zone_change_generation)
                        && cx
                            .engine
                            .state
                            .player_idx(object.owner)
                            .is_some_and(|index| !cx.engine.state.players[index].has_lost)
                })
        })
        .collect::<Vec<_>>();
    // The shared owner-destination transaction freezes these exact live Exile incarnations and
    // collects every CR 903.9b choice before moving the whole cohort. If it parks, its stack
    // retains the occurrence key and ordinary stack completion retires the cohort after commit.
    let ids = cards.into_iter().map(|card| card.object_id).collect();
    let outcome =
        super::battlefield_returns::begin_battlefield_cohort_move(cx, ids, Zone::Hand, Vec::new())?;
    if outcome == EffectOutcome::Continue {
        cx.engine.state.memory_jar_cohorts.remove(&key);
    }
    Ok(outcome)
}
