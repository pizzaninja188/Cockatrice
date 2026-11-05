// Strict normal-session As Foretold pilots. Every mutation is a captured player command.
use super::*;

const SEED: u64 = 3;

struct Pilot {
    session: EngineSession,
    root: std::path::PathBuf,
    rows: Vec<String>,
    manifest: Value,
    latest: IpcResponse,
    battlefield: Vec<BattlefieldObject>,
    opponent_battlefield: Vec<BattlefieldObject>,
    accepted: usize,
}

impl Pilot {
    fn new(game_id: u64) -> Self {
        let mut cards = Vec::new();
        for name in [
            "As Foretold",
            "Lightning Bolt",
            "Bonecrusher Giant // Stomp",
            "Tormenting Voice",
        ] {
            cards.extend(vec![name.to_string(); 4]);
        }
        cards.extend(vec!["Island".to_string(); 44]);
        let decks = vec![(17, cards), (29, vec!["Forest".to_string(); 60])]
            .into_iter()
            .map(|(player_id, mainboard_card_name)| {
                assert_eq!(mainboard_card_name.len(), 60);
                PlayerDeck {
                    player_id,
                    mainboard_card_name,
                    commander_setup: Some(CommanderSetup {
                        declaration: Some(commander_setup::Declaration::NoCommander(true)),
                    }),
                    ..Default::default()
                }
            })
            .collect::<Vec<_>>();
        let mut session = EngineSession::new(false);
        let validation = session
            .process(&IpcEnvelope {
                msg: Some(Msg::ValidateDeck(ValidateDeck {
                    card_names: decks
                        .iter()
                        .flat_map(|deck| deck.mainboard_card_name.iter().cloned())
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    ..Default::default()
                })),
            })
            .unwrap();
        assert!(validation.ok && validation.missing_card_names.is_empty());
        let captures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/deck-coverage/as-foretold-implementation-20261103/normal-captures");
        fs::create_dir_all(&captures).unwrap();
        let root = tempfile::Builder::new()
            .prefix(&format!("as-foretold-{game_id}-"))
            .tempdir_in(captures)
            .unwrap()
            .keep();
        fs::create_dir(root.join("raw")).unwrap();
        println!(
            "As Foretold retained_capture={} fixed_seed={SEED} decks={decks:?}",
            root.display()
        );
        let mut pilot = Self {
            session,
            root,
            rows: Vec::new(),
            manifest: serde_json::json!({
                "format_version":1, "source":"server", "privacy":"server_only", "capture_id":format!("as-foretold-{game_id}"),
                "engine_build":ENGINE_BUILD, "card_data_hash":"", "effective_dev_commands_enabled":false, "complete":false,
            }),
            latest: IpcResponse::default(),
            battlefield: Vec::new(),
            opponent_battlefield: Vec::new(),
            accepted: 0,
        };
        pilot.request(IpcEnvelope {
            msg: Some(Msg::SessionStart(SessionStart {
                game_id,
                seed: SEED,
                player_ids: vec![17, 29],
                player_decks: decks,
                dev_commands_enabled: false,
                diagnostic_capture_enabled: true,
                ..Default::default()
            })),
        });
        assert!(!pilot.latest.effective_dev_commands_enabled);
        pilot.manifest["card_data_hash"] = pilot.latest.card_data_hash.clone().into();
        let starting = kaya_starting_player_offer(&pilot.latest).unwrap();
        pilot.command(
            starting.deciding_player_id,
            Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                starting_player_id: 17,
            }),
        );
        for _ in 0..2 {
            let Some(opening) = kaya_mulligan_offer(&pilot.latest) else {
                break;
            };
            pilot.command(
                opening.deciding_player_id,
                Cmd::Mulligan(MulliganDecision { keep: true }),
            );
        }
        assert!(kaya_mulligan_offer(&pilot.latest).is_none());
        pilot
    }

    fn request(&mut self, request: IpcEnvelope) {
        let response = self
            .session
            .process(&request)
            .expect("normal connection remains open");
        pair(&self.root, &mut self.rows, &request, &response);
        assert!(
            response.ok,
            "normal As Foretold request rejected: {}",
            response.error
        );
        assert!(!response.effective_dev_commands_enabled);
        if matches!(request.msg, Some(Msg::PlayerCommand(_))) {
            self.accepted += 1;
        }
        assert!(self.accepted <= 8192);
        kaya_capture_battlefield(&response, 17, &mut self.battlefield);
        kaya_capture_battlefield(&response, 29, &mut self.opponent_battlefield);
        self.latest = response;
    }

    fn command(&mut self, actor: i32, cmd: Cmd) {
        self.request(IpcEnvelope {
            msg: Some(Msg::PlayerCommand(PlayerCommand {
                player_id: actor,
                ruled_command: RuledCommand { cmd: Some(cmd) }.encode_to_vec(),
            })),
        });
    }

    fn state(&self) -> &tricerules_core::state::GameState {
        &self.session.engine.as_ref().unwrap().state
    }

    fn advance_main(&mut self, player: i32) {
        for _ in 0..512 {
            let state = self.state();
            assert!(
                state.pending_resolution.is_none(),
                "answer choices explicitly before advancing"
            );
            if state.turn_step == tricerules_core::TurnStep::Main1
                && state.active_player_id() == player
                && state.priority_player_id() == player
                && state.stack.is_empty()
            {
                return;
            }
            let actor = state.priority_player_id();
            let excess = if state.turn_step == tricerules_core::TurnStep::Cleanup
                && !state.cleanup_priority_active
                && state.cleanup_discard_player == Some(actor)
            {
                state.players[state.player_idx(actor).unwrap()]
                    .hand
                    .len()
                    .saturating_sub(7)
            } else {
                0
            };
            if excess > 0 {
                let mut offers = kaya_batch(&self.latest).legal_by_player[&actor]
                    .hand_actions
                    .iter()
                    .filter(|offer| offer.kind == HandActionKind::HandActionCleanupDiscard as i32)
                    .collect::<Vec<_>>();
                offers.sort_by_key(|offer| {
                    (
                        !matches!(offer.card_name.as_str(), "Island" | "Plains" | "Forest"),
                        offer.hand_index,
                    )
                });
                let indices = offers
                    .iter()
                    .take(excess)
                    .map(|offer| offer.hand_index)
                    .collect::<Vec<_>>();
                assert_eq!(indices.len(), excess);
                self.command(
                    actor,
                    Cmd::DiscardToHandSize(DiscardToHandSize {
                        hand_card_indices: indices,
                    }),
                );
            } else {
                self.command(actor, Cmd::PassPriority(PassPriority {}));
            }
        }
        panic!("normal As Foretold driver did not reach requested Main1");
    }

    fn play_land(&mut self, actor: i32) {
        let preferred = if actor == 29 { "Forest" } else { "Island" };
        let offer = kaya_batch(&self.latest).legal_by_player[&actor]
            .hand_actions
            .iter()
            .filter(|a| a.kind == HandActionKind::HandActionPlayLand as i32)
            .min_by_key(|a| (a.card_name != preferred, a.hand_index))
            .cloned();
        if let Some(offer) = offer {
            self.command(
                actor,
                Cmd::PlayLand(PlayLand {
                    source: Some(LandSource {
                        location: Some(
                            tricerules_proto::ruled::v1::land_source::Location::HandIndex(
                                offer.hand_index,
                            ),
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            );
        }
    }
    fn can_cast(&self, actor: i32, name: &str, colors: &[char], generic: usize) -> bool {
        let board = if actor == 17 {
            &self.battlefield
        } else {
            &self.opponent_battlefield
        };
        let lands = board
            .iter()
            .filter(|p| !p.tapped && matches!(p.card_id.as_str(), "forest" | "plains" | "island"))
            .collect::<Vec<_>>();
        kaya_batch(&self.latest).legal_by_player[&actor]
            .hand_actions
            .iter()
            .any(|a| {
                a.card_name == name
                    && a.kind == HandActionKind::HandActionCastSpell as i32
                    && a.cast_method == CastMethod::Normal as i32
            })
            && lands.len() >= colors.len() + generic
            && colors.iter().all(|c| {
                lands
                    .iter()
                    .filter(|p| land_color(&p.card_id) == *c)
                    .count()
                    >= colors.iter().filter(|v| *v == c).count()
            })
    }
    fn land_mana(&mut self, actor: i32, color: Option<char>) -> char {
        let board = if actor == 17 {
            &self.battlefield
        } else {
            &self.opponent_battlefield
        };
        let (oid, generation, index, produced) = board
            .iter()
            .filter(|p| !p.tapped && matches!(p.card_id.as_str(), "forest" | "plains" | "island"))
            .flat_map(|p| {
                p.activated_abilities
                    .iter()
                    .filter(|a| a.is_mana_ability && a.activatable)
                    .map(move |a| {
                        (
                            p.object_id,
                            p.zone_change_generation,
                            a.ability_index,
                            land_color(&p.card_id),
                        )
                    })
            })
            .find(|(_, _, _, c)| color.is_none_or(|required| required == *c))
            .unwrap();
        self.command(
            actor,
            Cmd::ActivateAbility(ActivateAbility {
                source_object_id: oid,
                expected_zone_change_generation: generation,
                ability_index: index,
                source_zone: AbilitySourceZone::Battlefield as i32,
                ..Default::default()
            }),
        );
        produced
    }
    fn cast(
        &mut self,
        actor: i32,
        name: &str,
        colors: &[char],
        generic: usize,
        method: CastMethod,
    ) -> u32 {
        let offer = kaya_batch(&self.latest).legal_by_player[&actor]
            .hand_actions
            .iter()
            .find(|a| {
                a.card_name == name
                    && a.kind == HandActionKind::HandActionCastSpell as i32
                    && a.cast_method == method as i32
            })
            .unwrap()
            .clone();
        let object = self.state().players[self.state().player_idx(actor).unwrap()].hand
            [offer.hand_index as usize];
        self.command(
            actor,
            Cmd::BeginSpellCast(BeginSpellCast {
                announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                    source: Some(CastSource {
                        location: Some(
                            tricerules_proto::ruled::v1::cast_source::Location::HandIndex(
                                offer.hand_index,
                            ),
                        ),
                        expected_zone_change_generation: Some(
                            self.state()
                                .zone_change_generation
                                .get(&object)
                                .copied()
                                .unwrap_or(0),
                        ),
                    }),
                    face_index: offer.face_index,
                    cast_method: method as i32,
                    ..Default::default()
                }),
            }),
        );
        let transaction_id = self
            .state()
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id;
        let mut mana = PaymentMana::default();
        for color in colors
            .iter()
            .copied()
            .map(Some)
            .chain(std::iter::repeat_n(None, generic))
        {
            match self.land_mana(actor, color) {
                'G' => mana.g += 1,
                'W' => mana.w += 1,
                'U' => mana.u += 1,
                _ => unreachable!(),
            }
        }
        let revision = self.state().command_index;
        let proposal = CommitSpellCast {
            transaction_id,
            payment: Some(PaymentSelection {
                expected_state_revision: revision,
                mana: Some(mana),
                ..Default::default()
            }),
            ..Default::default()
        };
        self.request(IpcEnvelope {
            msg: Some(Msg::PaymentQuery(PaymentQuery {
                player_id: actor,
                preview: Some(PreviewPayment {
                    transaction_id,
                    revision,
                    commit_spell_cast: Some(proposal),
                    ..Default::default()
                }),
            })),
        });
        let preview = kaya_batch(&self.latest)
            .payment_preview
            .as_ref()
            .unwrap()
            .clone();
        assert!(preview.valid && preview.complete, "{name}: {preview:?}");
        self.command(
            actor,
            Cmd::CommitSpellCast(CommitSpellCast {
                transaction_id,
                payment: Some(preview.selection.unwrap()),
                restricted_mana: preview.restricted_mana,
            }),
        );
        object
    }
    fn resolve(&mut self) {
        for _ in 0..32 {
            if let Some(order) = &self.state().pending_trigger_order {
                let actor = order.deciding_player;
                let trigger_object_id = order.candidates[0].object_id;
                self.command(
                    actor,
                    Cmd::SubmitTriggerOrder(tricerules_proto::ruled::v1::SubmitTriggerOrder {
                        trigger_object_id,
                    }),
                );
            } else if self.state().stack.is_empty() {
                return;
            } else {
                let actor = self.state().priority_player_id();
                self.command(actor, Cmd::PassPriority(PassPriority {}));
            }
        }
        panic!("normal As Foretold stack did not settle");
    }

    fn cast_with_source(
        &mut self,
        name: &str,
        face_index: u32,
        cost_source: u32,
        exile: bool,
    ) -> u32 {
        let actor = 17;
        let legal = &kaya_batch(&self.latest).legal_by_player[&actor];
        let (object, mut announcement, costs, targeted) = if exile {
            let offer = legal
                .zone_cast_actions
                .iter()
                .find(|a| a.card_name == name && a.face_index == face_index)
                .unwrap();
            let option = offer
                .as_foretold_costs
                .iter()
                .find(|a| a.source.is_some_and(|s| s.object_id == cost_source))
                .unwrap();
            (
                offer.object_id,
                tricerules_proto::ruled::v1::SpellCastAnnouncement {
                    source: Some(CastSource {
                        location: Some(
                            tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(
                                offer.object_id,
                            ),
                        ),
                        expected_zone_change_generation: Some(offer.zone_change_generation),
                    }),
                    face_index: offer.face_index,
                    cast_method: offer.cast_method,
                    casting_permission_id: offer.casting_permission_id,
                    as_foretold_source: option.source,
                    ..Default::default()
                },
                offer.cost_choices.clone(),
                offer.needs_target,
            )
        } else {
            let offer = legal
                .hand_actions
                .iter()
                .find(|a| {
                    a.card_name == name
                        && a.face_index == face_index
                        && a.cast_method == CastMethod::Normal as i32
                })
                .unwrap();
            let option = offer
                .as_foretold_costs
                .iter()
                .find(|a| a.source.is_some_and(|s| s.object_id == cost_source))
                .unwrap();
            let object = self.state().players[0].hand[offer.hand_index as usize];
            if offer.needs_target {
                assert!(
                    legal.valid_targets_by_hand_slot[&((offer.hand_index << 8) | face_index)]
                        .groups[0]
                        .can_target_opponent
                );
            }
            (
                object,
                tricerules_proto::ruled::v1::SpellCastAnnouncement {
                    source: Some(CastSource {
                        location: Some(
                            tricerules_proto::ruled::v1::cast_source::Location::HandIndex(
                                offer.hand_index,
                            ),
                        ),
                        expected_zone_change_generation: Some(
                            self.state()
                                .zone_change_generation
                                .get(&object)
                                .copied()
                                .unwrap_or(0),
                        ),
                    }),
                    face_index: offer.face_index,
                    cast_method: offer.cast_method,
                    as_foretold_source: option.source,
                    ..Default::default()
                },
                offer.cost_choices.clone(),
                offer.needs_target,
            )
        };
        if targeted {
            announcement.targets = vec![TargetRef {
                object_id: 29,
                kind: TargetRefKind::Player as i32,
                ..Default::default()
            }];
        }
        let mut discarded = None;
        for choice in costs.iter().flat_map(|c| &c.choices) {
            assert_eq!(
                choice.kind,
                tricerules_proto::ruled::v1::CostChoiceKind::Discard as i32
            );
            let slot = *choice
                .candidate_ids
                .iter()
                .min_by_key(|slot| {
                    let id = self.state().players[0].hand[**slot as usize];
                    (self.state().objects[&id].card_id != "island", **slot)
                })
                .unwrap();
            discarded = Some(self.state().players[0].hand[slot as usize]);
            announcement
                .cost_selections
                .push(tricerules_proto::ruled::v1::CostSelection {
                    cost_index: choice.cost_index,
                    selection: Some(
                        tricerules_proto::ruled::v1::cost_selection::Selection::HandIndex(slot),
                    ),
                });
        }
        assert_eq!(self.state().players[0].mana_pool.red, 0);
        self.command(
            actor,
            Cmd::BeginSpellCast(BeginSpellCast {
                announcement: Some(announcement),
            }),
        );
        let transaction_id = self
            .state()
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id;
        let revision = self.state().command_index;
        self.request(IpcEnvelope {
            msg: Some(Msg::PaymentQuery(PaymentQuery {
                player_id: actor,
                preview: Some(PreviewPayment {
                    transaction_id,
                    revision,
                    commit_spell_cast: Some(CommitSpellCast {
                        transaction_id,
                        payment: Some(PaymentSelection {
                            expected_state_revision: revision,
                            mana: Some(PaymentMana::default()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            })),
        });
        let preview = kaya_batch(&self.latest)
            .payment_preview
            .as_ref()
            .unwrap()
            .clone();
        assert!(preview.valid && preview.complete, "{name}: {preview:?}");
        self.command(
            actor,
            Cmd::CommitSpellCast(CommitSpellCast {
                transaction_id,
                payment: Some(preview.selection.unwrap()),
                restricted_mana: preview.restricted_mana,
            }),
        );
        if let Some(discarded) = discarded {
            assert_eq!(
                self.state().objects[&discarded].zone,
                tricerules_core::Zone::Graveyard
            );
            self.manifest["mandatory_discard"] = true.into();
        }
        object
    }
    fn next_turn(&mut self) {
        self.advance_main(29);
        self.play_land(29);
        self.advance_main(17);
        self.play_land(17);
    }
    fn finish(mut self) {
        assert!(self.state().pending_resolution.is_none());
        assert!(!self.latest.effective_dev_commands_enabled);
        self.manifest["complete"] = true.into();
        self.manifest["accepted_commands"] = self.accepted.into();
        fs::write(
            self.root.join("timeline.jsonl"),
            self.rows.join("\n") + "\n",
        )
        .unwrap();
        fs::write(self.root.join("manifest.json"), self.manifest.to_string()).unwrap();
        let expected = self
            .session
            .engine
            .as_ref()
            .unwrap()
            .diagnostic_snapshot()
            .unwrap();
        let capture = load_capture(&self.root).unwrap();
        assert!(capture.warnings.is_empty(), "{:?}", capture.warnings);
        let replayed = reconstruct(&capture, &ReplayOptions::default()).unwrap();
        assert!(replayed.warnings.is_empty(), "{:?}", replayed.warnings);
        assert!(
            replayed.differences.is_empty(),
            "{:?}",
            replayed.differences
        );
        assert_eq!(replayed.accepted_commands, self.accepted as u64);
        assert_eq!(replayed.state, expected);
        println!(
            "As Foretold complete normal capture replayed exactly: {} accepted commands at {}",
            self.accepted,
            self.root.display()
        );
    }
}

fn land_color(id: &str) -> char {
    match id {
        "forest" => 'G',
        "plains" => 'W',
        "island" => 'U',
        _ => panic!("not a basic land"),
    }
}

#[test]
fn as_foretold_normal_upkeep_each_turn_adventure_and_discard_replay_exactly() {
    let mut pilot = Pilot::new(7_313_801);
    let source = loop {
        pilot.advance_main(17);
        pilot.play_land(17);
        if pilot.can_cast(17, "As Foretold", &['U'], 2) {
            let source = pilot.cast(17, "As Foretold", &['U'], 2, CastMethod::Normal);
            pilot.resolve();
            break source;
        }
        assert!(pilot.state().turn <= 12);
        pilot.advance_main(29);
        pilot.play_land(29);
    };
    pilot.manifest["paid_source_turn"] = pilot.state().turn.into();
    assert_eq!(
        pilot.state().objects[&source].counter_count(tricerules_cards::CounterKind::Time),
        0
    );
    pilot.next_turn();
    assert_eq!(
        pilot.state().objects[&source].counter_count(tricerules_cards::CounterKind::Time),
        1
    );
    pilot.cast_with_source("Lightning Bolt", 0, source, false);
    pilot.resolve();
    assert!(kaya_batch(&pilot.latest).legal_by_player[&17]
        .hand_actions
        .iter()
        .all(|a| a.as_foretold_costs.is_empty()));
    pilot.advance_main(29);
    pilot.play_land(29);
    pilot.command(29, Cmd::PassPriority(PassPriority {}));
    assert_eq!(pilot.state().priority_player_id(), 17);
    pilot.cast_with_source("Lightning Bolt", 0, source, false);
    pilot.resolve();
    pilot.manifest["opponent_turn_instant"] = true.into();
    pilot.advance_main(17);
    pilot.play_land(17);
    assert_eq!(
        pilot.state().objects[&source].counter_count(tricerules_cards::CounterKind::Time),
        2
    );
    pilot.cast_with_source("Tormenting Voice", 0, source, false);
    pilot.resolve();
    pilot.next_turn();
    assert_eq!(
        pilot.state().objects[&source].counter_count(tricerules_cards::CounterKind::Time),
        3
    );
    let adventure = pilot.cast_with_source("Stomp", 1, source, false);
    pilot.resolve();
    assert_eq!(
        pilot.state().objects[&adventure].zone,
        tricerules_core::Zone::Exile
    );
    pilot.next_turn();
    assert_eq!(
        pilot.cast_with_source("Bonecrusher Giant", 0, source, true),
        adventure
    );
    pilot.resolve();
    assert_eq!(
        pilot.state().objects[&adventure].zone,
        tricerules_core::Zone::Battlefield
    );
    pilot.manifest["all_clauses"] = true.into();
    pilot.manifest["independent_adventure_permission"] = true.into();
    pilot.manifest["no_red_mana"] = true.into();
    pilot.finish();
}
