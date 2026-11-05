// Strict normal-session Kaya pilot, fixed seed32 after bounded separate preparation.
// Legal60-card decks; every mutation is a captured command, Dev disabled. No injected state.
use super::*;

const KAYA_ID: &str = "kaya_the_inexorable";

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
        let decks = vec![
            (
                17,
                [
                    vec!["Kaya the Inexorable".to_string(); 4],
                    vec!["Ornithopter".to_string(); 4],
                    vec!["Plains".to_string(); 26],
                    vec!["Swamp".to_string(); 26],
                ]
                .concat(),
            ),
            (
                29,
                [
                    vec!["Swords to Plowshares".to_string(); 4],
                    vec!["Plains".to_string(); 56],
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
                ..Default::default()
            }
        })
        .collect::<Vec<_>>();
        let mut session = EngineSession::new(false);
        let validation_request = IpcEnvelope {
            msg: Some(Msg::ValidateDeck(ValidateDeck {
                card_names: decks
                    .iter()
                    .flat_map(|deck| deck.mainboard_card_name.iter().cloned())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                ..Default::default()
            })),
        };
        let validation = session.process(&validation_request).unwrap();
        assert!(validation.ok && validation.missing_card_names.is_empty());
        let captures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/deck-coverage/kaya-whole-card-20261103/normal-captures");
        fs::create_dir_all(&captures).unwrap();
        let root = tempfile::Builder::new()
            .prefix(&format!("kaya-{game_id}-"))
            .tempdir_in(captures)
            .unwrap()
            .keep();
        fs::create_dir(root.join("raw")).unwrap();
        fs::write(
            root.join("preflight-validation-request.pb"),
            validation_request.encode_to_vec(),
        )
        .unwrap();
        fs::write(
            root.join("preflight-validation-response.pb"),
            validation.encode_to_vec(),
        )
        .unwrap();
        println!(
            "Kaya retained_capture={} fixed_seed=32 decks={decks:?}",
            root.display()
        );
        let mut pilot = Self {
            session,
            root,
            rows: Vec::new(),
            manifest: serde_json::json!({
                "format_version":1, "source":"server", "privacy":"server_only", "capture_id":format!("kaya-{game_id}"),
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
                seed: 32,
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
            "normal Kaya request rejected: {}",
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
        self.advance_to(player, tricerules_core::TurnStep::Main1);
    }
    fn advance_to(&mut self, player: i32, step: tricerules_core::TurnStep) {
        for _ in 0..512 {
            let state = self.state();
            assert!(
                state.pending_resolution.is_none(),
                "answer choices explicitly before advancing to {player}/{step:?}: step={:?} active={} pending={:?}", state.turn_step,state.active_player_id(),state.pending_resolution
            );
            if state.turn_step == step
                && state.active_player_id() == player
                && state.priority_player_id() == player
                && (step == tricerules_core::TurnStep::Upkeep || state.stack.is_empty())
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
                        !matches!(offer.card_name.as_str(), "Plains" | "Swamp"),
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
        panic!("normal Kaya driver did not reach requested Main1");
    }

    fn play_land(&mut self, actor: i32) {
        let preferred = if actor == 29
            || self
                .battlefield
                .iter()
                .filter(|p| p.card_id == "plains")
                .count()
                <= self
                    .battlefield
                    .iter()
                    .filter(|p| p.card_id == "swamp")
                    .count()
        {
            "Plains"
        } else {
            "Swamp"
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
            .filter(|p| !p.tapped && matches!(p.card_id.as_str(), "plains" | "swamp"))
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
            .filter(|p| !p.tapped && matches!(p.card_id.as_str(), "plains" | "swamp"))
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
        targets: &[u32],
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
                    targets: targets
                        .iter()
                        .map(|id| TargetRef {
                            object_id: *id,
                            kind: tricerules_proto::ruled::v1::TargetRefKind::Permanent as i32,
                            expected_zone_change_generation: Some(
                                self.state()
                                    .zone_change_generation
                                    .get(id)
                                    .copied()
                                    .unwrap_or(0),
                            ),
                            ..Default::default()
                        })
                        .collect(),
                    cast_method: method as i32,
                    ..Default::default()
                }),
            }),
        );
        self.pay_locked_cast(actor, colors, generic, name);
        object
    }
    fn pay_locked_cast(&mut self, actor: i32, colors: &[char], generic: usize, name: &str) {
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
                'B' => mana.b += 1,
                'W' => mana.w += 1,

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
    }
    fn resolve(&mut self) {
        self.resolve_keeping(None);
    }
    fn resolve_keeping(&mut self, keep: Option<u32>) {
        for _ in 0..32 {
            if let Some(pending) = self.state().pending_resolution.as_ref() {
                assert_eq!(
                    pending.presentation.choice_kind,
                    ChoiceKind::LegendKeep,
                    "answer the optional cast explicitly"
                );
                let keep = keep.expect("explicit legend choice");
                assert!(pending.presentation.candidates.contains(&keep));
                let prompt = pending.resolution_prompt_id.get();
                self.command(
                    17,
                    Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                        resolution_prompt_id: prompt,
                        chosen_object_ids: vec![keep],
                        ..Default::default()
                    }),
                );
            } else if let Some(order) = &self.state().pending_trigger_order {
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
        panic!("normal Kaya stack did not settle");
    }
    fn activate(&mut self, stable_id: &str, objects: &[u32]) {
        let source = self
            .battlefield
            .iter()
            .find(|p| p.card_id == KAYA_ID)
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
            "Kaya complete normal capture replayed exactly: {} accepted commands at {}",
            self.accepted,
            self.root.display()
        );
    }
}

fn land_color(id: &str) -> char {
    match id {
        "plains" => 'W',
        "swamp" => 'B',
        _ => panic!("not a supported basic land"),
    }
}

impl Pilot {
    fn setup(&mut self) -> (u32, u32) {
        let mut thopter = None;
        for turn in 1..=5 {
            self.advance_main(17);
            self.play_land(17);
            if thopter.is_none() && self.can_cast(17, "Ornithopter", &[], 0) {
                thopter = Some(self.cast(17, "Ornithopter", &[], 0, CastMethod::Normal, &[]));
                self.resolve();
            }
            if turn == 5 {
                assert!(self.can_cast(17, "Kaya the Inexorable", &['W', 'B'], 3));
                let source = self.cast(
                    17,
                    "Kaya the Inexorable",
                    &['W', 'B'],
                    3,
                    CastMethod::Normal,
                    &[],
                );
                self.resolve();
                self.manifest["setup_turn"] = turn.into();
                return (source, thopter.expect("Ornithopter by fourth draw"));
            }
            self.advance_main(29);
            self.play_land(29);
        }
        unreachable!()
    }
    fn next_main(&mut self) {
        self.advance_main(29);
        self.play_land(29);
        self.advance_main(17);
        self.play_land(17);
    }
    fn next_upkeep(&mut self) {
        self.advance_main(29);
        self.play_land(29);
        self.advance_to(17, tricerules_core::TurnStep::Upkeep);
    }
    fn resolve_to_offer(&mut self) {
        for _ in 0..16 {
            if self.state().pending_resolution.is_some() {
                return;
            }
            assert!(!self.state().stack.is_empty());
            let actor = self.state().priority_player_id();
            self.command(actor, Cmd::PassPriority(PassPriority {}));
        }
        panic!("bounded emblem resolution");
    }
    fn special_cast(&mut self, oid: u32, zone: tricerules_core::Zone, cancel: bool) {
        use tricerules_core::Zone;
        let generation = self
            .state()
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0);
        let prompt = kaya_choice(&self.latest, ChoiceKind::SpecialCast).unwrap();
        let ci = prompt
            .candidate_object_ids
            .iter()
            .position(|id| *id == oid)
            .expect("exact offered card");
        let expected_zone = match zone {
            Zone::Hand => tricerules_proto::ruled::v1::ChoiceCandidateSourceZone::Hand,
            Zone::Graveyard => tricerules_proto::ruled::v1::ChoiceCandidateSourceZone::Graveyard,
            Zone::Exile => tricerules_proto::ruled::v1::ChoiceCandidateSourceZone::Exile,
            _ => unreachable!(),
        };
        assert_eq!(prompt.candidate_source_zones[ci], expected_zone as i32);
        let prompt_id = prompt.resolution_prompt_id;
        let location = match zone {
            Zone::Hand => tricerules_proto::ruled::v1::cast_source::Location::HandIndex(
                self.state().players[0]
                    .hand
                    .iter()
                    .position(|id| *id == oid)
                    .unwrap() as u32,
            ),
            Zone::Graveyard => {
                tricerules_proto::ruled::v1::cast_source::Location::GraveyardObjectId(oid)
            }
            Zone::Exile => tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(oid),
            _ => unreachable!(),
        };
        let before_casts = self.state().turn_history.current.spell_casts.len();
        self.command(
            17,
            Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                resolution_prompt_id: prompt_id,
                decision: ResolutionChoiceDecision::CastSpell as i32,
                spell_cast_announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                    source: Some(CastSource {
                        location: Some(location),
                        expected_zone_change_generation: (zone != Zone::Hand).then_some(generation),
                    }),
                    cast_method: CastMethod::WithoutManaCost as i32,
                    ..Default::default()
                }),
                ..Default::default()
            }),
        );
        assert_eq!(self.state().objects[&oid].zone, Zone::Stack);
        assert_eq!(
            self.state().turn_history.current.spell_casts.len(),
            before_casts
        );
        assert_eq!(
            self.state()
                .pending_spell_cast
                .as_ref()
                .unwrap()
                .original_zone,
            zone
        );
        if cancel {
            let transaction_id = self
                .state()
                .pending_spell_cast
                .as_ref()
                .unwrap()
                .transaction_id;
            self.command(
                17,
                Cmd::CancelSpellCast(tricerules_proto::ruled::v1::CancelSpellCast {
                    transaction_id,
                }),
            );
            assert_eq!(self.state().objects[&oid].zone, zone);
            assert_eq!(
                self.state()
                    .zone_change_generation
                    .get(&oid)
                    .copied()
                    .unwrap_or(0),
                generation
            );
            assert_ne!(
                kaya_choice(&self.latest, ChoiceKind::SpecialCast)
                    .unwrap()
                    .resolution_prompt_id,
                prompt_id
            );
            self.special_cast(oid, zone, false);
            return;
        }
        self.pay_locked_cast(17, &[], 0, "Kaya emblem cast");
        assert!(self.state().pending_resolution.is_none());
        assert_eq!(self.state().stack.last().unwrap().id, oid);
        assert_eq!(
            self.state()
                .turn_history
                .current
                .spell_casts
                .last()
                .unwrap()
                .origin,
            zone
        );
    }
}

#[test]
fn kaya_normal_two_ghostform_grants_and_hand_exile_graveyard_emblem_casts_replay_exactly() {
    use tricerules_core::Zone;
    let mut p = Pilot::new(7_311_101);
    let (source, thopter) = p.setup();
    p.activate("activated_01", &[thopter]);
    assert_eq!(
        p.state().objects[&source].counter_count(tricerules_cards::CounterKind::Loyalty),
        6
    );
    p.next_main();
    p.activate("activated_01", &[thopter]);
    assert_eq!(
        p.state().objects[&source].counter_count(tricerules_cards::CounterKind::Loyalty),
        7
    );
    p.advance_main(29);
    p.play_land(29);
    assert!(p.can_cast(29, "Swords to Plowshares", &['W'], 0));
    p.cast(
        29,
        "Swords to Plowshares",
        &['W'],
        0,
        CastMethod::Normal,
        &[thopter],
    );
    p.resolve();
    assert_eq!(p.state().objects[&thopter].zone, Zone::Hand);
    assert_eq!(
        p.state().players[0]
            .battlefield
            .iter()
            .filter(|id| p.state().objects[id].card_id == "spirit_w_1_1_flying")
            .count(),
        2
    );
    p.advance_main(17);
    p.play_land(17);
    p.activate("activated_03", &[]);
    assert_eq!(p.state().objects[&source].zone, Zone::Graveyard);
    assert_eq!(p.state().static_emblems.len(), 1);
    p.next_upkeep();
    p.resolve_to_offer();
    let second = *p.state().players[0]
        .hand
        .iter()
        .find(|id| p.state().objects[id].card_id == KAYA_ID)
        .unwrap();
    p.special_cast(second, Zone::Hand, true);
    p.resolve();
    assert_eq!(p.state().objects[&second].zone, Zone::Battlefield);
    p.advance_main(17);
    p.activate("activated_02", &[second]);
    assert_eq!(p.state().objects[&second].zone, Zone::Exile);
    p.next_upkeep();
    p.resolve_to_offer();
    p.special_cast(second, Zone::Exile, false);
    p.resolve();
    assert_eq!(p.state().objects[&second].zone, Zone::Battlefield);
    p.next_upkeep();
    p.resolve_to_offer();
    p.special_cast(source, Zone::Graveyard, false);
    p.resolve_keeping(Some(source));
    assert_eq!(p.state().objects[&source].zone, Zone::Battlefield);
    assert_eq!(p.state().objects[&second].zone, Zone::Graveyard);
    assert_eq!(p.state().static_emblems.len(), 1);
    p.finish();
}
