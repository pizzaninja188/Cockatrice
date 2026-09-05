use super::casting::castable_at_instant_speed;
use super::combat::priority_locked_for_combat_declaration;
use super::events::object_display_name;
use super::presentation::{presentation_ref, PresentationPath};
use super::priority::{instant_timing_step_allowed, sorcery_speed_available};
use super::targeting::{
    compute_ability_targets, compute_ability_targets_with_context, compute_spell_targets,
    target_filter_legal_at_resolution, target_schema, TargetSourceIdentity,
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
                let cast_reveals = item.cast_cost_receipts.iter().flat_map(|receipt| {
                    receipt.objects.iter().filter_map(|object| {
                        let CastCostObjectReceipt::RevealedHand {
                            card_id, card_name, ..
                        } = object
                        else {
                            return None;
                        };
                        Some(rv1::ActivePublicReveal {
                            source_stack_object_id: item.id,
                            group_index: receipt.group_index,
                            revealing_player_id: item.controller,
                            source_description: object_display_name(
                                &eng.state,
                                eng.registry,
                                item.id,
                            ),
                            card_id: card_id.clone(),
                            card_name: card_name.clone(),
                        })
                    })
                });
                let ninjutsu_reveal = item
                    .activated_ability
                    .as_ref()
                    .filter(|ability| {
                        ability
                            .costs
                            .contains(&AbilityCost::ReturnUnblockedAttacker)
                    })
                    .map(|_| {
                        let card_name = eng
                            .registry
                            .get(&item.card_id)
                            .map(|definition| definition.name.clone())
                            .unwrap_or_else(|| item.card_id.clone());
                        rv1::ActivePublicReveal {
                            source_stack_object_id: item.id,
                            group_index: 0,
                            revealing_player_id: item.controller,
                            source_description: card_name.clone(),
                            card_id: item.card_id.clone(),
                            card_name,
                        }
                    });
                cast_reveals.chain(ninjutsu_reveal)
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
                    let mut t = compute_spell_targets(
                        eng,
                        p.id,
                        TargetSourceIdentity::spell_face(eng, oid, face_index),
                        &face.spell_effect,
                        face.targeting.as_ref(),
                        &face.cost_modifiers,
                    );
                    if let Some(costs) = hand_actions
                        .iter()
                        .find(|action| {
                            action.kind == rv1::HandActionKind::HandActionCastSpell as i32
                                && action.hand_index == slot as u32
                                && action.face_index == face_index as u32
                        })
                        .and_then(|action| action.cost_choices.as_ref())
                    {
                        apply_cast_cost_target_requirements(
                            eng,
                            p.id,
                            TargetSourceIdentity::spell_face(eng, oid, face_index),
                            face,
                            costs,
                            face.targeting.as_ref(),
                            &mut t,
                        );
                    }
                    let key = (slot as u32) << 8 | face_index as u32;
                    valid_targets_by_hand_slot.insert(key, t);
                }
            }
            for &poid in &eng.state.players[idx].battlefield {
                if !eng.state.objects.contains_key(&poid) {
                    continue;
                }
                for (ai, ability, _, _) in eng.effective_activated_abilities(poid) {
                    let key = (poid as u64) << 32 | ai as u64;
                    mana_payment_by_ability.insert(
                        key,
                        rv1::ManaPaymentEligibility {
                            has_waterbend: ability
                                .costs
                                .iter()
                                .any(|c| matches!(c, AbilityCost::Waterbend(_))),
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
                        has_waterbend: ability
                            .costs
                            .iter()
                            .any(|c| matches!(c, AbilityCost::Waterbend(_))),
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
            let mut targets = compute_spell_targets(
                eng,
                p.id,
                TargetSourceIdentity::spell_face(eng, action.object_id, action.face_index as usize),
                &face.spell_effect,
                face.targeting.as_ref(),
                &face.cost_modifiers,
            );
            if let Some(costs) = action.cost_choices.as_ref() {
                apply_cast_cost_target_requirements(
                    eng,
                    p.id,
                    TargetSourceIdentity::spell_face(
                        eng,
                        action.object_id,
                        action.face_index as usize,
                    ),
                    face,
                    costs,
                    face.targeting.as_ref(),
                    &mut targets,
                );
            }
            valid_targets_by_zone_object.insert(
                (u64::from(action.object_id) << 8) | u64::from(action.face_index),
                targets,
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
        let legal_attack_assignments = if attackers_open && p.id == eng.state.active_player_id() {
            eng.legal_attack_assignments(p.id)
        } else {
            Vec::new()
        };
        let blocks_open = eng.state.turn_step == TurnStep::DeclareBlockers
            && !combat.map(|c| c.blockers_declared).unwrap_or(false);
        let (legal_block_pairs, required_blocker_ids) =
            if blocks_open && eng.state.is_defending_player(p.id) {
                eng.blocking_options(p.id)
            } else {
                (Vec::new(), Vec::new())
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
                legal_attack_assignments,
            },
        );
    }
}

pub(super) fn activated_ability_info(
    eng: &GameEngine,
    source_id: ObjectId,
    face_index: usize,
    ability_index: usize,
    ability_path: &[tricerules_cards::AbilityId],
    ability: &ActivatedAbilityDef,
) -> rv1::AbilityInfo {
    let controller = eng
        .state
        .objects
        .get(&source_id)
        .map(|object| object.controller)
        .unwrap_or_default();
    let mana_cost = eng.effective_ability_mana_cost(controller, source_id, ability);
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
            AbilityCost::RemoveCounters { counter, count, .. } => format!(
                "Remove {count} {}counter(s)",
                counter
                    .map(|k| format!("{} ", k.label()))
                    .unwrap_or_default()
            ),
            AbilityCost::Tap => "{T}".to_string(),
            AbilityCost::TapPermanents { constraint, .. } => match constraint {
                ObjectPaymentConstraint::ExactCount(count) => format!("Tap {count} permanents"),
                ObjectPaymentConstraint::AggregateMinimum { minimum, .. } => {
                    format!("Tap permanents with total power {minimum} or greater")
                }
            },
            AbilityCost::Blight { count } => format!("Blight {count}"),
            AbilityCost::PayLife { amount } => format!("Pay {amount} life"),
            AbilityCost::ReturnUnblockedAttacker => "Return an unblocked attacker".to_string(),
            AbilityCost::Loyalty(delta) if *delta >= 0 => format!("+{delta}"),
            AbilityCost::Loyalty(delta) => delta.to_string(),
            AbilityCost::Mana(cost) => cost.to_string(),
            AbilityCost::Waterbend(cost) => format!("Waterbend {cost}"),
            AbilityCost::Discard => "Discard a card".to_string(),
            AbilityCost::DiscardSelf => "Discard this card".to_string(),
            AbilityCost::ExileSelf => "Exile this card".to_string(),
            AbilityCost::SacrificeSelf => "Sacrifice this".to_string(),
            AbilityCost::SacrificePermanent { .. } => "Sacrifice a permanent".to_string(),
            AbilityCost::ExileGraveyardCards { constraint, .. } => match constraint {
                ObjectPaymentConstraint::ExactCount(count) => {
                    format!("Exile {count} graveyard cards")
                }
                ObjectPaymentConstraint::AggregateMinimum { minimum, .. } => {
                    format!("Collect evidence {minimum}")
                }
            },
        })
        .collect::<Vec<_>>()
        .join(", ");
    let fallback = ability.fallback_text_with_path(
        &eng.effective_face(source_id)
            .map(|face| face.name.clone())
            .unwrap_or_else(|| "Unknown card".into()),
        ability_path,
    );
    let definition = eng.ability_definition(source_id, face_index, ability_path.to_vec());
    rv1::AbilityInfo {
        text: fallback.clone(),
        mana_cost,
        mana_produced,
        cost_label,
        activatable: eng.ability_activatable(source_id, ability_index, ability),
        presentation: Some(presentation_ref(
            eng.registry,
            &definition.card_id,
            &definition.face_id,
            definition
                .ability_path
                .iter()
                .map(PresentationPath::Ability),
            &ability.presentation,
            fallback,
        )),
        ability_index: ability_index as u32,
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
            for (ability_index, ability, face_index) in
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
                        face_index,
                        ability_index,
                        std::slice::from_ref(&ability.ability_id),
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
                        eligible_restricted_mana_group_ids: eng
                            .eligible_restricted_mana_for_special_action(
                                player_index,
                                SpecialActionManaPurpose::TurnFaceUp,
                            ),
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
                        eligible_restricted_mana_group_ids: eng
                            .eligible_restricted_mana_for_special_action(
                                player_index,
                                SpecialActionManaPurpose::UnlockRoomDoor,
                            ),
                        face_index: Some(face_index as u32),
                    });
                }
            }
        }
    }
    actions
}

enum ObjectPaymentRequirement {
    Exact {
        candidates: Vec<ObjectId>,
        count: u32,
    },
    Aggregate {
        candidates: Vec<(ObjectId, i64)>,
        minimum: u32,
    },
}

#[derive(Clone, Copy)]
struct AggregateAssignmentProgress {
    start: usize,
    total: i64,
    minimum: i64,
}

fn payment_assignment_exists(
    requirements: &[ObjectPaymentRequirement],
    requirement_index: usize,
    consumed: &mut HashSet<ObjectId>,
    memo: &mut HashMap<(usize, Vec<ObjectId>), bool>,
) -> bool {
    if requirement_index == requirements.len() {
        return true;
    }
    let mut consumed_key: Vec<_> = consumed.iter().copied().collect();
    consumed_key.sort_unstable();
    let memo_key = (requirement_index, consumed_key);
    if let Some(result) = memo.get(&memo_key) {
        return *result;
    }
    let result = match &requirements[requirement_index] {
        ObjectPaymentRequirement::Exact { candidates, count } => choose_exact_assignment(
            requirements,
            requirement_index,
            candidates,
            0,
            *count,
            consumed,
            memo,
        ),
        ObjectPaymentRequirement::Aggregate {
            candidates,
            minimum,
        } => choose_aggregate_assignment(
            requirements,
            requirement_index,
            candidates,
            AggregateAssignmentProgress {
                start: 0,
                total: 0,
                minimum: i64::from(*minimum),
            },
            consumed,
            memo,
        ),
    };
    memo.insert(memo_key, result);
    result
}

fn choose_exact_assignment(
    requirements: &[ObjectPaymentRequirement],
    requirement_index: usize,
    candidates: &[ObjectId],
    start: usize,
    remaining: u32,
    consumed: &mut HashSet<ObjectId>,
    memo: &mut HashMap<(usize, Vec<ObjectId>), bool>,
) -> bool {
    if remaining == 0 {
        return payment_assignment_exists(requirements, requirement_index + 1, consumed, memo);
    }
    (start..candidates.len()).any(|index| {
        let oid = candidates[index];
        if !consumed.insert(oid) {
            return false;
        }
        let works = choose_exact_assignment(
            requirements,
            requirement_index,
            candidates,
            index + 1,
            remaining - 1,
            consumed,
            memo,
        );
        consumed.remove(&oid);
        works
    })
}

fn choose_aggregate_assignment(
    requirements: &[ObjectPaymentRequirement],
    requirement_index: usize,
    candidates: &[(ObjectId, i64)],
    progress: AggregateAssignmentProgress,
    consumed: &mut HashSet<ObjectId>,
    memo: &mut HashMap<(usize, Vec<ObjectId>), bool>,
) -> bool {
    if progress.total >= progress.minimum
        && payment_assignment_exists(requirements, requirement_index + 1, consumed, memo)
    {
        return true;
    }
    let remaining_positive: i64 = candidates[progress.start..]
        .iter()
        .filter(|(_, contribution)| *contribution > 0)
        .map(|(_, contribution)| *contribution)
        .sum();
    if progress.total + remaining_positive < progress.minimum {
        return false;
    }
    (progress.start..candidates.len()).any(|index| {
        let (oid, contribution) = candidates[index];
        if contribution <= 0 || !consumed.insert(oid) {
            return false;
        }
        let works = choose_aggregate_assignment(
            requirements,
            requirement_index,
            candidates,
            AggregateAssignmentProgress {
                start: index + 1,
                total: progress.total + contribution,
                ..progress
            },
            consumed,
            memo,
        );
        consumed.remove(&oid);
        works
    })
}

fn contribution_kind_proto(kind: ObjectContributionKind) -> i32 {
    match kind {
        ObjectContributionKind::ManaValue => rv1::ObjectContributionKind::ManaValue as i32,
        ObjectContributionKind::CurrentPower => rv1::ObjectContributionKind::CurrentPower as i32,
    }
}

fn aggregate_constraint_proto(
    constraint: ObjectPaymentConstraint,
) -> Option<rv1::AggregateMinimumConstraint> {
    constraint
        .aggregate_minimum()
        .map(|(minimum, contribution)| rv1::AggregateMinimumConstraint {
            minimum,
            contribution_kind: contribution_kind_proto(contribution),
        })
}

fn cost_object_candidate(
    eng: &GameEngine,
    oid: ObjectId,
    contribution: i64,
) -> rv1::CostObjectCandidate {
    rv1::CostObjectCandidate {
        object: Some(rv1::CostObjectRef {
            object_id: oid,
            zone_change_generation: eng
                .state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0),
        }),
        contribution,
    }
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
    let mut requirements = vec![];
    let mut consumed = HashSet::new();
    let mut structurally_payable = eng.ability_activatable(source, ability_index, ability);

    for (cost_index, cost) in ability.costs.iter().enumerate() {
        match cost {
            AbilityCost::RemoveCounters {
                counter,
                count,
                payment_source,
            } => {
                let choice = match payment_source {
                    CounterRemovalPaymentSource::Source => {
                        eng.counter_removal_choice(source, cost_index, *counter, *count)
                    }
                    CounterRemovalPaymentSource::SelectedPermanent(filter) => {
                        let Some(counter) = counter else {
                            structurally_payable = false;
                            continue;
                        };
                        eng.selected_counter_removal_choice(
                            player, source, cost_index, *counter, *count, filter,
                        )
                    }
                };
                structurally_payable &=
                    matches!(payment_source, CounterRemovalPaymentSource::Source)
                        || !choice.candidate_objects.is_empty();
                choices.push(choice);
            }
            AbilityCost::Blight { count } => {
                let choice = eng.blight_cost_choice(player, cost_index, *count);
                structurally_payable &= !choice.candidate_ids.is_empty();
                choices.push(choice);
            }
            AbilityCost::ReturnUnblockedAttacker => {
                let candidate_ids = eng.ninjutsu_return_candidates(player);
                structurally_payable &= !candidate_ids.is_empty();
                requirements.push(ObjectPaymentRequirement::Exact {
                    candidates: candidate_ids.clone(),
                    count: 1,
                });
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Battlefield as i32,
                    candidate_ids: candidate_ids.clone(),
                    min: 1,
                    max: 1,
                    blight_count: 0,
                    counter_removal: None,
                    kind: rv1::CostChoiceKind::ReturnUnblockedAttacker as i32,
                    candidate_objects: candidate_ids
                        .iter()
                        .map(|oid| cost_object_candidate(eng, *oid, 0))
                        .collect(),
                    aggregate_minimum: None,
                });
            }
            AbilityCost::Discard => {
                let candidate_ids: Vec<u32> = (0..eng.state.players[player_idx].hand.len())
                    .map(|slot| slot as u32)
                    .collect();
                requirements.push(ObjectPaymentRequirement::Exact {
                    candidates: eng.state.players[player_idx].hand.clone(),
                    count: 1,
                });
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Hand as i32,
                    candidate_ids: candidate_ids.clone(),
                    min: 1,
                    max: 1,
                    blight_count: 0,
                    counter_removal: None,
                    kind: rv1::CostChoiceKind::Discard as i32,
                    candidate_objects: vec![],
                    aggregate_minimum: None,
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
                requirements.push(ObjectPaymentRequirement::Exact {
                    candidates: candidate_ids.clone(),
                    count: 1,
                });
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Battlefield as i32,
                    candidate_ids: candidate_ids.clone(),
                    min: 1,
                    max: 1,
                    blight_count: 0,
                    counter_removal: None,
                    kind: rv1::CostChoiceKind::Sacrifice as i32,
                    candidate_objects: candidate_ids
                        .iter()
                        .map(|oid| cost_object_candidate(eng, *oid, 0))
                        .collect(),
                    aggregate_minimum: None,
                });
            }
            AbilityCost::TapPermanents {
                constraint,
                filter,
                exclude_source,
            } => {
                let candidate_ids: Vec<ObjectId> = eng
                    .state
                    .players
                    .iter()
                    .flat_map(|state| state.battlefield.iter().copied())
                    .filter(|oid| !exclude_source || *oid != source)
                    .filter(|&oid| {
                        eng.ability_cost_permanent_matches(player, Some(source), oid, filter)
                            && !eng.state.objects[&oid].tapped
                    })
                    .collect();
                match *constraint {
                    ObjectPaymentConstraint::ExactCount(count) => {
                        requirements.push(ObjectPaymentRequirement::Exact {
                            candidates: candidate_ids.clone(),
                            count,
                        });
                    }
                    ObjectPaymentConstraint::AggregateMinimum {
                        minimum,
                        contribution,
                    } => requirements.push(ObjectPaymentRequirement::Aggregate {
                        candidates: candidate_ids
                            .iter()
                            .filter_map(|oid| {
                                eng.object_payment_contribution(*oid, contribution)
                                    .map(|value| (*oid, value))
                            })
                            .collect(),
                        minimum,
                    }),
                }
                let exact_count = constraint.exact_count();
                let contribution_kind = constraint.aggregate_minimum().map(|(_, kind)| kind);
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Battlefield as i32,
                    candidate_ids: candidate_ids.clone(),
                    min: exact_count.unwrap_or(0),
                    max: exact_count.unwrap_or(candidate_ids.len() as u32),
                    blight_count: 0,
                    counter_removal: None,
                    kind: rv1::CostChoiceKind::Tap as i32,
                    candidate_objects: candidate_ids
                        .iter()
                        .map(|oid| {
                            cost_object_candidate(
                                eng,
                                *oid,
                                contribution_kind
                                    .and_then(|kind| eng.object_payment_contribution(*oid, kind))
                                    .unwrap_or(0),
                            )
                        })
                        .collect(),
                    aggregate_minimum: aggregate_constraint_proto(*constraint),
                });
            }
            AbilityCost::ExileGraveyardCards {
                constraint,
                filter,
                exclude_source,
            } => {
                let candidate_ids: Vec<ObjectId> = eng.state.players[player_idx]
                    .graveyard
                    .iter()
                    .copied()
                    .filter(|oid| !exclude_source || *oid != source)
                    .filter(|oid| {
                        super::card_predicates::zone_card_matches_filter(
                            &eng.state,
                            eng.registry,
                            *oid,
                            Some(filter),
                        )
                    })
                    .collect();
                match *constraint {
                    ObjectPaymentConstraint::ExactCount(count) => {
                        requirements.push(ObjectPaymentRequirement::Exact {
                            candidates: candidate_ids.clone(),
                            count,
                        });
                    }
                    ObjectPaymentConstraint::AggregateMinimum {
                        minimum,
                        contribution,
                    } => requirements.push(ObjectPaymentRequirement::Aggregate {
                        candidates: candidate_ids
                            .iter()
                            .filter_map(|oid| {
                                eng.object_payment_contribution(*oid, contribution)
                                    .map(|value| (*oid, value))
                            })
                            .collect(),
                        minimum,
                    }),
                }
                let exact_count = constraint.exact_count();
                let contribution_kind = constraint.aggregate_minimum().map(|(_, kind)| kind);
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Graveyard as i32,
                    candidate_ids: candidate_ids.clone(),
                    min: exact_count.unwrap_or(0),
                    max: exact_count.unwrap_or(candidate_ids.len() as u32),
                    blight_count: 0,
                    counter_removal: None,
                    kind: rv1::CostChoiceKind::Exile as i32,
                    candidate_objects: candidate_ids
                        .iter()
                        .map(|oid| {
                            cost_object_candidate(
                                eng,
                                *oid,
                                contribution_kind
                                    .and_then(|kind| eng.object_payment_contribution(*oid, kind))
                                    .unwrap_or(0),
                            )
                        })
                        .collect(),
                    aggregate_minimum: aggregate_constraint_proto(*constraint),
                });
            }
            AbilityCost::Tap
            | AbilityCost::PayLife { .. }
            | AbilityCost::Mana(_)
            | AbilityCost::Waterbend(_)
            | AbilityCost::Loyalty(_) => {}
        }
    }
    structurally_payable &=
        payment_assignment_exists(&requirements, 0, &mut consumed, &mut HashMap::new());
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
    card_id: &str,
    face: &CardFace,
    sneak_candidates: Option<&[ObjectId]>,
) -> rv1::LegalCostChoices {
    let Some(player_idx) = eng.state.player_idx(player) else {
        return rv1::LegalCostChoices::default();
    };
    let mut choices = vec![];
    let mut requirements = vec![];
    let costs = &face.additional_costs;
    let cast_cost_groups = &face.cast_cost_groups;
    for (cost_index, cost) in costs.iter().enumerate() {
        match cost {
            AdditionalCost::Blight { count } => {
                choices.push(eng.blight_cost_choice(player, cost_index, *count))
            }
            AdditionalCost::DiscardCard => {
                let candidates: Vec<(u32, ObjectId)> = eng.state.players[player_idx]
                    .hand
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(_, oid)| *oid != source)
                    .map(|(slot, oid)| (slot as u32, oid))
                    .collect();
                requirements.push(ObjectPaymentRequirement::Exact {
                    candidates: candidates.iter().map(|(_, oid)| *oid).collect(),
                    count: 1,
                });
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Hand as i32,
                    candidate_ids: candidates.into_iter().map(|(slot, _)| slot).collect(),
                    min: 1,
                    max: 1,
                    blight_count: 0,
                    counter_removal: None,
                    kind: rv1::CostChoiceKind::Discard as i32,
                    candidate_objects: vec![],
                    aggregate_minimum: None,
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
                requirements.push(ObjectPaymentRequirement::Exact {
                    candidates: candidate_ids.clone(),
                    count: 1,
                });
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Battlefield as i32,
                    candidate_ids: candidate_ids.clone(),
                    min: 1,
                    max: 1,
                    blight_count: 0,
                    counter_removal: None,
                    kind: rv1::CostChoiceKind::Sacrifice as i32,
                    candidate_objects: candidate_ids
                        .iter()
                        .map(|oid| cost_object_candidate(eng, *oid, 0))
                        .collect(),
                    aggregate_minimum: None,
                });
            }
            AdditionalCost::TapPermanents {
                constraint,
                filter,
                exclude_source,
            } => {
                let candidate_ids: Vec<ObjectId> = eng
                    .state
                    .players
                    .iter()
                    .flat_map(|state| state.battlefield.iter().copied())
                    .filter(|oid| !exclude_source || *oid != source)
                    .filter(|&oid| {
                        eng.ability_cost_permanent_matches(player, None, oid, filter)
                            && !eng.state.objects[&oid].tapped
                    })
                    .collect();
                match *constraint {
                    ObjectPaymentConstraint::ExactCount(count) => {
                        requirements.push(ObjectPaymentRequirement::Exact {
                            candidates: candidate_ids.clone(),
                            count,
                        });
                    }
                    ObjectPaymentConstraint::AggregateMinimum {
                        minimum,
                        contribution,
                    } => requirements.push(ObjectPaymentRequirement::Aggregate {
                        candidates: candidate_ids
                            .iter()
                            .filter_map(|oid| {
                                eng.object_payment_contribution(*oid, contribution)
                                    .map(|value| (*oid, value))
                            })
                            .collect(),
                        minimum,
                    }),
                }
                let exact_count = constraint.exact_count();
                let contribution_kind = constraint.aggregate_minimum().map(|(_, kind)| kind);
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Battlefield as i32,
                    candidate_ids: candidate_ids.clone(),
                    min: exact_count.unwrap_or(0),
                    max: exact_count.unwrap_or(candidate_ids.len() as u32),
                    blight_count: 0,
                    counter_removal: None,
                    kind: rv1::CostChoiceKind::Tap as i32,
                    candidate_objects: candidate_ids
                        .iter()
                        .map(|oid| {
                            cost_object_candidate(
                                eng,
                                *oid,
                                contribution_kind
                                    .and_then(|kind| eng.object_payment_contribution(*oid, kind))
                                    .unwrap_or(0),
                            )
                        })
                        .collect(),
                    aggregate_minimum: aggregate_constraint_proto(*constraint),
                });
            }
            AdditionalCost::ExileGraveyardCards {
                constraint,
                filter,
                exclude_source,
            } => {
                let candidate_ids: Vec<ObjectId> = eng.state.players[player_idx]
                    .graveyard
                    .iter()
                    .copied()
                    .filter(|oid| !exclude_source || *oid != source)
                    .filter(|oid| {
                        super::card_predicates::zone_card_matches_filter(
                            &eng.state,
                            eng.registry,
                            *oid,
                            Some(filter),
                        )
                    })
                    .collect();
                match *constraint {
                    ObjectPaymentConstraint::ExactCount(count) => {
                        requirements.push(ObjectPaymentRequirement::Exact {
                            candidates: candidate_ids.clone(),
                            count,
                        });
                    }
                    ObjectPaymentConstraint::AggregateMinimum {
                        minimum,
                        contribution,
                    } => requirements.push(ObjectPaymentRequirement::Aggregate {
                        candidates: candidate_ids
                            .iter()
                            .filter_map(|oid| {
                                eng.object_payment_contribution(*oid, contribution)
                                    .map(|value| (*oid, value))
                            })
                            .collect(),
                        minimum,
                    }),
                }
                let exact_count = constraint.exact_count();
                let contribution_kind = constraint.aggregate_minimum().map(|(_, kind)| kind);
                choices.push(rv1::LegalCostChoice {
                    cost_index: cost_index as u32,
                    zone: rv1::CostChoiceZone::Graveyard as i32,
                    candidate_ids: candidate_ids.clone(),
                    min: exact_count.unwrap_or(0),
                    max: exact_count.unwrap_or(candidate_ids.len() as u32),
                    blight_count: 0,
                    counter_removal: None,
                    kind: rv1::CostChoiceKind::Exile as i32,
                    candidate_objects: candidate_ids
                        .iter()
                        .map(|oid| {
                            cost_object_candidate(
                                eng,
                                *oid,
                                contribution_kind
                                    .and_then(|kind| eng.object_payment_contribution(*oid, kind))
                                    .unwrap_or(0),
                            )
                        })
                        .collect(),
                    aggregate_minimum: aggregate_constraint_proto(*constraint),
                });
            }
        }
    }
    if let Some(candidates) = sneak_candidates {
        requirements.push(ObjectPaymentRequirement::Exact {
            candidates: candidates.to_vec(),
            count: 1,
        });
        choices.push(rv1::LegalCostChoice {
            cost_index: costs.len() as u32,
            zone: rv1::CostChoiceZone::Battlefield as i32,
            candidate_ids: candidates.to_vec(),
            min: 1,
            max: 1,
            blight_count: 0,
            counter_removal: None,
            kind: rv1::CostChoiceKind::ReturnUnblockedAttacker as i32,
            candidate_objects: candidates
                .iter()
                .map(|oid| cost_object_candidate(eng, *oid, 0))
                .collect(),
            aggregate_minimum: None,
        });
    }
    let mut non_mana_costs_payable =
        payment_assignment_exists(&requirements, 0, &mut HashSet::new(), &mut HashMap::new())
            && choices
                .iter()
                .all(|choice| !choice.candidate_ids.is_empty());
    let legal_cast_cost_groups = cast_cost_groups
        .iter()
        .enumerate()
        .map(|(group_index, group)| {
            let options = group
                .options
                .iter()
                .enumerate()
                .map(|(option_index, option)| match option {
                    CastCostOptionDef::Mana {
                        option_id,
                        presentation,
                        cost,
                        ..
                    } => rv1::LegalCastCostOption {
                        option_index: option_index as u32,
                        label: option.fallback_label(),
                        kind: rv1::CastCostOptionKind::Mana as i32,
                        additional_mana_cost: cost.to_string(),
                        selectable: true,
                        presentation: Some(presentation_ref(
                            eng.registry,
                            card_id,
                            &face.face_id,
                            [
                                PresentationPath::Spell,
                                PresentationPath::CastCostGroup(&group.group_id),
                                PresentationPath::CastCostOption(option_id),
                            ],
                            presentation,
                            option.fallback_label(),
                        )),
                        ..Default::default()
                    },
                    CastCostOptionDef::Blight {
                        option_id,
                        presentation,
                        ..
                    } => {
                        let candidates = eng.blight_candidates(player);
                        rv1::LegalCastCostOption {
                            option_index: option_index as u32,
                            label: option.fallback_label(),
                            kind: rv1::CastCostOptionKind::Blight as i32,
                            selectable: !candidates.is_empty(),
                            presentation: Some(presentation_ref(
                                eng.registry,
                                card_id,
                                &face.face_id,
                                [
                                    PresentationPath::Spell,
                                    PresentationPath::CastCostGroup(&group.group_id),
                                    PresentationPath::CastCostOption(option_id),
                                ],
                                presentation,
                                option.fallback_label(),
                            )),
                            valid_permanent_generations: candidates
                                .iter()
                                .map(|oid| {
                                    eng.state
                                        .zone_change_generation
                                        .get(oid)
                                        .copied()
                                        .unwrap_or(0)
                                })
                                .collect(),
                            valid_permanent_ids: candidates,
                            ..Default::default()
                        }
                    }
                    CastCostOptionDef::Behold {
                        option_id,
                        presentation,
                        hand_filter,
                        permanent_filter,
                        ..
                    } => {
                        let valid_hand_indices = eng.state.players[player_idx]
                            .hand
                            .iter()
                            .copied()
                            .enumerate()
                            .filter(|(_, oid)| {
                                *oid != source
                                    && super::card_predicates::zone_card_matches_filter(
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
                            label: option.fallback_label(),
                            kind: rv1::CastCostOptionKind::Behold as i32,
                            additional_mana_cost: String::new(),
                            valid_hand_indices,
                            valid_permanent_ids,
                            valid_permanent_generations,
                            selectable,
                            presentation: Some(presentation_ref(
                                eng.registry,
                                card_id,
                                &face.face_id,
                                [
                                    PresentationPath::Spell,
                                    PresentationPath::CastCostGroup(&group.group_id),
                                    PresentationPath::CastCostOption(option_id),
                                ],
                                presentation,
                                option.fallback_label(),
                            )),
                            ..Default::default()
                        }
                    }
                    CastCostOptionDef::DiscardCard {
                        option_id,
                        presentation,
                    } => {
                        let valid_hand_indices = eng.state.players[player_idx]
                            .hand
                            .iter()
                            .enumerate()
                            .filter_map(|(slot, oid)| (*oid != source).then_some(slot as u32))
                            .collect::<Vec<_>>();
                        rv1::LegalCastCostOption {
                            option_index: option_index as u32,
                            label: option.fallback_label(),
                            kind: rv1::CastCostOptionKind::DiscardCard as i32,
                            selectable: !valid_hand_indices.is_empty(),
                            valid_hand_indices,
                            presentation: Some(presentation_ref(
                                eng.registry,
                                card_id,
                                &face.face_id,
                                [
                                    PresentationPath::Spell,
                                    PresentationPath::CastCostGroup(&group.group_id),
                                    PresentationPath::CastCostOption(option_id),
                                ],
                                presentation,
                                option.fallback_label(),
                            )),
                            ..Default::default()
                        }
                    }
                    CastCostOptionDef::PayLife {
                        option_id,
                        presentation,
                        amount,
                    } => rv1::LegalCastCostOption {
                        option_index: option_index as u32,
                        label: option.fallback_label(),
                        kind: rv1::CastCostOptionKind::PayLife as i32,
                        selectable: u32::try_from(eng.state.players[player_idx].life)
                            .is_ok_and(|life| life >= *amount),
                        presentation: Some(presentation_ref(
                            eng.registry,
                            card_id,
                            &face.face_id,
                            [
                                PresentationPath::Spell,
                                PresentationPath::CastCostGroup(&group.group_id),
                                PresentationPath::CastCostOption(option_id),
                            ],
                            presentation,
                            option.fallback_label(),
                        )),
                        ..Default::default()
                    },
                    CastCostOptionDef::TapPermanents {
                        option_id,
                        presentation,
                        constraint,
                        filter,
                        ..
                    } => {
                        let candidates = eng
                            .state
                            .players
                            .iter()
                            .flat_map(|state| state.battlefield.iter().copied())
                            .filter(|oid| {
                                eng.ability_cost_permanent_matches(player, None, *oid, filter)
                                    && eng
                                        .state
                                        .objects
                                        .get(oid)
                                        .is_some_and(|object| !object.tapped)
                            })
                            .collect::<Vec<_>>();
                        let selectable = match constraint {
                            ObjectPaymentConstraint::ExactCount(count) => {
                                candidates.len() >= *count as usize
                            }
                            ObjectPaymentConstraint::AggregateMinimum { minimum, .. } => {
                                candidates
                                    .iter()
                                    .filter_map(|oid| {
                                        eng.object_payment_contribution(
                                            *oid,
                                            ObjectContributionKind::CurrentPower,
                                        )
                                    })
                                    .filter(|value| *value > 0)
                                    .sum::<i64>()
                                    >= i64::from(*minimum)
                            }
                        };
                        let (object_min, object_max) = constraint
                            .exact_count()
                            .map_or((1, candidates.len() as u32), |count| (count, count));
                        rv1::LegalCastCostOption {
                            option_index: option_index as u32,
                            label: option.fallback_label(),
                            kind: rv1::CastCostOptionKind::TapPermanents as i32,
                            selectable,
                            candidate_objects: candidates
                                .iter()
                                .map(|oid| {
                                    cost_object_candidate(
                                        eng,
                                        *oid,
                                        eng.object_payment_contribution(
                                            *oid,
                                            ObjectContributionKind::CurrentPower,
                                        )
                                        .unwrap_or(0),
                                    )
                                })
                                .collect(),
                            object_min,
                            object_max,
                            aggregate_minimum: aggregate_constraint_proto(*constraint),
                            presentation: Some(presentation_ref(
                                eng.registry,
                                card_id,
                                &face.face_id,
                                [
                                    PresentationPath::Spell,
                                    PresentationPath::CastCostGroup(&group.group_id),
                                    PresentationPath::CastCostOption(option_id),
                                ],
                                presentation,
                                option.fallback_label(),
                            )),
                            ..Default::default()
                        }
                    }
                    CastCostOptionDef::SacrificePermanent {
                        option_id,
                        presentation,
                        filter,
                        ..
                    } => {
                        let candidates = eng
                            .state
                            .players
                            .iter()
                            .flat_map(|state| state.battlefield.iter().copied())
                            .filter(|oid| {
                                eng.ability_cost_permanent_matches(player, None, *oid, filter)
                            })
                            .collect::<Vec<_>>();
                        rv1::LegalCastCostOption {
                            option_index: option_index as u32,
                            label: option.fallback_label(),
                            kind: rv1::CastCostOptionKind::SacrificePermanent as i32,
                            selectable: !candidates.is_empty(),
                            candidate_objects: candidates
                                .iter()
                                .map(|oid| cost_object_candidate(eng, *oid, 0))
                                .collect(),
                            object_min: 1,
                            object_max: 1,
                            presentation: Some(presentation_ref(
                                eng.registry,
                                card_id,
                                &face.face_id,
                                [
                                    PresentationPath::Spell,
                                    PresentationPath::CastCostGroup(&group.group_id),
                                    PresentationPath::CastCostOption(option_id),
                                ],
                                presentation,
                                option.fallback_label(),
                            )),
                            ..Default::default()
                        }
                    }
                })
                .collect::<Vec<_>>();
            if options.iter().filter(|option| option.selectable).count() < group.min as usize {
                non_mana_costs_payable = false;
            }
            rv1::LegalCastCostGroup {
                group_index: group_index as u32,
                prompt: group.fallback_prompt(),
                min: group.min,
                max: group.max,
                options,
                skip_label: String::new(),
                presentation: Some(presentation_ref(
                    eng.registry,
                    card_id,
                    &face.face_id,
                    [
                        PresentationPath::Spell,
                        PresentationPath::CastCostGroup(&group.group_id),
                    ],
                    &group.presentation,
                    group.fallback_prompt(),
                )),
            }
        })
        .collect();
    rv1::LegalCostChoices {
        non_mana_costs_payable,
        choices,
        cast_cost_groups: legal_cast_cost_groups,
    }
}

fn harmonize_cast_cost_group(
    eng: &GameEngine,
    player: PlayerId,
    group_index: usize,
) -> rv1::LegalCastCostGroup {
    let candidates = eng
        .state
        .players
        .iter()
        .flat_map(|state| state.battlefield.iter().copied())
        .filter_map(|oid| {
            let object = eng.state.objects.get(&oid)?;
            let characteristics = eng.characteristics(oid)?;
            (object.zone == Zone::Battlefield
                && !object.tapped
                && characteristics.controller == player
                && characteristics.is_creature())
            .then_some((
                oid,
                eng.state
                    .zone_change_generation
                    .get(&oid)
                    .copied()
                    .unwrap_or(0),
                characteristics.power.unwrap_or(0),
            ))
        })
        .collect::<Vec<_>>();
    rv1::LegalCastCostGroup {
        group_index: group_index as u32,
        prompt: "Harmonize: you may tap an untapped creature you control to reduce the generic cost by its power."
            .to_string(),
        min: 0,
        max: 1,
        options: vec![rv1::LegalCastCostOption {
            option_index: 0,
            label: "Tap a creature".to_string(),
            kind: rv1::CastCostOptionKind::TapPermanentForGenericReduction as i32,
            valid_permanent_ids: candidates.iter().map(|(oid, _, _)| *oid).collect(),
            valid_permanent_generations: candidates
                .iter()
                .map(|(_, generation, _)| *generation)
                .collect(),
            valid_permanent_generic_reductions: candidates
                .iter()
                .map(|(_, _, power)| *power)
                .collect(),
            selectable: !candidates.is_empty(),
            ..Default::default()
        }],
        skip_label: "Pay full Harmonize cost".to_string(),
        presentation: None,
    }
}

fn legal_mode_cast_cost_link(
    face: &CardFace,
    costs: &rv1::LegalCostChoices,
    mode: &tricerules_cards::ModeDef,
) -> Option<(rv1::LinkedCastCostOption, bool)> {
    legal_cast_cost_link(face, costs, mode.linked_cast_cost.as_ref()?)
}

fn legal_cast_cost_link(
    face: &CardFace,
    costs: &rv1::LegalCostChoices,
    link: &CastCostOptionRef,
) -> Option<(rv1::LinkedCastCostOption, bool)> {
    let group_index = face
        .cast_cost_groups
        .iter()
        .position(|group| group.group_id == link.group_id)?;
    let option_index = face.cast_cost_groups[group_index]
        .options
        .iter()
        .position(|option| option.option_id() == &link.option_id)?;
    let selectable = costs
        .cast_cost_groups
        .iter()
        .find(|group| group.group_index == group_index as u32)?
        .options
        .iter()
        .find(|option| option.option_index == option_index as u32)?
        .selectable;
    Some((
        rv1::LinkedCastCostOption {
            group_index: group_index as u32,
            option_index: option_index as u32,
        },
        selectable,
    ))
}

fn legal_all_modes_cast_cost(
    face: &CardFace,
    costs: &rv1::LegalCostChoices,
    modal: &ModalDef,
) -> (u32, Option<rv1::LinkedCastCostOption>) {
    let Some(link) = &modal.all_modes_cast_cost else {
        return (modal.max_modes, None);
    };
    match legal_cast_cost_link(face, costs, link) {
        Some((link, true)) => (modal.max_modes, Some(link)),
        _ => (
            modal
                .max_modes
                .min(modal.modes.len().saturating_sub(1) as u32),
            None,
        ),
    }
}

fn apply_cast_cost_target_requirements(
    eng: &GameEngine,
    player: PlayerId,
    source: TargetSourceIdentity,
    face: &CardFace,
    costs: &rv1::LegalCostChoices,
    targeting: Option<&TargetingDef>,
    targets: &mut rv1::SpellTargets,
) {
    let Some(targeting) = targeting else {
        return;
    };
    for (group_index, authored_group) in targeting.groups.iter().enumerate() {
        let Some(expansion) = &authored_group.cast_cost_expansion else {
            continue;
        };
        let link = CastCostOptionRef {
            group_id: expansion.condition.group_id.clone(),
            option_id: expansion.condition.option_id.clone(),
        };
        let linked = legal_cast_cost_link(face, costs, &link);
        let Some(group) = targets
            .groups
            .iter_mut()
            .find(|group| group.group_index == group_index as u32)
        else {
            continue;
        };
        let affected = group
            .valid_permanent_ids
            .iter()
            .copied()
            .filter(|oid| {
                !target_filter_legal_at_resolution(
                    eng,
                    &expansion.without_cost,
                    *oid,
                    player,
                    source,
                    TriggerContext::default(),
                )
            })
            .collect::<Vec<_>>();
        match linked {
            Some((required_cost, true)) if !affected.is_empty() => {
                targets
                    .cast_cost_requirements
                    .push(rv1::TargetCastCostRequirement {
                        group_index: group_index as u32,
                        required_cost: Some(required_cost),
                        affected_targets: affected
                            .into_iter()
                            .map(|object_id| rv1::TargetCandidateRef {
                                kind: rv1::TargetRefKind::Permanent as i32,
                                object_id,
                            })
                            .collect(),
                    });
            }
            _ => group
                .valid_permanent_ids
                .retain(|oid| !affected.contains(oid)),
        }
    }
}

fn cast_cost_timing_condition_available(
    face: &CardFace,
    costs: &rv1::LegalCostChoices,
    condition: &CastCostReceiptCondition,
) -> bool {
    let Some((group_index, option_index)) = face
        .cast_cost_groups
        .iter()
        .position(|group| group.group_id == condition.group_id)
        .and_then(|group_index| {
            face.cast_cost_groups[group_index]
                .options
                .iter()
                .position(|option| option.option_id() == &condition.option_id)
                .map(|option_index| (group_index as u32, option_index as u32))
        })
    else {
        return false;
    };
    let Some(group) = costs
        .cast_cost_groups
        .iter()
        .find(|group| group.group_index == group_index)
    else {
        return false;
    };
    let selected_available = group
        .options
        .iter()
        .any(|option| option.option_index == option_index && option.selectable);
    if condition.expected_selected {
        selected_available
    } else {
        group.min == 0
            || group
                .options
                .iter()
                .any(|option| option.option_index != option_index && option.selectable)
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
                .as_ref()
                .is_some_and(|condition| {
                    cast_cost_timing_condition_available(face, costs, condition)
                }))
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
        has_convoke: false,
        cast_method: rv1::CastMethod::Normal as i32,
        all_modes_cast_cost: None,
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

/// Number of distinct faces the engine is currently publishing for this exact physical source.
/// Alternative methods such as Warp may publish several actions for one face, but stack display
/// names the chosen face only when the card actually offered more than one face.
pub(super) fn cast_face_count_for_source(
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
            .map(|action| action.face_index)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        rv1::cast_source::Location::GraveyardObjectId(object_id)
        | rv1::cast_source::Location::ExileObjectId(object_id) => legal_zone_cast_actions(eng, pid)
            .iter()
            .filter(|action| action.object_id == *object_id)
            .map(|action| action.face_index)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
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
            let cost_choices = legal_spell_cost_choices(eng, pid, oid, card_id, face, None);
            let cast_ok = face_cast_timing_available(face, &cost_choices, instant_ok, sorcery_ok);
            let sneak_candidates = eng.sneak_return_candidates(pid);
            let sneak_cost_choices = face.sneak_cost.as_ref().map(|_| {
                legal_spell_cost_choices(eng, pid, oid, card_id, face, Some(&sneak_candidates))
            });
            let sneak_ok = sneak_cost_choices
                .as_ref()
                .is_some_and(|choices| choices.non_mana_costs_payable);
            if cast_ok || sneak_ok {
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
                action.has_convoke = face.keywords.contains(&Keyword::Convoke);
                action.generic_cost_reduction =
                    eng.spell_generic_reduction(pid, oid, face, &face.cost_modifiers);
                if !cost_choices.non_mana_costs_payable {
                    continue;
                }
                let mode_cost_choices = cost_choices.clone();
                action.cost_choices = Some(cost_choices);
                if let Some(modal) = &face.modal_spell {
                    let (max_modes, all_modes_cost) =
                        legal_all_modes_cast_cost(face, &mode_cost_choices, modal);
                    action.all_modes_cast_cost = all_modes_cost;
                    action.min_modes = modal.min_modes;
                    action.max_modes = max_modes;
                    action.modes = modal
                        .modes
                        .iter()
                        .enumerate()
                        .map(|(mode_index, mode)| {
                            let needs_target =
                                target_schema(&mode.effects, mode.targeting.as_ref()).has_targets();
                            let mut targets = compute_spell_targets(
                                eng,
                                pid,
                                TargetSourceIdentity::spell_face(eng, oid, face_index),
                                &mode.effects,
                                mode.targeting.as_ref(),
                                &face.cost_modifiers,
                            );
                            apply_cast_cost_target_requirements(
                                eng,
                                pid,
                                TargetSourceIdentity::spell_face(eng, oid, face_index),
                                face,
                                &mode_cost_choices,
                                mode.targeting.as_ref(),
                                &mut targets,
                            );
                            let linked_cost =
                                legal_mode_cast_cost_link(face, &mode_cost_choices, mode);
                            let selectable = (!needs_target
                                || spell_targets_have_candidate(&targets))
                                && linked_cost
                                    .as_ref()
                                    .is_none_or(|(_, selectable)| *selectable);
                            rv1::LegalSpellMode {
                                mode_index: mode_index as u32,
                                label: mode_fallback(&face.name, &mode.mode_id),
                                selectable,
                                needs_target,
                                targets: Some(targets),
                                presentation: Some(presentation_ref(
                                    eng.registry,
                                    card_id,
                                    &face.face_id,
                                    [
                                        PresentationPath::Spell,
                                        PresentationPath::Mode(&mode.mode_id),
                                    ],
                                    &mode.presentation,
                                    mode_fallback(&face.name, &mode.mode_id),
                                )),
                                linked_cast_cost: linked_cost.map(|(link, _)| link),
                            }
                        })
                        .collect();
                    let selectable_count =
                        action.modes.iter().filter(|mode| mode.selectable).count();
                    if selectable_count < modal.min_modes as usize
                        || action.max_modes < action.min_modes
                    {
                        continue;
                    }
                }
                let warp = face.warp_cost.as_ref().map(|cost| {
                    let mut warp = action.clone();
                    warp.cast_method = rv1::CastMethod::Warp as i32;
                    warp.cost = cost.to_string();
                    warp
                });
                let sneak = face.sneak_cost.as_ref().and_then(|cost| {
                    let choices =
                        sneak_cost_choices.filter(|choices| choices.non_mana_costs_payable)?;
                    let mut sneak = action.clone();
                    sneak.cast_method = rv1::CastMethod::Sneak as i32;
                    sneak.cost = cost.to_string();
                    sneak.cost_choices = Some(choices);
                    Some(sneak)
                });
                if cast_ok {
                    actions.push(action);
                    actions.extend(warp);
                }
                actions.extend(sneak);
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
            if face.is_land {
                continue;
            }
            let methods = [
                (rv1::CastMethod::Flashback, face.flashback_cost.as_ref()),
                (rv1::CastMethod::Harmonize, face.harmonize_cost.as_ref()),
            ];
            for (cast_method, method_cost) in methods {
                let Some(method_cost) = method_cost else {
                    continue;
                };
                let mut cost_choices = legal_spell_cost_choices(eng, pid, oid, card_id, face, None);
                if cast_method == rv1::CastMethod::Harmonize {
                    cost_choices
                        .cast_cost_groups
                        .push(harmonize_cast_cost_group(
                            eng,
                            pid,
                            face.cast_cost_groups.len(),
                        ));
                }
                let cast_ok =
                    face_cast_timing_available(face, &cost_choices, instant_ok, sorcery_ok);
                if !cast_ok {
                    continue;
                }
                let mut action = rv1::LegalZoneCastAction {
                    source_zone: rv1::CastSourceZone::Graveyard as i32,
                    object_id: oid,
                    zone_change_generation: eng
                        .state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                    card_name: face.name.clone(),
                    face_index: face_index as u32,
                    needs_target: target_schema(&face.spell_effect, face.targeting.as_ref())
                        .has_targets(),
                    min_modes: 0,
                    max_modes: 0,
                    modes: vec![],
                    cost: method_cost.to_string(),
                    cost_choices: None,
                    eligible_restricted_mana_group_ids: eng
                        .eligible_restricted_mana_for_spell(player_index, face),
                    generic_cost_reduction: eng.spell_generic_reduction(
                        pid,
                        oid,
                        face,
                        &face.cost_modifiers,
                    ),
                    cast_method: cast_method as i32,
                    has_convoke: face.keywords.contains(&Keyword::Convoke),
                    casting_permission_id: None,
                    all_modes_cast_cost: None,
                };
                if !cost_choices.non_mana_costs_payable {
                    continue;
                }
                let mode_cost_choices = cost_choices.clone();
                action.cost_choices = Some(cost_choices);
                if let Some(modal) = &face.modal_spell {
                    let (max_modes, all_modes_cost) =
                        legal_all_modes_cast_cost(face, &mode_cost_choices, modal);
                    action.all_modes_cast_cost = all_modes_cost;
                    action.min_modes = modal.min_modes;
                    action.max_modes = max_modes;
                    action.modes = modal
                        .modes
                        .iter()
                        .enumerate()
                        .map(|(mode_index, mode)| {
                            let needs_target =
                                target_schema(&mode.effects, mode.targeting.as_ref()).has_targets();
                            let mut targets = compute_spell_targets(
                                eng,
                                pid,
                                TargetSourceIdentity::spell_face(eng, oid, face_index),
                                &mode.effects,
                                mode.targeting.as_ref(),
                                &face.cost_modifiers,
                            );
                            apply_cast_cost_target_requirements(
                                eng,
                                pid,
                                TargetSourceIdentity::spell_face(eng, oid, face_index),
                                face,
                                &mode_cost_choices,
                                mode.targeting.as_ref(),
                                &mut targets,
                            );
                            let linked_cost =
                                legal_mode_cast_cost_link(face, &mode_cost_choices, mode);
                            let selectable = (!needs_target
                                || spell_targets_have_candidate(&targets))
                                && linked_cost
                                    .as_ref()
                                    .is_none_or(|(_, selectable)| *selectable);
                            rv1::LegalSpellMode {
                                mode_index: mode_index as u32,
                                label: mode_fallback(&face.name, &mode.mode_id),
                                selectable,
                                needs_target,
                                targets: Some(targets),
                                presentation: Some(presentation_ref(
                                    eng.registry,
                                    card_id,
                                    &face.face_id,
                                    [
                                        PresentationPath::Spell,
                                        PresentationPath::Mode(&mode.mode_id),
                                    ],
                                    &mode.presentation,
                                    mode_fallback(&face.name, &mode.mode_id),
                                )),
                                linked_cast_cost: linked_cost.map(|(link, _)| link),
                            }
                        })
                        .collect();
                    if action.modes.iter().filter(|mode| mode.selectable).count()
                        < modal.min_modes as usize
                        || action.max_modes < action.min_modes
                    {
                        continue;
                    }
                }
                actions.push(action);
            }
        }
    }

    let mut emitted = BTreeSet::new();
    for permission in eng
        .state
        .active_exile_play_permissions
        .iter()
        .filter(|permission| {
            permission.player_id == pid && permission.available_on_turn(eng.state.turn_instance)
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
        let face_indices: Vec<_> = match permission.scope {
            ExilePlayPermissionScope::CastFace(face_index) => vec![face_index],
            ExilePlayPermissionScope::PlayCard => (0..definition.faces_iter().count()).collect(),
            ExilePlayPermissionScope::CastCard => (0..definition.faces_iter().count())
                .filter(|&face| definition.face_available_from_hand(face))
                .collect(),
        };
        for face_index in face_indices {
            if !emitted.insert((object.id, face_index, permission.group_id)) {
                continue;
            }
            let Some(face) = definition.face(face_index) else {
                continue;
            };
            if face.is_land {
                continue;
            }
            let cost_choices =
                legal_spell_cost_choices(eng, pid, object.id, &object.card_id, face, None);
            let cast_ok = face_cast_timing_available(face, &cost_choices, instant_ok, sorcery_ok);
            if !cast_ok {
                continue;
            }
            let (cost, cast_method) = match &permission.cast_cost {
                crate::state::ExilePermissionCastCost::PrintedManaCost => {
                    (face.mana_cost.to_string(), rv1::CastMethod::Normal)
                }
                crate::state::ExilePermissionCastCost::AlternativeManaCost(cost) => {
                    (cost.to_string(), rv1::CastMethod::Permission)
                }
            };
            let mut action = rv1::LegalZoneCastAction {
                source_zone: rv1::CastSourceZone::Exile as i32,
                object_id: object.id,
                zone_change_generation: generation,
                card_name: face.name.clone(),
                face_index: face_index as u32,
                needs_target: target_schema(&face.spell_effect, face.targeting.as_ref())
                    .has_targets(),
                min_modes: 0,
                max_modes: 0,
                modes: vec![],
                cost,
                cost_choices: None,
                eligible_restricted_mana_group_ids: eng
                    .eligible_restricted_mana_for_spell(player_index, face),
                generic_cost_reduction: eng.spell_generic_reduction(
                    pid,
                    object.id,
                    face,
                    &face.cost_modifiers,
                ),
                cast_method: cast_method as i32,
                has_convoke: face.keywords.contains(&Keyword::Convoke),
                casting_permission_id: Some(permission.group_id),
                all_modes_cast_cost: None,
            };
            if !cost_choices.non_mana_costs_payable {
                continue;
            }
            let mode_cost_choices = cost_choices.clone();
            action.cost_choices = Some(cost_choices);
            if let Some(modal) = &face.modal_spell {
                let (max_modes, all_modes_cost) =
                    legal_all_modes_cast_cost(face, &mode_cost_choices, modal);
                action.all_modes_cast_cost = all_modes_cost;
                action.min_modes = modal.min_modes;
                action.max_modes = max_modes;
                action.modes = modal
                    .modes
                    .iter()
                    .enumerate()
                    .map(|(mode_index, mode)| {
                        let needs_target =
                            target_schema(&mode.effects, mode.targeting.as_ref()).has_targets();
                        let mut targets = compute_spell_targets(
                            eng,
                            pid,
                            TargetSourceIdentity::current(eng, object.id),
                            &mode.effects,
                            mode.targeting.as_ref(),
                            &face.cost_modifiers,
                        );
                        apply_cast_cost_target_requirements(
                            eng,
                            pid,
                            TargetSourceIdentity::current(eng, object.id),
                            face,
                            &mode_cost_choices,
                            mode.targeting.as_ref(),
                            &mut targets,
                        );
                        let linked_cost = legal_mode_cast_cost_link(face, &mode_cost_choices, mode);
                        rv1::LegalSpellMode {
                            mode_index: mode_index as u32,
                            label: mode_fallback(&face.name, &mode.mode_id),
                            selectable: (!needs_target || spell_targets_have_candidate(&targets))
                                && linked_cost
                                    .as_ref()
                                    .is_none_or(|(_, selectable)| *selectable),
                            needs_target,
                            targets: Some(targets),
                            presentation: Some(presentation_ref(
                                eng.registry,
                                &object.card_id,
                                &face.face_id,
                                [
                                    PresentationPath::Spell,
                                    PresentationPath::Mode(&mode.mode_id),
                                ],
                                &mode.presentation,
                                mode_fallback(&face.name, &mode.mode_id),
                            )),
                            linked_cast_cost: linked_cost.map(|(link, _)| link),
                        }
                    })
                    .collect();
                if action.modes.iter().filter(|mode| mode.selectable).count()
                    < modal.min_modes as usize
                    || action.max_modes < action.min_modes
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
            if !definition.face_available_from_hand(face_index)
                || !face.is_land
                || !emitted.insert((object.id, face_index))
            {
                continue;
            }
            actions.push(rv1::LegalZoneLandAction {
                source_zone: rv1::CastSourceZone::Exile as i32,
                object_id: object.id,
                card_name: face.name.clone(),
                face_index: face_index as u32,
                zone_change_generation: generation,
            });
        }
    }
    if eng.can_play_lands_from_own_graveyard(pid) {
        let Some(player_index) = eng.state.player_idx(pid) else {
            return actions;
        };
        for object_id in &eng.state.players[player_index].graveyard {
            let Some(object) = eng.state.objects.get(object_id) else {
                continue;
            };
            if object.zone != Zone::Graveyard || object.owner != pid {
                continue;
            }
            let Some(definition) = eng.registry.get(&object.card_id) else {
                continue;
            };
            if !definition.matches_card_type_outside_stack(CardTypeFilter::Land) {
                continue;
            }
            let generation = eng
                .state
                .zone_change_generation
                .get(object_id)
                .copied()
                .unwrap_or(0);
            for (face_index, face) in definition.faces_iter().enumerate() {
                if !definition.face_available_from_hand(face_index)
                    || !face.is_land
                    || !emitted.insert((object.id, face_index))
                {
                    continue;
                }
                actions.push(rv1::LegalZoneLandAction {
                    source_zone: rv1::CastSourceZone::Graveyard as i32,
                    object_id: object.id,
                    card_name: face.name.clone(),
                    face_index: face_index as u32,
                    zone_change_generation: generation,
                });
            }
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
                    let costs = legal_spell_cost_choices(eng, pid, oid, cid, face, None);
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
        let group_id = tricerules_cards::ChoiceId::new("cast_cost_01").unwrap();
        let option_id = tricerules_cards::ChoiceId::new("option_01").unwrap();
        let face = CardFace {
            is_sorcery: true,
            cast_cost_groups: vec![CastCostGroupDef {
                group_id: group_id.clone(),
                presentation: tricerules_cards::AbilityPresentation::Fallback,
                min: 0,
                max: 1,
                options: vec![CastCostOptionDef::Mana {
                    option_id: option_id.clone(),
                    presentation: tricerules_cards::AbilityPresentation::Fallback,
                    kind: tricerules_cards::primitives::ManaCostChoiceKind::AdditionalPayment,
                    cost: ManaCost::parse("{1}").unwrap(),
                }],
            }],
            instant_speed_cast_cost: Some(CastCostReceiptCondition {
                group_id,
                option_id,
                expected_selected: true,
            }),
            ..Default::default()
        };
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
