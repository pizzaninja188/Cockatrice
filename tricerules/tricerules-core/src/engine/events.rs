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
                .push(ev_phase_labeled(self, "opening_choose_first"));
            batch.events.push(ev_priority_changed(self));
            batch
                .events
                .push(ev_log(format!("P{} chooses who goes first.", op.chooser)));
            fill_legal(&mut batch, self);
            return batch;
        }
        batch.events.push(ev_phase_labeled(self, "upkeep"));
        batch.events.push(ev_priority_changed(self));
        batch.events.push(ev_log(format!(
            "Game started — active P{}, priority P{} (upkeep).",
            self.state.active_player_id(),
            self.state.priority_player_id(),
        )));
        fill_legal(&mut batch, self);
        batch
    }

    pub fn game_over_batch_winner(&self, w: PlayerId) -> RuledEventBatch {
        let mut b = RuledEventBatch::default();
        b.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::Log(rv1::LogMessage {
                text: format!("Game over. Winner: {w}"),
            })),
        });
        b
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
                hand: p
                    .hand
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect(),
                hand_object_id: p.hand.clone(),
                lib_ids_csv: p
                    .library
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join(","),
                battlefield: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect(),
                battlefield_tapped: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.tapped)
                            .unwrap_or(false)
                    })
                    .collect(),
                battlefield_object_id: p.battlefield.to_vec(),
                battlefield_summoning_sick: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.summoning_sick)
                            .unwrap_or(false)
                    })
                    .collect(),
                battlefield_power: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        if self
                            .state
                            .objects
                            .get(&oid)
                            .is_some_and(|o| o.is_creature(self.registry))
                        {
                            self.effective_power(oid).unwrap_or(0)
                        } else {
                            0
                        }
                    })
                    .collect(),
                battlefield_toughness: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        if self
                            .state
                            .objects
                            .get(&oid)
                            .is_some_and(|o| o.is_creature(self.registry))
                        {
                            self.effective_toughness(oid).unwrap_or(0)
                        } else {
                            0
                        }
                    })
                    .collect(),
                battlefield_damage: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .filter(|o| o.is_creature(self.registry))
                            .map_or(0, |o| o.damage)
                    })
                    .collect(),
                battlefield_is_creature: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.is_creature(self.registry))
                            .unwrap_or(false)
                    })
                    .collect(),
                // CR 702.10: clients use this to suppress the summoning-sick indicator
                // and allow attacker selection for creatures that entered this turn.
                battlefield_haste: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.has_keyword(self.registry, tricerules_cards::Keyword::Haste))
                            .unwrap_or(false)
                    })
                    .collect(),
                // CR 702.19: clients use this to enable trample damage assignment UI.
                battlefield_trample: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| {
                                o.has_keyword(self.registry, tricerules_cards::Keyword::Trample)
                            })
                            .unwrap_or(false)
                    })
                    .collect(),
                // CR 702.7: informational flag for the client UI (independent of pending state).
                battlefield_first_strike: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| {
                                o.has_keyword(self.registry, tricerules_cards::Keyword::FirstStrike)
                            })
                            .unwrap_or(false)
                    })
                    .collect(),
                // CR 702.4: informational flag for the client UI.
                battlefield_double_strike: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| {
                                o.has_keyword(
                                    self.registry,
                                    tricerules_cards::Keyword::DoubleStrike,
                                )
                            })
                            .unwrap_or(false)
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
                            && combat::combat_needs_first_strike_step(&self.state, self.registry, c)
                    })
                    .unwrap_or(false),
                // Pipe-delimited activated ability texts per battlefield permanent (empty if none).
                battlefield_activated_ability_texts: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .and_then(|o| self.registry.get(&o.card_id))
                            .map(|def| {
                                def.activated_abilities
                                    .iter()
                                    .map(|a| a.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join("|")
                            })
                            .unwrap_or_default()
                    })
                    .collect(),
                // Parallel to `battlefield_activated_ability_texts`: pipe-delimited mana cost
                // strings extracted from AbilityCost in canonical Scryfall brace form. Tap/Sacrifice
                // → "", Mana/TapAndMana → "{4}"/"{R}"/etc. The client parses both braces and the
                // legacy compact form (see PlayerActions::parseSimpleManaCost).
                battlefield_activated_ability_mana_costs: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .and_then(|o| self.registry.get(&o.card_id))
                            .map(|def| {
                                def.activated_abilities
                                    .iter()
                                    .map(|a| match &a.cost {
                                        AbilityCost::Mana(c) | AbilityCost::TapAndMana(c) => {
                                            c.to_string()
                                        }
                                        _ => String::new(),
                                    })
                                    .collect::<Vec<_>>()
                                    .join("|")
                            })
                            .unwrap_or_default()
                    })
                    .collect(),
                // Parallel to `battlefield_activated_ability_texts`: mana-ability production (CR 605).
                battlefield_activated_ability_mana_produced: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .and_then(|o| self.registry.get(&o.card_id))
                            .map(|def| {
                                def.activated_abilities
                                    .iter()
                                    .map(|a| match &a.effect {
                                        SpellEffectKind::ProduceMana { options } => options
                                            .iter()
                                            .map(mana_amount_symbols)
                                            .collect::<Vec<_>>()
                                            .join("/"),
                                        _ => String::new(),
                                    })
                                    .collect::<Vec<_>>()
                                    .join("|")
                            })
                            .unwrap_or_default()
                    })
                    .collect(),
                // Parallel to `battlefield_activated_ability_texts`: pipe-delimited display-cost
                // labels for each ability (e.g. "{T}", "{4}", "{T}, {4}", "Sacrifice this").
                // Used by the client to build "cost: text" labels in the activation context menu.
                battlefield_activated_ability_cost_labels: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .and_then(|o| self.registry.get(&o.card_id))
                            .map(|def| {
                                def.activated_abilities
                                    .iter()
                                    .map(|a| match &a.cost {
                                        AbilityCost::Tap => "{T}".to_string(),
                                        AbilityCost::Mana(c) => c.to_string(),
                                        AbilityCost::TapAndMana(c) => {
                                            format!("{{T}}, {c}")
                                        }
                                        AbilityCost::Sacrifice => "Sacrifice this".to_string(),
                                    })
                                    .collect::<Vec<_>>()
                                    .join("|")
                            })
                            .unwrap_or_default()
                    })
                    .collect(),
                // Parallel to `battlefield`: per-permanent counter annotation for client display
                // (e.g. "1 +1/+1 counter(s)"). Empty when the permanent has no counters.
                battlefield_counters_annotation: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.counter_annotation())
                            .unwrap_or_default()
                    })
                    .collect(),
            })
            .collect();
        RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ZoneView(rv1::ZoneViewSync {
                per_player,
            })),
        }
    }
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
        ev: Some(rv1::ruled_event::Ev::Log(rv1::LogMessage { text })),
    }
}

pub(super) fn ev_phase_labeled(eng: &GameEngine, name: &str) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PhaseChanged(rv1::PhaseChanged {
            phase: name.to_string(),
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
