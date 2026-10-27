use super::*;

fn filter_checks_color(filter: &ZoneCardFilter) -> bool {
    filter.is_colorless.is_some()
        || filter
            .any_of
            .as_ref()
            .is_some_and(|branches| branches.iter().any(filter_checks_color))
}

/// Keep each printed non-color predicate intact while evaluating color in the current layer view.
/// OR branches must be evaluated independently so their other constraints remain associated.
pub(super) fn matches_filter_with_current_color(
    definition: &CardDefinition,
    filter: &ZoneCardFilter,
    colorless: Option<bool>,
    face_index: Option<usize>,
) -> bool {
    if let Some(branches) = &filter.any_of {
        return branches.iter().any(|branch| {
            matches_filter_with_current_color(definition, branch, colorless, face_index)
        });
    }
    if filter
        .is_colorless
        .is_some_and(|required| colorless != Some(required))
    {
        return false;
    }
    if filter.is_colorless.is_none() {
        return match face_index {
            Some(index) => definition.matches_zone_card_filter_for_face(index, filter),
            None => definition.matches_zone_card_filter(filter),
        };
    }
    let mut printed = filter.clone();
    printed.is_colorless = None;
    match face_index {
        Some(index) => definition.matches_zone_card_filter_for_face(index, &printed),
        None => definition.matches_zone_card_filter(&printed),
    }
}

/// The selected face of this exact proposed spell is checked for a library-top permission.
pub(super) fn proposed_face_matches_filter(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    controller: PlayerId,
    definition: &CardDefinition,
    face_index: usize,
    filter: &ZoneCardFilter,
) -> bool {
    let Some(face) = definition.face(face_index) else {
        return false;
    };
    let colorless = filter_checks_color(filter).then(|| {
        super::characteristics::spell_cast_characteristics(state, registry, oid, controller, face)
            .colors
            .is_empty()
    });
    matches_filter_with_current_color(definition, filter, colorless, Some(face_index))
}

/// Resolve the printed card definition for library searches, hand costs, and graveyard counts.
/// The caller supplies the zone cohort and handles visibility and generation checks.
pub(super) fn zone_card_matches_filter(
    state: &GameState,
    registry: &'static CardRegistry,
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
    registry.get(&object.card_id).is_some_and(|definition| {
        let colorless = filter_checks_color(filter)
            .then(|| {
                super::characteristics::characteristics_from(state, registry, oid)
                    .map(|values| values.colors.is_empty())
            })
            .flatten();
        matches_filter_with_current_color(definition, filter, colorless, None)
    })
}

/// A linked Hideaway permission may use the underlying identity only for a player who retained
/// CR 702.75a look permission for this exact source incarnation and exiled-card generation.
pub(super) fn linked_exile_card_matches_filter(
    state: &GameState,
    registry: &'static CardRegistry,
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
        registry.get(&object.card_id).is_some_and(|definition| {
            let colorless = filter_checks_color(filter)
                .then(|| {
                    super::characteristics::characteristics_from(state, registry, oid)
                        .map(|values| values.colors.is_empty())
                })
                .flatten();
            matches_filter_with_current_color(definition, filter, colorless, None)
        })
    })
}
