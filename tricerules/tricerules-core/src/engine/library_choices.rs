//! Generation-bound library operations shared by ordinary resolution and draw replacements.
use super::*;

pub(super) fn capture(engine: &GameEngine, cards: &[ObjectId]) -> Vec<(ObjectId, u64)> {
    cards
        .iter()
        .map(|oid| {
            (
                *oid,
                engine
                    .state
                    .zone_change_generation
                    .get(oid)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect()
}

pub(super) fn current(engine: &GameEngine, player: PlayerId, cards: &[(ObjectId, u64)]) -> bool {
    let Some(index) = engine.state.player_idx(player) else {
        return false;
    };
    cards.iter().all(|(oid, generation)| {
        engine.state.players[index].library.contains(oid)
            && engine
                .state
                .objects
                .get(oid)
                .is_some_and(|object| object.zone == Zone::Library && object.owner == player)
            && engine
                .state
                .zone_change_generation
                .get(oid)
                .copied()
                .unwrap_or(0)
                == *generation
    })
}

pub(super) fn put_into_hand(
    engine: &mut GameEngine,
    player: PlayerId,
    cards: &[ObjectId],
    public: bool,
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<(), EngineError> {
    for oid in cards {
        let name = events::object_display_name(&engine.state, engine.registry, *oid);
        let owner = engine.state.objects[oid].owner;
        resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            *oid,
            Zone::Hand,
            None,
        )?;
        let message = format!("P{player} puts {name} into their hand.");
        events.push(if public {
            events::ev_log(message)
        } else {
            events::ev_log_private(message, player)
        });
        events.push(resolution::permanent_moved_event(
            &engine.state,
            *oid,
            owner,
            rv1::permanent_moved::Destination::Hand,
        ));
    }
    if !public {
        events.push(events::ev_log(format!(
            "P{player} puts {} cards into their hand.",
            cards.len()
        )));
    }
    Ok(())
}
