// Strict normal-session Onyx pilots. Every mutation is a captured player command.
use super::*;

const ONYX_ID: &str = "professor_onyx";

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
            "Professor Onyx",
            "Opt",
            "Twincast",
        ] {
            cards.extend(vec![name.to_string(); 4]);
        }
        cards.extend(vec!["Island".to_string(); 24]);
        cards.extend(vec!["Swamp".to_string(); 24]);
        let decks = vec![(17, cards), (29, [vec!["Faerie Mastermind".to_string(); 4],vec!["Island".to_string(); 56]].concat())]
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
            .join("../../build/deck-coverage/onyx-discard-round-20261103/normal-captures");
        fs::create_dir_all(&captures).unwrap();
        let root = tempfile::Builder::new()
            .prefix(&format!("onyx-{game_id}-"))
            .tempdir_in(captures)
            .unwrap()
            .keep();
        fs::create_dir(root.join("raw")).unwrap();
        println!(
            "Onyx retained_capture={} fixed_seed=19 decks={decks:?}",
            root.display()
        );
        let mut pilot = Self {
            session,
            root,
            rows: Vec::new(),
            manifest: serde_json::json!({
                "format_version":1, "source":"server", "privacy":"server_only", "capture_id":format!("onyx-{game_id}"),
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
            "normal Onyx request rejected: {}",
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
                        !matches!(offer.card_name.as_str(), "Island" | "Swamp"),
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
        panic!("normal Onyx driver did not reach requested Main1");
    }

    fn play_land(&mut self, actor: i32) {
        let preferred = if self
            .battlefield
            .iter()
            .filter(|card| card.card_id == "island")
            .count()
            < 3
        {
            "Island"
        } else {
            "Swamp"
        };
        let offer = kaya_batch(&self.latest).legal_by_player[&actor]
            .hand_actions
            .iter()
            .filter(|offer| offer.kind == HandActionKind::HandActionPlayLand as i32)
            .min_by_key(|offer| (offer.card_name != preferred, offer.hand_index))
            .unwrap()
            .clone();
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

    fn can_cast(&self, name: &str, blue: usize, black: usize, generic: usize) -> bool {
        let available = self
            .battlefield
            .iter()
            .filter(|card| !card.tapped && matches!(card.card_id.as_str(), "island" | "swamp"))
            .collect::<Vec<_>>();
        kaya_batch(&self.latest).legal_by_player[&17]
            .hand_actions
            .iter()
            .any(|offer| {
                offer.card_name == name && offer.kind == HandActionKind::HandActionCastSpell as i32
            })
            && available.len() >= blue + black + generic
            && available
                .iter()
                .filter(|card| card.card_id == "island")
                .count()
                >= blue
            && available
                .iter()
                .filter(|card| card.card_id == "swamp")
                .count()
                >= black
    }

    fn land_mana(&mut self, actor: i32, color: Option<char>) -> char {
        let (oid, generation, index, produced) = (if actor == 17 { &self.battlefield } else { &self.opponent_battlefield })
            .iter()
            .filter(|card| !card.tapped && matches!(card.card_id.as_str(), "island" | "swamp"))
            .flat_map(|card| {
                card.activated_abilities
                    .iter()
                    .filter(|ability| ability.is_mana_ability && ability.activatable)
                    .map(move |ability| {
                        (
                            card.object_id,
                            card.zone_change_generation,
                            ability.ability_index,
                            if card.card_id == "island" { 'U' } else { 'B' },
                            ability.mana_produced.clone(),
                        )
                    })
            })
            .find(|(_, _, _, candidate, output)| {
                color.is_none_or(|color| color == *candidate)
                    && kaya_output_has_color(output, *candidate)
            })
            .map(|(a, b, c, d, _)| (a, b, c, d))
            .expect("a current basic-land mana offer pays this pip");
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
        &mut self, actor: i32, name: &str, blue: usize, black: usize, generic: usize,
    ) -> u32 {
        self.cast_targeted(actor,name,blue,black,generic,Vec::new())
    }

    fn cast_targeted(
        &mut self,
        actor: i32,
        name: &str,
        blue: usize,
        black: usize,
        generic: usize,
        targets: Vec<TargetRef>,
    ) -> u32 {
        let offer = kaya_batch(&self.latest).legal_by_player[&actor]
            .hand_actions
            .iter()
            .find(|offer| {
                offer.card_name == name && offer.kind == HandActionKind::HandActionCastSpell as i32
            })
            .unwrap()
            .clone();
        let object_id = self.state().players[self.state().player_idx(actor).unwrap()].hand[offer.hand_index as usize];
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
                        ..Default::default()
                    }),
                    face_index: offer.face_index,
                    cast_method: CastMethod::Normal as i32,
                    targets,
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
        for color in std::iter::repeat_n(Some('U'), blue)
            .chain(std::iter::repeat_n(Some('B'), black))
            .chain(std::iter::repeat_n(None, generic))
        {
            match self.land_mana(actor,color) {
                'U' => mana.u += 1,
                'B' => mana.b += 1,
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
        assert_eq!(preview.transaction_id, transaction_id);
        self.command(
            actor,
            Cmd::CommitSpellCast(CommitSpellCast {
                transaction_id,
                payment: Some(preview.selection.unwrap()),
                restricted_mana: preview.restricted_mana,
            }),
        );
        object_id
    }

    fn resolve(&mut self) {
        for _ in 0..32 {
            if self.state().pending_resolution.is_some() || self.state().stack.is_empty() {
                return;
            }
            let actor = self.state().priority_player_id();
            self.command(actor, Cmd::PassPriority(PassPriority {}));
        }
        panic!("normal Onyx stack failed to settle");
    }

    fn resolve_top(&mut self) {
        for _ in 0..2 {
            let actor = self.state().priority_player_id();
            self.command(actor,Cmd::PassPriority(PassPriority {}));
        }
    }

    fn offered_choice(&self) -> tricerules_proto::ruled::v1::ResolutionChoiceRequired {
        kaya_batch(&self.latest)
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice.clone()),
                _ => None,
            })
            .expect("current response contains the exact private choice offer")
    }

    fn activate_onyx(&mut self, stable_id: &str, target: Option<TargetRef>) {
        let source = self
            .battlefield
            .iter()
            .find(|card| card.card_id == ONYX_ID)
            .unwrap();
        let ability = luxior_normal_current_ability(source, stable_id);
        assert!(ability.activatable);
        let command = ActivateAbility {
            source_object_id: source.object_id,
            expected_zone_change_generation: source.zone_change_generation,
            ability_index: ability.ability_index,
            source_zone: AbilitySourceZone::Battlefield as i32,
            targets: target.into_iter().collect(),
            ..Default::default()
        };
        self.command(17, Cmd::ActivateAbility(command));
        self.resolve();
    }

    fn setup(&mut self) -> u32 {
        for turn in 1..=24 {
            self.advance_main(17);
            self.play_land(17);
            if self.can_cast("Professor Onyx",0,2,4) {
                let onyx = self.cast(17,"Professor Onyx",0,2,4);
                self.resolve();
                assert_eq!(self.state().objects[&onyx].counter_count(tricerules_cards::CounterKind::Loyalty),5);
                self.manifest["setup_turn"] = turn.into();
                return onyx;
            }
            self.advance_main(29);
            self.play_land(29);
        }
        panic!("fixed-seed normal Onyx setup missed its explicit 24-turn bound");
    }

    fn next_turn(&mut self) {
        self.advance_main(29);
        self.play_land(29);
        self.advance_main(17);
        self.play_land(17);
    }

    fn answer(&mut self, chosen: Vec<u32>) {
        let choice = self.offered_choice();
        self.command(choice.deciding_player_id,Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id:choice.resolution_prompt_id,chosen_object_ids:chosen,..Default::default()
        }));
    }

    fn plus_one(&mut self) {
        self.activate_onyx("activated_01",None);
        let offer = self.offered_choice();
        assert_eq!(offer.deciding_player_id,17);
        assert_eq!(offer.candidate_object_ids.len(),3);
        let chosen = offer.candidate_object_ids[0];
        self.answer(vec![chosen]);
        let rest = self.offered_choice();
        assert!(rest.ordered);
        assert_eq!(rest.candidate_object_ids.len(),2);
        self.answer(rest.candidate_object_ids);
        assert_eq!(self.state().objects[&chosen].zone,tricerules_core::Zone::Hand);
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
            "Onyx complete normal capture replayed exactly: {} accepted commands at {}",
            self.accepted,
            self.root.display()
        );
    }
}

#[test]
fn professor_onyx_normal_three_distinct_plus_turns_then_seven_round_ultimate_replays() {
    let mut pilot = Pilot::new(7_281_105);
    let onyx = pilot.setup();
    for loyalty in 6..=8 {
        if loyalty > 6 { pilot.next_turn(); }
        pilot.plus_one();
        assert_eq!(pilot.state().objects[&onyx].counter_count(tricerules_cards::CounterKind::Loyalty),loyalty);
    }
    assert_eq!(pilot.state().players[0].life,17);
    pilot.next_turn();
    pilot.activate_onyx("activated_03",None);
    assert_eq!(pilot.state().objects[&onyx].zone,tricerules_core::Zone::Graveyard);
    for round in 0..7 {
        let offer = pilot.offered_choice();
        assert_eq!(offer.deciding_player_id,29);
        assert_eq!((offer.min,offer.max),(0,1));
        // Alternate legal pay/decline while enough ordinary cards remain in hand.
        pilot.answer(if round % 2 == 0 { vec![offer.candidate_object_ids[0]] } else { vec![] });
    }
    assert_eq!(pilot.state().players[1].life,11);
    assert!(pilot.state().pending_resolution.is_none());
    assert!(pilot.state().stack.is_empty());
    pilot.finish();
}

#[test]
fn professor_onyx_normal_opt_and_twincast_commit_three_magecraft_occurrences_and_replay() {
    let mut pilot = Pilot::new(7_281_106);
    pilot.setup();
    let mut ready = false;
    for _ in 0..24 {
        pilot.next_turn();
        if pilot.can_cast("Opt",1,0,0) && pilot.can_cast("Twincast",2,0,0)
            && pilot.battlefield.iter().filter(|card| card.card_id == "island" && !card.tapped).count() >= 3 {
            ready = true;
            break;
        }
        pilot.plus_one();
    }
    assert!(ready,"bounded ordinary draw/look path finds both spells and three blue mana");
    let before = [pilot.state().players[0].life,pilot.state().players[1].life];
    let opt = pilot.cast(17,"Opt",1,0,0);
    pilot.resolve_top();
    assert_eq!(pilot.state().players[0].life,before[0]+2);
    pilot.cast_targeted(17,"Twincast",2,0,0,vec![TargetRef { object_id:opt,..Default::default() }]);
    pilot.resolve_top();
    assert_eq!(pilot.state().players[0].life,before[0]+4);
    pilot.resolve_top(); // Twincast commits a virtual Opt copy.
    assert!(pilot.state().pending_resolution.is_none());
    assert_eq!(pilot.state().stack.iter().filter(|item| item.is_copy).count(),1);
    pilot.resolve_top(); // Copy Magecraft.
    assert_eq!(pilot.state().players[0].life,before[0]+6);
    assert_eq!(pilot.state().players[1].life,before[1]-6);
    while !pilot.state().stack.is_empty() || pilot.state().pending_resolution.is_some() {
        if pilot.state().pending_resolution.is_some() {
            assert_eq!(pilot.offered_choice().deciding_player_id,17);
            pilot.answer(vec![]);
        } else { pilot.resolve(); }
    }
    pilot.finish();
}

#[test]
fn professor_onyx_normal_minus_three_sacrifices_normally_cast_opponent_creature_and_replays() {
    let mut pilot = Pilot::new(7_281_107);
    let onyx = pilot.setup();
    let mut creature = None;
    for _ in 0..24 {
        pilot.advance_main(29);
        pilot.play_land(29);
        let offered = kaya_batch(&pilot.latest).legal_by_player[&29].hand_actions.iter().any(|offer|
            offer.card_name == "Faerie Mastermind" && offer.kind == HandActionKind::HandActionCastSpell as i32);
        if offered {
            creature = Some(pilot.cast(29,"Faerie Mastermind",1,0,1));
            pilot.resolve();
            pilot.advance_main(17);
            break;
        }
        pilot.advance_main(17);
        pilot.play_land(17);
    }
    let creature = creature.expect("bounded normal opponent draw path finds an actual creature");
    assert_eq!(pilot.state().objects[&creature].zone,tricerules_core::Zone::Battlefield);
    pilot.activate_onyx("activated_02",None);
    assert!(pilot.state().pending_resolution.is_none(),"the sole greatest-power creature is automatic");
    assert_eq!(pilot.state().objects[&creature].zone,tricerules_core::Zone::Graveyard);
    assert_eq!(pilot.state().objects[&onyx].counter_count(tricerules_cards::CounterKind::Loyalty),2);
    pilot.finish();
}
