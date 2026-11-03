//! Zone movement preparation, commit, disclosure, and destruction helpers.

use super::*;

/// A face-up public zone or a visible battlefield/stack object makes the movement observable.
pub(in crate::engine) fn movement_identity_is_public(
    origin: Zone,
    origin_face_down: bool,
    destination: Zone,
    destination_face_down: bool,
) -> bool {
    let public_source = matches!(
        origin,
        Zone::Battlefield | Zone::Graveyard | Zone::Stack | Zone::Command
    ) || (origin == Zone::Exile && !origin_face_down);
    let public_destination = !destination_face_down
        && matches!(
            destination,
            Zone::Battlefield | Zone::Graveyard | Zone::Exile | Zone::Command
        );
    public_source || public_destination
}

/// Build a `PermanentMoved` event with the private physical identity needed by the relay.
pub(crate) fn permanent_moved_event(
    state: &GameState,
    oid: ObjectId,
    owner_player_id: PlayerId,
    mut destination: rv1::permanent_moved::Destination,
) -> rv1::RuledEvent {
    if destination == rv1::permanent_moved::Destination::Graveyard
        && state
            .objects
            .get(&oid)
            .is_some_and(|object| object.zone == Zone::Exile)
    {
        destination = rv1::permanent_moved::Destination::Exile;
    }
    let face_down_exile = state
        .objects
        .get(&oid)
        .is_some_and(|object| object.zone == Zone::Exile && object.face_down);
    let card_id = if face_down_exile {
        String::new()
    } else {
        state
            .objects
            .get(&oid)
            .map(|o| o.card_id.clone())
            .unwrap_or_default()
    };
    // Callers emit this *after* the move, so the object already carries its post-move controller:
    // the new controller for a battlefield entry, and the owner again everywhere else (CR 400.7).
    // Always populated — proto3 scalars have no presence and player id 0 is valid, so a defaulted
    // 0 would be indistinguishable from "player 0 controls it".
    let controller_player_id = state
        .objects
        .get(&oid)
        .map(|o| o.controller)
        .unwrap_or(owner_player_id);
    let reported_zone = match destination {
        rv1::permanent_moved::Destination::Hand => Some(Zone::Hand),
        rv1::permanent_moved::Destination::Library => Some(Zone::Library),
        rv1::permanent_moved::Destination::Battlefield => Some(Zone::Battlefield),
        rv1::permanent_moved::Destination::Graveyard => Some(Zone::Graveyard),
        rv1::permanent_moved::Destination::Exile => Some(Zone::Exile),
        rv1::permanent_moved::Destination::Command => Some(Zone::Command),
        rv1::permanent_moved::Destination::Unspecified => None,
    };
    let object_id_publicly_observable = state.objects.get(&oid).is_some_and(|object| {
        state
            .move_disclosure_receipts
            .get(&oid)
            .is_some_and(|receipt| {
                receipt.command_index == state.command_index
                    && receipt.destination == object.zone
                    && reported_zone == Some(receipt.destination)
                    && state.zone_change_generation.get(&oid).copied()
                        == Some(receipt.destination_generation)
                    && movement_identity_is_public(
                        receipt.origin,
                        receipt.origin_face_down,
                        receipt.destination,
                        object.face_down,
                    )
            })
    });
    rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PermanentMoved(rv1::PermanentMoved {
            object_id: oid,
            owner_player_id,
            destination: destination as i32,
            card_id,
            controller_player_id,
            face_down: state.objects.get(&oid).is_some_and(|object| {
                object.face_down && matches!(object.zone, Zone::Battlefield | Zone::Exile)
            }),
            source_library_position: None,
            object_id_publicly_observable,
        })),
    }
}

/// Move `oid` into zone `z`, maintaining every zone list and the CR 400.7 new-object resets.
///
/// `controller` names the player the permanent enters the battlefield **under** (CR 110.2);
/// `None` means "its owner controls it", which is what every non-control-changing caller passes.
/// It is ignored for non-battlefield zones — those belong to the owner (CR 400.3).
pub(crate) fn move_object_to_zone(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    z: Zone,
    controller: Option<PlayerId>,
) -> Result<(), EngineError> {
    if let Some(fact) = move_object_to_zone_with_entry_receipt(state, registry, oid, z, controller)?
    {
        state
            .turn_history
            .current
            .permanent_cards_entered_graveyard
            .push(fact);
    }
    Ok(())
}

/// Ordinary moves commit the returned history receipt immediately. The sole exception is
/// early custom-resolution bookkeeping, whose receipt belongs to the resolution completion.
/// Record source qualities without inventing a zone change (also used by CR 800.4a removal).
pub(in crate::engine) fn record_last_known_characteristics(
    state: &mut GameState,
    oid: ObjectId,
    generation: u64,
    characteristics: super::super::characteristics::Characteristics,
) {
    state
        .last_known_information
        .record_mana_value((oid, generation), characteristics.mana_value);
    state.last_known_information.record_power_toughness(
        (oid, generation),
        (
            characteristics.signed_power,
            characteristics.signed_toughness,
        ),
    );
    state
        .last_known_information
        .record_controller((oid, generation), characteristics.controller);
    state
        .last_known_information
        .record_keywords((oid, generation), characteristics.keywords);
    state
        .last_known_information
        .record_colors((oid, generation), characteristics.colors);
    state
        .last_known_information
        .record_types((oid, generation), characteristics.types);
    state
        .last_known_information
        .record_names((oid, generation), characteristics.names);
}

/// Prepare every departure in a simultaneous instruction against the same battlefield.
pub(in crate::engine) fn prepare_zone_move(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    mut z: Zone,
    controller: Option<PlayerId>,
) -> Result<Option<PreparedZoneMove>, EngineError> {
    let owner = state
        .objects
        .get(&oid)
        .map(|o| o.owner)
        .ok_or(EngineError::Illegal("no object"))?;
    let old_zone = state.objects.get(&oid).map(|o| o.zone);
    let leaving_battlefield = old_zone == Some(Zone::Battlefield) && z != Zone::Battlefield;
    let prior_generation = state.zone_change_generation.get(&oid).copied().unwrap_or(0);
    // CR 111.8: after leaving the battlefield a token cannot change zones again, even
    // while resolution defers SBAs. Proposed, not-yet-entered tokens have generation zero.
    if old_zone != Some(Zone::Battlefield)
        && prior_generation > 0
        && state
            .objects
            .get(&oid)
            .is_some_and(|object| object.token_origin.is_some())
    {
        return Ok(None);
    }
    let has_finality = state
        .objects
        .get(&oid)
        .is_some_and(|object| object.counter_count(CounterKind::Finality) > 0);
    if old_zone == Some(Zone::Battlefield)
        && z == Zone::Graveyard
        && (has_finality
            || state.death_replacement_effects.iter().any(|effect| {
                effect.object_id == oid && effect.zone_change_generation == prior_generation
            }))
    {
        z = Zone::Exile;
    }
    // An ability can be activated from hand or graveyard and its source can leave as a cost.
    // Capture the layered qualities at the actual departure, not from its newly changed zone.
    let nonbattlefield_ability_source = match old_zone {
        Some(Zone::Hand) => Some(AbilitySourceZone::Hand),
        Some(Zone::Graveyard) => Some(AbilitySourceZone::Graveyard),
        _ => None,
    }
    .is_some_and(|source_zone| {
        state
            .objects
            .get(&oid)
            .and_then(|object| registry.get(&object.card_id))
            .is_some_and(|definition| {
                definition.faces_iter().any(|face| {
                    face.activated_abilities
                        .iter()
                        .any(|ability| ability.source_zone == source_zone)
                })
            })
    });
    let was_phased_out = state
        .objects
        .get(&oid)
        .is_some_and(|object| object.phase_status.is_some());
    let last_known_characteristics = ((leaving_battlefield && !was_phased_out)
        || nonbattlefield_ability_source)
        .then(|| super::super::characteristics::characteristics_from(state, registry, oid))
        .flatten();
    let copy = leaving_battlefield
        .then(|| copying::token_copy_snapshot_from(state, registry, oid))
        .flatten();
    let last_known_attached_object = leaving_battlefield
        .then(|| {
            state
                .objects
                .get(&oid)
                .and_then(|object| object.attached_to)
        })
        .flatten()
        .and_then(|recipient| match recipient {
            AttachmentRecipient::Object(target_oid)
                if state
                    .objects
                    .get(&target_oid)
                    .is_some_and(|target| target.zone == Zone::Battlefield) =>
            {
                Some(crate::state::AttachedObjectLki {
                    object_id: target_oid,
                    zone_change_generation: state
                        .zone_change_generation
                        .get(&target_oid)
                        .copied()
                        .unwrap_or(0),
                    owner: state.objects.get(&target_oid)?.owner,
                })
            }
            AttachmentRecipient::Object(_) | AttachmentRecipient::Player(_) => None,
        });
    let front_face_values = leaving_battlefield
        .then(|| {
            state
                .objects
                .get(&oid)
                .and_then(|object| registry.get(&object.card_id))
                .map(|definition| {
                    let face = definition.primary_face();
                    (
                        face.power,
                        face.toughness,
                        face.must_attack_if_able,
                        face.must_block_if_able,
                    )
                })
        })
        .flatten();
    let object = &state.objects[&oid];
    let counters = object.counters.clone();
    let tapped = object.tapped;
    let old_controller = object.controller;
    let holder = if z == Zone::Battlefield {
        controller.unwrap_or(owner)
    } else {
        owner
    };
    let holder_index = state
        .player_idx(holder)
        .ok_or(EngineError::Illegal("no such player"))?;
    Ok(Some(PreparedZoneMove {
        oid,
        owner,
        old_zone,
        prior_generation,
        z,
        leaving_battlefield,
        last_known_characteristics,
        copy,
        last_known_attached_object,
        front_face_values,
        counters,
        tapped,
        old_controller,
        holder,
        holder_index,
    }))
}

fn move_object_to_zone_with_entry_receipt(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    z: Zone,
    controller: Option<PlayerId>,
) -> Result<Option<crate::state::PermanentHistoryFact>, EngineError> {
    let Some(prepared) = prepare_zone_move(state, registry, oid, z, controller)? else {
        return Ok(None);
    };
    commit_zone_move(state, registry, prepared)
}

pub(in crate::engine) fn commit_zone_move(
    state: &mut GameState,
    registry: &'static CardRegistry,
    prepared: PreparedZoneMove,
) -> Result<Option<crate::state::PermanentHistoryFact>, EngineError> {
    let PreparedZoneMove {
        oid,
        owner,
        old_zone,
        prior_generation,
        z,
        leaving_battlefield,
        last_known_characteristics,
        copy,
        last_known_attached_object,
        front_face_values,
        counters,
        tapped,
        old_controller,
        holder,
        holder_index,
    } = prepared;
    debug_assert_eq!(state.objects[&oid].owner, owner);
    debug_assert_eq!(state.objects[&oid].zone, old_zone.unwrap());
    debug_assert_eq!(
        state.zone_change_generation.get(&oid).copied().unwrap_or(0),
        prior_generation
    );
    if leaving_battlefield {
        state.active_temporary_copies.retain(|effect| {
            effect.object_id != oid || effect.zone_change_generation != prior_generation
        });
        // A resolving "for as long as you control this source" effect belongs to this exact
        // source incarnation. Drain it before the zone-change generation is incremented; the
        // ability's stack object is a different id and cannot serve as this binding.
        state.continuous_effects.retain(|effect| {
            !matches!(
                &effect.duration,
                EffectDuration::WhileSourceControlledBy {
                    source_object_id,
                    source_zone_change_generation,
                    ..
                } if *source_object_id == oid
                    && *source_zone_change_generation == prior_generation
            )
        });
        state.death_replacement_effects.retain(|effect| {
            effect.object_id != oid || effect.zone_change_generation != prior_generation
        });
        if let Some(snapshot) = copy {
            state
                .last_known_information
                .record_copy_snapshot((oid, prior_generation), snapshot);
        }
    }
    if leaving_battlefield {
        state.battlefield_entry_timestamps.remove(&oid);
        state.room_states.remove(&oid);
        state.class_levels.remove(&oid);
        state.case_solved_generations.remove(&oid);
        super::super::preparation::end_prepared_designation(state, oid, prior_generation);
        state.battle_protectors.remove(&oid);
        state
            .chosen_opponents
            .retain(|record| record.key.source_object_id != oid);
        state
            .chosen_creature_types
            .retain(|record| record.key.source_object_id != oid);
        {
            let object = TriggerObjectRef {
                object_id: oid,
                zone_change_generation: prior_generation,
                controller_at_event: old_controller,
            };
            let delayed = state.dispatch_event_observers(ObservedGameEvent::ControllerChanged {
                object,
                old_controller,
                new_controller: None,
            });
            state.stage_delayed_batch(delayed);
            let delayed =
                state.dispatch_event_observers(ObservedGameEvent::LeavesBattlefield(object));
            state.stage_delayed_batch(delayed);
        }
        // An attachment's recipient relation is tied to this battlefield object. Clear inbound
        // links only after the departure observers have captured their source snapshots, but
        // before this ObjectId advances to a new zone-change generation.
        for attachment in state.objects.values_mut() {
            if attachment.attached_to == Some(AttachmentRecipient::Object(oid)) {
                attachment.attached_to = None;
            }
        }
    }
    // A move to the same named zone is still a zone change and creates a new object (CR 400.7).
    // This matters for exile permissions: exiling an already-exiled card cannot preserve an old
    // Adventure or "play it" permission merely because the destination enum is unchanged.
    if let Some(origin) = old_zone {
        state.activated_ability_slots.remove(&oid);
        state.spell_effects_carry_to_permanent.remove(&oid);
        state.spell_entry_facts.remove(&oid);
        state
            .discard_reference_successors
            .retain(|(object_id, _), _| *object_id != oid);
        state
            .warped_permanent_incarnations
            .retain(|&(object_id, generation)| object_id != oid || generation != prior_generation);
        *state.zone_change_generation.entry(oid).or_insert(0) += 1;
        state.move_disclosure_receipts.insert(
            oid,
            crate::state::MoveDisclosureReceipt {
                origin,
                destination: z,
                origin_face_down: state.objects[&oid].face_down,
                destination_generation: prior_generation + 1,
                command_index: state.command_index,
            },
        );
        state
            .active_exile_play_permissions
            .retain(|permission| permission.object_id != oid);
    }

    // CR 400.7: a zone change creates a new game object. Remove any Single-target continuous
    // effects on this object so they don't apply if the same ObjectId is reused later.
    // CR 604.3 / 611.3: also drain any `WhileSourceOnBattlefield` effects this permanent was the
    // source of (anthems) — a static ability stops applying the moment its source leaves (LTB).
    // One-shot `UntilEndOfTurn` effects (Giant Growth, firebreathing) are deliberately NOT drained
    // here: once created they are independent of their source (CR 611.2g) and only end at cleanup.
    state.continuous_effects.retain(|e| {
        let single_on_this =
            old_zone != Some(z) && matches!(&e.affected, AffectedScope::Single(id) if *id == oid);
        let static_from_this = leaving_battlefield
            && e.source_id == Some(oid)
            && e.duration == EffectDuration::WhileSourceOnBattlefield;
        let graveyard_static_from_this = old_zone == Some(Zone::Graveyard)
            && e.source_id == Some(oid)
            && e.duration == EffectDuration::WhileSourceInGraveyard;
        !single_on_this && !static_from_this && !graveyard_static_from_this
    });
    if leaving_battlefield {
        if let Some(attached_object) = last_known_attached_object {
            state
                .last_known_information
                .record_attached_object((oid, prior_generation), attached_object);
        }
        state
            .skip_next_untap
            .retain(|&(object_id, _)| object_id != oid);
        state.damage_prevention_effects.retain(|effect| {
            let recipient_is_this_object = match effect.scope {
                DamagePreventionScope::Recipient(recipient) => recipient == oid,
                DamagePreventionScope::CombatRecipient { object_id, .. }
                | DamagePreventionScope::CombatSource { object_id, .. } => object_id == oid,
                DamagePreventionScope::Combat
                | DamagePreventionScope::OtherCreaturesYouControl { .. }
                | DamagePreventionScope::ControllerAndPlaneswalkers { .. } => false,
            };
            let static_from_this = effect.source_id == Some(oid)
                && effect.duration == EffectDuration::WhileSourceOnBattlefield;
            !recipient_is_this_object && !static_from_this
        });
        state.damage_doubling_effects.retain(|effect| {
            !(effect.source_id == Some(oid)
                && effect.duration == EffectDuration::WhileSourceOnBattlefield)
        });
        // CR 400.7 / 121.2: a zone change makes this a new game object — transient
        // battlefield-only state (marked damage, deathtouch marking, tap status, regeneration
        // shields) and all counters do not carry over. Centralized here so every leave path
        // (SBA destroy, sacrifice, bounce, discard, mill, exile) is correct by construction.
        if let Some(o) = state.objects.get_mut(&oid) {
            o.damage = 0;
            o.deathtouch_damage = false;
            // CR 608.2h: snapshot before clearing — an ability still on the stack that asks about
            // this permanent's tap status gets its last known information, not the reset value.
            let was_tapped = tapped;
            o.tapped = false;
            state
                .last_known_information
                .record_counters((oid, prior_generation), counters);
            o.counters.clear();
            o.counter_timestamps.clear();
            o.attached_to = None;
            o.regeneration_shields = 0;
            o.face_up_index = 0;
            o.face_down = false;
            o.copiable_values = None;
            o.copy_revision = 0;
            o.active_copy_occurrence = None;
            if let Some((power, toughness, must_attack, must_block)) = front_face_values {
                o.power = power;
                o.toughness = toughness;
                o.must_attack_if_able = must_attack;
                o.must_block_if_able = must_block;
            }
            let generation = state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0)
                .saturating_sub(1);
            state
                .last_known_information
                .record_tapped((oid, generation), was_tapped);
            state
                .last_known_information
                .record_latest_tapped(oid, was_tapped);
        }
    }
    if let Some(characteristics) = last_known_characteristics {
        record_last_known_characteristics(state, oid, prior_generation, characteristics);
    }

    // Remove from *every* player's lists, not just the owner's: `battlefield` is keyed by
    // controller, so a permanent under someone else's control lives in their vec. Scoping this to
    // the owner would strand a ghost oid that still blocks, still gets SBA-checked, and desyncs
    // the zone-view size check in the relay's `applyRuledEngineZoneView`.
    for p in &mut state.players {
        p.library.retain(|&x| x != oid);
        p.hand.retain(|&x| x != oid);
        p.battlefield.retain(|&x| x != oid);
        p.graveyard.retain(|&x| x != oid);
        p.exile.retain(|&x| x != oid);
        p.command_zone.retain(|&x| x != oid);
    }
    // CR 400.3: the battlefield is entered under a *controller*; every other zone belongs to the
    // card's owner, so that is where a permanent goes when it leaves.
    let p = &mut state.players[holder_index];
    match z {
        Zone::Graveyard => p.graveyard.push(oid),
        Zone::Hand => p.hand.push(oid),
        Zone::Battlefield => p.battlefield.push(oid),
        Zone::Library => p.library.push_back(oid),
        Zone::Exile => p.exile.push(oid),
        Zone::Stack => {}
        Zone::Command => p.command_zone.push(oid),
    }
    if z == Zone::Battlefield {
        state.next_game_rule_timestamp = state.next_game_rule_timestamp.saturating_add(1);
        state
            .battlefield_entry_timestamps
            .insert(oid, state.next_game_rule_timestamp);
    }
    if let Some(o) = state.objects.get_mut(&oid) {
        o.zone = z;
        // A zone move creates a new object incarnation. Phasing is tied to the old
        // battlefield incarnation and cannot carry into the destination or a later return.
        o.phase_status = None;
        if old_zone == Some(Zone::Exile) && matches!(z, Zone::Hand | Zone::Command) {
            // The receipt already captured the old face-down incarnation above.
            o.face_down = false;
        }
        if z != Zone::Battlefield {
            o.prototyped = false;
        }
        // CR 110.2 / 400.7: control is a battlefield-only property, and a zone change makes this a
        // new object — so entering sets the new controller and leaving resets it to the owner.
        let controller = if z == Zone::Battlefield {
            holder
        } else {
            o.owner
        };
        o.base_controller = controller;
        o.controller = controller;
        // CR 302.6: a permanent entering the battlefield has not been controlled continuously
        // since its controller's most recent turn began, so it is summoning sick. Assert this on
        // entry rather than trusting a persisted flag — a prior bounce/leave clears transient
        // state, so a creature returned to hand and recast (or reanimated/flickered) the same turn
        // must still be sick. Haste exempts the *use* of this (checked at attack/tap time).
        if z == Zone::Battlefield {
            o.summoning_sick = true;
        }
    }
    if z == Zone::Graveyard {
        super::super::continuous::emit_graveyard_static_abilities(state, registry, oid);
    }
    Ok((old_zone != Some(Zone::Graveyard))
        .then(|| super::super::history::graveyard_entry_fact(state, registry, oid))
        .flatten())
}

pub(in crate::engine) fn destroy_permanent(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Result<bool, EngineError> {
    move_object_to_zone(state, registry, oid, Zone::Graveyard, None)?;
    Ok(state
        .objects
        .get(&oid)
        .is_some_and(|object| object.zone == Zone::Graveyard))
}

/// Move a permanent to its owner's graveyard without assigning a semantic cause. The legend
/// rule and other direct rule actions use this seam so they produce leave/death events but never
/// masquerade as sacrifices.
pub(in crate::engine) fn put_permanent_in_graveyard(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Result<bool, EngineError> {
    move_object_to_zone(state, registry, oid, Zone::Graveyard, None)?;
    Ok(state
        .objects
        .get(&oid)
        .is_some_and(|object| object.zone == Zone::Graveyard))
}

/// Sacrifice a permanent (CR 701.21). Both costs and effects use this seam. Sacrifice bypasses
/// indestructible and regeneration, but a zone-change replacement can change its destination.
pub(in crate::engine) fn sacrifice_permanent(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Result<bool, EngineError> {
    if !state
        .objects
        .get(&oid)
        .is_some_and(|object| object.zone == Zone::Battlefield)
    {
        return Err(EngineError::Illegal(
            "only a battlefield permanent can be sacrificed",
        ));
    }
    put_permanent_in_graveyard(state, registry, oid)
}

/// One sacrifice instruction freezes all replacement destinations and departure LKI before
/// the first move. The caller owns its single zone-event boundary and per-object receipts.
pub(in crate::engine) fn sacrifice_permanents(
    state: &mut GameState,
    registry: &'static CardRegistry,
    objects: &[ObjectId],
) -> Result<Vec<bool>, EngineError> {
    let mut distinct = HashSet::new();
    let mut prepared = Vec::with_capacity(objects.len());
    for &oid in objects {
        if !distinct.insert(oid)
            || !state
                .objects
                .get(&oid)
                .is_some_and(|object| object.zone == Zone::Battlefield)
        {
            return Err(EngineError::Illegal(
                "sacrifice group requires distinct battlefield permanents",
            ));
        }
        prepared.push(
            prepare_zone_move(state, registry, oid, Zone::Graveyard, None)?
                .ok_or(EngineError::Illegal("sacrifice permanent missing"))?,
        );
    }
    let mut deaths = Vec::with_capacity(prepared.len());
    for movement in prepared {
        let died = movement.z == Zone::Graveyard;
        if let Some(fact) = commit_zone_move(state, registry, movement)? {
            state
                .turn_history
                .current
                .permanent_cards_entered_graveyard
                .push(fact);
        }
        deaths.push(died);
    }
    Ok(deaths)
}

/// CR 608.2n: the spell enters its owner's graveyard after its effects have been applied. A
/// self-targeted Tome Scour must therefore end up beneath the five cards it milled, not on top
/// of them.
///
/// A no-op unless `oid` is currently in a graveyard and not already last, so it is safe to call
/// on any resolution path, including ones that end with the spell on the battlefield.
pub(in crate::engine) fn seat_resolved_spell_last_in_graveyard(
    state: &mut GameState,
    oid: ObjectId,
) {
    let Some(owner) = state.objects.get(&oid).map(|o| o.owner) else {
        return;
    };
    let Some(idx) = state.player_idx(owner) else {
        return;
    };
    let graveyard = &mut state.players[idx].graveyard;
    if graveyard.last() == Some(&oid) || !graveyard.contains(&oid) {
        return;
    }
    graveyard.retain(|&x| x != oid);
    graveyard.push(oid);
}

pub(crate) fn permanent_moved_event_with_library_position(
    state: &GameState,
    oid: ObjectId,
    owner_player_id: PlayerId,
    destination: rv1::permanent_moved::Destination,
    source_library_position: u32,
) -> rv1::RuledEvent {
    let mut event = permanent_moved_event(state, oid, owner_player_id, destination);
    if let Some(rv1::ruled_event::Ev::PermanentMoved(moved)) = event.ev.as_mut() {
        moved.source_library_position = Some(source_library_position);
    }
    event
}

/// CR 614.8 / 701.19: attempt to consume one regeneration shield from `oid`. If a shield is present,
/// taps the creature, removes it from combat, clears all marked damage, and returns `true` plus
/// the tap edge, if any. The caller is responsible for not destroying the creature. Returns
/// `false` if no shield exists.
/// Does NOT emit a zone-change event (the creature stays on the battlefield).
pub(in crate::engine) fn consume_regen_shield(
    engine: &mut GameEngine,
    oid: ObjectId,
    events: &mut Vec<rv1::RuledEvent>,
) -> (bool, Option<GameEvent>) {
    let shields = engine
        .state
        .objects
        .get(&oid)
        .map(|o| o.regeneration_shields)
        .unwrap_or(0);
    if shields == 0 {
        return (false, None);
    }
    // CR 701.19a: regenerating taps the permanent — a real "becomes tapped" edge, so it goes
    // through the shared funnel rather than writing the flag inline.
    // CR 701.19: the permanent's controller taps it, not the shield's creator or destroyer.
    let actor = engine
        .characteristics(oid)
        .map(|c| c.controller)
        .unwrap_or(engine.state.objects[&oid].controller);
    let tap_event = engine.tap_permanents(actor, &[oid]).pop();
    let state = &mut engine.state;
    if let Some(o) = state.objects.get_mut(&oid) {
        o.regeneration_shields -= 1;
        o.damage = 0;
        o.deathtouch_damage = false;
    }
    engine.remove_combat_participants(&[oid], events);
    (true, tap_event)
}
