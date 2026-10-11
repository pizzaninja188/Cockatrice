use super::*;

/// Resolve the printed card definition for library searches, hand costs, and graveyard counts.
/// The caller supplies the zone cohort and handles visibility and generation checks.
pub(super) fn zone_card_matches_filter(
    state: &GameState,
    registry: &CardRegistry,
    oid: ObjectId,
    filter: Option<&ZoneCardFilter>,
) -> bool {
    if !state.is_card_object(oid) {
        return false;
    }
    let Some(filter) = filter else {
        return true;
    };
    let Some(object) = state.objects.get(&oid) else {
        return false;
    };
    // CR 406.3a: a face-down exiled card has no characteristics for public card filters.
    // Unfiltered effects may still act on a card without reading those characteristics.
    if object.zone == Zone::Exile && object.face_down {
        return false;
    }
    registry
        .get(&object.card_id)
        .is_some_and(|definition| definition.matches_zone_card_filter(filter))
}

/// A linked Hideaway permission may use the underlying identity only for a player who retained
/// CR 702.75a look permission for this exact source incarnation and exiled-card generation.
pub(super) fn linked_exile_card_matches_filter(
    state: &GameState,
    registry: &CardRegistry,
    linked_key: &LinkedExileKey,
    viewer: PlayerId,
    oid: ObjectId,
    generation: u64,
    filter: Option<&ZoneCardFilter>,
) -> bool {
    if !state.is_card_object(oid) {
        return false;
    }
    let Some(object) = state.objects.get(&oid) else {
        return false;
    };
    if object.zone == Zone::Exile && object.face_down {
        let may_look = state
            .hideaway_look_permissions
            .get(&(linked_key.clone(), oid, generation))
            .is_some_and(|viewers| viewers.contains(&viewer));
        if !may_look {
            return false;
        }
    }
    filter.is_none_or(|filter| {
        registry
            .get(&object.card_id)
            .is_some_and(|definition| definition.matches_zone_card_filter(filter))
    })
}
