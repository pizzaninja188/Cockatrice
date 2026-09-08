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
    state
        .objects
        .get(&oid)
        .and_then(|object| registry.get(&object.card_id))
        .is_some_and(|definition| definition.matches_zone_card_filter(filter))
}
