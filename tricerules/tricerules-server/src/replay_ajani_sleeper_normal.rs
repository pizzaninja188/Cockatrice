// Strict normal-session Ajani pilot, fixed seed32 after bounded separate preparation.
// Legal60-card decks; every mutation is a captured command, Dev disabled. No injected state.
use super::*;

const AJANI_ID: &str = "ajani,_sleeper_agent";

struct Pilot {
    session: EngineSession,
    root: std::path::PathBuf,
    rows: Vec<String>,
    manifest: Value,
    latest: IpcResponse,
    battlefield: Vec<BattlefieldObject>,
    opponent_battlefield: Vec<BattlefieldObject>,
    accepted: usize,
    keep_choices: usize,
    bottom_choices: usize,
    target_choices: usize,
}

impl Pilot {
    fn new(game_id: u64) -> Self {
        let decks = vec![
            (
                17,
                [
                    vec!["Ajani, Sleeper Agent".to_string(); 4],
                    vec!["Ornithopter".to_string(); 4],
                    vec!["Plains".to_string(); 26],
                    vec!["Forest".to_string(); 26],
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
            })),
        };
        let validation = session.process(&validation_request).unwrap();
        assert!(validation.ok && validation.missing_card_names.is_empty());
        let captures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/deck-coverage/ajani-whole-card-20261103/normal-captures");
        fs::create_dir_all(&captures).unwrap();
        let root = tempfile::Builder::new()
            .prefix(&format!("ajani-sleeper-{game_id}-"))
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
            "Ajani retained_capture={} fixed_seed=32 decks={decks:?}",
            root.display()
        );
        let mut pilot = Self {
            session,
            root,
            rows: Vec::new(),
            manifest: serde_json::json!({
                "format_version":1, "source":"server", "privacy":"server_only", "capture_id":format!("ajani-sleeper-{game_id}"),
                "engine_build":ENGINE_BUILD, "card_data_hash":"", "effective_dev_commands_enabled":false, "complete":false,
            }),
            latest: IpcResponse::default(),
            battlefield: Vec::new(),
            opponent_battlefield: Vec::new(),
            accepted: 0,
            keep_choices: 0,
            bottom_choices: 0,
            target_choices: 0,
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
            "normal Ajani request rejected: {}",
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
                        !matches!(offer.card_name.as_str(), "Plains" | "Forest"),
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
        panic!("normal Ajani driver did not reach requested Main1");
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
                    .filter(|p| p.card_id == "forest")
                    .count()
        {
            "Plains"
        } else {
            "Forest"
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
            .filter(|p| !p.tapped && matches!(p.card_id.as_str(), "plains" | "forest"))
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
            .filter(|p| !p.tapped && matches!(p.card_id.as_str(), "plains" | "forest"))
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
        targets: &[u32],
        flex: Option<FlexPipPayment>,
    ) -> u32 {
        let offer = kaya_batch(&self.latest).legal_by_player[&actor]
            .hand_actions
            .iter()
            .find(|a| {
                a.card_name == name
                    && a.kind == HandActionKind::HandActionCastSpell as i32
                    && a.cast_method == CastMethod::Normal as i32
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
                    cast_method: CastMethod::Normal as i32,
                    flex_payments: flex.into_iter().collect(),
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
                'G' => mana.g += 1,
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
        self.resolve_choices(None, None);
    }
    fn resolve_choices(&mut self, keep: Option<u32>, branch: Option<u32>) {
        for _ in 0..128 {
            if self.state().is_terminal() {
                return;
            }
            if let Some(pending) = self.state().pending_resolution.as_ref() {
                let prompt = pending.resolution_prompt_id.get();
                let kind = pending.presentation.choice_kind;
                let offered = kaya_batch(&self.latest)
                    .events
                    .iter()
                    .find_map(|event| match &event.ev {
                        Some(Ev::ResolutionChoiceRequired(choice))
                            if choice.resolution_prompt_id == prompt =>
                        {
                            Some(choice)
                        }
                        _ => None,
                    })
                    .expect("the current resolution choice was offered");
                assert_eq!(offered.deciding_player_id, 17);
                let choice = match kind {
                    ChoiceKind::LegendKeep => {
                        let keep = keep.expect("explicit current legend choice");
                        assert!(offered.candidate_object_ids.contains(&keep));
                        SubmitResolutionChoice {
                            resolution_prompt_id: prompt,
                            chosen_object_ids: vec![keep],
                            ..Default::default()
                        }
                    }
                    ChoiceKind::ResolutionBranch => {
                        let branch = branch.expect("explicit Keep or Bottom");
                        assert!(offered
                            .resolution_branches
                            .iter()
                            .any(|option| option.branch_index == branch && option.selectable));
                        assert_eq!(offered.public_reveal.as_ref().unwrap().cards.len(), 1);
                        if branch == 0 {
                            self.keep_choices += 1;
                        } else {
                            assert_eq!(branch, 1);
                            self.bottom_choices += 1;
                        }
                        SubmitResolutionChoice {
                            resolution_prompt_id: prompt,
                            decision: ResolutionChoiceDecision::SelectBranch as i32,
                            selected_branch_index: branch,
                            ..Default::default()
                        }
                    }
                    _ => panic!("unexpected normal Ajani choice {kind:?}"),
                };
                self.command(17, Cmd::SubmitResolutionChoice(choice));
            } else if self.state().pending_triggers.front().is_some() {
                let offered = kaya_batch(&self.latest)
                    .events
                    .iter()
                    .find_map(|event| match &event.ev {
                        Some(Ev::TriggerNeedsTarget(choice)) => Some(choice),
                        _ => None,
                    })
                    .expect("current emblem target offer");
                assert_eq!(offered.controller_player_id, 17);
                let targets = offered.targets.as_ref().unwrap();
                assert_eq!(targets.groups.len(), 1);
                let group = &targets.groups[0];
                assert!(group.can_target_opponent && !group.can_target_self);
                assert!(group.valid_permanent_ids.is_empty());
                self.target_choices += 1;
                self.command(
                    17,
                    Cmd::ChooseTriggerTarget(tricerules_proto::ruled::v1::ChooseTriggerTarget {
                        targets: vec![TargetRef {
                            object_id: 29,
                            kind: TargetRefKind::Player as i32,
                            group_index: group.group_index,
                            ..Default::default()
                        }],
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
                self.command(
                    self.state().priority_player_id(),
                    Cmd::PassPriority(PassPriority {}),
                );
            }
        }
        panic!("normal Ajani stack did not settle");
    }
    fn activate(&mut self, stable_id: &str, shares: &[(u32, u32)], branch: Option<u32>) {
        let source = self
            .battlefield
            .iter()
            .find(|p| p.card_id == AJANI_ID)
            .unwrap();
        let ability = luxior_normal_current_ability(source, stable_id);
        assert!(ability.activatable);
        let targets = shares
            .iter()
            .map(|&(id, count)| TargetRef {
                object_id: id,
                kind: TargetRefKind::Permanent as i32,
                counter_amount: count,
                expected_zone_change_generation: Some(
                    self.state()
                        .zone_change_generation
                        .get(&id)
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
        self.resolve_choices(None, branch);
    }
    fn next_main(&mut self) {
        self.advance_main(29);
        self.play_land(29);
        self.advance_main(17);
        self.play_land(17);
    }
    fn loyalty(&self, source: u32) -> u32 {
        self.state().objects[&source].counter_count(tricerules_cards::CounterKind::Loyalty)
    }
    fn poison(&self) -> u32 {
        self.state().players[1]
            .counters
            .get(&tricerules_cards::CounterKind::Poison)
            .copied()
            .unwrap_or(0)
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
            "Ajani complete normal capture replayed exactly: {} accepted commands at {}",
            self.accepted,
            self.root.display()
        );
    }
}

fn land_color(id: &str) -> char {
    match id {
        "plains" => 'W',
        "forest" => 'G',
        _ => panic!("unsupported basic land"),
    }
}
fn flex(color: tricerules_proto::ruled::v1::ManaColor, life: bool) -> FlexPipPayment {
    FlexPipPayment {
        pip_index: 2,
        pay_life: life,
        mana_color: color as i32,
    }
}

#[test]
fn ajani_normal_three_payment_branches_all_loyalty_clauses_and_poison_win_replay_exactly() {
    use tricerules_core::Zone;
    use tricerules_proto::ruled::v1::ManaColor;
    let mut p = Pilot::new(7_311_701);
    for turn in 1..=3 {
        p.advance_main(17);
        p.play_land(17);
        if turn < 3 {
            p.advance_main(29);
            p.play_land(29);
        }
    }
    assert!(p.can_cast(17, "Ajani, Sleeper Agent", &['G', 'W'], 1));
    let first = p.cast(
        17,
        "Ajani, Sleeper Agent",
        &['G', 'W'],
        1,
        &[],
        Some(flex(ManaColor::Unspecified, true)),
    );
    p.resolve();
    assert_eq!(p.loyalty(first), 2);
    assert_eq!(p.state().players[0].life, 18);
    p.activate("activated_01", &[], None);
    assert_eq!(p.loyalty(first), 3);
    let thopter = p.cast(17, "Ornithopter", &[], 0, &[], None);
    p.resolve();
    p.next_main();
    p.activate("activated_02", &[(thopter, 3)], None);
    assert_eq!(p.state().objects[&first].zone, Zone::Graveyard);
    assert_eq!(
        p.state().objects[&thopter].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        3
    );
    assert!(p
        .session
        .engine
        .as_ref()
        .unwrap()
        .characteristics(thopter)
        .unwrap()
        .has_keyword(tricerules_cards::Keyword::Vigilance));
    assert!(p.can_cast(17, "Ajani, Sleeper Agent", &['G', 'G', 'W'], 1));
    let second = p.cast(
        17,
        "Ajani, Sleeper Agent",
        &['G', 'G', 'W'],
        1,
        &[],
        Some(flex(ManaColor::Green, false)),
    );
    p.resolve();
    assert_eq!(p.loyalty(second), 4);
    p.activate("activated_01", &[], None);
    assert_eq!(p.loyalty(second), 5);
    p.next_main();
    assert!(!p
        .session
        .engine
        .as_ref()
        .unwrap()
        .characteristics(thopter)
        .unwrap()
        .has_keyword(tricerules_cards::Keyword::Vigilance));
    p.activate("activated_01", &[], Some(0));
    assert_eq!(p.loyalty(second), 6);
    assert_eq!(p.keep_choices, 1);
    p.next_main();
    p.activate("activated_03", &[], None);
    assert_eq!(p.state().objects[&second].zone, Zone::Graveyard);
    assert_eq!(p.state().static_emblems.len(), 1);
    let third = p.cast(
        17,
        "Ajani, Sleeper Agent",
        &['G', 'W', 'W'],
        1,
        &[],
        Some(flex(ManaColor::White, false)),
    );
    p.resolve_choices(Some(third), None);
    assert_eq!(p.loyalty(third), 4);
    assert_eq!(p.poison(), 2);
    p.activate("activated_01", &[], Some(1));
    for _ in 0..50 {
        while !p.state().is_terminal()
            && (p.poison() < 8 || p.bottom_choices > 0)
            && p.can_cast(17, "Ornithopter", &[], 0)
        {
            p.cast(17, "Ornithopter", &[], 0, &[], None);
            p.resolve();
        }
        if !p.state().is_terminal()
            && (p.poison() < 8 || p.bottom_choices > 0)
            && p.can_cast(17, "Ajani, Sleeper Agent", &['G', 'W', 'W'], 1)
        {
            let next = p.cast(
                17,
                "Ajani, Sleeper Agent",
                &['G', 'W', 'W'],
                1,
                &[],
                Some(flex(ManaColor::White, false)),
            );
            p.resolve_choices(Some(next), None);
        }
        if p.state().is_terminal() {
            break;
        }
        p.next_main();
        p.activate(
            "activated_01",
            &[],
            Some(if p.bottom_choices == 0 { 1 } else { 0 }),
        );
    }
    assert_eq!(
        p.state().outcome,
        Some(tricerules_core::state::GameOutcome::Winner(17))
    );
    assert_eq!(p.poison(), 10);
    assert_eq!(p.state().players[0].life, 18);
    assert_eq!(p.target_choices, 5);
    assert!(p.keep_choices > 0 && p.bottom_choices > 0);
    assert_eq!(p.state().static_emblems.len(), 1);
    p.manifest["three_payment_branches"] = true.into();
    p.manifest["all_loyalty_clauses"] = true.into();
    p.manifest["poison_win"] = true.into();
    p.finish();
}
