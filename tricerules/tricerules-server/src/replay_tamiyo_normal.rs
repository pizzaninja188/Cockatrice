// Strict normal-session Tamiyo pilots. Every mutation is a captured player command.
use super::*;

const TAMIYO_ID: &str = "tamiyo,_field_researcher";

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
        for name in ["Tamiyo, Field Researcher", "Llanowar Elves", "Divination"] {
            cards.extend(vec![name.to_string(); 4]);
        }
        for land in ["Forest", "Plains", "Island"] {
            cards.extend(vec![land.to_string(); 16]);
        }
        let decks = vec![
            (17, cards),
            (
                29,
                [
                    vec!["Grizzly Bears".to_string(); 4],
                    vec!["Sol Ring".to_string(); 4],
                    vec!["Forest".to_string(); 52],
                ]
                .concat(),
            ),
        ]
        .into_iter()
        .map(|(player_id, mainboard_card_name)| {
            assert_eq!(mainboard_card_name.len(), 60);
            PlayerDeck {
                player_id,
                mainboard_card_name,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
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
                })),
            })
            .unwrap();
        assert!(validation.ok && validation.missing_card_names.is_empty());
        let captures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/deck-coverage/tamiyo-whole-card-20261103/normal-captures");
        fs::create_dir_all(&captures).unwrap();
        let root = tempfile::Builder::new()
            .prefix(&format!("tamiyo-{game_id}-"))
            .tempdir_in(captures)
            .unwrap()
            .keep();
        fs::create_dir(root.join("raw")).unwrap();
        println!(
            "Tamiyo retained_capture={} fixed_seed=19 decks={decks:?}",
            root.display()
        );
        let mut pilot = Self {
            session,
            root,
            rows: Vec::new(),
            manifest: serde_json::json!({
                "format_version":1, "source":"server", "privacy":"server_only", "capture_id":format!("tamiyo-{game_id}"),
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
                seed: 19,
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
            "normal Tamiyo request rejected: {}",
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
        panic!("normal Tamiyo driver did not reach requested Main1");
    }

    fn play_land(&mut self, actor: i32) {
        let preferred = if actor == 29 {
            "Forest"
        } else {
            [
                ("Forest", "forest"),
                ("Plains", "plains"),
                ("Island", "island"),
            ]
            .into_iter()
            .min_by_key(|(_, id)| self.battlefield.iter().filter(|p| p.card_id == *id).count())
            .unwrap()
            .0
        };
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
        panic!("normal Tamiyo stack did not settle");
    }
    fn activate(&mut self, stable_id: &str, objects: &[u32]) {
        let source = self
            .battlefield
            .iter()
            .find(|p| p.card_id == TAMIYO_ID)
            .unwrap();
        let ability = luxior_normal_current_ability(source, stable_id);
        assert!(ability.activatable);
        let targets = objects
            .iter()
            .map(|id| TargetRef {
                object_id: *id,
                expected_zone_change_generation: Some(
                    self.state()
                        .zone_change_generation
                        .get(id)
                        .copied()
                        .unwrap_or(0),
                ),
                ..Default::default()
            })
            .collect();
        self.command(
            17,
            Cmd::ActivateAbility(ActivateAbility {
                source_object_id: source.object_id,
                expected_zone_change_generation: source.zone_change_generation,
                ability_index: ability.ability_index,
                source_zone: AbilitySourceZone::Battlefield as i32,
                targets,
                ..Default::default()
            }),
        );
        self.resolve();
    }
    fn setup(&mut self) -> (u32, u32, u32, u32) {
        let mut elf = None;
        let mut bear = None;
        let mut ring = None;
        for turn in 1..=24 {
            self.advance_main(17);
            self.play_land(17);
            if elf.is_none() && self.can_cast(17, "Llanowar Elves", &['G'], 0) {
                elf = Some(self.cast(17, "Llanowar Elves", &['G'], 0, CastMethod::Normal));
                self.resolve();
            }
            if let (Some(elf), Some(bear), Some(ring)) = (elf, bear, ring) {
                if self.can_cast(17, "Tamiyo, Field Researcher", &['G', 'W', 'U'], 1) {
                    let source = self.cast(
                        17,
                        "Tamiyo, Field Researcher",
                        &['G', 'W', 'U'],
                        1,
                        CastMethod::Normal,
                    );
                    self.resolve();
                    assert_eq!(
                        self.state().objects[&source]
                            .counter_count(tricerules_cards::CounterKind::Loyalty),
                        4
                    );
                    self.manifest["setup_turn"] = turn.into();
                    // A fresh actual turn guarantees the normally cast Elf can attack.
                    self.advance_main(29);
                    self.play_land(29);
                    self.advance_main(17);
                    self.play_land(17);
                    return (source, elf, bear, ring);
                }
            }
            self.advance_main(29);
            self.play_land(29);
            if bear.is_none() && self.can_cast(29, "Grizzly Bears", &['G'], 1) {
                bear = Some(self.cast(29, "Grizzly Bears", &['G'], 1, CastMethod::Normal));
                self.resolve();
            }
            if ring.is_none() && self.can_cast(29, "Sol Ring", &[], 1) {
                ring = Some(self.cast(29, "Sol Ring", &[], 1, CastMethod::Normal));
                self.resolve();
            }
        }
        panic!("fixed-seed normal Tamiyo setup exceeded 24 turns");
    }
    fn next_turn(&mut self) {
        self.advance_main(29);
        self.play_land(29);
        self.advance_main(17);
        self.play_land(17);
    }
    fn combat(&mut self, elf: u32, bear: u32) {
        use tricerules_core::TurnStep;
        while self.state().turn_step != TurnStep::DeclareAttackers {
            let actor = self.state().priority_player_id();
            self.command(actor, Cmd::PassPriority(PassPriority {}));
        }
        self.command(
            17,
            Cmd::DeclareAttackers(tricerules_proto::ruled::v1::DeclareAttackers {
                assignments: vec![tricerules_proto::ruled::v1::AttackAssignment {
                    attacker_object_id: elf,
                    attacker_zone_change_generation: self
                        .state()
                        .zone_change_generation
                        .get(&elf)
                        .copied()
                        .unwrap_or(0),
                    defender: Some(TargetRef {
                        kind: tricerules_proto::ruled::v1::TargetRefKind::Player as i32,
                        object_id: 29,
                        ..Default::default()
                    }),
                    defending_player_id: 29,
                    ..Default::default()
                }],
            }),
        );
        while self.state().turn_step != TurnStep::DeclareBlockers {
            let actor = self.state().priority_player_id();
            self.command(actor, Cmd::PassPriority(PassPriority {}));
        }
        self.command(
            29,
            Cmd::DeclareBlockers(tricerules_proto::ruled::v1::DeclareBlockers {
                block_pairs: vec![tricerules_proto::ruled::v1::BlockPair {
                    attacker_id: elf,
                    blocker_id: bear,
                }],
            }),
        );
        while self.state().objects[&elf].zone == tricerules_core::Zone::Battlefield {
            let actor = self.state().priority_player_id();
            self.command(actor, Cmd::PassPriority(PassPriority {}));
        }
        self.resolve();
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
            "Tamiyo complete normal capture replayed exactly: {} accepted commands at {}",
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
fn tamiyo_normal_combat_three_plus_turns_ultimate_and_hand_cast_replay_exactly() {
    let mut p = Pilot::new(7_281_115);
    let (source, elf, bear, _) = p.setup();
    p.activate("activated_01", &[elf, bear]);
    let before = [
        p.state().players[0].hand.len(),
        p.state().players[1].hand.len(),
    ];
    p.combat(elf, bear);
    assert_eq!(p.state().players[0].hand.len(), before[0] + 2);
    assert_eq!(p.state().players[1].hand.len(), before[1]);
    assert_eq!(
        p.state().objects[&elf].zone,
        tricerules_core::Zone::Graveyard
    );
    for loyalty in 6..=7 {
        p.next_turn();
        p.activate("activated_01", &[]);
        assert_eq!(
            p.state().objects[&source].counter_count(tricerules_cards::CounterKind::Loyalty),
            loyalty
        );
    }
    p.next_turn();
    let before = p.state().players[0].hand.len();
    p.activate("activated_03", &[]);
    assert_eq!(p.state().players[0].hand.len(), before + 3);
    assert_eq!(
        p.state().objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(p.state().static_emblems.len(), 1);
    for _ in 0..12 {
        if kaya_batch(&p.latest).legal_by_player[&17]
            .hand_actions
            .iter()
            .any(|a| {
                a.card_name == "Divination" && a.cast_method == CastMethod::WithoutManaCost as i32
            })
        {
            break;
        }
        p.next_turn();
    }
    let spell = p.cast(17, "Divination", &[], 0, CastMethod::WithoutManaCost);
    assert_eq!(
        p.state().stack.last().unwrap().cast_method,
        tricerules_core::state::SpellCastMethod::WithoutManaCost
    );
    p.resolve();
    assert_eq!(
        p.state().objects[&spell].zone,
        tricerules_core::Zone::Graveyard
    );
    p.finish();
}
#[test]
fn tamiyo_normal_minus_two_skips_normally_cast_creature_and_artifact_next_untap_replays() {
    let mut p = Pilot::new(7_281_116);
    let (source, _, bear, ring) = p.setup();
    p.activate("activated_02", &[bear, ring]);
    assert_eq!(
        p.state().objects[&source].counter_count(tricerules_cards::CounterKind::Loyalty),
        2
    );
    assert!(p.state().objects[&bear].tapped && p.state().objects[&ring].tapped);
    p.advance_main(29);
    assert!(p.state().objects[&bear].tapped && p.state().objects[&ring].tapped);
    p.advance_main(17);
    p.advance_main(29);
    assert!(!p.state().objects[&bear].tapped && !p.state().objects[&ring].tapped);
    p.finish();
}
