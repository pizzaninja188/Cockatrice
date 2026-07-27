use super::casting::castable_at_instant_speed;
use super::combat::priority_locked_for_combat_declaration;
use super::events::object_display_name;
use super::priority::{instant_timing_step_allowed, sorcery_speed_available};
use super::targeting::{compute_spell_targets, spell_effect_kind_needs_target};
use super::*;

pub(super) fn fill_legal(batch: &mut RuledEventBatch, eng: &GameEngine) {
    for p in &eng.state.players {
        let labels = legal_labels(eng, p.id);
        let hand_actions = legal_hand_actions(eng, p.id);
        let mut valid_targets_by_hand_slot = BTreeMap::new();
        let mut valid_targets_by_ability = BTreeMap::new();

        if let Some(idx) = eng.state.player_idx(p.id) {
            for (slot, &oid) in eng.state.players[idx].hand.iter().enumerate() {
                let Some(obj) = eng.state.objects.get(&oid) else {
                    continue;
                };
                let Some(def) = eng.registry.get(&obj.card_id) else {
                    continue;
                };
                for (face_index, face) in def.faces_iter().enumerate() {
                    if face.is_land || !face.spell_effect.iter().any(spell_effect_kind_needs_target)
                    {
                        continue;
                    }
                    let t = compute_spell_targets(eng, p.id, &face.spell_effect);
                    let key = (slot as u32) << 8 | face_index as u32;
                    valid_targets_by_hand_slot.insert(key, t);
                }
            }

            for &poid in &eng.state.players[idx].battlefield {
                let Some(pobj) = eng.state.objects.get(&poid) else {
                    continue;
                };
                let Some(pdef) = eng.registry.get(&pobj.card_id) else {
                    continue;
                };
                // CR 712.4: read abilities from the active face (face_up_index) so multi-face
                // permanents expose the correct ability set when on the battlefield.
                let Some(face) = pdef.face(pobj.face_up_index) else {
                    continue;
                };
                for (ai, ability) in face.activated_abilities.iter().enumerate() {
                    if spell_effect_kind_needs_target(&ability.effect) {
                        let targets =
                            compute_spell_targets(eng, p.id, std::slice::from_ref(&ability.effect));
                        let key = (poid as u64) << 32 | ai as u64;
                        valid_targets_by_ability.insert(key, targets);
                    }
                }
            }
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
                if let Some(effect) = eng
                    .registry
                    .get(&pt.card_id)
                    .and_then(|def| def.primary_face().triggered_abilities.get(pt.ability_index))
                    .map(|ta| &ta.effect)
                {
                    let targets = compute_spell_targets(eng, p.id, std::slice::from_ref(effect));
                    let key = (pt.source_permanent_id as u64) << 32 | pt.ability_index as u64;
                    valid_targets_by_ability.insert(key, targets);
                }
            }
        }

        let undoable_mana_abilities = eng
            .state
            .undoable_mana_abilities
            .iter()
            .filter(|e| e.player == p.id)
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
        let blocks_open = eng.state.turn_step == TurnStep::DeclareBlockers
            && !combat.map(|c| c.blockers_declared).unwrap_or(false);
        let required_blocker_ids =
            if blocks_open && Some(p.id) == eng.state.defending_player_id_1v1() {
                eng.required_blocker_ids()
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
            },
        );
    }
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
    if eng.state.pending_resolution.is_some() || eng.state.pending_triggers.front().is_some() {
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
    if eng.state.turn_step == TurnStep::Cleanup {
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

    let instant_ok = instant_timing_step_allowed(eng.state.turn_step);
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
            let cast_ok = if castable_at_instant_speed(&face) {
                instant_ok
            } else {
                sorcery_ok
            };
            if cast_ok {
                actions.push(hand_action(
                    rv1::HandActionKind::HandActionCastSpell,
                    hand_index,
                    &face.name,
                    face_index,
                    face.spell_effect.iter().any(spell_effect_kind_needs_target),
                ));
            }
        }
    }
    actions
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
            vec![format!("Resolve: {}", pr.prompt)]
        } else {
            vec!["Waiting: opponent making a resolution choice".into()]
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
                    if blks.len() > 1 && !c.damage_assignments.contains_key(&att) {
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
    let instant_ok = instant_timing_step_allowed(eng.state.turn_step);
    let sorcery_ok = sorcery_speed_available(&eng.state, pid);
    let combat_decl_lock = priority_locked_for_combat_declaration(&eng.state);
    for (i, &oid) in eng.state.players[idx].hand.iter().enumerate() {
        let cid = &eng.state.objects.get(&oid).unwrap().card_id;
        if let Some(def) = eng.registry.get(cid) {
            for (face_index, face) in def.faces_iter().enumerate() {
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
                    let cast_ok = if castable_at_instant_speed(&face) {
                        instant_ok
                    } else {
                        sorcery_ok
                    };
                    if cast_ok {
                        let needs_target =
                            face.spell_effect.iter().any(spell_effect_kind_needs_target);
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
    // Transform right-click action for Transform/Flip layout permanents the priority player controls.
    if eng.state.priority_player_id() == pid {
        for &poid in &eng.state.players[idx].battlefield {
            let Some(pobj) = eng.state.objects.get(&poid) else {
                continue;
            };
            let Some(pdef) = eng.registry.get(&pobj.card_id) else {
                continue;
            };
            if matches!(
                pdef.layout,
                tricerules_cards::Layout::Transform | tricerules_cards::Layout::Flip
            ) {
                let name = pdef.name.as_str();
                v.push(format!("Transform {name} (oid {poid})"));
            }
        }
    }
    v
}
