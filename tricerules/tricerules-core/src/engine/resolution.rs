use super::events::{color_string, ev_log, object_display_name};
use super::targeting::{
    battlefield_objects_matching, compute_spell_targets, spell_has_no_legal_targets_at_resolution,
};
use super::*;

impl GameEngine {
    pub(super) fn resolve_top_of_stack(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let top = self
            .state
            .stack
            .pop()
            .ok_or(EngineError::Illegal("empty stack"))?;
        let controller = top.controller;
        let card_id = top.card_id.clone();
        let targets = top.targets.clone();

        // Abilities — and spell copies (CR 707.10d) — leave no object behind when they resolve;
        // only a genuinely cast spell has a backing card that moves to a zone. A copy has no
        // `GameObject` in `objects`, so it must take the same no-zone-move path as an ability.
        let is_ability = top.ability_text.is_some();
        let leaves_no_object = is_ability || top.is_copy;
        if leaves_no_object {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                    object_id: top.id,
                    // Abilities cease to exist on resolution; graveyard tells the C++ server
                    // not to expect a permanent to land.
                    destination: rv1::StackResolveDestination::Graveyard as i32,
                })),
            });
        } else {
            // CR 709/712/715: permanence is the *cast face's* (Ice resolves to graveyard; an MDFC
            // permanent face resolves to the battlefield as that face).
            let resolves_to_battlefield_raw = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index))
                .map(|f| f.is_permanent())
                .unwrap_or(false);
            // CR 303.4f: an aura whose enchant target is no longer on the battlefield at resolution
            // is countered (goes to owner's graveyard) rather than entering the battlefield orphaned.
            let is_aura = resolves_to_battlefield_raw
                && self
                    .registry
                    .get(&card_id)
                    .map(|d| d.is_aura)
                    .unwrap_or(false);
            let aura_target_valid = !is_aura
                || targets.first().is_some_and(|&tid| {
                    self.state
                        .objects
                        .get(&tid)
                        .map(|o| o.zone == Zone::Battlefield)
                        .unwrap_or(false)
                });
            let resolves_to_battlefield = resolves_to_battlefield_raw && aura_target_valid;
            let destination = if resolves_to_battlefield {
                rv1::StackResolveDestination::Battlefield as i32
            } else {
                rv1::StackResolveDestination::Graveyard as i32
            };
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                    object_id: top.id,
                    destination,
                })),
            });
            move_object_to_zone(
                &mut self.state,
                top.id,
                if resolves_to_battlefield {
                    Zone::Battlefield
                } else {
                    Zone::Graveyard
                },
            )?;
            if resolves_to_battlefield {
                // Set attached_to before ETB triggers so emit_static_abilities_on_enter can read it.
                if is_aura {
                    if let (Some(aura_obj), Some(&enchanted_oid)) =
                        (self.state.objects.get_mut(&top.id), targets.first())
                    {
                        aura_obj.attached_to = Some(enchanted_oid);
                    }
                }
                self.fire_triggers(GameEvent::EntersBattlefield { object_id: top.id }, events);
            } else if is_aura {
                let aura_name = self
                    .registry
                    .get(&card_id)
                    .map(|d| d.name.as_str())
                    .unwrap_or("Aura");
                events.push(ev_log(format!(
                    "{aura_name} fizzles (enchant target left the battlefield)."
                )));
                return Ok(());
            }
        }

        // Determine effects: for spells use spell_effect (Vec); for abilities wrap the single
        // effect. Triggered and activated abilities are now uniform — both carry a plain
        // `SpellEffectKind` (self-referencing effects use a `Self_` target filter, bound below).
        let (effects, spell_label): (Vec<SpellEffectKind>, String) = if is_ability {
            let ability_index = top.ability_index.unwrap_or(0);
            let def = self.registry.get(&card_id);
            let name = def
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "Ability".into());
            let abilities = if top.is_triggered {
                def.map(|d| &d.triggered_abilities[..])
                    .and_then(|a| a.get(ability_index))
                    .map(|a| a.effect.clone())
            } else {
                def.and_then(|d| d.activated_abilities.get(ability_index))
                    .map(|a| a.effect.clone())
            };
            (vec![abilities.unwrap_or(SpellEffectKind::None)], name)
        } else {
            // CR 709/712/715: resolve the cast face's effects and show its name.
            let face = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index));
            let effects = face.map(|f| f.spell_effect.to_vec()).unwrap_or_default();
            let name = face
                .map(|f| f.name.to_string())
                .unwrap_or_else(|| "Spell".into());
            (effects, name)
        };

        // Tier-3 (CR 608): a custom effect owns this spell's resolution. The spell card has
        // already moved to its zone (graveyard/battlefield above); hand off the algorithm to the
        // registered `CardEffect`, which either completes now or parks awaiting a player choice.
        // A copy is excluded: the resumable custom machinery (`begin_custom_resolution`) expects the
        // spell's backing `GameObject`, which a copy lacks. Copying a tier-3 spell is a documented
        // limitation (the copy resolves its non-custom effects only, if any).
        if !is_ability && !top.is_copy {
            let custom_key = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index))
                .and_then(|f| f.custom_effect.map(str::to_string));
            if let Some(custom_key) = custom_key {
                return self.begin_custom_resolution(top, custom_key, events);
            }
        }

        let fizzle = spell_has_no_legal_targets_at_resolution(
            &self.state,
            self.registry,
            &effects,
            &targets,
            controller,
        );
        if fizzle {
            events.push(ev_log(format!("{spell_label} fizzles (no legal targets).")));
            return Ok(());
        }

        for effect in effects {
            match effect {
                SpellEffectKind::DamageTarget { amount, .. } => {
                    // CR 107.3: `amount` may be the cast-time X (Fireball) or a literal (Bolt).
                    let amount = amount.resolve(top.chosen_x);
                    if let Some(&tid) = targets.first() {
                        if let Some(pi) = self.state.player_idx(tid as i32) {
                            let pid = self.state.players[pi].id;
                            self.state.players[pi].life -= amount as i32;
                            events.push(rv1::RuledEvent {
                                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                    player_id: self.state.players[pi].id,
                                    new_total: self.state.players[pi].life,
                                    delta: -(amount as i32),
                                })),
                            });
                            events.push(ev_log(format!(
                                "{spell_label} deals {amount} damage to P{pid}"
                            )));
                        } else {
                            let tgt = object_display_name(&self.state, self.registry, tid);
                            if let Some(t) = self.state.objects.get_mut(&tid) {
                                if t.zone == Zone::Battlefield && t.is_creature(self.registry) {
                                    t.damage += amount;
                                    events.push(ev_log(format!(
                                        "{spell_label} deals {amount} damage to {tgt}"
                                    )));
                                }
                            }
                        }
                    }
                }
                SpellEffectKind::DamageTargets { target: filter, .. } => {
                    // CR 608.2b: skip targets that became illegal at resolution; apply damage
                    // to each remaining legal target using its allocated amount.
                    for (i, &tid) in targets.iter().enumerate() {
                        let damage_amount = top.target_damage.get(i).copied().unwrap_or(0);
                        if damage_amount == 0 {
                            continue;
                        }
                        if !super::targeting::target_filter_legal_at_resolution(
                            &self.state,
                            self.registry,
                            &filter,
                            tid,
                            controller,
                        ) {
                            events.push(ev_log(format!(
                                "{spell_label}: target {} is no longer legal, skipping.",
                                tid
                            )));
                            continue;
                        }
                        if let Some(pi) = self.state.player_idx(tid as i32) {
                            let pid = self.state.players[pi].id;
                            self.state.players[pi].life -= damage_amount as i32;
                            events.push(rv1::RuledEvent {
                                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                    player_id: pid,
                                    new_total: self.state.players[pi].life,
                                    delta: -(damage_amount as i32),
                                })),
                            });
                            events.push(ev_log(format!(
                                "{spell_label} deals {damage_amount} damage to P{pid}"
                            )));
                        } else {
                            let tgt = object_display_name(&self.state, self.registry, tid);
                            if let Some(t) = self.state.objects.get_mut(&tid) {
                                if t.zone == Zone::Battlefield && t.is_creature(self.registry) {
                                    t.damage += damage_amount;
                                    events.push(ev_log(format!(
                                        "{spell_label} deals {damage_amount} damage to {tgt}"
                                    )));
                                }
                            }
                        }
                    }
                }
                SpellEffectKind::Draw { count } => {
                    // Blue Sun's Zenith / Braingeyser: `count` may be the cast-time X.
                    let count = count.resolve(top.chosen_x);
                    let idx = self.state.player_idx(controller).unwrap();
                    // CR 120.3 / 104.3c: drawing from an empty library does NOT fail the spell —
                    // draw as many as possible, then the player loses as a state-based action
                    // (swept in by `sweep_life`). Aborting resolution here would corrupt state
                    // (cards already drawn, stack already popped).
                    let mut drawn = 0u32;
                    let mut decked_out = false;
                    for _ in 0..count {
                        if self.state.players[idx].library.is_empty() {
                            decked_out = true;
                            break;
                        }
                        draw_card(&mut self.state.players[idx], &mut self.state.objects)?;
                        drawn += 1;
                    }
                    let noun = if drawn == 1 { "card" } else { "cards" };
                    events.push(ev_log(format!(
                        "P{controller} draws {drawn} {noun} ({spell_label})."
                    )));
                    if decked_out {
                        self.state.players[idx].has_lost = true;
                        events.push(ev_log(format!(
                            "P{controller} tried to draw from an empty library and loses (CR 104.3c)."
                        )));
                    }
                }
                SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    target,
                } => {
                    // `Self_` is auto-bound to the source permanent (CR 115 — not a chosen target);
                    // every other filter uses the player's selected target.
                    let tid = if matches!(target.kind, TargetKind::Self_) {
                        top.source_permanent_id
                    } else {
                        targets.first().copied()
                    };
                    if let Some(tid) = tid {
                        let is_valid_target = self
                            .state
                            .objects
                            .get(&tid)
                            .map(|t| t.zone == Zone::Battlefield && t.is_creature(self.registry))
                            .unwrap_or(false);
                        if is_valid_target {
                            let tgt = object_display_name(&self.state, self.registry, tid);
                            self.state.continuous_effects.push(ContinuousEffect {
                                source_id: top.source_permanent_id,
                                affected: AffectedScope::Single(tid),
                                kind: ContinuousEffectKind::PtModify {
                                    delta_power: power,
                                    delta_toughness: toughness,
                                },
                                duration: EffectDuration::UntilEndOfTurn,
                                timestamp: self.state.command_index,
                            });
                            events.push(ev_log(format!(
                                "{spell_label} gives +{power}/+{toughness} to {tgt}"
                            )));
                        }
                    }
                }
                SpellEffectKind::PumpAll {
                    filter,
                    power,
                    toughness,
                } => {
                    // CR 613.4 layer 7c, one-shot: an UntilEndOfTurn continuous effect over the
                    // filtered creature set (controller resolved from the spell's controller).
                    // The resolving spell is the nominal source; it does not persist as a creature,
                    // so the scope drains at cleanup (UntilEndOfTurn), not at LTB.
                    self.state.continuous_effects.push(ContinuousEffect {
                        source_id: Some(top.id),
                        affected: resolve_anthem_scope(&filter, controller, top.id),
                        kind: ContinuousEffectKind::PtModify {
                            delta_power: power,
                            delta_toughness: toughness,
                        },
                        duration: EffectDuration::UntilEndOfTurn,
                        timestamp: self.state.command_index,
                    });
                    events.push(ev_log(format!(
                        "{spell_label} gives +{power}/+{toughness} to each affected creature"
                    )));
                }
                SpellEffectKind::GrantKeywordsAll { filter, keywords } => {
                    // CR 613 layer 6, one-shot: add a Layer6AddKeyword continuous effect for each
                    // granted keyword. Overrun → Trample; Trumpet Blast → First Strike; etc.
                    let scope = resolve_anthem_scope(&filter, controller, top.id);
                    let kw_names: Vec<&str> = keywords.iter().map(|k| k.as_str()).collect();
                    for kw in keywords {
                        self.state.continuous_effects.push(ContinuousEffect {
                            source_id: Some(top.id),
                            affected: scope.clone(),
                            kind: ContinuousEffectKind::Layer6AddKeyword(kw),
                            duration: EffectDuration::UntilEndOfTurn,
                            timestamp: self.state.command_index,
                        });
                    }
                    events.push(ev_log(format!(
                        "{spell_label} grants {} to each affected creature until end of turn",
                        kw_names.join(", ")
                    )));
                }
                SpellEffectKind::PutCounters {
                    counter,
                    count,
                    target,
                } => {
                    // `Self_` is auto-bound to the source permanent (CR 115); any other filter
                    // uses the chosen target. Counters go on a permanent on the battlefield.
                    let tid = if matches!(target.kind, TargetKind::Self_) {
                        top.source_permanent_id
                    } else {
                        targets.first().copied()
                    };
                    if let Some(tid) = tid {
                        let is_valid_target = self
                            .state
                            .objects
                            .get(&tid)
                            .map(|t| t.zone == Zone::Battlefield && t.is_creature(self.registry))
                            .unwrap_or(false);
                        if is_valid_target {
                            let tgt = object_display_name(&self.state, self.registry, tid);
                            if let Some(t) = self.state.objects.get_mut(&tid) {
                                *t.counters.entry(counter).or_insert(0) += count;
                            }
                            events.push(ev_log(format!(
                                "{spell_label} puts {count} {} counter{} on {tgt}",
                                counter_label(counter),
                                if count == 1 { "" } else { "s" },
                            )));
                            // Annihilation / toughness-0 death are checked by the SBA pass that
                            // runs after this resolution (CR 122.3, CR 704.5f).
                        }
                    }
                }
                SpellEffectKind::DestroyTarget { .. } => {
                    if let Some(&tid) = targets.first() {
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        let indestructible =
                            self.effective_has_keyword(tid, Keyword::Indestructible);
                        if indestructible {
                            events.push(ev_log(format!(
                                "{spell_label} has no effect: {tgt} is indestructible."
                            )));
                        } else if consume_regen_shield(&mut self.state, tid, events) {
                            events.push(ev_log(format!("{tgt} regenerates.")));
                        } else {
                            events.push(ev_log(format!("{spell_label} destroys {tgt}")));
                            let owner = self.state.objects.get(&tid).map(|o| o.owner);
                            let card_id_t = self.state.objects.get(&tid).map(|o| o.card_id.clone());
                            destroy_permanent(&mut self.state, tid)?;
                            if let Some(owner_id) = owner {
                                events.push(permanent_moved_event(
                                    &self.state,
                                    tid,
                                    owner_id,
                                    rv1::permanent_moved::Destination::Graveyard,
                                ));
                            }
                            if let (Some(cid), Some(ctrl)) = (card_id_t, owner) {
                                self.fire_triggers(
                                    GameEvent::Dies {
                                        object_id: tid,
                                        card_id: cid,
                                        controller: ctrl,
                                    },
                                    events,
                                );
                            }
                        }
                    }
                }
                SpellEffectKind::CounterTargetSpell { .. } => {
                    if let Some(&tid) = targets.first() {
                        if let Some(pos) = self.state.stack.iter().position(|s| s.id == tid) {
                            let st = self.state.stack.remove(pos);
                            let tgt = self
                                .registry
                                .get(&st.card_id)
                                .map(|d| d.name.as_str())
                                .unwrap_or("spell");
                            // CR 707.10d: a copy of a spell has no backing card — it simply ceases
                            // to exist when it leaves the stack. Only a genuinely cast spell has a
                            // `GameObject` that moves; CR 701.6a sends that card to its OWNER's
                            // graveyard via an explicit PermanentMoved so the C++ relay routes the
                            // physical card off the shared stack. Moving a copy would error on the
                            // missing object and corrupt the already-popped stack.
                            if !st.is_copy {
                                let owner = self.state.objects.get(&st.id).map(|o| o.owner);
                                move_object_to_zone(&mut self.state, st.id, Zone::Graveyard)?;
                                if let Some(owner) = owner {
                                    events.push(permanent_moved_event(
                                        &self.state,
                                        st.id,
                                        owner,
                                        rv1::permanent_moved::Destination::Graveyard,
                                    ));
                                }
                            }
                            events.push(ev_log(format!("{spell_label} counters {tgt}")));
                        }
                    }
                }
                SpellEffectKind::CopyTargetSpell { count, .. } => {
                    // CR 707.10: create `count` copies of the target spell on the stack, each
                    // controlled by this spell's controller. The copy is not cast (no mana/cast
                    // triggers) and ceases to exist after resolving (handled by `is_copy`). It uses
                    // the original's chosen X and face. CR 707.10c: the copy's controller may choose
                    // new targets for each copy; if the copied spell has targets, we prompt before
                    // placing the copy on the stack. Only the first copy that needs targets is
                    // handled via pending_resolution (count=1 covers Twincast/Fork/Reverberate).
                    if let Some(&tid) = targets.first() {
                        if let Some(src) = self.state.stack.iter().find(|s| s.id == tid).cloned() {
                            let copied_name = self
                                .registry
                                .get(&src.card_id)
                                .and_then(|d| d.face(src.face_index))
                                .map(|f| f.name.to_string())
                                .unwrap_or_else(|| src.card_id.clone());
                            let src_effects = self
                                .registry
                                .get(&src.card_id)
                                .and_then(|d| d.face(src.face_index))
                                .map(|f| f.spell_effect.to_vec())
                                .unwrap_or_default();
                            let needs_target_choice = !src.targets.is_empty();
                            for copy_num in 0..count {
                                let copy_id = self.state.next_object_id;
                                self.state.next_object_id += 1;
                                let copy_template = StackItem {
                                    id: copy_id,
                                    controller,
                                    card_id: src.card_id.clone(),
                                    targets: src.targets.clone(),
                                    ability_text: None,
                                    source_permanent_id: None,
                                    ability_index: None,
                                    is_triggered: false,
                                    is_copy: true,
                                    chosen_x: src.chosen_x,
                                    face_index: src.face_index,
                                    target_damage: src.target_damage.clone(),
                                };
                                // CR 707.10c: prompt for new targets on the first copy; push any
                                // additional copies immediately with the original targets.
                                if needs_target_choice && copy_num == 0 {
                                    let sp = compute_spell_targets(
                                        &self.state,
                                        self.registry,
                                        controller,
                                        &src_effects,
                                    );
                                    let mut candidates: Vec<ObjectId> =
                                        sp.valid_permanent_ids.clone();
                                    candidates.extend(sp.valid_stack_ids.iter().copied());
                                    for p in &self.state.players {
                                        if (sp.can_target_self && p.id == controller)
                                            || (sp.can_target_opponent && p.id != controller)
                                        {
                                            candidates.push(p.id as ObjectId);
                                        }
                                    }
                                    // CR 707.10c: may keep original targets even if now illegal.
                                    for &ot in &src.targets {
                                        if !candidates.contains(&ot) {
                                            candidates.push(ot);
                                        }
                                    }
                                    let candidate_card_ids: Vec<String> = candidates
                                        .iter()
                                        .map(|&oid| {
                                            self.state
                                                .objects
                                                .get(&oid)
                                                .map(|o| o.card_id.clone())
                                                .unwrap_or_default()
                                        })
                                        .collect();
                                    let candidate_names: Vec<String> = candidates
                                        .iter()
                                        .map(|&oid| {
                                            if self.state.player_idx(oid as i32).is_some() {
                                                format!("Player {oid}")
                                            } else {
                                                self.state
                                                    .objects
                                                    .get(&oid)
                                                    .and_then(|o| self.registry.get(&o.card_id))
                                                    .map(|d| d.name.clone())
                                                    .unwrap_or_else(|| format!("[object {oid}]"))
                                            }
                                        })
                                        .collect();
                                    let prompt =
                                        format!("Choose new targets for {copied_name} (copy)");
                                    events.push(rv1::RuledEvent {
                                        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                                            rv1::ResolutionChoiceRequired {
                                                deciding_player_id: controller,
                                                source_object_id: copy_id,
                                                prompt_text: prompt.clone(),
                                                // choice_kind 3 = target objects: client uses
                                                // click-to-target instead of a list dialog.
                                                choice_kind: 3,
                                                candidate_object_ids: candidates.clone(),
                                                candidate_card_ids,
                                                candidate_names,
                                                min: 1,
                                                max: 1,
                                                ordered: false,
                                                unique_names: false,
                                                candidate_server_card_ids: Vec::new(),
                                            },
                                        )),
                                    });
                                    events.push(ev_log(prompt.clone()));
                                    self.state.pending_resolution = Some(PendingResolution {
                                        item: copy_template,
                                        custom_key: "__copy_targets".to_string(),
                                        step: 0,
                                        scratch: vec![],
                                        deciding_player: controller,
                                        candidates,
                                        min: 1,
                                        max: 1,
                                        ordered: false,
                                        prompt,
                                        choice_kind: 3,
                                        unique_names: false,
                                        copy_source_object_id: src.id,
                                        search_destination: SearchDestination::Hand,
                                        search_shuffle: false,
                                        search_reveal: false,
                                    });
                                    // Copy will be pushed to the stack after target is submitted.
                                } else {
                                    self.state.stack.push(copy_template);
                                    events.push(rv1::RuledEvent {
                                        ev: Some(rv1::ruled_event::Ev::StackPushed(
                                            rv1::StackPushed {
                                                object_id: copy_id,
                                                description: copied_name.clone(),
                                                targets: src
                                                    .targets
                                                    .iter()
                                                    .map(|&o| rv1::TargetRef {
                                                        object_id: o,
                                                        damage_amount: 0,
                                                    })
                                                    .collect(),
                                                ability_annotation: "(copy)".to_string(),
                                                card_id: src.card_id.clone(),
                                                is_copy: true,
                                                copy_source_object_id: src.id,
                                            },
                                        )),
                                    });
                                    events.push(ev_log(format!(
                                        "{spell_label} copies {copied_name} (P{controller})"
                                    )));
                                }
                            }
                        }
                    }
                }
                SpellEffectKind::GainLife { amount } => {
                    let amount = amount.resolve(top.chosen_x);
                    let pi = self.state.player_idx(controller).unwrap();
                    self.state.players[pi].life += amount as i32;
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                            player_id: controller,
                            new_total: self.state.players[pi].life,
                            delta: amount as i32,
                        })),
                    });
                    events.push(ev_log(format!(
                        "P{controller} gains {amount} life ({spell_label})."
                    )));
                }
                SpellEffectKind::TargetPlayerGainsLife { amount, .. } => {
                    if let Some(&tid) = targets.first() {
                        if let Some(pi) = self.state.player_idx(tid as i32) {
                            let pid = self.state.players[pi].id;
                            self.state.players[pi].life += amount as i32;
                            events.push(rv1::RuledEvent {
                                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                    player_id: pid,
                                    new_total: self.state.players[pi].life,
                                    delta: amount as i32,
                                })),
                            });
                            events.push(ev_log(format!(
                                "P{pid} gains {amount} life ({spell_label})."
                            )));
                        }
                    }
                }
                SpellEffectKind::TargetPlayerLosesLife { amount, .. } => {
                    if let Some(&tid) = targets.first() {
                        if let Some(pi) = self.state.player_idx(tid as i32) {
                            let pid = self.state.players[pi].id;
                            self.state.players[pi].life -= amount as i32;
                            events.push(rv1::RuledEvent {
                                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                    player_id: pid,
                                    new_total: self.state.players[pi].life,
                                    delta: -(amount as i32),
                                })),
                            });
                            events.push(ev_log(format!(
                                "P{pid} loses {amount} life ({spell_label})."
                            )));
                        }
                    }
                }
                SpellEffectKind::EachOpponentLosesLifeYouGainEqual { amount } => {
                    let opps: Vec<(usize, PlayerId)> = self
                        .state
                        .players
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| p.id != controller && !p.has_lost)
                        .map(|(i, p)| (i, p.id))
                        .collect();
                    let mut total_lost: u32 = 0;
                    for (pi, pid) in opps {
                        self.state.players[pi].life -= amount as i32;
                        total_lost += amount;
                        events.push(rv1::RuledEvent {
                            ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                player_id: pid,
                                new_total: self.state.players[pi].life,
                                delta: -(amount as i32),
                            })),
                        });
                        events.push(ev_log(format!(
                            "P{pid} loses {amount} life ({spell_label})."
                        )));
                    }
                    if total_lost > 0 {
                        if let Some(ci) = self.state.player_idx(controller) {
                            self.state.players[ci].life += total_lost as i32;
                            events.push(rv1::RuledEvent {
                                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                    player_id: controller,
                                    new_total: self.state.players[ci].life,
                                    delta: total_lost as i32,
                                })),
                            });
                            events.push(ev_log(format!(
                                "P{controller} gains {total_lost} life ({spell_label})."
                            )));
                        }
                    }
                }
                SpellEffectKind::ExileTarget => {
                    if let Some(&tid) = targets.first() {
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        let owner = self.state.objects.get(&tid).map(|o| o.owner);
                        move_object_to_zone(&mut self.state, tid, Zone::Exile)?;
                        events.push(ev_log(format!("{spell_label} exiles {tgt}")));
                        if let Some(owner_id) = owner {
                            events.push(permanent_moved_event(
                                &self.state,
                                tid,
                                owner_id,
                                rv1::permanent_moved::Destination::Exile,
                            ));
                        }
                    }
                }
                SpellEffectKind::ExileTargetGainLifeEqualToPower => {
                    if let Some(&tid) = targets.first() {
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        // CR 608: read effective power at resolution before the object leaves.
                        let power = self.effective_power(tid).unwrap_or(0);
                        let owner = self.state.objects.get(&tid).map(|o| o.owner);
                        let target_controller = owner.unwrap_or(controller);
                        move_object_to_zone(&mut self.state, tid, Zone::Exile)?;
                        events.push(ev_log(format!("{spell_label} exiles {tgt}")));
                        if let Some(owner_id) = owner {
                            events.push(permanent_moved_event(
                                &self.state,
                                tid,
                                owner_id,
                                rv1::permanent_moved::Destination::Exile,
                            ));
                        }
                        if power > 0 {
                            if let Some(pi) = self.state.player_idx(target_controller) {
                                self.state.players[pi].life += power as i32;
                                events.push(rv1::RuledEvent {
                                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                        player_id: target_controller,
                                        new_total: self.state.players[pi].life,
                                        delta: power as i32,
                                    })),
                                });
                                events.push(ev_log(format!(
                                    "P{target_controller} gains {power} life."
                                )));
                            }
                        }
                    }
                }
                SpellEffectKind::ReturnTargetCreatureToHand
                | SpellEffectKind::ReturnTargetPermanentToHand => {
                    if let Some(&tid) = targets.first() {
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        let owner = self.state.objects.get(&tid).map(|o| o.owner);
                        // Transient battlefield state (damage, counters, tap) is reset centrally
                        // by move_object_to_zone on leaving the battlefield (CR 400.7 / 121.2).
                        move_object_to_zone(&mut self.state, tid, Zone::Hand)?;
                        events.push(ev_log(format!(
                            "{spell_label} returns {tgt} to its owner's hand"
                        )));
                        if let Some(owner_id) = owner {
                            events.push(permanent_moved_event(
                                &self.state,
                                tid,
                                owner_id,
                                rv1::permanent_moved::Destination::Hand,
                            ));
                        }
                    }
                }
                SpellEffectKind::MillTargetPlayer { count, .. } => {
                    if let Some(&tid) = targets.first() {
                        if let Some(pi) = self.state.player_idx(tid as i32) {
                            let pid = self.state.players[pi].id;
                            let mut milled = 0u32;
                            for _ in 0..count {
                                let Some(oid) = self.state.players[pi].library.pop_front() else {
                                    break;
                                };
                                self.state.players[pi].graveyard.push(oid);
                                if let Some(o) = self.state.objects.get_mut(&oid) {
                                    o.zone = Zone::Graveyard;
                                }
                                events.push(permanent_moved_event(
                                    &self.state,
                                    oid,
                                    pid,
                                    rv1::permanent_moved::Destination::Graveyard,
                                ));
                                milled += 1;
                            }
                            events.push(ev_log(format!(
                                "{spell_label} mills {milled} card(s) from P{pid}"
                            )));
                        }
                    }
                }
                SpellEffectKind::TapTarget { .. } => {
                    if let Some(&tid) = targets.first() {
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        if let Some(o) = self.state.objects.get_mut(&tid) {
                            if o.zone == Zone::Battlefield && !o.tapped {
                                o.tapped = true;
                                events.push(ev_log(format!("{spell_label} taps {tgt}")));
                            }
                        }
                    }
                }
                SpellEffectKind::DestroyAll {
                    kind,
                    prevent_regeneration,
                } => {
                    // CR 701.7 / 704.4: all matching permanents are destroyed simultaneously,
                    // then their "dies" triggers fire together. Indestructible permanents survive
                    // (CR 702.12b). `prevent_regeneration` bypasses shields (Wrath of God).
                    // Untargeted, so hexproof/shroud are irrelevant.
                    let victims = battlefield_objects_matching(&self.state, self.registry, &kind);
                    let mut destroyed: Vec<(ObjectId, String, PlayerId)> = Vec::new();
                    for tid in victims {
                        let indestructible =
                            self.effective_has_keyword(tid, Keyword::Indestructible);
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        if indestructible {
                            events.push(ev_log(format!(
                                "{tgt} is indestructible and survives {spell_label}."
                            )));
                            continue;
                        }
                        // CR 701.15b: "can't be regenerated" bypasses shields.
                        if !prevent_regeneration
                            && consume_regen_shield(&mut self.state, tid, events)
                        {
                            events.push(ev_log(format!("{tgt} regenerates.")));
                            continue;
                        }
                        let owner = self.state.objects.get(&tid).map(|o| o.owner);
                        let card_id_t = self.state.objects.get(&tid).map(|o| o.card_id.clone());
                        destroy_permanent(&mut self.state, tid)?;
                        events.push(ev_log(format!("{spell_label} destroys {tgt}")));
                        if let Some(owner_id) = owner {
                            events.push(permanent_moved_event(
                                &self.state,
                                tid,
                                owner_id,
                                rv1::permanent_moved::Destination::Graveyard,
                            ));
                        }
                        if let (Some(cid), Some(ctrl)) = (card_id_t, owner) {
                            destroyed.push((tid, cid, ctrl));
                        }
                    }
                    for (tid, cid, ctrl) in destroyed {
                        self.fire_triggers(
                            GameEvent::Dies {
                                object_id: tid,
                                card_id: cid,
                                controller: ctrl,
                            },
                            events,
                        );
                    }
                }
                SpellEffectKind::DamageAll { amount, kind } => {
                    // CR 119: deal damage to each matching permanent. Marking damage mirrors
                    // DamageTarget; lethal-damage destruction is left to state-based actions
                    // (CR 704.5g), which run immediately after this spell resolves.
                    let affected = battlefield_objects_matching(&self.state, self.registry, &kind);
                    for tid in &affected {
                        let tgt = object_display_name(&self.state, self.registry, *tid);
                        if let Some(o) = self.state.objects.get_mut(tid) {
                            o.damage += amount;
                        }
                        events.push(ev_log(format!(
                            "{spell_label} deals {amount} damage to {tgt}"
                        )));
                    }
                }
                SpellEffectKind::CreateTokens {
                    token,
                    count,
                    controller: who,
                } => {
                    self.create_tokens(&token, count, who, controller, &spell_label, events);
                }
                // CR 702.6a: the equip activated ability resolves — move the equipment's
                // `attached_to` pointer to the chosen creature (detaching from any previous one
                // automatically). P/T bonus follows dynamically via `AffectedScope::EquippedBy`.
                SpellEffectKind::Equip { .. } => {
                    let equip_oid = match top.source_permanent_id {
                        Some(id) => id,
                        None => {
                            events.push(ev_log(format!(
                                "{spell_label}: equip ability has no source permanent."
                            )));
                            continue;
                        }
                    };
                    if let Some(&target_id) = targets.first() {
                        let valid = self
                            .state
                            .objects
                            .get(&target_id)
                            .map(|t| t.zone == Zone::Battlefield && t.is_creature(self.registry))
                            .unwrap_or(false);
                        let equip_on_battlefield = self
                            .state
                            .objects
                            .get(&equip_oid)
                            .map(|e| e.zone == Zone::Battlefield)
                            .unwrap_or(false);
                        if valid && equip_on_battlefield {
                            let tgt = object_display_name(&self.state, self.registry, target_id);
                            let eq_name =
                                object_display_name(&self.state, self.registry, equip_oid);
                            if let Some(eq) = self.state.objects.get_mut(&equip_oid) {
                                eq.attached_to = Some(target_id);
                            }
                            events.push(ev_log(format!(
                                "{spell_label} attaches {eq_name} to {tgt}."
                            )));
                        }
                    }
                }
                // CR 605.3b: a mana ability never uses the stack, so a ProduceMana effect is
                // resolved immediately in `resolve_mana_ability` and can never reach this generic
                // stack-resolution path. Defensive no-op (registry validation also forbids it on
                // spells); producing mana here would be off-stack-timing and is intentionally not done.
                SpellEffectKind::ReturnFromGraveyard {
                    filter,
                    destination,
                } => {
                    if let Some(&tid) = targets.first() {
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        let is_legal = {
                            use tricerules_cards::primitives::{GraveyardCardType, GraveyardOwner};
                            let obj = self.state.objects.get(&tid);
                            let in_graveyard = obj.is_some_and(|o| o.zone == Zone::Graveyard);
                            let owner_ok = obj.is_some_and(|o| match filter.owner {
                                GraveyardOwner::Controller => o.owner == controller,
                                GraveyardOwner::AnyPlayer => true,
                            });
                            let type_ok = if let Some(ct) = filter.card_type {
                                obj.and_then(|o| self.registry.get(&o.card_id))
                                    .is_some_and(|def| match ct {
                                        GraveyardCardType::Creature => {
                                            def.is_creature
                                                || def.faces.iter().any(|f| f.is_creature)
                                        }
                                    })
                            } else {
                                true
                            };
                            in_graveyard && owner_ok && type_ok
                        };
                        if !is_legal {
                            events.push(ev_log(format!(
                                "{spell_label} fizzles: {tgt} is no longer a legal graveyard target."
                            )));
                        } else {
                            let owner = self.state.objects.get(&tid).map(|o| o.owner);
                            use tricerules_cards::primitives::GraveyardDestination;
                            let dest_zone = match destination {
                                GraveyardDestination::Hand => Zone::Hand,
                                GraveyardDestination::Battlefield => Zone::Battlefield,
                            };
                            let dest_proto = match destination {
                                GraveyardDestination::Hand => {
                                    rv1::permanent_moved::Destination::Hand
                                }
                                GraveyardDestination::Battlefield => {
                                    rv1::permanent_moved::Destination::Battlefield
                                }
                            };
                            move_object_to_zone(&mut self.state, tid, dest_zone)?;
                            let dest_name = match destination {
                                GraveyardDestination::Hand => "hand",
                                GraveyardDestination::Battlefield => "battlefield",
                            };
                            events.push(ev_log(format!(
                                "{spell_label} returns {tgt} from graveyard to {dest_name}."
                            )));
                            if let Some(owner_id) = owner {
                                events.push(permanent_moved_event(
                                    &self.state,
                                    tid,
                                    owner_id,
                                    dest_proto,
                                ));
                            }
                            if dest_zone == Zone::Battlefield {
                                self.fire_triggers(
                                    GameEvent::EntersBattlefield { object_id: tid },
                                    events,
                                );
                            }
                        }
                    }
                }
                SpellEffectKind::ProduceMana { .. } => {}
                // CR 701.18: pause resolution and ask the controller to choose from their library.
                // Uses choice_kind 2 (LibrarySearch) so the relay redacts candidates from opponents.
                SpellEffectKind::SearchLibrary {
                    filter,
                    destination,
                    shuffle,
                    reveal,
                } => {
                    let candidates: Vec<ObjectId> = {
                        let Some(idx) = self.state.player_idx(controller) else {
                            events.push(ev_log(format!("{spell_label} resolves (no library).")));
                            return Ok(());
                        };
                        self.state.players[idx]
                            .library
                            .iter()
                            .copied()
                            .filter(|&oid| {
                                library_card_matches_filter(
                                    &self.state,
                                    self.registry,
                                    oid,
                                    filter.as_ref(),
                                )
                            })
                            .collect()
                    };
                    let min = if candidates.is_empty() { 0u32 } else { 1u32 };
                    let prompt = match &filter {
                        None => format!("P{controller}: search your library for a card."),
                        Some(f) => format!(
                            "P{controller}: search your library for a {} card.",
                            spell_type_filter_desc(f)
                        ),
                    };
                    let candidate_card_ids: Vec<String> = candidates
                        .iter()
                        .map(|&oid| {
                            self.state
                                .objects
                                .get(&oid)
                                .map(|o| o.card_id.clone())
                                .unwrap_or_default()
                        })
                        .collect();
                    let candidate_names: Vec<String> = candidate_card_ids
                        .iter()
                        .map(|cid| {
                            self.registry
                                .get(cid)
                                .map(|d| d.name.clone())
                                .unwrap_or_else(|| cid.clone())
                        })
                        .collect();
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                            rv1::ResolutionChoiceRequired {
                                deciding_player_id: controller,
                                source_object_id: top.id,
                                prompt_text: prompt.clone(),
                                choice_kind: custom::ChoiceKind::LibrarySearch.as_proto(),
                                candidate_object_ids: candidates.clone(),
                                candidate_card_ids,
                                candidate_names,
                                min,
                                max: 1,
                                ordered: false,
                                unique_names: false,
                                candidate_server_card_ids: Vec::new(),
                            },
                        )),
                    });
                    events.push(ev_log(prompt.clone()));
                    self.state.pending_resolution = Some(PendingResolution {
                        item: top.clone(),
                        custom_key: "__search_library".to_string(),
                        step: 0,
                        scratch: vec![],
                        deciding_player: controller,
                        candidates,
                        min,
                        max: 1,
                        ordered: false,
                        unique_names: false,
                        prompt,
                        choice_kind: custom::ChoiceKind::LibrarySearch.as_proto(),
                        copy_source_object_id: 0,
                        search_destination: destination,
                        search_shuffle: shuffle,
                        search_reveal: reveal,
                    });
                    // Resolution is now parked; the "resolves." log is emitted by finish_library_search.
                    return Ok(());
                }
                // CR 701.15: put a regeneration shield on the target. Resolved only as an
                // activated ability (`Self_` auto-bound to the source permanent). Each activation
                // adds one shield; shields expire at cleanup (like marked damage).
                SpellEffectKind::Regenerate { target } => {
                    let tid = if matches!(target.kind, TargetKind::Self_) {
                        top.source_permanent_id
                    } else {
                        targets.first().copied()
                    };
                    if let Some(tid) = tid {
                        let is_creature = self
                            .state
                            .objects
                            .get(&tid)
                            .map(|o| o.zone == Zone::Battlefield && o.is_creature(self.registry))
                            .unwrap_or(false);
                        if is_creature {
                            let tgt = object_display_name(&self.state, self.registry, tid);
                            if let Some(o) = self.state.objects.get_mut(&tid) {
                                o.regeneration_shields += 1;
                            }
                            events.push(ev_log(format!(
                                "{tgt} has a regeneration shield ({spell_label})."
                            )));
                        }
                    }
                }
                SpellEffectKind::None => {}
                // CR 303.4: the AuraAttach effect is the "Enchant <type>" clause of an aura spell.
                // The attached_to field was already set before fire_triggers; emit the proto event
                // so the relay can issue Event_AttachCard to connected clients.
                SpellEffectKind::AuraAttach { .. } => {
                    if let (Some(enchanted_oid), Some(obj)) =
                        (targets.first().copied(), self.state.objects.get(&top.id))
                    {
                        if obj.zone == Zone::Battlefield {
                            let tgt =
                                object_display_name(&self.state, self.registry, enchanted_oid);
                            events.push(rv1::RuledEvent {
                                ev: Some(rv1::ruled_event::Ev::AuraAttached(rv1::AuraAttached {
                                    aura_object_id: top.id,
                                    enchanted_object_id: enchanted_oid,
                                })),
                            });
                            events.push(ev_log(format!("{spell_label} attaches to {tgt}.")));
                        }
                    }
                }
            }
        }
        events.push(ev_log(format!("{spell_label} resolves.")));
        Ok(())
    }

    /// CR 111: mint `count` tokens of `token_id` for each recipient and put them onto the
    /// battlefield. Each minted token is a fresh [`GameObject`] whose characteristics come from
    /// the token's [`CardDefinition`] (via the registry's token namespace), so combat, P/T, and
    /// keyword queries treat it exactly like any other permanent. Entering tokens fire ETB
    /// triggers (CR 603.6) through the same hook as a resolved creature spell, so Soul Warden et al.
    /// see them. A [`TokenCreated`](rv1::TokenCreated) event carries the self-describing identity
    /// the relay needs (tokens have no deck card / Oracle entry).
    pub(super) fn create_tokens(
        &mut self,
        token_id: &str,
        count: u32,
        who: TokenController,
        spell_controller: PlayerId,
        spell_label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let registry = self.registry;
        let Some(def) = registry.get(token_id) else {
            // Registry load validates every CreateTokens reference, so this is unreachable;
            // fail safe by doing nothing rather than panicking (server-authoritative).
            events.push(ev_log(format!(
                "{spell_label} could not create unknown token '{token_id}'."
            )));
            return;
        };
        let name = def.name.clone();
        let is_creature = def.is_creature;
        let power = def.power;
        let toughness = def.toughness;
        let types = def.types.clone();
        let keywords: Vec<String> = def
            .keywords
            .iter()
            .map(|k| k.as_str().to_string())
            .collect();
        let color = color_string(&def.colors());
        let pt = if is_creature {
            format!("{}/{}", power.unwrap_or(0), toughness.unwrap_or(0))
        } else {
            String::new()
        };

        let recipients: Vec<PlayerId> = match who {
            TokenController::Controller => vec![spell_controller],
            // CR 111.3: each token's owner/controller is the player it is created under.
            TokenController::EachPlayer => self
                .state
                .players
                .iter()
                .filter(|p| !p.has_lost)
                .map(|p| p.id)
                .collect(),
        };

        for pid in recipients {
            let Some(pidx) = self.state.player_idx(pid) else {
                continue;
            };
            for _ in 0..count {
                let oid = self.state.next_object_id;
                self.state.next_object_id += 1;
                self.state.objects.insert(
                    oid,
                    GameObject {
                        id: oid,
                        owner: pid,
                        card_id: token_id.to_string(),
                        zone: Zone::Battlefield,
                        tapped: false,
                        summoning_sick: is_creature,
                        power,
                        toughness,
                        damage: 0,
                        deathtouch_damage: false,
                        counters: BTreeMap::new(),
                        attached_to: None,
                        regeneration_shields: 0,
                        must_attack_if_able: false,
                        must_block_if_able: false,
                    },
                );
                self.state.players[pidx].battlefield.push(oid);
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::TokenCreated(rv1::TokenCreated {
                        object_id: oid,
                        controller_player_id: pid,
                        card_id: token_id.to_string(),
                        identity: Some(rv1::TokenIdentity {
                            name: name.clone(),
                            pt: pt.clone(),
                            color: color.clone(),
                            types: types.clone(),
                            is_creature,
                            keywords: keywords.clone(),
                        }),
                    })),
                });
                // CR 603.6: a token entering the battlefield triggers ETB watchers.
                self.fire_triggers(GameEvent::EntersBattlefield { object_id: oid }, events);
            }
            let noun = if count == 1 { "token" } else { "tokens" };
            events.push(ev_log(format!(
                "P{pid} creates {count} {name} {noun} ({spell_label})."
            )));
        }
    }
}

pub(super) fn draw_card(
    p: &mut PlayerState,
    objects: &mut HashMap<ObjectId, GameObject>,
) -> Result<(), EngineError> {
    let oid = p
        .library
        .pop_front()
        .ok_or(EngineError::Illegal("library empty"))?;
    p.hand.push(oid);
    if let Some(o) = objects.get_mut(&oid) {
        o.zone = Zone::Hand;
    }
    Ok(())
}

/// Build a `PermanentMoved` event, stamping the tricerules `card_id` from the object so
/// servers can resolve cards that have no engine-oid mapping (e.g. milled library cards).
pub(crate) fn permanent_moved_event(
    state: &GameState,
    oid: ObjectId,
    owner_player_id: PlayerId,
    destination: rv1::permanent_moved::Destination,
) -> rv1::RuledEvent {
    let card_id = state
        .objects
        .get(&oid)
        .map(|o| o.card_id.clone())
        .unwrap_or_default();
    rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PermanentMoved(rv1::PermanentMoved {
            object_id: oid,
            owner_player_id,
            destination: destination as i32,
            card_id,
        })),
    }
}

/// Resolve an [`AnthemFilter`] into an [`AffectedScope`] for a continuous effect, given the
/// effect's `controller` and the source permanent `source`. Shared by static anthems
/// (`AnthemPt`, source on the battlefield) and one-shot mass pumps (`PumpAll`, `source` is the
/// resolving spell). `exclude_self` only applies when the source persists, so a spell passes a
/// `source` that is harmlessly never on the battlefield as a creature.
pub(super) fn resolve_anthem_scope(
    filter: &AnthemFilter,
    controller: PlayerId,
    source: ObjectId,
) -> AffectedScope {
    AffectedScope::CreaturesMatching {
        controller: filter
            .controller
            .map(|AnthemController::YouControl| controller),
        subtype: filter.subtype.clone(),
        color: filter.color,
        exclude: if filter.exclude_self {
            Some(source)
        } else {
            None
        },
    }
}

pub(super) fn move_object_to_zone(
    state: &mut GameState,
    oid: ObjectId,
    z: Zone,
) -> Result<(), EngineError> {
    let owner = state
        .objects
        .get(&oid)
        .map(|o| o.owner)
        .ok_or(EngineError::Illegal("no object"))?;

    // CR 400.7: a zone change creates a new game object. Remove any Single-target continuous
    // effects on this object so they don't apply if the same ObjectId is reused later.
    // CR 604.3 / 611.3: also drain any `WhileSourceOnBattlefield` effects this permanent was the
    // source of (anthems) — a static ability stops applying the moment its source leaves (LTB).
    // One-shot `UntilEndOfTurn` effects (Giant Growth, firebreathing) are deliberately NOT drained
    // here: once created they are independent of their source (CR 611.2g) and only end at cleanup.
    let leaving_battlefield = state.objects.get(&oid).map(|o| o.zone) == Some(Zone::Battlefield);
    if leaving_battlefield && z != Zone::Battlefield {
        state.continuous_effects.retain(|e| {
            let single_on_this = matches!(&e.affected, AffectedScope::Single(id) if *id == oid);
            let static_from_this =
                e.source_id == Some(oid) && e.duration == EffectDuration::WhileSourceOnBattlefield;
            !single_on_this && !static_from_this
        });
        // CR 400.7 / 121.2: a zone change makes this a new game object — transient
        // battlefield-only state (marked damage, deathtouch marking, tap status, regeneration
        // shields) and all counters do not carry over. Centralized here so every leave path
        // (SBA destroy, sacrifice, bounce, discard, mill, exile) is correct by construction.
        if let Some(o) = state.objects.get_mut(&oid) {
            o.damage = 0;
            o.deathtouch_damage = false;
            o.tapped = false;
            o.counters.clear();
            o.attached_to = None;
            o.regeneration_shields = 0;
        }
    }

    let idx = state.player_idx(owner).unwrap();
    let p = &mut state.players[idx];
    p.library.retain(|&x| x != oid);
    p.hand.retain(|&x| x != oid);
    p.battlefield.retain(|&x| x != oid);
    p.graveyard.retain(|&x| x != oid);
    p.exile.retain(|&x| x != oid);
    match z {
        Zone::Graveyard => p.graveyard.push(oid),
        Zone::Hand => p.hand.push(oid),
        Zone::Battlefield => p.battlefield.push(oid),
        Zone::Library => p.library.push_back(oid),
        Zone::Exile => p.exile.push(oid),
        Zone::Stack => {}
    }
    if let Some(o) = state.objects.get_mut(&oid) {
        o.zone = z;
        // CR 302.6: a permanent entering the battlefield has not been controlled continuously
        // since its controller's most recent turn began, so it is summoning sick. Assert this on
        // entry rather than trusting a persisted flag — a prior bounce/leave clears transient
        // state, so a creature returned to hand and recast (or reanimated/flickered) the same turn
        // must still be sick. Haste exempts the *use* of this (checked at attack/tap time).
        if z == Zone::Battlefield {
            o.summoning_sick = true;
        }
    }
    Ok(())
}

pub(super) fn destroy_permanent(state: &mut GameState, oid: ObjectId) -> Result<(), EngineError> {
    move_object_to_zone(state, oid, Zone::Graveyard)
}

/// Sacrifice a permanent (CR 701.17). Unlike destroy, sacrifice bypasses indestructible and
/// regeneration — it is always a cost, never a triggered or replacement effect that can be
/// redirected.
pub(super) fn sacrifice_permanent(state: &mut GameState, oid: ObjectId) -> Result<(), EngineError> {
    move_object_to_zone(state, oid, Zone::Graveyard)
}

fn counter_label(kind: CounterKind) -> &'static str {
    match kind {
        CounterKind::PlusOnePlusOne => "+1/+1",
        CounterKind::MinusOneMinusOne => "-1/-1",
    }
}

/// Return true if the library card `oid` satisfies `filter` (None = any card).
/// Uses the card's derived type flags from `CardDefinition`.
pub(super) fn library_card_matches_filter(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    filter: Option<&SpellTypeFilter>,
) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let Some(obj) = state.objects.get(&oid) else {
        return false;
    };
    let Some(def) = registry.get(&obj.card_id) else {
        return false;
    };
    match filter {
        SpellTypeFilter::Instant => def.is_instant,
        SpellTypeFilter::Sorcery => def.is_sorcery,
        SpellTypeFilter::InstantOrSorcery => def.is_instant || def.is_sorcery,
        SpellTypeFilter::Creature => def.is_creature,
        SpellTypeFilter::Artifact => def.is_artifact,
        SpellTypeFilter::Enchantment => def.is_enchantment,
        SpellTypeFilter::Noncreature => !def.is_creature,
    }
}

/// Human-readable description of a [`SpellTypeFilter`] for prompt text.
fn spell_type_filter_desc(f: &SpellTypeFilter) -> &'static str {
    match f {
        SpellTypeFilter::Instant => "instant",
        SpellTypeFilter::Sorcery => "sorcery",
        SpellTypeFilter::InstantOrSorcery => "instant or sorcery",
        SpellTypeFilter::Creature => "creature",
        SpellTypeFilter::Artifact => "artifact",
        SpellTypeFilter::Enchantment => "enchantment",
        SpellTypeFilter::Noncreature => "noncreature",
    }
}

/// CR 701.15: attempt to consume one regeneration shield from `oid`. If a shield is present,
/// taps the creature, removes it from combat, clears all marked damage, and returns `true`.
/// The caller is responsible for not destroying the creature. Returns `false` if no shield exists.
/// Does NOT emit a zone-change event (the creature stays on the battlefield).
pub(super) fn consume_regen_shield(
    state: &mut GameState,
    oid: ObjectId,
    events: &mut Vec<rv1::RuledEvent>,
) -> bool {
    let shields = state
        .objects
        .get(&oid)
        .map(|o| o.regeneration_shields)
        .unwrap_or(0);
    if shields == 0 {
        return false;
    }
    if let Some(o) = state.objects.get_mut(&oid) {
        o.regeneration_shields -= 1;
        o.damage = 0;
        o.deathtouch_damage = false;
        o.tapped = true;
    }
    // CR 701.15a: remove from combat (attacker/blocker lists). This mirrors what happens when
    // a creature is removed from combat by a tap effect.
    if let Some(combat) = state.combat.as_mut() {
        let was_in_combat = combat.attacking.contains(&oid)
            || combat.blockers.contains_key(&oid)
            || combat.blockers.values().any(|v| v.contains(&oid));
        combat.attacking.retain(|&id| id != oid);
        combat.blockers.remove(&oid);
        for v in combat.blockers.values_mut() {
            v.retain(|&id| id != oid);
        }
        if was_in_combat {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::RemovedFromCombat(
                    rv1::CreaturesRemovedFromCombat {
                        object_ids: vec![oid],
                    },
                )),
            });
        }
    }
    true
}
