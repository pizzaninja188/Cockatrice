use super::casting::castable_at_instant_speed;
use super::combat::priority_locked_for_combat_declaration;
use super::events::object_display_name;
use super::priority::{instant_timing_step_allowed, sorcery_speed_available};
use super::targeting::{
    compute_ability_targets, compute_ability_targets_with_context, compute_spell_targets,
    target_schema, TargetSourceIdentity,
};
use super::*;

pub(super) fn fill_legal(batch: &mut RuledEventBatch, eng: &GameEngine) {
    batch.events.retain(|event| {
        !matches!(
            event.ev,
            Some(rv1::ruled_event::Ev::ActivePublicRevealSnapshot(_))
        )
    });
    let reveals = if eng.state.winner.is_some() {
        vec![]
    } else {
        eng.state
            .stack
            .iter()
            .filter(|item| !item.is_copy)
            .flat_map(|item| {
                item.cast_cost_receipts.iter().filter_map(|receipt| {
                    let CastCostObjectReceipt::RevealedHand {
                        card_id, card_name, ..
                    } = receipt.object.as_ref()?
                    else {
                        return None;
                    };
                    Some(rv1::ActivePublicReveal {
                        source_stack_object_id: item.id,
                        group_index: receipt.group_index,
                        revealing_player_id: item.controller,
                        source_description: object_display_name(&eng.state, eng.registry, item.id),
                        card_id: card_id.clone(),
                        card_name: card_name.clone(),
                    })
                })
            })
            .collect()
    };
    batch.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ActivePublicRevealSnapshot(
            rv1::ActivePublicRevealSnapshot { reveals },
        )),
    });
    if eng.state.winner.is_some() {
        batch.legal_by_player.clear();
        return;
    }
    for p in &eng.state.players {
        let labels = legal_labels(eng, p.id);
        let hand_actions = legal_hand_actions(eng, p.id);
        let zone_cast_actions = legal_zone_cast_actions(eng, p.id);
        let zone_land_actions = legal_zone_land_actions(eng, p.id);
        let exile_play_permission_groups = exile_play_permission_groups(eng, p.id);
        let permanent_actions = legal_permanent_actions(eng, p.id);
        let zone_ability_actions = legal_zone_ability_actions(eng, p.id);
        let mut valid_targets_by_hand_slot = BTreeMap::new();
        let mut valid_targets_by_zone_object = BTreeMap::new();
        let mut valid_targets_by_ability = BTreeMap::new();
        let mut cost_choices_by_ability = BTreeMap::new();
        let mut mana_payment_by_ability = BTreeMap::new();

        if let Some(idx) = eng.state.player_idx(p.id) {
            for (slot, &oid) in eng.state.players[idx].hand.iter().enumerate() {
                let Some(obj) = eng.state.objects.get(&oid) else {
                    continue;
                };
                let Some(def) = eng.registry.get(&obj.card_id) else {
                    continue;
                };
                for (face_index, face) in def.faces_iter().enumerate() {
                    if !def.face_available_from_hand(face_index)
                        || face.is_land
                        || !target_schema(&face.spell_effect, face.targeting.as_ref()).has_targets()
                    {
                        continue;
                    }
                    let t = compute_spell_targets(
                        eng,
                        p.id,
                        TargetSourceIdentity::spell_face(eng, oid, face_index),
                        &face.spell_effect,
                        face.targeting.as_ref(),
                    );
                    let key = (slot as u32) << 8 | face_index as u32;
                    valid_targets_by_hand_slot.insert(key, t);
                }
            }
            for &poid in &eng.state.players[idx].battlefield {
                if !eng.state.objects.contains_key(&poid) {
                    continue;
                }
                for (ai, ability, _) in eng.effective_activated_abilities(poid) {
                    let key = (poid as u64) << 32 | ai as u64;
                    mana_payment_by_ability.insert(
                        key,
                        rv1::ManaPaymentEligibility {
                            eligible_restricted_mana_group_ids: eng
                                .eligible_restricted_mana_for_ability(idx, poid),
                        },
                    );
                    cost_choices_by_ability.insert(
                        key,
                        legal_ability_cost_choices(eng, p.id, poid, ai, &ability),
                    );
                    if target_schema(&ability.effect, ability.targeting.as_ref()).has_targets() {
                        let targets = compute_ability_targets(
                            eng,
                            p.id,
                            TargetSourceIdentity::current(eng, poid),
                            &ability.effect,
                            ability.targeting.as_ref(),
                        );
                        valid_targets_by_ability.insert(key, targets);
                    }
                }
            }
            for action in &zone_ability_actions {
                let source_zone = match action.source_zone() {
                    rv1::AbilitySourceZone::Hand => AbilitySourceZone::Hand,
                    rv1::AbilitySourceZone::Graveyard => AbilitySourceZone::Graveyard,
                    rv1::AbilitySourceZone::Battlefield => continue,
                };
                let Some((_, ability, _)) = eng
                    .authored_zone_activated_abilities(action.object_id, source_zone)
                    .into_iter()
                    .find(|(index, _, _)| *index == action.ability_index as usize)
                else {
                    continue;
                };
                let key = (u64::from(action.object_id) << 32) | u64::from(action.ability_index);
                mana_payment_by_ability.insert(
                    key,
                    rv1::ManaPaymentEligibility {
                        eligible_restricted_mana_group_ids: eng
                            .eligible_restricted_mana_for_ability(idx, action.object_id),
                    },
                );
                cost_choices_by_ability.insert(
                    key,
                    legal_ability_cost_choices(
                        eng,
                        p.id,
                        action.object_id,
                        action.ability_index as usize,
                        &ability,
                    ),
                );
                if target_schema(&ability.effect, ability.targeting.as_ref()).has_targets() {
                    valid_targets_by_ability.insert(
                        key,
                        compute_ability_targets(
                            eng,
                            p.id,
                            TargetSourceIdentity::captured(
                                action.object_id,
                                action.zone_change_generation,
                            ),
                            &ability.effect,
                            ability.targeting.as_ref(),
                        ),
                    );
                }
            }
        }
        for action in &zone_cast_actions {
            if !action.needs_target {
                continue;
            }
            let Some(face) = eng
                .state
                .objects
                .get(&action.object_id)
                .and_then(|object| eng.registry.get(&object.card_id))
                .and_then(|definition| definition.face(action.face_index as usize))
            else {
                continue;
            };
            valid_targets_by_zone_object.insert(
                (u64::from(action.object_id) << 8) | u64::from(action.face_index),
                compute_spell_targets(
                    eng,
                    p.id,
                    TargetSourceIdentity::spell_face(
                        eng,
                        action.object_id,
                        action.face_index as usize,
                    ),
                    &face.spell_effect,
                    face.targeting.as_ref(),
                ),
            );
        }

        // CR 603.3d: a triggered ability chooses its targets as it is put on the stack. While one
        // is parked waiting on its controller, publish its legal targets under the same
        // (source_oid << 32 | ability_index) key an activated ability would use, so the client can
        // highlight them and open the zone they live in (e.g. Gravedigger's graveyard). Only the
        // controller gets them — nobody else may answer the trigger.
        //
        // A same-index activated ability on the same permanent would be overwritten here, which is
        // the right precedence: priority is blocked while a trigger is pending (see priority.rs),
        // so the parked trigger is the only thing its controller can be choosing targets for.
        if let Some(pt) = eng.state.pending_triggers.front() {
            if pt.controller == p.id {
                let targets = compute_ability_targets_with_context(
                    eng,
                    p.id,
                    TargetSourceIdentity::captured(pt.source_permanent_id, pt.source_zone_change),
                    &pt.ability.effect,
                    pt.ability.targeting.as_ref(),
                    pt.trigger_context,
                );
                let key = (pt.source_permanent_id as u64) << 32 | pt.ability_index as u64;
                valid_targets_by_ability.insert(key, targets);
            }
        }

        let payment_undo_start = eng
            .state
            .pending_resolution
            .as_ref()
            .and_then(|pending| {
                (pending.deciding_player == p.id)
                    .then_some(pending.continuation.mana_payment()?.undo_history_start)
            })
            .unwrap_or(0);
        let undoable_mana_abilities = eng
            .state
            .undoable_mana_abilities
            .iter()
            .enumerate()
            .filter(|(index, _)| *index >= payment_undo_start)
            .filter(|(_, e)| e.player == p.id)
            .count() as u32;

        // CR 508.1d / 509.1c: surface must-attack / must-block sets so the client can gate its
        // combat confirm controls exactly as the engine enforces (see combat.rs). Only the player
        // making that declaration gets the ids: the active player is given required attackers while
        // attackers are still open; the defending player is given required blockers after attackers
        // are declared and before blocks are locked in.
        let combat = eng.state.combat.as_ref();
        let attackers_open = eng.state.turn_step == TurnStep::DeclareAttackers
            && !combat.map(|c| c.attackers_declared).unwrap_or(false);
        let required_attacker_ids = if attackers_open && p.id == eng.state.active_player_id() {
            eng.required_attacker_ids()
        } else {
            Vec::new()
        };
        let selectable_attacker_ids = if attackers_open && p.id == eng.state.active_player_id() {
            eng.eligible_attacker_ids(p.id)
        } else {
            Vec::new()
        };
        let blocks_open = eng.state.turn_step == TurnStep::DeclareBlockers
            && !combat.map(|c| c.blockers_declared).unwrap_or(false);
        let required_blocker_ids = if blocks_open && eng.state.is_defending_player(p.id) {
            eng.required_blocker_ids()
        } else {
            Vec::new()
        };
        let legal_block_pairs = if blocks_open && eng.state.is_defending_player(p.id) {
            eng.legal_block_pairs(p.id)
        } else {
            Vec::new()
        };

        batch.legal_by_player.insert(
            p.id,
            LegalActions {
                labels,
                valid_targets_by_hand_slot,
                valid_targets_by_ability,
                undoable_mana_abilities,
                required_attacker_ids,
                required_blocker_ids,
                hand_actions,
                selectable_attacker_ids,
                zone_cast_actions,
                valid_targets_by_zone_object,
                cost_choices_by_ability,
                legal_block_pairs,
                mana_payment_by_ability,
                permanent_actions,
                zone_ability_actions,
                zone_land_actions,
                exile_play_permission_groups,
            },
        );
    }
}

fn activated_ability_info(
    eng: &GameEngine,
    source_id: ObjectId,
    ability_index: usize,
    ability: &ActivatedAbilityDef,
) -> rv1::AbilityInfo {
    let mana_cost = ability
        .costs
        .iter()
        .find_map(|cost| match cost {
            AbilityCost::Mana(cost) => Some(cost.to_string()),
            _ => None,
        })
        .unwrap_or_default();
    let mana_produced = eng
        .active_mana_options(source_id, ability)
        .map(|options| {
            options
                .iter()
                .map(super::events::mana_amount_symbols)
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default();
    let cost_label = ability
        .costs
        .iter()
        .map(|cost| match cost {
            AbilityCost::Tap => "{T}".to_string(),
            AbilityCost::Mana(cost) => cost.to_string(),
            AbilityCost::Discard => "Discard a card".to_string(),
            AbilityCost::DiscardSelf => "Discard this card".to_string(),
            AbilityCost::ExileSelf => "Exile this card".to_string(),
            AbilityCost::SacrificeSelf => "Sacrifice this".to_string(),
            AbilityCost::SacrificePermanent { .. } => "Sacrifice a permanent".to_string(),
            AbilityCost::ExileGraveyardCards { count, .. } => {
                format!("Exile {count} graveyard cards")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    rv1::AbilityInfo {
        text: ability.text.clone(),
        mana_cost,
        mana_produced,
        cost_label,
        activatable: eng.ability_activatable(source_id, ability_index, ability),
    }
}

fn legal_zone_ability_actions(
    eng: &GameEngine,
    player: PlayerId,
) -> Vec<rv1::LegalZoneAbilityAction> {
    let Some(player_index) = eng.state.player_idx(player) else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    for (source_zone, objects) in [
        (
            AbilitySourceZone::Hand,
            &eng.state.players[player_index].hand,
        ),
        (
            AbilitySourceZone::Graveyard,
            &eng.state.players[player_index].graveyard,
        ),
    ] {
        for (zone_index, &object_id) in objects.iter().enumerate() {
            let Some(object) = eng.state.objects.get(&object_id) else {
                continue;
            };
            let Some(definition) = eng.registry.get(&object.card_id) else {
                continue;
            };
            let generation = eng
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0);
            for (ability_index, ability, _) in
                eng.authored_zone_activated_abilities(object_id, source_zone)
            {
                actions.push(rv1::LegalZoneAbilityAction {
                    source_zone: match source_zone {
                        AbilitySourceZone::Hand => rv1::AbilitySourceZone::Hand as i32,
                        AbilitySourceZone::Graveyard => rv1::AbilitySourceZone::Graveyard as i32,
                        AbilitySourceZone::Battlefield => {
                            rv1::AbilitySourceZone::Battlefield as i32
                        }
                    },
                    object_id,
                    zone_change_generation: generation,
                    hand_index: (source_zone == AbilitySourceZone::Hand)
                        .then_some(zone_index as u32),
                    ability_index: ability_index as u32,
                    card_name: definition.name.clone(),
                    ability: Some(activated_ability_info(
                        eng,
                        object_id,
                        ability_index,
                        &ability,
                    )),
                });
            }
        }
    }
    actions
}

fn legal_permanent_actions(eng: &GameEngine, player: PlayerId) -> Vec<rv1::LegalPermanentAction> {
    if eng.state.opening.is_some()
        || eng.state.blocking_choice().is_some()
        || eng.state.priority_player_id() != player
    {
        return Vec::new();
    }
    let Some(player_index) = eng.state.player_idx(player) else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    for &object_id in &eng.state.players[player_index].battlefield {
        let Some(object) = eng.state.objects.get(&object_id) else {
            continue;
        };
        if object.controller != player {
            continue;
        }
        let generation = eng
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        if object.face_down
            && !eng.special_action_prohibited(object_id, SpecialActionKind::TurnFaceUp)
        {
            if let Some(face) = eng
                .registry
                .get(&object.card_id)
                .map(|card| card.primary_face())
            {
                if face.is_creature && !face.mana_cost.pips.is_empty() {
                    let mana_cost = face.mana_cost.to_string();
                    actions.push(rv1::LegalPermanentAction {
                        kind: rv1::PermanentActionKind::TurnFaceUp as i32,
                        object_id,
                        zone_change_generation: generation,
                        label: format!("Turn face up — {mana_cost}"),
                        mana_cost,
                        eligible_restricted_mana_group_ids: Vec::new(),
                        face_index: None,
                    });
                }
            }
        }

        let room_timing = eng.state.active_player_id() == player
            && matches!(eng.state.turn_step, TurnStep::Main1 | TurnStep::Main2)
            && eng.state.stack.is_empty();
        if room_timing {
            if let (Some(room), Some(faces)) = (
                eng.state.room_states.get(&object_id),
                eng.room_faces(object_id),
            ) {
                for (face_index, face) in faces.iter().enumerate() {
                    if room.unlocked.get(face_index).copied() != Some(false) {
                        continue;
                    }
                    let mana_cost = face.mana_cost.to_string();
                    actions.push(rv1::LegalPermanentAction {
                        kind: rv1::PermanentActionKind::UnlockRoomDoor as i32,
                        object_id,
                        zone_change_generation: generation,
                        label: format!("Unlock {} — {mana_cost}", face.name),
                        mana_cost,
                        eligible_restricted_mana_group_ids: Vec::new(),
                        face_index: Some(face_index as u32),
                    });
                }
            }
        }
    }
    actions
}

fn distinct_assignment_exists(
    candidates: &[Vec<ObjectId>],
    choice_index: usize,
    consumed: &mut HashSet<ObjectId>,
) -> bool {
    if choice_index == candidates.len() {
        return true;
    }
    candidates[choice_index].iter().copied().any(|oid| {
        if !consumed.insert(oid) {
            return false;
        }
        let works = distinct_assignment_exists(candidates, choice_index + 1, consumed);
        consumed.remove(&oid);
        works
    })
}

fn legal_ability_cost_choices(
    eng: &GameEngine,
    player: PlayerId,
    source: ObjectId,
    ability_index: usize,
    ability: &tricerules_cards::ActivatedAbilityDef,
) -> rv1::LegalCostChoices {
    let Some(player_idx) = eng.state.player_idx(player) else {
        return rv1::LegalCostChoices::default();
    };
    let mut choices = vec![];
    let mut assignment_candidates = vec![];
    let mut consumed = HashSet::new();
    let mut structurally_payable = eng.ability_activatable(source, ability_index, ability);

    for (cost_index, cost) in ability.costs.iter().enumerate() {
        match cost {
            AbilityCost::Discard => {
                let candidate_ids: Vec<u32> = (0..eng.state.players[player_idx].hand.len())
                    .map(|slot| slot as u32)
                    .collect();
                assignment_candidates.push(eng.state.players[player_idx].hand.clone());
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Hand as i32,
                    candidate_ids,
                    min: 1,
                    max: 1,
                });
            }
            AbilityCost::SacrificeSelf | AbilityCost::DiscardSelf | AbilityCost::ExileSelf => {
                structurally_payable &= consumed.insert(source);
            }
            AbilityCost::SacrificePermanent { filter } => {
                let candidate_ids: Vec<u32> = eng
                    .state
                    .players
                    .iter()
                    .flat_map(|state| state.battlefield.iter().copied())
                    .filter(|&oid| {
                        eng.ability_cost_permanent_matches(player, Some(source), oid, filter)
                    })
                    .collect();
                assignment_candidates.push(candidate_ids.clone());
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Battlefield as i32,
                    candidate_ids,
                    min: 1,
                    max: 1,
                });
            }
            AbilityCost::ExileGraveyardCards {
                count,
                filter,
                exclude_source,
            } => {
                let candidate_ids: Vec<ObjectId> = eng.state.players[player_idx]
                    .graveyard
                    .iter()
                    .copied()
                    .filter(|oid| !exclude_source || *oid != source)
                    .filter(|oid| {
                        super::resolution::library_card_matches_filter(
                            &eng.state,
                            eng.registry,
                            *oid,
                            Some(filter),
                        )
                    })
                    .collect();
                for _ in 0..*count {
                    assignment_candidates.push(candidate_ids.clone());
                }
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Graveyard as i32,
                    candidate_ids,
                    min: *count,
                    max: *count,
                });
            }
            AbilityCost::Tap | AbilityCost::Mana(_) => {}
        }
    }
    structurally_payable &= distinct_assignment_exists(&assignment_candidates, 0, &mut consumed);
    rv1::LegalCostChoices {
        non_mana_costs_payable: structurally_payable,
        choices,
        cast_cost_groups: vec![],
    }
}

fn legal_spell_cost_choices(
    eng: &GameEngine,
    player: PlayerId,
    source: ObjectId,
    costs: &[AdditionalCost],
    cast_cost_groups: &[CastCostGroupDef],
) -> rv1::LegalCostChoices {
    let Some(player_idx) = eng.state.player_idx(player) else {
        return rv1::LegalCostChoices::default();
    };
    let mut choices = vec![];
    let mut assignment_candidates = vec![];
    for (cost_index, cost) in costs.iter().enumerate() {
        match cost {
            AdditionalCost::DiscardCard => {
                let candidates: Vec<(u32, ObjectId)> = eng.state.players[player_idx]
                    .hand
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(_, oid)| *oid != source)
                    .map(|(slot, oid)| (slot as u32, oid))
                    .collect();
                assignment_candidates.push(candidates.iter().map(|(_, oid)| *oid).collect());
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Hand as i32,
                    candidate_ids: candidates.into_iter().map(|(slot, _)| slot).collect(),
                    min: 1,
                    max: 1,
                });
            }
            AdditionalCost::SacrificePermanent { filter } => {
                let candidate_ids: Vec<ObjectId> = eng
                    .state
                    .players
                    .iter()
                    .flat_map(|state| state.battlefield.iter().copied())
                    .filter(|&oid| eng.ability_cost_permanent_matches(player, None, oid, filter))
                    .collect();
                assignment_candidates.push(candidate_ids.clone());
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Battlefield as i32,
                    candidate_ids,
                    min: 1,
                    max: 1,
                });
            }
        }
    }
    let mut non_mana_costs_payable =
        distinct_assignment_exists(&assignment_candidates, 0, &mut HashSet::new());
    let legal_cast_cost_groups = cast_cost_groups
        .iter()
        .enumerate()
        .map(|(group_index, group)| {
            let options = group
                .options
                .iter()
                .enumerate()
                .map(|(option_index, option)| match option {
                    CastCostOptionDef::Mana { label, cost } => rv1::LegalCastCostOption {
                        option_index: option_index as u32,
                        label: label.clone(),
                        kind: rv1::CastCostOptionKind::Mana as i32,
                        additional_mana_cost: cost.to_string(),
                        selectable: true,
                        ..Default::default()
                    },
                    CastCostOptionDef::Behold {
                        label,
                        hand_filter,
                        permanent_filter,
                    } => {
                        let valid_hand_indices = eng.state.players[player_idx]
                            .hand
                            .iter()
                            .copied()
                            .enumerate()
                            .filter(|(_, oid)| {
                                *oid != source
                                    && super::resolution::library_card_matches_filter(
                                        &eng.state,
                                        eng.registry,
                                        *oid,
                                        Some(hand_filter),
                                    )
                            })
                            .map(|(slot, _)| slot as u32)
                            .collect::<Vec<_>>();
                        let valid_permanent_ids = eng
                            .state
                            .players
                            .iter()
                            .flat_map(|state| state.battlefield.iter().copied())
                            .filter(|oid| {
                                eng.ability_cost_permanent_matches(
                                    player,
                                    None,
                                    *oid,
                                    permanent_filter,
                                )
                            })
                            .collect::<Vec<_>>();
                        let valid_permanent_generations = valid_permanent_ids
                            .iter()
                            .map(|oid| {
                                eng.state
                                    .zone_change_generation
                                    .get(oid)
                                    .copied()
                                    .unwrap_or(0)
                            })
                            .collect::<Vec<_>>();
                        let selectable =
                            !valid_hand_indices.is_empty() || !valid_permanent_ids.is_empty();
                        rv1::LegalCastCostOption {
                            option_index: option_index as u32,
                            label: label.clone(),
                            kind: rv1::CastCostOptionKind::Behold as i32,
                            additional_mana_cost: String::new(),
                            valid_hand_indices,
                            valid_permanent_ids,
                            valid_permanent_generations,
                            selectable,
                        }
                    }
                })
                .collect::<Vec<_>>();
            if group.min > 0 && !options.iter().any(|option| option.selectable) {
                non_mana_costs_payable = false;
            }
            rv1::LegalCastCostGroup {
                group_index: group_index as u32,
                prompt: group.prompt.clone(),
                min: group.min,
                max: group.max,
                options,
            }
        })
        .collect();
    rv1::LegalCostChoices {
        non_mana_costs_payable,
        choices,
        cast_cost_groups: legal_cast_cost_groups,
    }
}

fn cast_cost_timing_condition_available(
    costs: &rv1::LegalCostChoices,
    condition: CastCostReceiptCondition,
) -> bool {
    let Some(group) = costs
        .cast_cost_groups
        .iter()
        .find(|group| group.group_index == condition.group_index)
    else {
        return false;
    };
    let selected_available = group
        .options
        .iter()
        .any(|option| option.option_index == condition.option_index && option.selectable);
    if condition.expected_selected {
        selected_available
    } else {
        group.min == 0
            || group
                .options
                .iter()
                .any(|option| option.option_index != condition.option_index && option.selectable)
    }
}

fn face_cast_timing_available(
    face: &CardFace,
    costs: &rv1::LegalCostChoices,
    instant_ok: bool,
    sorcery_ok: bool,
) -> bool {
    if castable_at_instant_speed(&face) {
        return instant_ok;
    }
    sorcery_ok
        || (instant_ok
            && face
                .instant_speed_cast_cost
                .is_some_and(|condition| cast_cost_timing_condition_available(costs, condition)))
}

fn hand_action(
    kind: rv1::HandActionKind,
    hand_index: usize,
    card_name: &str,
    face_index: usize,
    needs_target: bool,
) -> rv1::LegalHandAction {
    rv1::LegalHandAction {
        kind: kind as i32,
        hand_index: hand_index as u32,
        card_name: card_name.to_string(),
        face_index: face_index as u32,
        needs_target,
        min_modes: 0,
        max_modes: 0,
        modes: vec![],
        cost: String::new(),
        cost_choices: None,
        eligible_restricted_mana_group_ids: vec![],
        generic_cost_reduction: 0,
    }
}

fn spell_targets_have_candidate(targets: &rv1::SpellTargets) -> bool {
    targets.groups.iter().any(|group| {
        !group.valid_permanent_ids.is_empty()
            || !group.valid_stack_ids.is_empty()
            || !group.valid_graveyard_ids.is_empty()
            || group.can_target_self
            || group.can_target_opponent
            || group.min == 0
    })
}

/// Number of face-level cast actions the engine is currently publishing for this exact physical
/// source. Stack display uses this to annotate a chosen face only when the player actually had a
/// choice; modal spell choices remain separate annotations.
pub(super) fn cast_option_count_for_source(
    eng: &GameEngine,
    pid: PlayerId,
    source: &rv1::cast_source::Location,
) -> usize {
    match source {
        rv1::cast_source::Location::HandIndex(hand_index) => legal_hand_actions(eng, pid)
            .iter()
            .filter(|action| {
                action.hand_index == *hand_index
                    && action.kind == rv1::HandActionKind::HandActionCastSpell as i32
            })
            .count(),
        rv1::cast_source::Location::GraveyardObjectId(object_id)
        | rv1::cast_source::Location::ExileObjectId(object_id) => legal_zone_cast_actions(eng, pid)
            .iter()
            .filter(|action| action.object_id == *object_id)
            .count(),
    }
}

fn legal_hand_actions(eng: &GameEngine, pid: PlayerId) -> Vec<rv1::LegalHandAction> {
    if let Some(opening) = &eng.state.opening {
        let Some((bottoming_player, _)) = opening.bottom else {
            return Vec::new();
        };
        if pid != bottoming_player {
            return Vec::new();
        }
        let Some(player_index) = eng.state.player_idx(pid) else {
            return Vec::new();
        };
        return eng.state.players[player_index]
            .hand
            .iter()
            .enumerate()
            .map(|(hand_index, &oid)| {
                let name = object_display_name(&eng.state, eng.registry, oid);
                hand_action(
                    rv1::HandActionKind::HandActionOpeningBottom,
                    hand_index,
                    &name,
                    0,
                    false,
                )
            })
            .collect();
    }
    if eng.state.blocking_choice().is_some() {
        return Vec::new();
    }
    if eng.state.combat.as_ref().is_some_and(|combat| {
        combat.blockers_declared
            && combat.damage_assignment_needed
            && combat.assign_combat_damage_phase
    }) {
        return Vec::new();
    }
    if eng.state.priority_player_id() != pid {
        return Vec::new();
    }

    let Some(player_index) = eng.state.player_idx(pid) else {
        return Vec::new();
    };
    if eng.state.turn_step == TurnStep::Cleanup && !eng.state.cleanup_priority_active {
        if eng.state.cleanup_discard_player == Some(pid)
            && eng.state.players[player_index].hand.len() > MAX_HAND_SIZE
        {
            return eng.state.players[player_index]
                .hand
                .iter()
                .enumerate()
                .map(|(hand_index, &oid)| {
                    let name = object_display_name(&eng.state, eng.registry, oid);
                    hand_action(
                        rv1::HandActionKind::HandActionCleanupDiscard,
                        hand_index,
                        &name,
                        0,
                        false,
                    )
                })
                .collect();
        }
        return Vec::new();
    }

    let instant_ok = instant_timing_step_allowed(&eng.state);
    let sorcery_ok = sorcery_speed_available(&eng.state, pid);
    let combat_decl_lock = priority_locked_for_combat_declaration(&eng.state);
    let mut actions = Vec::new();
    for (hand_index, &oid) in eng.state.players[player_index].hand.iter().enumerate() {
        let Some(card_id) = eng.state.objects.get(&oid).map(|object| &object.card_id) else {
            continue;
        };
        let Some(definition) = eng.registry.get(card_id) else {
            continue;
        };
        for (face_index, face) in definition.faces_iter().enumerate() {
            if !definition.face_available_from_hand(face_index) {
                continue;
            }
            if face.is_land {
                let max_lands = 1 + eng.extra_land_plays_for(pid);
                if sorcery_ok && eng.state.lands_played_this_turn < max_lands {
                    actions.push(hand_action(
                        rv1::HandActionKind::HandActionPlayLand,
                        hand_index,
                        &face.name,
                        face_index,
                        false,
                    ));
                }
                continue;
            }
            if combat_decl_lock {
                continue;
            }
            let cost_choices = legal_spell_cost_choices(
                eng,
                pid,
                oid,
                &face.additional_costs,
                &face.cast_cost_groups,
            );
            let cast_ok = face_cast_timing_available(face, &cost_choices, instant_ok, sorcery_ok);
            if cast_ok {
                let mut action = hand_action(
                    rv1::HandActionKind::HandActionCastSpell,
                    hand_index,
                    &face.name,
                    face_index,
                    target_schema(&face.spell_effect, face.targeting.as_ref()).has_targets(),
                );
                action.eligible_restricted_mana_group_ids =
                    eng.eligible_restricted_mana_for_spell(player_index, face);
                action.cost = face.mana_cost.to_string();
                action.generic_cost_reduction =
                    eng.spell_generic_reduction(pid, oid, &face.cost_modifiers);
                if !cost_choices.non_mana_costs_payable {
                    continue;
                }
                action.cost_choices = Some(cost_choices);
                if let Some(modal) = &face.modal_spell {
                    action.min_modes = modal.min_modes;
                    action.max_modes = modal.max_modes;
                    action.modes = modal
                        .modes
                        .iter()
                        .enumerate()
                        .map(|(mode_index, mode)| {
                            let needs_target =
                                target_schema(&mode.effects, mode.targeting.as_ref()).has_targets();
                            let targets = compute_spell_targets(
                                eng,
                                pid,
                                TargetSourceIdentity::spell_face(eng, oid, face_index),
                                &mode.effects,
                                mode.targeting.as_ref(),
                            );
                            let selectable =
                                !needs_target || spell_targets_have_candidate(&targets);
                            rv1::LegalSpellMode {
                                mode_index: mode_index as u32,
                                label: mode.label.clone(),
                                selectable,
                                needs_target,
                                targets: Some(targets),
                            }
                        })
                        .collect();
                    let selectable_count =
                        action.modes.iter().filter(|mode| mode.selectable).count();
                    if selectable_count < modal.min_modes as usize {
                        continue;
                    }
                }
                actions.push(action);
            }
        }
    }
    actions
}

fn legal_zone_cast_actions(eng: &GameEngine, pid: PlayerId) -> Vec<rv1::LegalZoneCastAction> {
    if eng.state.opening.is_some()
        || eng.state.blocking_choice().is_some()
        || eng.state.turn_step == TurnStep::Cleanup
        || eng.state.priority_player_id() != pid
        || priority_locked_for_combat_declaration(&eng.state)
        || eng.state.combat.as_ref().is_some_and(|combat| {
            combat.blockers_declared
                && combat.damage_assignment_needed
                && combat.assign_combat_damage_phase
        })
    {
        return Vec::new();
    }
    let Some(player_index) = eng.state.player_idx(pid) else {
        return Vec::new();
    };
    let instant_ok = instant_timing_step_allowed(&eng.state);
    let sorcery_ok = sorcery_speed_available(&eng.state, pid);
    let mut actions = Vec::new();
    for &oid in &eng.state.players[player_index].graveyard {
        let Some(card_id) = eng.state.objects.get(&oid).map(|object| &object.card_id) else {
            continue;
        };
        let Some(definition) = eng.registry.get(card_id) else {
            continue;
        };
        for (face_index, face) in definition.faces_iter().enumerate() {
            if face.flashback_cost.is_none() || face.is_land {
                continue;
            }
            let cost_choices = legal_spell_cost_choices(
                eng,
                pid,
                oid,
                &face.additional_costs,
                &face.cast_cost_groups,
            );
            let cast_ok = face_cast_timing_available(face, &cost_choices, instant_ok, sorcery_ok);
            if !cast_ok {
                continue;
            }
            let mut action = rv1::LegalZoneCastAction {
                source_zone: rv1::CastSourceZone::Graveyard as i32,
                object_id: oid,
                card_name: face.name.clone(),
                face_index: face_index as u32,
                needs_target: target_schema(&face.spell_effect, face.targeting.as_ref())
                    .has_targets(),
                min_modes: 0,
                max_modes: 0,
                modes: vec![],
                cost: face
                    .flashback_cost
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                cost_choices: None,
                eligible_restricted_mana_group_ids: eng
                    .eligible_restricted_mana_for_spell(player_index, face),
                generic_cost_reduction: eng.spell_generic_reduction(pid, oid, &face.cost_modifiers),
            };
            if !cost_choices.non_mana_costs_payable {
                continue;
            }
            action.cost_choices = Some(cost_choices);
            if let Some(modal) = &face.modal_spell {
                action.min_modes = modal.min_modes;
                action.max_modes = modal.max_modes;
                action.modes = modal
                    .modes
                    .iter()
                    .enumerate()
                    .map(|(mode_index, mode)| {
                        let needs_target =
                            target_schema(&mode.effects, mode.targeting.as_ref()).has_targets();
                        let targets = compute_spell_targets(
                            eng,
                            pid,
                            TargetSourceIdentity::spell_face(eng, oid, face_index),
                            &mode.effects,
                            mode.targeting.as_ref(),
                        );
                        let selectable = !needs_target || spell_targets_have_candidate(&targets);
                        rv1::LegalSpellMode {
                            mode_index: mode_index as u32,
                            label: mode.label.clone(),
                            selectable,
                            needs_target,
                            targets: Some(targets),
                        }
                    })
                    .collect();
                if action.modes.iter().filter(|mode| mode.selectable).count()
                    < modal.min_modes as usize
                {
                    continue;
                }
            }
            actions.push(action);
        }
    }

    let mut emitted = BTreeSet::new();
    for permission in eng
        .state
        .active_exile_play_permissions
        .iter()
        .filter(|permission| permission.player_id == pid)
    {
        let Some(object) = eng.state.objects.get(&permission.object_id) else {
            continue;
        };
        let generation = eng
            .state
            .zone_change_generation
            .get(&object.id)
            .copied()
            .unwrap_or(0);
        if object.zone != Zone::Exile || generation != permission.zone_change_generation {
            continue;
        }
        let Some(definition) = eng.registry.get(&object.card_id) else {
            continue;
        };
        let face_indices: Vec<_> = match permission.scope {
            ExilePlayPermissionScope::CastFace(face_index) => vec![face_index],
            ExilePlayPermissionScope::PlayCard => (0..definition.faces_iter().count()).collect(),
        };
        for face_index in face_indices {
            if !emitted.insert((object.id, face_index)) {
                continue;
            }
            let Some(face) = definition.face(face_index) else {
                continue;
            };
            if face.is_land {
                continue;
            }
            let cost_choices = legal_spell_cost_choices(
                eng,
                pid,
                object.id,
                &face.additional_costs,
                &face.cast_cost_groups,
            );
            let cast_ok = face_cast_timing_available(face, &cost_choices, instant_ok, sorcery_ok);
            if !cast_ok {
                continue;
            }
            let mut action = rv1::LegalZoneCastAction {
                source_zone: rv1::CastSourceZone::Exile as i32,
                object_id: object.id,
                card_name: face.name.clone(),
                face_index: face_index as u32,
                needs_target: target_schema(&face.spell_effect, face.targeting.as_ref())
                    .has_targets(),
                min_modes: 0,
                max_modes: 0,
                modes: vec![],
                cost: face.mana_cost.to_string(),
                cost_choices: None,
                eligible_restricted_mana_group_ids: eng
                    .eligible_restricted_mana_for_spell(player_index, face),
                generic_cost_reduction: eng.spell_generic_reduction(
                    pid,
                    object.id,
                    &face.cost_modifiers,
                ),
            };
            if !cost_choices.non_mana_costs_payable {
                continue;
            }
            action.cost_choices = Some(cost_choices);
            if let Some(modal) = &face.modal_spell {
                action.min_modes = modal.min_modes;
                action.max_modes = modal.max_modes;
                action.modes = modal
                    .modes
                    .iter()
                    .enumerate()
                    .map(|(mode_index, mode)| {
                        let needs_target =
                            target_schema(&mode.effects, mode.targeting.as_ref()).has_targets();
                        let targets = compute_spell_targets(
                            eng,
                            pid,
                            TargetSourceIdentity::current(eng, object.id),
                            &mode.effects,
                            mode.targeting.as_ref(),
                        );
                        rv1::LegalSpellMode {
                            mode_index: mode_index as u32,
                            label: mode.label.clone(),
                            selectable: !needs_target || spell_targets_have_candidate(&targets),
                            needs_target,
                            targets: Some(targets),
                        }
                    })
                    .collect();
                if action.modes.iter().filter(|mode| mode.selectable).count()
                    < modal.min_modes as usize
                {
                    continue;
                }
            }
            actions.push(action);
        }
    }
    actions
}

fn legal_zone_land_actions(eng: &GameEngine, pid: PlayerId) -> Vec<rv1::LegalZoneLandAction> {
    let max_lands = 1 + eng.extra_land_plays_for(pid);
    if eng.state.opening.is_some()
        || eng.state.blocking_choice().is_some()
        || eng.state.lands_played_this_turn >= max_lands
        || !sorcery_speed_available(&eng.state, pid)
    {
        return Vec::new();
    }
    let mut emitted = BTreeSet::new();
    let mut actions = Vec::new();
    for permission in eng
        .state
        .active_exile_play_permissions
        .iter()
        .filter(|permission| {
            permission.player_id == pid && permission.scope == ExilePlayPermissionScope::PlayCard
        })
    {
        let Some(object) = eng.state.objects.get(&permission.object_id) else {
            continue;
        };
        let generation = eng
            .state
            .zone_change_generation
            .get(&object.id)
            .copied()
            .unwrap_or(0);
        if object.zone != Zone::Exile || generation != permission.zone_change_generation {
            continue;
        }
        let Some(definition) = eng.registry.get(&object.card_id) else {
            continue;
        };
        for (face_index, face) in definition.faces_iter().enumerate() {
            if !face.is_land || !emitted.insert((object.id, face_index)) {
                continue;
            }
            actions.push(rv1::LegalZoneLandAction {
                source_zone: rv1::CastSourceZone::Exile as i32,
                object_id: object.id,
                card_name: face.name.clone(),
                face_index: face_index as u32,
            });
        }
    }
    actions
}

fn exile_play_permission_groups(
    eng: &GameEngine,
    pid: PlayerId,
) -> Vec<rv1::ExilePlayPermissionGroup> {
    let mut groups: BTreeMap<u64, (String, BTreeSet<ObjectId>)> = BTreeMap::new();
    for permission in eng
        .state
        .active_exile_play_permissions
        .iter()
        .filter(|permission| permission.player_id == pid)
    {
        let Some(object) = eng.state.objects.get(&permission.object_id) else {
            continue;
        };
        let generation = eng
            .state
            .zone_change_generation
            .get(&permission.object_id)
            .copied()
            .unwrap_or(0);
        if object.zone != Zone::Exile || generation != permission.zone_change_generation {
            continue;
        }
        groups
            .entry(permission.group_id)
            .or_insert_with(|| (permission.source_label.clone(), BTreeSet::new()))
            .1
            .insert(permission.object_id);
    }
    groups
        .into_iter()
        .map(
            |(group_id, (source_label, object_ids))| rv1::ExilePlayPermissionGroup {
                group_id,
                source_label,
                object_ids: object_ids.into_iter().collect(),
            },
        )
        .collect()
}

fn opening_legal_labels(eng: &GameEngine, pid: PlayerId, op: &OpeningSequence) -> Vec<String> {
    if op.starting_player.is_none() {
        if pid == op.chooser {
            return vec![
                "You start (opening pick)".into(),
                "Opponent starts (opening pick)".into(),
            ];
        }
        return vec!["Wait: opponent chooses who goes first (opening)".into()];
    }
    if let Some((bp, _rem)) = op.bottom {
        if pid != bp {
            return vec!["Wait: opponent is bottoming cards (opening)".into()];
        }
        let idx = eng.state.player_idx(bp).unwrap();
        let hand = &eng.state.players[idx].hand;
        let mut out = Vec::new();
        for (i, &oid) in hand.iter().enumerate() {
            let name = eng
                .state
                .objects
                .get(&oid)
                .and_then(|o| eng.registry.get(&o.card_id))
                .map(|d| d.name.as_str())
                .unwrap_or("card");
            out.push(format!("Put {name} on bottom (opening, hand idx {i})"));
        }
        return out;
    }
    if let Some(actor) = op.mulligan_actor {
        if pid != actor {
            return vec!["Wait: opponent mulligan decision (opening)".into()];
        }
        return vec![
            "Keep opening hand (opening)".into(),
            "Mulligan — redraw to 7 (opening)".into(),
        ];
    }
    vec!["Wait (opening)".into()]
}

fn legal_labels(eng: &GameEngine, pid: PlayerId) -> Vec<String> {
    if let Some(op) = &eng.state.opening {
        return opening_legal_labels(eng, pid, op);
    }
    if let Some(pr) = &eng.state.pending_resolution {
        return if pr.deciding_player == pid {
            vec![format!("Resolve: {}", pr.presentation.prompt)]
        } else {
            vec!["Waiting: opponent making a resolution choice".into()]
        };
    }
    // CR 603.3b, ahead of the target prompt below: the order is fixed before any of the block is
    // placed, so a player never sees both at once.
    if let Some(pto) = &eng.state.pending_trigger_order {
        return if pto.deciding_player == pid {
            vec![format!(
                "Order {} simultaneous triggers",
                pto.candidates.len()
            )]
        } else {
            vec!["Waiting: opponent ordering triggers".into()]
        };
    }
    if let Some(pt) = eng.state.pending_triggers.front() {
        return if pt.controller == pid {
            vec![format!("Choose target for trigger: {}", pt.ability_text)]
        } else {
            vec!["Waiting: opponent choosing trigger target".into()]
        };
    }
    if let Some(c) = &eng.state.combat {
        if c.blockers_declared && c.damage_assignment_needed && c.assign_combat_damage_phase {
            if pid == eng.state.active_player_id() {
                let mut out = Vec::new();
                for (&att, blks) in &c.blockers {
                    if eng.attacker_needs_explicit_damage_assignment(att, blks.len())
                        && !c.damage_assignments.contains_key(&att)
                    {
                        let name = object_display_name(&eng.state, eng.registry, att);
                        out.push(format!("Assign combat damage for {name}"));
                    }
                }
                return out;
            } else {
                return vec!["Waiting: opponent assigning combat damage".into()];
            }
        }
    }
    let mut v = vec!["Pass priority".into()];
    if eng.state.priority_player_id() != pid {
        return v;
    }
    if eng.state.turn_step == TurnStep::Cleanup {
        if let Some(cp) = eng.state.cleanup_discard_player {
            if pid != cp {
                return vec!["Waiting (opponent cleanup discard)".into()];
            }
            let idx = eng.state.player_idx(cp).unwrap();
            let hand = &eng.state.players[idx].hand;
            if hand.len() <= MAX_HAND_SIZE {
                return v;
            }
            let mut out = Vec::new();
            for (i, &oid) in hand.iter().enumerate() {
                let name = eng
                    .state
                    .objects
                    .get(&oid)
                    .and_then(|o| eng.registry.get(&o.card_id))
                    .map(|d| d.name.as_str())
                    .unwrap_or("card");
                out.push(format!("Discard {name} (cleanup, hand idx {i})"));
            }
            return out;
        }
    }
    let idx = match eng.state.player_idx(pid) {
        Some(i) => i,
        None => return v,
    };
    let instant_ok = instant_timing_step_allowed(&eng.state);
    let sorcery_ok = sorcery_speed_available(&eng.state, pid);
    let combat_decl_lock = priority_locked_for_combat_declaration(&eng.state);
    for (i, &oid) in eng.state.players[idx].hand.iter().enumerate() {
        let cid = &eng.state.objects.get(&oid).unwrap().card_id;
        if let Some(def) = eng.registry.get(cid) {
            for (face_index, face) in def.faces_iter().enumerate() {
                if !def.face_available_from_hand(face_index) {
                    continue;
                }
                let name = &face.name;
                if face.is_land {
                    let max_lands = 1 + eng.extra_land_plays_for(pid);
                    if sorcery_ok && eng.state.lands_played_this_turn < max_lands {
                        if def.is_multiface() {
                            v.push(format!(
                                "Play land {name} (hand idx {i}, face {face_index})"
                            ));
                        } else {
                            v.push(format!("Play land {name} (hand idx {i})"));
                        }
                    }
                } else if !combat_decl_lock {
                    let costs = legal_spell_cost_choices(
                        eng,
                        pid,
                        oid,
                        &face.additional_costs,
                        &face.cast_cost_groups,
                    );
                    let cast_ok = face_cast_timing_available(face, &costs, instant_ok, sorcery_ok);
                    if cast_ok {
                        let needs_target =
                            target_schema(&face.spell_effect, face.targeting.as_ref())
                                .has_targets();
                        if needs_target {
                            v.push(format!("Cast {name} (hand idx {i}, target)"));
                        } else {
                            v.push(format!("Cast {name} (hand idx {i})"));
                        }
                    }
                }
            }
        } else if !combat_decl_lock && (instant_ok || sorcery_ok) {
            v.push(format!("Play unknown card (hand idx {i})"));
        }
    }
    v
}

#[cfg(test)]
mod cast_cost_timing_tests {
    use super::*;

    #[test]
    fn conditional_instant_timing_is_published_only_with_a_live_cost_option() {
        let condition = CastCostReceiptCondition {
            group_index: 0,
            option_index: 0,
            expected_selected: true,
        };
        let mut face = CardFace {
            instant_speed_cast_cost: Some(condition),
            ..Default::default()
        };
        face.is_sorcery = true;
        let mut costs = rv1::LegalCostChoices {
            non_mana_costs_payable: true,
            cast_cost_groups: vec![rv1::LegalCastCostGroup {
                group_index: 0,
                min: 0,
                max: 1,
                options: vec![rv1::LegalCastCostOption {
                    option_index: 0,
                    selectable: false,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(!face_cast_timing_available(&face, &costs, true, false));
        costs.cast_cost_groups[0].options[0].selectable = true;
        assert!(face_cast_timing_available(&face, &costs, true, false));
        assert!(face_cast_timing_available(&face, &costs, true, true));
    }
}
