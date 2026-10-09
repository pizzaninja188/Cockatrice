//! Public reveal snapshots. Capture these before moving cards; presentation must never need
//! to recover a revealed identity from a later hidden-zone position.

use super::*;

pub(super) fn activation_reveal_id(
    source_id: ObjectId,
    source_zone_change: u64,
    ability_index: usize,
    transaction_id: u64,
) -> String {
    format!("activation:{source_id}:{source_zone_change}:{ability_index}:{transaction_id}")
}

pub(crate) fn reveal_cards(
    state: &GameState,
    registry: &CardRegistry,
    object_ids: &[ObjectId],
    source_object_id: ObjectId,
    source_description: &str,
) -> Vec<rv1::RuledEvent> {
    let mut groups: Vec<rv1::CardsRevealed> = Vec::new();
    for &object_id in object_ids {
        let Some(object) = state.objects.get(&object_id) else {
            continue;
        };
        let source_zone = match object.zone {
            Zone::Hand => rv1::ChoiceCandidateSourceZone::Hand,
            Zone::Library => rv1::ChoiceCandidateSourceZone::Library,
            Zone::Graveyard => rv1::ChoiceCandidateSourceZone::Graveyard,
            Zone::Battlefield => rv1::ChoiceCandidateSourceZone::Battlefield,
            Zone::Exile => rv1::ChoiceCandidateSourceZone::Exile,
            Zone::Stack => rv1::ChoiceCandidateSourceZone::Stack,
            Zone::Command => rv1::ChoiceCandidateSourceZone::Command,
        } as i32;
        let group_index = groups
            .iter()
            .position(|group| {
                group.zone_owner_player_id == object.owner && group.source_zone == source_zone
            })
            .unwrap_or_else(|| {
                groups.push(rv1::CardsRevealed {
                    zone_owner_player_id: object.owner,
                    source_zone,
                    cards: Vec::new(),
                    reveal_id: String::new(),
                    source_object_id,
                    source_description: source_description.into(),
                });
                groups.len() - 1
            });
        groups[group_index].cards.push(rv1::RevealedCard {
            object_id,
            zone_change_generation: state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
            card_id: object.card_id.clone(),
            card_name: registry
                .get(&object.card_id)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| object.card_id.clone()),
        });
    }
    groups
        .into_iter()
        .map(|group| rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::CardsRevealed(group)),
        })
        .collect()
}

/// A public choice over one zone uses exactly the same snapshot as an instant reveal.
/// Its candidates are already validated by the effect; an empty cohort produces no window.
pub(crate) fn reveal_choice(
    state: &GameState,
    registry: &CardRegistry,
    object_ids: &[ObjectId],
    source_object_id: ObjectId,
    source_description: &str,
) -> Option<rv1::CardsRevealed> {
    let mut events = reveal_cards(
        state,
        registry,
        object_ids,
        source_object_id,
        source_description,
    );
    assert!(
        events.len() <= 1,
        "public choice must describe one revealed zone cohort"
    );
    events.pop().and_then(|event| match event.ev {
        Some(rv1::ruled_event::Ev::CardsRevealed(reveal)) => Some(reveal),
        _ => None,
    })
}

/// Event position distinguishes repeated reveals during one command. Existing identities are
/// preserved when finish helpers enrich a batch again and when the relay replays a pending choice.
pub(super) fn identify_reveals(batch: &mut RuledEventBatch, command_index: u64) {
    for (index, event) in batch.events.iter_mut().enumerate() {
        let reveal = match &mut event.ev {
            Some(rv1::ruled_event::Ev::CardsRevealed(reveal)) => Some(reveal),
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                choice.public_reveal.as_mut()
            }
            _ => None,
        };
        if let Some(reveal) = reveal {
            if reveal.reveal_id.is_empty() {
                reveal.reveal_id = format!("event:{command_index}:{index}");
            }
        }
    }
}

/// Reveals paid as costs and activations from hand remain public while their stack item exists.
/// Receipts carry the original identity, even if a responding spell moves that physical card.
pub(super) fn active_reveals(eng: &GameEngine) -> Vec<rv1::CardsRevealed> {
    if eng.state.is_terminal() {
        return Vec::new();
    }
    let mut reveals = Vec::new();
    reveals.extend(eng.draw_action_reveal());
    if let Some(super::replacement::PendingReplacementEvent::BattlefieldEntry(entry)) =
        &eng.state.pending_replacement_event
    {
        reveals.extend(entry.event.entry_reveal_receipts.iter().cloned());
        match &entry.completion {
            BattlefieldEntryCompletion::ZoneEntryBatch(batch) => {
                for event in &batch.ready {
                    reveals.extend(event.entry_reveal_receipts.iter().cloned());
                }
            }
            BattlefieldEntryCompletion::TokenBatch(batch) => {
                for entry in batch.ready.iter().chain(&batch.remaining) {
                    reveals.extend(entry.event.entry_reveal_receipts.iter().cloned());
                }
            }
            _ => {}
        }
    }
    if let Some(batch) = &eng.state.pending_observer_return_batch {
        for entry in &batch.ready {
            reveals.extend(entry.event.entry_reveal_receipts.iter().cloned());
        }
    }
    if let Some(PendingResolution {
        continuation: ResolutionContinuation::SimultaneousEntryOrder { order, .. },
        ..
    }) = &eng.state.pending_resolution
    {
        match &order.batch {
            SimultaneousEntryBatch::Zone(batch) => {
                for event in &batch.ready {
                    reveals.extend(event.entry_reveal_receipts.iter().cloned());
                }
            }
            SimultaneousEntryBatch::Token(batch) => {
                for entry in &batch.ready {
                    reveals.extend(entry.event.entry_reveal_receipts.iter().cloned());
                }
            }
            SimultaneousEntryBatch::Observer(batch) => {
                for entry in &batch.ready {
                    reveals.extend(entry.event.entry_reveal_receipts.iter().cloned());
                }
            }
        }
    }
    if let Some(pending) = eng
        .state
        .pending_ability_activation
        .as_ref()
        .filter(|pending| pending.source_zone() == rv1::AbilitySourceZone::Hand)
    {
        let ability = eng
            .pending_ability_activation_internal
            .as_ref()
            .map(|internal| &internal.effective.definition);
        if ability.is_some_and(|ability| ability.costs.contains(&AbilityCost::ReturnTappedCreature))
        {
            let source = eng.state.objects.get(&pending.source_object_id);
            if let Some(source) = source.filter(|source| {
                source.zone == Zone::Hand
                    && source.owner == pending.actor_player_id
                    && eng.payment_object_ref(source.id).zone_change_generation
                        == pending.source_zone_change_generation
            }) {
                let card_name = eng
                    .registry
                    .get(&source.card_id)
                    .map(|definition| definition.name.clone())
                    .unwrap_or_else(|| source.card_id.clone());
                reveals.push(rv1::CardsRevealed {
                    reveal_id: activation_reveal_id(
                        pending.source_object_id,
                        pending.source_zone_change_generation,
                        pending.ability_index as usize,
                        pending.transaction_id,
                    ),
                    source_object_id: pending.source_object_id,
                    source_description: pending.source_description.clone(),
                    zone_owner_player_id: source.owner,
                    source_zone: rv1::ChoiceCandidateSourceZone::Hand as i32,
                    cards: vec![rv1::RevealedCard {
                        object_id: source.id,
                        zone_change_generation: pending.source_zone_change_generation,
                        card_id: source.card_id.clone(),
                        card_name,
                    }],
                });
            }
        }
    }
    for item in eng.state.stack.iter().filter(|item| !item.is_copy) {
        let source_description =
            super::events::object_display_name(&eng.state, eng.registry, item.id);
        for receipt in &item.cast_cost_receipts {
            let cards: Vec<_> = receipt
                .objects
                .iter()
                .filter_map(|object| {
                    if let CastCostObjectReceipt::RevealedHand {
                        object_id,
                        zone_change_generation,
                        card_id,
                        card_name,
                    } = object
                    {
                        Some(rv1::RevealedCard {
                            object_id: *object_id,
                            zone_change_generation: *zone_change_generation,
                            card_id: card_id.clone(),
                            card_name: card_name.clone(),
                        })
                    } else {
                        None
                    }
                })
                .collect();
            if !cards.is_empty() {
                reveals.push(rv1::CardsRevealed {
                    reveal_id: format!("stack:{}:cost:{}", item.id, receipt.group_index),
                    source_object_id: item.id,
                    source_description: source_description.clone(),
                    zone_owner_player_id: item.controller,
                    source_zone: rv1::ChoiceCandidateSourceZone::Hand as i32,
                    cards,
                });
            }
        }
        if item.activated_ability.as_ref().is_some_and(|ability| {
            ability
                .costs
                .contains(&AbilityCost::ReturnUnblockedAttacker)
                || ability.costs.contains(&AbilityCost::ReturnTappedCreature)
        }) {
            let source_id = item.source_permanent_id.unwrap_or(0);
            let has_tapped_creature_return = item
                .activated_ability
                .as_ref()
                .is_some_and(|ability| ability.costs.contains(&AbilityCost::ReturnTappedCreature));
            let card_name = eng
                .registry
                .get(&item.card_id)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| item.card_id.clone());
            reveals.push(rv1::CardsRevealed {
                reveal_id: if has_tapped_creature_return {
                    eng.state
                        .stack_presentations
                        .get(&item.id)
                        .and_then(|presentation| presentation.activation_reveal_id.clone())
                        .unwrap_or_else(|| {
                            format!(
                                "activation:{source_id}:{}:{}",
                                item.source_zone_change,
                                item.ability_index.unwrap_or(0)
                            )
                        })
                } else {
                    format!("stack:{}:activation", item.id)
                },
                source_object_id: item.id,
                source_description: card_name.clone(),
                zone_owner_player_id: item.source_owner.unwrap_or(item.controller),
                source_zone: rv1::ChoiceCandidateSourceZone::Hand as i32,
                cards: vec![rv1::RevealedCard {
                    object_id: source_id,
                    zone_change_generation: item.source_zone_change,
                    card_id: item.card_id.clone(),
                    card_name,
                }],
            });
        }
    }
    let mut seen = HashSet::new();
    reveals.retain(|reveal| seen.insert(reveal.reveal_id.clone()));
    reveals
}

/// A later legal-actions snapshot replaces the active set, but must not erase occurrences
/// earlier in the same command (including automatic priority passes).
pub(super) fn preserve_active_occurrences(
    batch: &mut RuledEventBatch,
    current: &[rv1::CardsRevealed],
) {
    let still_active: HashSet<_> = current
        .iter()
        .map(|reveal| reveal.reveal_id.as_str())
        .collect();
    let mut seen: HashSet<String> = batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::CardsRevealed(reveal)) if !reveal.reveal_id.is_empty() => {
                Some(reveal.reveal_id.clone())
            }
            _ => None,
        })
        .collect();
    batch.events = std::mem::take(&mut batch.events)
        .into_iter()
        .flat_map(|event| {
            if let Some(rv1::ruled_event::Ev::ActivePublicRevealSnapshot(snapshot)) = &event.ev {
                snapshot
                    .reveals
                    .iter()
                    .filter(|reveal| {
                        !still_active.contains(reveal.reveal_id.as_str())
                            && seen.insert(reveal.reveal_id.clone())
                    })
                    .map(|reveal| rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::CardsRevealed(reveal.clone())),
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![event]
            }
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_active_reveal_survives_batch_settlement() {
        let engine = GameEngine::new(
            tricerules_cards::registry::global(),
            710,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let reveal = rv1::CardsRevealed {
            reveal_id: "stack:42:cost:0".into(),
            cards: vec![rv1::RevealedCard {
                card_id: "forest".into(),
                card_name: "Forest".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut batch = RuledEventBatch::default();
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ActivePublicRevealSnapshot(
                rv1::ActivePublicRevealSnapshot {
                    reveals: vec![reveal.clone()],
                },
            )),
        });
        super::super::legal_actions::fill_legal(&mut batch, &engine);
        assert!(
            batch.events.iter().any(|event| matches!(&event.ev,
                Some(rv1::ruled_event::Ev::CardsRevealed(snapshot)) if snapshot == &reveal
            )),
            "settlement must retain a reveal whose source already left the stack"
        );
    }
}
