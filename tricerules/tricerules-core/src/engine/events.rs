use super::legal_actions::fill_legal;
use super::*;

impl GameEngine {
    fn rules_annotation_labels(
        &self,
        oid: ObjectId,
        object: &GameObject,
        characteristics: Option<&Characteristics>,
        face: Option<&CardFace>,
    ) -> Vec<String> {
        let mut labels: Vec<String> = characteristics
            .zip(face)
            .map(|(effective, intrinsic)| {
                effective
                    .keywords
                    .iter()
                    .copied()
                    .filter(|keyword| !intrinsic.keywords.contains(keyword))
                    .map(|keyword| keyword.as_str().to_string())
                    .collect()
            })
            .unwrap_or_default();

        if let Some(characteristics) = characteristics {
            labels.extend(
                characteristics
                    .protections
                    .iter()
                    .copied()
                    .map(ProtectionQuality::label),
            );
        }

        if let Some(characteristics) = characteristics {
            let restrictions = self.combat_restrictions_for(oid, characteristics);
            if restrictions.cant_attack {
                labels.push("Can't attack".to_string());
            }
            if restrictions.cant_block {
                labels.push("Can't block".to_string());
            }
            if restrictions.cant_be_blocked {
                labels.push("Can't be blocked".to_string());
            }
        }

        let generation = self
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0);
        if self.state.skip_next_untap.contains(&(oid, generation)) {
            labels.push("Doesn't untap during its controller's next untap step".to_string());
        }
        if self.doesnt_untap_during_untap_step(oid) {
            labels.push("Doesn't untap during its controller's untap step".to_string());
        }

        for (_, ability, granted) in self.effective_activated_abilities(oid) {
            if granted && !labels.contains(&ability.text) {
                labels.push(ability.text);
            }
        }

        match object.regeneration_shields {
            0 => {}
            1 => labels.push("Regeneration shield".to_string()),
            count => labels.push(format!("{count} regeneration shields")),
        }

        let mut remaining_prevention = 0u32;
        let mut per_event_prevention = 0u32;
        let mut prevents_all = false;
        let mut prevents_all_combat = false;
        for effect in &self.state.damage_prevention_effects {
            if effect.duration != EffectDuration::UntilEndOfTurn {
                continue;
            }
            if effect.scope
                == (DamagePreventionScope::CombatRecipient {
                    object_id: oid,
                    zone_change_generation: generation,
                })
            {
                prevents_all_combat |= effect.amount == DamagePreventionAmount::All;
                continue;
            }
            if effect.scope != DamagePreventionScope::Recipient(oid) {
                continue;
            }
            match effect.amount {
                DamagePreventionAmount::Remaining(amount) => {
                    remaining_prevention = remaining_prevention.saturating_add(amount);
                }
                DamagePreventionAmount::FixedPerEvent(amount) => {
                    per_event_prevention = per_event_prevention.saturating_add(amount);
                }
                DamagePreventionAmount::All => prevents_all = true,
            }
        }
        if remaining_prevention > 0 {
            labels.push(format!("Prevent next {remaining_prevention} damage"));
        }
        if prevents_all {
            labels.push("Prevent all damage".to_string());
        }
        if prevents_all_combat {
            labels.push("Prevent all combat damage".to_string());
        }
        if per_event_prevention > 0 {
            labels.push(format!(
                "Prevent up to {per_event_prevention} damage per event"
            ));
        }

        labels
    }

    pub fn initial_response_batch(&mut self) -> RuledEventBatch {
        let mut batch = RuledEventBatch::default();
        // Catalog first: Servatrice resolves the zone-view card ids below through it.
        batch.events.push(self.ev_card_catalog());
        batch.events.push(self.ev_zone_view_sync());
        if let Some(op) = &self.state.opening {
            batch
                .events
                .push(ev_phase(self, rv1::PhaseId::OpeningChooseFirst));
            batch.events.push(ev_priority_changed(self));
            batch
                .events
                .push(ev_log(format!("P{} chooses who goes first.", op.chooser)));
            fill_legal(&mut batch, self);
            return batch;
        }
        batch.events.push(ev_phase(self, rv1::PhaseId::Upkeep));
        batch.events.push(ev_priority_changed(self));
        batch.events.push(ev_log(format!(
            "Game started — active P{}, priority P{} (upkeep).",
            self.state.active_player_id(),
            self.state.priority_player_id(),
        )));
        fill_legal(&mut batch, self);
        batch
    }

    /// Engine-owned card identity for the session: the union of all deck card ids mapped
    /// to Oracle names plus the mechanical info Servatrice needs without querying Oracle.
    /// Server-only — Servatrice strips it from client broadcasts (it enumerates decks).
    pub(super) fn ev_card_catalog(&self) -> RuledEvent {
        // BTreeSet: dedupe + deterministic entry order for replays.
        let ids: BTreeSet<&str> = self
            .state
            .objects
            .values()
            .map(|o| o.card_id.as_str())
            .collect();
        let entries = ids
            .into_iter()
            .filter_map(|id| self.registry.get(id))
            .map(|def| rv1::card_catalog::Entry {
                card_id: def.id.clone(),
                name: def.name.clone(),
                // Multi-face cards carry no flat types; describe by the primary (front) face.
                types: def.primary_face().types.to_vec(),
                is_permanent: def.primary_face().is_permanent(),
                // CR 709/712/715/720: per-face labels for cast choices and name lookup aliases;
                // empty for single-face cards.
                face_names: if def.is_multiface() {
                    def.faces.iter().map(|f| f.name.clone()).collect()
                } else {
                    Vec::new()
                },
                // Cockatrice's cards.xml stores Transform, Flip, and Modal DFC faces as separate
                // cards, while Split and Adventure cards have one whole-card entry. Keep the
                // physical CardRef mapping separate from the face labels used by cast choices.
                face_display_names: if def.is_multiface() {
                    match def.layout {
                        Layout::Transform | Layout::Flip | Layout::ModalDfc => {
                            def.faces.iter().map(|f| f.name.clone()).collect()
                        }
                        Layout::Split | Layout::Room | Layout::Adventure | Layout::Omen => {
                            vec![def.name.clone(); def.faces.len()]
                        }
                        Layout::Normal => Vec::new(),
                    }
                } else {
                    Vec::new()
                },
            })
            .collect();
        RuledEvent {
            ev: Some(rv1::ruled_event::Ev::CardCatalog(rv1::CardCatalog {
                entries,
            })),
        }
    }

    /// Deck + hand for Cockatrice server to line up with tricerules state.
    /// Build a `ManaPoolUpdated` event (CR 106) carrying player `idx`'s current absolute pool.
    pub(super) fn ev_mana_pool_updated(&self, idx: usize) -> RuledEvent {
        let p = &self.state.players[idx];
        let pool = &p.mana_pool;
        let mut grouped: BTreeMap<u32, ManaAmount> = BTreeMap::new();
        for contribution in &p.restricted_mana {
            let total = grouped
                .entry(contribution.restriction_group_id)
                .or_default();
            total.w += contribution.amount.w;
            total.u += contribution.amount.u;
            total.b += contribution.amount.b;
            total.r += contribution.amount.r;
            total.g += contribution.amount.g;
            total.c += contribution.amount.c;
        }
        let restricted_groups = grouped
            .into_iter()
            .filter_map(|(restriction_group_id, amount)| {
                let restriction = self
                    .state
                    .mana_restrictions
                    .get(restriction_group_id.checked_sub(1)? as usize)?;
                Some(rv1::RestrictedManaGroup {
                    restriction_group_id,
                    w: amount.w,
                    u: amount.u,
                    b: amount.b,
                    r: amount.r,
                    g: amount.g,
                    c: amount.c,
                    display_label: restriction.label.clone(),
                })
            })
            .collect();
        RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ManaPoolUpdated(
                rv1::ManaPoolUpdated {
                    player_id: p.id,
                    w: pool.white,
                    u: pool.blue,
                    b: pool.black,
                    r: pool.red,
                    g: pool.green,
                    c: pool.colorless,
                    restricted_groups,
                },
            )),
        }
    }

    /// A zone view with every player's hand and library spelled out in full.
    ///
    /// The startup path needs this: Servatrice seeds each player's physical deck and hand from the
    /// first view it sees, so that one can never be an omission. Every later view goes through
    /// [`GameEngine::ev_zone_view_sync_tracked`] instead.
    pub(super) fn ev_zone_view_sync(&mut self) -> RuledEvent {
        let battlefield_snapshot = self.battlefield_view_snapshot();
        let first_strike_step_pending = self.current_first_strike_step_pending();
        let per_player = self
            .state
            .players
            .iter()
            .enumerate()
            .map(|(idx, _)| self.per_player_view(idx, true, true, first_strike_step_pending))
            .collect();
        self.private_zone_cache = self
            .state
            .players
            .iter()
            .map(|player| {
                (
                    player.id,
                    PrivateZoneSnapshot {
                        hand: player.hand.clone(),
                        library: player.library.iter().copied().collect(),
                    },
                )
            })
            .collect();
        self.battlefield_view_cache = Some(battlefield_snapshot);
        self.first_strike_step_pending_cache = first_strike_step_pending;
        RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ZoneView(rv1::ZoneViewSync {
                per_player,
                battlefields_unchanged: false,
            })),
        }
    }

    /// The same zone view, but omitting the concealed zones of any player whose hand and library
    /// are unchanged since their last broadcast view (`private_zones_unchanged`).
    ///
    /// This is the emission path for every batch after startup. Most commands — priority passes,
    /// mana taps, phase rolls — touch neither zone, and re-sending ~60 library card ids per player
    /// per batch cost a clone in the engine, a serialization per participant in the relay, and an
    /// O(n²) card-by-card pool reconcile in Servatrice that concluded "identical" every time.
    ///
    /// The decision is per player, so a draw re-sends only the player who drew. Hand and library
    /// are omitted **jointly**: Servatrice reconciles them against a single pool of physical cards
    /// (deck zone + hand zone), so half a snapshot is not something it can apply.
    ///
    /// A batch may carry two views (the untap-step roll emits one mid-batch); the first updates the
    /// cache, so the second correctly reports unchanged.
    pub(super) fn ev_zone_view_sync_tracked(&mut self) -> RuledEvent {
        let battlefield_snapshot = self.battlefield_view_snapshot();
        let battlefields_unchanged =
            self.battlefield_view_cache.as_ref() == Some(&battlefield_snapshot);
        let first_strike_step_pending = if battlefields_unchanged {
            self.first_strike_step_pending_cache
        } else {
            self.current_first_strike_step_pending()
        };
        let mut per_player = Vec::with_capacity(self.state.players.len());
        for idx in 0..self.state.players.len() {
            let p = &self.state.players[idx];
            let current = PrivateZoneSnapshot {
                hand: p.hand.to_vec(),
                library: p.library.iter().copied().collect(),
            };
            let unchanged = self.private_zone_cache.get(&p.id) == Some(&current);
            let mut view = self.per_player_view(
                idx,
                !unchanged,
                !battlefields_unchanged,
                first_strike_step_pending,
            );
            if unchanged {
                view.private_zones_unchanged = true;
            } else {
                self.private_zone_cache.insert(view.player_id, current);
            }
            per_player.push(view);
        }
        if !battlefields_unchanged {
            self.battlefield_view_cache = Some(battlefield_snapshot);
            self.first_strike_step_pending_cache = first_strike_step_pending;
        }
        RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ZoneView(rv1::ZoneViewSync {
                per_player,
                battlefields_unchanged,
            })),
        }
    }

    /// One player's zone view. `include_private` fills the server-only hand and library; when
    /// false the caller is asserting they are unchanged and sets `private_zones_unchanged`.
    fn per_player_view(
        &self,
        idx: usize,
        include_private: bool,
        include_battlefield: bool,
        first_strike_step_pending: bool,
    ) -> rv1::RuledPerPlayerView {
        let p = &self.state.players[idx];
        rv1::RuledPerPlayerView {
            player_id: p.id,
            private_zones_unchanged: false,
            hand_cards: if include_private {
                p.hand
                    .iter()
                    .map(|&oid| {
                        let card_id = self
                            .state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default();
                        rv1::HandCard {
                            card_id,
                            object_id: oid,
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            },
            library_cards: if include_private {
                p.library
                    .iter()
                    .map(|&oid| {
                        let card_id = self
                            .state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default();
                        rv1::LibraryCard {
                            card_id,
                            object_id: oid,
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            },
            battlefield_objects: if include_battlefield {
                p.battlefield
                    .iter()
                    .map(|&oid| {
                        let Some(object) = self.state.objects.get(&oid) else {
                            return rv1::BattlefieldObject {
                                object_id: oid,
                                ..Default::default()
                            };
                        };
                        let characteristics = self.characteristics(oid);
                        let is_creature = characteristics
                            .as_ref()
                            .is_some_and(Characteristics::is_creature);
                        let is_land = characteristics
                            .as_ref()
                            .is_some_and(|value| value.has_type("Land"));
                        let face = self.effective_face(oid);
                        let rules_annotation_labels = self.rules_annotation_labels(
                            oid,
                            object,
                            characteristics.as_ref(),
                            face.as_deref(),
                        );
                        let activated_abilities = self
                            .effective_activated_abilities(oid)
                            .into_iter()
                            .map(|(ability_index, ability, _)| {
                                let mana_cost = ability
                                    .costs
                                    .iter()
                                    .find_map(|cost| match cost {
                                        AbilityCost::Mana(cost) => Some(cost.to_string()),
                                        _ => None,
                                    })
                                    .unwrap_or_default();
                                let mana_produced = self
                                    .active_mana_options(oid, &ability)
                                    .map(|options| {
                                        options
                                            .iter()
                                            .map(mana_amount_symbols)
                                            .collect::<Vec<_>>()
                                            .join("/")
                                    })
                                    .unwrap_or_default();
                                let cost_label = ability
                                    .costs
                                    .iter()
                                    .map(|cost| match cost {
                                        AbilityCost::Tap => "{T}".to_string(),
                                        AbilityCost::Loyalty(delta) if *delta >= 0 => {
                                            format!("+{delta}")
                                        }
                                        AbilityCost::Loyalty(delta) => delta.to_string(),
                                        AbilityCost::Mana(cost) => cost.to_string(),
                                        AbilityCost::Discard => "Discard a card".to_string(),
                                        AbilityCost::DiscardSelf => "Discard this card".to_string(),
                                        AbilityCost::ExileSelf => "Exile this card".to_string(),
                                        AbilityCost::SacrificeSelf => "Sacrifice this".to_string(),
                                        AbilityCost::SacrificePermanent { .. } => {
                                            "Sacrifice a permanent".to_string()
                                        }
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
                                    activatable: self.ability_activatable(
                                        oid,
                                        ability_index,
                                        &ability,
                                    ),
                                }
                            })
                            .collect();
                        let keywords = [
                            tricerules_cards::Keyword::Flying,
                            tricerules_cards::Keyword::Reach,
                            tricerules_cards::Keyword::Intimidate,
                            tricerules_cards::Keyword::Vigilance,
                            tricerules_cards::Keyword::Lifelink,
                            tricerules_cards::Keyword::Haste,
                            tricerules_cards::Keyword::Deathtouch,
                            tricerules_cards::Keyword::Menace,
                            tricerules_cards::Keyword::Trample,
                            tricerules_cards::Keyword::FirstStrike,
                            tricerules_cards::Keyword::DoubleStrike,
                            tricerules_cards::Keyword::Indestructible,
                            tricerules_cards::Keyword::Hexproof,
                            tricerules_cards::Keyword::Shroud,
                            tricerules_cards::Keyword::Defender,
                            tricerules_cards::Keyword::Flash,
                        ]
                        .into_iter()
                        .filter(|&keyword| {
                            characteristics
                                .as_ref()
                                .is_some_and(|value| value.has_keyword(keyword))
                        })
                        .map(|keyword| match keyword {
                            tricerules_cards::Keyword::FirstStrike => "FirstStrike".to_string(),
                            tricerules_cards::Keyword::DoubleStrike => "DoubleStrike".to_string(),
                            _ => keyword.as_str().to_string(),
                        })
                        .collect();
                        let effective_display_name = if object.face_down {
                            "Face-down creature".to_string()
                        } else {
                            object
                                .copiable_values
                                .as_ref()
                                .map(|values| values.display_name.clone())
                                .or_else(|| {
                                    self.registry.get(&object.card_id).and_then(|definition| {
                                        definition
                                            .face_display_name(object.face_up_index)
                                            .map(str::to_string)
                                    })
                                })
                                .unwrap_or_default()
                        };
                        let copy_annotation = if object.face_down {
                            String::new()
                        } else {
                            object
                                .copiable_values
                                .as_ref()
                                .and_then(|_| self.registry.get(&object.card_id))
                                .map(|definition| format!("Copy: {}", definition.name))
                                .unwrap_or_default()
                        };
                        let room_doors = self
                            .state
                            .room_states
                            .get(&oid)
                            .copied()
                            .zip(self.room_faces(oid))
                            .map(|(room, faces)| {
                                faces
                                    .iter()
                                    .enumerate()
                                    .map(|(face_index, face)| rv1::RoomDoorState {
                                        face_index: face_index as u32,
                                        name: face.name.clone(),
                                        unlocked: room.unlocked[face_index],
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        rv1::BattlefieldObject {
                            object_id: oid,
                            card_id: object.card_id.clone(),
                            tapped: object.tapped,
                            summoning_sick: object.summoning_sick,
                            is_creature,
                            power: if is_creature {
                                characteristics
                                    .as_ref()
                                    .and_then(|value| value.power)
                                    .unwrap_or(0)
                            } else {
                                0
                            },
                            toughness: if is_creature {
                                characteristics
                                    .as_ref()
                                    .and_then(|value| value.toughness)
                                    .unwrap_or(0)
                            } else {
                                0
                            },
                            damage: if is_creature { object.damage } else { 0 },
                            keywords,
                            activated_abilities,
                            counters_annotation: object.counter_annotation(),
                            attachment_recipient: object
                                .attached_to
                                .map(attachment_recipient_proto),
                            face_up_index: object.face_up_index as u32,
                            // CR 108.3. The per-player view already says who *controls* this
                            // permanent (it is listed under its controller); the owner is the
                            // half the relay needs for the "Owner:" annotation and for routing
                            // the card home when it leaves the battlefield.
                            owner_player_id: object.owner,
                            rules_annotation_labels,
                            effective_display_name,
                            copy_annotation,
                            face_down: object.face_down,
                            zone_change_generation: self
                                .state
                                .zone_change_generation
                                .get(&oid)
                                .copied()
                                .unwrap_or(0),
                            room_doors,
                            is_land,
                            is_planeswalker: characteristics
                                .as_ref()
                                .is_some_and(|value| value.has_type("Planeswalker")),
                            is_battle: characteristics
                                .as_ref()
                                .is_some_and(|value| value.has_type("Battle")),
                            loyalty: object.counter_count(CounterKind::Loyalty),
                            defense: object.counter_count(CounterKind::Defense),
                            battle_protector_player_id: self
                                .state
                                .battle_protectors
                                .get(&oid)
                                .copied(),
                            controller_player_id: Some(object.controller),
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            },
            // CR 510.4: true while combat is set up with at least one attacker or blocker
            // having FirstStrike/DoubleStrike and the first-strike step has not yet resolved.
            first_strike_step_pending,
            // Engine ObjectIds for each card in this player's graveyard (in graveyard order).
            graveyard_object_ids: p.graveyard.clone(),
            // Engine ObjectIds for each card in this player's public exile zone.
            exile_object_ids: p.exile.clone(),
        }
    }

    fn battlefield_view_snapshot(&self) -> BattlefieldViewSnapshot {
        let players = self
            .state
            .players
            .iter()
            .map(|player| PlayerBattlefieldSnapshot {
                player_id: player.id,
                object_ids: player.battlefield.clone(),
                objects: player
                    .battlefield
                    .iter()
                    .filter_map(|oid| self.state.objects.get(oid))
                    .map(|object| BattlefieldObjectSnapshot {
                        object_id: object.id,
                        card_id: object.card_id.clone(),
                        owner: object.owner,
                        controller: object.controller,
                        zone: object.zone,
                        tapped: object.tapped,
                        summoning_sick: object.summoning_sick,
                        power: object.power,
                        toughness: object.toughness,
                        damage: object.damage,
                        counters: object.counters.clone(),
                        attached_to: object.attached_to,
                        face_up_index: object.face_up_index,
                        face_down: object.face_down,
                        room_state: self.state.room_states.get(&object.id).copied(),
                        battle_protector: self.state.battle_protectors.get(&object.id).copied(),
                        zone_change_generation: self
                            .state
                            .zone_change_generation
                            .get(&object.id)
                            .copied()
                            .unwrap_or(0),
                        copy_revision: object.copy_revision,
                    })
                    .collect(),
            })
            .collect();
        BattlefieldViewSnapshot {
            players,
            continuous_effects: self.state.continuous_effects.clone(),
            activation_uses_this_turn: self.state.activation_uses_this_turn.clone(),
            activation_uses_per_object: self.state.activation_uses_per_object.clone(),
            turn_history: self.state.turn_history.clone(),
            active_player: self.state.active_player_id(),
            turn_step: self.state.turn_step,
            stack_empty: self.state.stack.is_empty(),
            combat: self.state.combat.as_ref().map(|combat| {
                let mut blockers: Vec<_> = combat
                    .blockers
                    .iter()
                    .map(|(&attacker, blockers)| (attacker, blockers.clone()))
                    .collect();
                blockers.sort_by_key(|(attacker, _)| *attacker);
                BattlefieldCombatSnapshot {
                    attacking: combat.attacking.clone(),
                    blockers,
                    first_strike_damage_done: combat.first_strike_damage_done,
                }
            }),
        }
    }

    fn current_first_strike_step_pending(&self) -> bool {
        self.state.combat.as_ref().is_some_and(|combat| {
            !combat.first_strike_damage_done && combat::combat_needs_first_strike_step(self, combat)
        })
    }
}

pub(super) fn ev_game_over(winner: PlayerId) -> RuledEvent {
    ev_log(format!("Game over. Winner: {winner}"))
}

/// Render a color set as Cockatrice's lowercase WUBRG color string (e.g. `[White, Blue]` → "wu",
/// colorless → ""). Used to populate the token identity the relay feeds to Event_CreateToken.
pub(super) fn color_string(colors: &[Color]) -> String {
    // Canonical WUBRG order regardless of input order.
    [
        (Color::White, 'w'),
        (Color::Blue, 'u'),
        (Color::Black, 'b'),
        (Color::Red, 'r'),
        (Color::Green, 'g'),
    ]
    .iter()
    .filter(|(c, _)| colors.contains(c))
    .map(|(_, ch)| ch)
    .collect()
}

pub(super) fn object_display_name(
    state: &GameState,
    registry: &CardRegistry,
    oid: ObjectId,
) -> String {
    state
        .objects
        .get(&oid)
        .and_then(|object| {
            object
                .copiable_values
                .as_ref()
                .map(|values| values.display_name.clone())
                .or_else(|| registry.get(&object.card_id).map(|d| d.name.clone()))
        })
        .unwrap_or_else(|| format!("[object {}]", oid))
}

fn describe_target_for_log(state: &GameState, registry: &CardRegistry, tid: ObjectId) -> String {
    if state.player_idx(tid as i32).is_some() {
        format!("P{tid}")
    } else {
        object_display_name(state, registry, tid)
    }
}

pub(super) fn format_spell_targets_log(
    state: &GameState,
    registry: &CardRegistry,
    targets: &[ObjectId],
) -> String {
    if targets.is_empty() {
        String::new()
    } else {
        let s: Vec<String> = targets
            .iter()
            .map(|&t| describe_target_for_log(state, registry, t))
            .collect();
        format!(" — {}", s.join(", "))
    }
}

pub(super) fn default_deck_list(player_index: usize) -> Vec<String> {
    if player_index == 0 {
        let mut d: Vec<String> = std::iter::repeat_n("mountain".into(), 20).collect();
        d.extend(std::iter::repeat_n("lightning_bolt".into(), 20));
        d.extend(std::iter::repeat_n("grizzly_bears".into(), 20));
        d.truncate(60);
        d
    } else {
        let mut d: Vec<String> = std::iter::repeat_n("forest".into(), 20).collect();
        d.extend(std::iter::repeat_n("giant_growth".into(), 20));
        d.extend(std::iter::repeat_n("counterspell".into(), 20));
        d.truncate(60);
        d
    }
}

pub(super) fn finish_with_events(eng: &GameEngine, events: Vec<RuledEvent>) -> RuledEventBatch {
    let mut b = RuledEventBatch {
        events,
        legal_by_player: Default::default(),
    };
    legal_actions::fill_legal(&mut b, eng);
    b
}

/// Render one [`ManaAmount`] as a brace-less symbol run for the zone view's mana-produced field
/// (e.g. `{g:1}` → `"G"`, `{c:2}` → `"CC"`, `{w:1,u:1}` → `"WU"`). Order W U B R G C is canonical.
pub(super) fn mana_amount_symbols(a: &tricerules_cards::ManaAmount) -> String {
    let mut s = String::new();
    for (sym, n) in [
        ('W', a.w),
        ('U', a.u),
        ('B', a.b),
        ('R', a.r),
        ('G', a.g),
        ('C', a.c),
    ] {
        for _ in 0..n {
            s.push(sym);
        }
    }
    s
}

pub(super) fn ev_log(text: String) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::Log(rv1::LogMessage {
            text,
            visible_to_player_id: None,
            hidden_from_player_id: None,
        })),
    }
}

pub(super) fn ev_log_private(text: String, player_id: i32) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::Log(rv1::LogMessage {
            text,
            visible_to_player_id: Some(player_id),
            hidden_from_player_id: None,
        })),
    }
}

pub(super) fn ev_log_hidden_from(text: String, player_id: i32) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::Log(rv1::LogMessage {
            text,
            visible_to_player_id: None,
            hidden_from_player_id: Some(player_id),
        })),
    }
}

/// CR 603.3b: ask `deciding_player` for the order their simultaneous triggers go on the stack.
///
/// Self-describing by design — the candidates are not on the stack yet, so a client cannot resolve
/// them to anything it already holds. `source_card_name` is carried rather than looked up because a
/// dies trigger's source may have left the battlefield in the very event that triggered it
/// (CR 603.6/603.10).
pub(super) fn ev_trigger_order_required(
    deciding_player: PlayerId,
    candidates: &[StagedTrigger],
) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::TriggerOrderRequired(
            rv1::TriggerOrderRequired {
                deciding_player_id: deciding_player,
                candidates: candidates
                    .iter()
                    .map(|staged| rv1::TriggerOrderCandidate {
                        trigger_object_id: staged.object_id,
                        source_permanent_id: staged.source_permanent_id,
                        ability_index: staged.ability_index as u32,
                        source_card_name: staged.card_name.clone(),
                        ability_text: staged.ability_text.clone(),
                    })
                    .collect(),
            },
        )),
    }
}

pub(super) fn ev_phase(eng: &GameEngine, phase: rv1::PhaseId) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PhaseChanged(rv1::PhaseChanged {
            phase_id: phase as i32,
            active_player_id: eng.state.active_player_id(),
        })),
    }
}

pub(super) fn ev_priority_changed(eng: &GameEngine) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PriorityChanged(
            rv1::PriorityChanged {
                player_id: eng.state.priority_player_id(),
            },
        )),
    }
}
