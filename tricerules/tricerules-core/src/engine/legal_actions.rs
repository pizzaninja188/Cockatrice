use super::casting::castable_at_instant_speed;
use super::combat::priority_locked_for_combat_declaration;
use super::events::object_display_name;
use super::priority::{instant_timing_step_allowed, sorcery_speed_available};
use super::targeting::{compute_spell_targets, spell_effect_kind_needs_target};
use super::*;

pub(super) fn fill_legal(batch: &mut RuledEventBatch, eng: &GameEngine) {
    for p in &eng.state.players {
        let labels = legal_labels(eng, p.id);
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
                    let t =
                        compute_spell_targets(&eng.state, eng.registry, p.id, face.spell_effect);
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
                        let targets = compute_spell_targets(
                            &eng.state,
                            eng.registry,
                            p.id,
                            std::slice::from_ref(&ability.effect),
                        );
                        let key = (poid as u64) << 32 | ai as u64;
                        valid_targets_by_ability.insert(key, targets);
                    }
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
            },
        );
    }
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
                let name = face.name;
                if face.is_land {
                    if sorcery_ok && !eng.state.land_dropped_this_turn {
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
