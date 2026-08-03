use super::legal_actions::fill_legal;
use super::*;

impl GameEngine {
    pub fn initial_response_batch(&self) -> RuledEventBatch {
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
                // CR 709/712/715: per-face names so the relay can display a cast/active half;
                // empty for single-face cards.
                face_names: if def.is_multiface() {
                    def.faces.iter().map(|f| f.name.clone()).collect()
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
                },
            )),
        }
    }

    pub(super) fn ev_zone_view_sync(&self) -> RuledEvent {
        let per_player: Vec<rv1::RuledPerPlayerView> = self
            .state
            .players
            .iter()
            .map(|p| rv1::RuledPerPlayerView {
                player_id: p.id,
                hand_cards: p
                    .hand
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
                    .collect(),
                library_card_ids: p
                    .library
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>(),
                battlefield_objects: p
                    .battlefield
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
                        let face = self
                            .registry
                            .get(&object.card_id)
                            .and_then(|definition| definition.face(object.face_up_index));
                        let activated_abilities = face
                            .map(|face| {
                                face.activated_abilities
                                    .iter()
                                    .map(|ability| {
                                        let mana_cost = match &ability.cost {
                                            AbilityCost::Mana(cost)
                                            | AbilityCost::TapAndMana(cost) => cost.to_string(),
                                            _ => String::new(),
                                        };
                                        let mana_produced = ability
                                            .mana_options()
                                            .map(|options| {
                                                options
                                                    .iter()
                                                    .map(mana_amount_symbols)
                                                    .collect::<Vec<_>>()
                                                    .join("/")
                                            })
                                            .unwrap_or_default();
                                        let cost_label = match &ability.cost {
                                            AbilityCost::Tap => "{T}".to_string(),
                                            AbilityCost::Mana(cost) => cost.to_string(),
                                            AbilityCost::TapAndMana(cost) => {
                                                format!("{{T}}, {cost}")
                                            }
                                            AbilityCost::Sacrifice => "Sacrifice this".to_string(),
                                        };
                                        rv1::AbilityInfo {
                                            text: ability.text.clone(),
                                            mana_cost,
                                            mana_produced,
                                            cost_label,
                                            activatable: self.ability_activatable(oid, ability),
                                        }
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
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
                            attached_to_oid: object.attached_to.unwrap_or(0),
                            face_up_index: object.face_up_index as u32,
                            // CR 108.3. The per-player view already says who *controls* this
                            // permanent (it is listed under its controller); the owner is the
                            // half the relay needs for the "Owner:" annotation and for routing
                            // the card home when it leaves the battlefield.
                            owner_player_id: object.owner,
                        }
                    })
                    .collect(),
                // CR 510.4: true while combat is set up with at least one attacker or blocker
                // having FirstStrike/DoubleStrike and the first-strike step has not yet resolved.
                first_strike_step_pending: self
                    .state
                    .combat
                    .as_ref()
                    .map(|c| {
                        !c.first_strike_damage_done
                            && combat::combat_needs_first_strike_step(self, c)
                    })
                    .unwrap_or(false),
                // Engine ObjectIds for each card in this player's graveyard (in graveyard order).
                graveyard_object_ids: p.graveyard.clone(),
            })
            .collect();
        RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ZoneView(rv1::ZoneViewSync {
                per_player,
            })),
        }
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
        .and_then(|o| registry.get(&o.card_id))
        .map(|d| d.name.clone())
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
fn mana_amount_symbols(a: &tricerules_cards::ManaAmount) -> String {
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
