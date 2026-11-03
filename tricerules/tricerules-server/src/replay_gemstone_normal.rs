// Actual Gemstone Caverns, legal sixty-card decks, dev-disabled startup and complete replay.
use super::*;
use tricerules_proto::ruled::v1::{
    BeginWithGemstoneCaverns, FinishOpeningHandActions, PutOpeningHandOnBottom,
};

struct Pilot {
    session: EngineSession,
    root: std::path::PathBuf,
    rows: Vec<String>,
    manifest: Value,
    latest: IpcResponse,
    accepted: usize,
}
impl Pilot {
    fn new(game_id: u64, seed: u64, mulligan: bool) -> Self {
        let captures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/deck-coverage/gemstone-caverns-20261103/normal-captures");
        fs::create_dir_all(&captures).unwrap();
        let root = tempfile::Builder::new()
            .prefix(&format!("gemstone-{game_id}-"))
            .tempdir_in(captures)
            .unwrap()
            .keep();
        fs::create_dir(root.join("raw")).unwrap();
        let mut pilot = Self {
            session: EngineSession::new(false),
            root,
            rows: Vec::new(),
            latest: IpcResponse::default(),
            accepted: 0,
            manifest: serde_json::json!({"format_version":1,"source":"server","privacy":"server_only",
                "capture_id":format!("gemstone-{game_id}"),"engine_build":ENGINE_BUILD,"card_data_hash":"",
                "effective_dev_commands_enabled":false,"complete":false,"fixed_seed":seed}),
        };
        let decks = [
            (17, vec!["Forest".to_string(); 60]),
            (
                29,
                [
                    vec!["Gemstone Caverns".to_string(); 4],
                    vec!["Forest".to_string(); 56],
                ]
                .concat(),
            ),
        ]
        .into_iter()
        .map(|(player_id, mainboard_card_name)| PlayerDeck {
            player_id,
            mainboard_card_name,
            commander_setup: Some(CommanderSetup {
                declaration: Some(commander_setup::Declaration::NoCommander(true)),
            }),
        })
        .collect::<Vec<_>>();
        assert!(decks
            .iter()
            .all(|deck| deck.mainboard_card_name.len() == 60));
        let validation_request = IpcEnvelope {
            msg: Some(Msg::ValidateDeck(ValidateDeck {
                card_names: vec!["Gemstone Caverns".into(), "Forest".into()],
            })),
        };
        let validation = pilot.session.process(&validation_request).unwrap();
        assert!(validation.ok && validation.missing_card_names.is_empty());
        // The selected game capture starts with SessionStart. Retain the read-only deck
        // preflight separately, so its query is recorded without inventing a game startup.
        fs::write(
            pilot.root.join("preflight-validate.request.pb"),
            validation_request.encode_to_vec(),
        )
        .unwrap();
        fs::write(
            pilot.root.join("preflight-validate.response.pb"),
            validation.encode_to_vec(),
        )
        .unwrap();
        pilot.request(IpcEnvelope {
            msg: Some(Msg::SessionStart(SessionStart {
                game_id,
                seed,
                player_ids: vec![17, 29],
                player_decks: decks,
                dev_commands_enabled: false,
                diagnostic_capture_enabled: true,
                ..Default::default()
            })),
        });
        pilot.manifest["card_data_hash"] = pilot.latest.card_data_hash.clone().into();
        let chooser = kaya_starting_player_offer(&pilot.latest)
            .unwrap()
            .deciding_player_id;
        pilot.command(
            chooser,
            Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                starting_player_id: 17,
            }),
        );
        pilot.command(17, Cmd::Mulligan(MulliganDecision { keep: true }));
        if mulligan {
            pilot.command(29, Cmd::Mulligan(MulliganDecision { keep: false }));
        }
        pilot.command(29, Cmd::Mulligan(MulliganDecision { keep: true }));
        if mulligan {
            assert_eq!(
                pilot.state().opening.as_ref().unwrap().bottom,
                Some((29, 1))
            );
            let hand = &pilot.state().players[1].hand;
            let slot = hand
                .iter()
                .position(|id| pilot.state().objects[id].card_id == "forest")
                .unwrap();
            pilot.command(
                29,
                Cmd::PutOpeningHandOnBottom(PutOpeningHandOnBottom {
                    hand_card_index: slot as u32,
                }),
            );
        }
        assert!(
            pilot.state().opening.is_some(),
            "fixed seed must retain an actual Caverns after mulligans; hand={:?}",
            pilot.state().players[1]
                .hand
                .iter()
                .map(|id| &pilot.state().objects[id].card_id)
                .collect::<Vec<_>>()
        );
        assert!(!pilot.offer().gemstone_caverns.is_empty());
        assert!(kaya_batch(&pilot.latest).legal_by_player[&17]
            .opening
            .as_ref()
            .unwrap()
            .gemstone_caverns
            .is_empty());
        pilot
    }
    fn state(&self) -> &tricerules_core::state::GameState {
        &self.session.engine.as_ref().unwrap().state
    }
    fn offer(&self) -> &tricerules_proto::ruled::v1::OpeningActions {
        kaya_batch(&self.latest).legal_by_player[&29]
            .opening
            .as_ref()
            .unwrap()
    }
    fn request(&mut self, request: IpcEnvelope) {
        let response = self.session.process(&request).unwrap();
        pair(&self.root, &mut self.rows, &request, &response);
        assert!(
            response.ok,
            "normal Gemstone request rejected: {}",
            response.error
        );
        assert!(!response.effective_dev_commands_enabled);
        if matches!(request.msg, Some(Msg::PlayerCommand(_))) {
            self.accepted += 1;
        }
        self.latest = response;
    }
    fn command(&mut self, actor: i32, command: Cmd) {
        self.request(IpcEnvelope {
            msg: Some(Msg::PlayerCommand(PlayerCommand {
                player_id: actor,
                ruled_command: RuledCommand { cmd: Some(command) }.encode_to_vec(),
            })),
        });
    }
    fn finish(mut self) {
        assert!(self.state().opening.is_none() && self.state().pending_resolution.is_none());
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
        let replay = reconstruct(&capture, &ReplayOptions::default()).unwrap();
        assert!(
            replay.warnings.is_empty() && replay.differences.is_empty(),
            "{:?} {:?}",
            replay.warnings,
            replay.differences
        );
        assert_eq!(replay.accepted_commands, self.accepted as u64);
        assert_eq!(replay.state, expected);
        println!(
            "Gemstone retained_capture={} accepted_commands={} replay_exact=true dev=false",
            self.root.display(),
            self.accepted
        );
    }
}

#[test]
fn gemstone_normal_mulligan_bottom_opening_entry_and_blue_mana_replay_exactly() {
    let mut pilot = Pilot::new(7_051_201, 5, true);
    let source = pilot.offer().gemstone_caverns[0].object.unwrap();
    let exile = pilot
        .offer()
        .exile_candidates
        .iter()
        .find(|candidate| {
            candidate.object.as_ref().is_some_and(|reference| {
                pilot.state().objects[&reference.object_id].card_id == "forest"
            })
        })
        .unwrap()
        .object;
    let hand = pilot.state().players[1].hand.len();
    pilot.command(
        29,
        Cmd::BeginWithGemstoneCaverns(BeginWithGemstoneCaverns {
            source: Some(source),
            exile,
        }),
    );
    assert_eq!(
        pilot.state().objects[&source.object_id].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        pilot.state().objects[&source.object_id].counter_count(tricerules_cards::CounterKind::Luck),
        1
    );
    assert_eq!(
        pilot.state().objects[&exile.unwrap().object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(pilot.state().players[1].hand.len(), hand - 2);
    assert_eq!(pilot.state().lands_played_this_turn, 0);
    if pilot.state().opening.is_some() {
        pilot.command(
            29,
            Cmd::FinishOpeningHandActions(FinishOpeningHandActions {}),
        );
    }
    pilot.command(17, Cmd::PassPriority(PassPriority {}));
    let generation = pilot.state().zone_change_generation[&source.object_id];
    pilot.command(
        29,
        Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source.object_id,
            expected_zone_change_generation: generation,
            ability_index: 0,
            mana_option_index: 1,
            ..Default::default()
        }),
    );
    assert_eq!(pilot.state().players[1].mana_pool.blue, 1);
    assert_eq!(pilot.state().players[1].mana_pool.colorless, 0);
    assert!(pilot.state().stack.is_empty());
    pilot.finish();
}

#[test]
fn gemstone_normal_optional_opening_decline_replays_without_entry_or_exile() {
    let mut pilot = Pilot::new(7_051_202, 5, true);
    let hand = pilot.state().players[1].hand.len();
    pilot.command(
        29,
        Cmd::FinishOpeningHandActions(FinishOpeningHandActions {}),
    );
    assert_eq!(pilot.state().players[1].hand.len(), hand);
    assert!(
        pilot.state().players[1].battlefield.is_empty()
            && pilot.state().players[1].exile.is_empty()
    );
    pilot.finish();
}
