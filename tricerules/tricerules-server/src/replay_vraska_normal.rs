// Strict normal-session Vraska pilots. Every mutation is a captured player command.
use super::*;

const VRASKA_ID: &str = "vraska,_betrayals_sting";

struct Pilot {
    session: EngineSession,
    root: std::path::PathBuf,
    rows: Vec<String>,
    manifest: Value,
    latest: IpcResponse,
    battlefield: Vec<BattlefieldObject>,
    accepted: usize,
}

impl Pilot {
    fn new(game_id: u64) -> Self {
        let mut cards = Vec::new();
        for name in [
            "Vraska, Betrayal's Sting",
            "Doubling Season",
            "Llanowar Elves",
        ] {
            cards.extend(vec![name.to_string(); 4]);
        }
        cards.extend(vec!["Forest".to_string(); 24]);
        cards.extend(vec!["Swamp".to_string(); 24]);
        let decks = vec![(17, cards), (29, vec!["Island".to_string(); 60])]
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
        let root = tempfile::tempdir().unwrap().keep();
        fs::create_dir(root.join("raw")).unwrap();
        println!(
            "Vraska retained_capture={} fixed_seed=19 decks={decks:?}",
            root.display()
        );
        let mut pilot = Self {
            session,
            root,
            rows: Vec::new(),
            manifest: serde_json::json!({
                "format_version":1, "source":"server", "privacy":"server_only", "capture_id":format!("vraska-{game_id}"),
                "engine_build":ENGINE_BUILD, "card_data_hash":"", "effective_dev_commands_enabled":false, "complete":false,
            }),
            latest: IpcResponse::default(),
            battlefield: Vec::new(),
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
            "normal Vraska request rejected: {}",
            response.error
        );
        assert!(!response.effective_dev_commands_enabled);
        if matches!(request.msg, Some(Msg::PlayerCommand(_))) {
            self.accepted += 1;
        }
        assert!(self.accepted <= 8192);
        kaya_capture_battlefield(&response, 17, &mut self.battlefield);
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
                        !matches!(offer.card_name.as_str(), "Forest" | "Swamp" | "Island"),
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
        panic!("normal Vraska driver did not reach requested Main1");
    }

    fn play_land(&mut self, actor: i32) {
        let preferred = if self
            .battlefield
            .iter()
            .filter(|card| card.card_id == "forest")
            .count()
            < 2
        {
            "Forest"
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

    fn can_cast(&self, name: &str, green: usize, black: usize, generic: usize) -> bool {
        let available = self
            .battlefield
            .iter()
            .filter(|card| !card.tapped && matches!(card.card_id.as_str(), "forest" | "swamp"))
            .collect::<Vec<_>>();
        kaya_batch(&self.latest).legal_by_player[&17]
            .hand_actions
            .iter()
            .any(|offer| {
                offer.card_name == name && offer.kind == HandActionKind::HandActionCastSpell as i32
            })
            && available.len() >= green + black + generic
            && available
                .iter()
                .filter(|card| card.card_id == "forest")
                .count()
                >= green
            && available
                .iter()
                .filter(|card| card.card_id == "swamp")
                .count()
                >= black
    }

    fn land_mana(&mut self, color: Option<char>) -> char {
        let (oid, generation, index, produced) = self
            .battlefield
            .iter()
            .filter(|card| !card.tapped && matches!(card.card_id.as_str(), "forest" | "swamp"))
            .flat_map(|card| {
                card.activated_abilities
                    .iter()
                    .filter(|ability| ability.is_mana_ability && ability.activatable)
                    .map(move |ability| {
                        (
                            card.object_id,
                            card.zone_change_generation,
                            ability.ability_index,
                            if card.card_id == "forest" { 'G' } else { 'B' },
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
            17,
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
        name: &str,
        green: usize,
        black: usize,
        generic: usize,
        pay_life: bool,
    ) -> u32 {
        let offer = kaya_batch(&self.latest).legal_by_player[&17]
            .hand_actions
            .iter()
            .find(|offer| {
                offer.card_name == name && offer.kind == HandActionKind::HandActionCastSpell as i32
            })
            .unwrap()
            .clone();
        let object_id = self.state().players[0].hand[offer.hand_index as usize];
        self.command(
            17,
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
                    flex_payments: if pay_life {
                        vec![FlexPipPayment {
                            pip_index: 2,
                            pay_life: true,
                            mana_color: 0,
                        }]
                    } else {
                        Vec::new()
                    },
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
        for color in std::iter::repeat_n(Some('G'), green)
            .chain(std::iter::repeat_n(Some('B'), black))
            .chain(std::iter::repeat_n(None, generic))
        {
            match self.land_mana(color) {
                'G' => mana.g += 1,
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
                player_id: 17,
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
            17,
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
        panic!("normal Vraska stack failed to settle");
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

    fn activate_vraska(&mut self, stable_id: &str, target: Option<TargetRef>) {
        let source = self
            .battlefield
            .iter()
            .find(|card| card.card_id == VRASKA_ID)
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

    fn setup(&mut self) -> (u32, u32) {
        let mut elf = None;
        let mut season = None;
        for turn in 1..=24 {
            self.advance_main(17);
            self.play_land(17);
            if elf.is_none() && self.can_cast("Llanowar Elves", 1, 0, 0) {
                elf = Some(self.cast("Llanowar Elves", 1, 0, 0, false));
                self.resolve();
            }
            if season.is_none() && self.can_cast("Doubling Season", 2, 0, 3) {
                season = Some(self.cast("Doubling Season", 2, 0, 3, false));
                self.resolve();
            }
            if let Some(elf) = elf
                .filter(|_| season.is_some() && self.can_cast("Vraska, Betrayal's Sting", 0, 1, 4))
            {
                let vraska = self.cast("Vraska, Betrayal's Sting", 0, 1, 4, true);
                self.resolve();
                let choice = self.offered_choice();
                assert_eq!(choice.choice_kind, ChoiceKind::ReplacementEffect as i32);
                assert_eq!(choice.deciding_player_id, 17);
                let option = choice
                    .replacement_options
                    .iter()
                    .find(|option| Some(option.source_object_id) == season)
                    .unwrap();
                self.command(
                    17,
                    Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                        chosen_object_ids: vec![option.application_id],
                        resolution_prompt_id: choice.resolution_prompt_id,
                        ..Default::default()
                    }),
                );
                self.resolve();
                assert_eq!(
                    self.state().objects[&vraska]
                        .counter_count(tricerules_cards::CounterKind::Loyalty),
                    10
                );
                assert_eq!(self.state().players[0].life, 18);
                self.manifest["setup_turn"] = turn.into();
                println!("Vraska fixed-seed legal curve reached turn {turn}; Season-first entry=10 loyalty, life=18");
                return (vraska, elf);
            }
            self.advance_main(29);
            self.play_land(29);
        }
        panic!("fixed-seed normal Vraska setup missed its explicit 24-turn bound");
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
            "Vraska complete normal capture replayed exactly: {} accepted commands at {}",
            self.accepted,
            self.root.display()
        );
    }
}

#[test]
fn vraska_normal_poison_then_proliferate_records_and_replays_exactly() {
    let mut pilot = Pilot::new(7_281_103);
    let (vraska, _) = pilot.setup();
    let source = pilot
        .battlefield
        .iter()
        .find(|card| card.object_id == vraska)
        .unwrap();
    let ability = luxior_normal_current_ability(source, "activated_03");
    let group = luxior_normal_target_group(&pilot.latest, vraska, ability.ability_index);
    assert!(group.can_target_self && group.can_target_opponent);
    let target = TargetRef {
        object_id: 29,
        group_index: group.group_index,
        kind: TargetRefKind::Player as i32,
        ..Default::default()
    };
    pilot.activate_vraska("activated_03", Some(target));
    assert_eq!(
        pilot.state().players[1].counters[&tricerules_cards::CounterKind::Poison],
        9
    );
    assert!(!pilot.state().players[1].has_lost);
    pilot.advance_main(29);
    pilot.play_land(29);
    pilot.advance_main(17);
    let hand_before = pilot.state().players[0].hand.len();
    pilot.activate_vraska("activated_01", None);
    assert_eq!(pilot.state().players[0].hand.len(), hand_before + 1);
    assert_eq!(pilot.state().players[0].life, 17);
    let choice = pilot.offered_choice();
    assert_eq!(choice.choice_kind, ChoiceKind::Proliferate as i32);
    assert!(choice.candidate_player_ids.contains(&29));
    pilot.command(
        17,
        Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_player_ids: vec![29],
            resolution_prompt_id: choice.resolution_prompt_id,
            ..Default::default()
        }),
    );
    assert_eq!(
        pilot.state().players[1].counters[&tricerules_cards::CounterKind::Poison],
        10
    );
    assert!(pilot.state().players[1].has_lost);
    pilot.finish();
}

#[test]
fn vraska_normal_treasure_consumes_the_exact_normally_cast_creature_and_replays() {
    let mut pilot = Pilot::new(7_281_104);
    let (vraska, elf) = pilot.setup();
    let source = pilot
        .battlefield
        .iter()
        .find(|card| card.object_id == vraska)
        .unwrap();
    let ability = luxior_normal_current_ability(source, "activated_02");
    let group = luxior_normal_target_group(&pilot.latest, vraska, ability.ability_index);
    let victim = pilot
        .battlefield
        .iter()
        .find(|card| card.object_id == elf)
        .unwrap();
    let generation = victim.zone_change_generation;
    let target = luxior_normal_target_ref(group, victim);
    pilot.activate_vraska("activated_02", Some(target));
    let victim = pilot
        .battlefield
        .iter()
        .find(|card| card.object_id == elf)
        .unwrap();
    assert_eq!(victim.card_id, "llanowar_elves");
    assert_eq!(victim.zone_change_generation, generation);
    assert!(!victim.is_creature);
    assert!(victim
        .rules_annotation_labels
        .contains(&"Types: Artifact, Treasure".into()));
    assert_eq!(victim.activated_abilities.len(), 1);
    // Granted occurrences retain their recipient slot; their presentation can identify the
    // granting definition. Submit the current engine slot rather than a repeated-field index.
    let ability = &victim.activated_abilities[0];
    assert!(ability.is_mana_ability && ability.activatable && !ability.has_only_tap_cost);
    let mut activation = ActivateAbility {
        source_object_id: elf,
        expected_zone_change_generation: generation,
        ability_index: ability.ability_index,
        source_zone: AbilitySourceZone::Battlefield as i32,
        mana_option_index: 4,
        ..Default::default()
    };
    let revision = pilot.state().command_index;
    pilot.request(IpcEnvelope {
        msg: Some(Msg::PaymentQuery(PaymentQuery {
            player_id: 17,
            preview: Some(PreviewPayment {
                revision,
                activate_ability: Some(activation.clone()),
                ..Default::default()
            }),
        })),
    });
    let preview = kaya_batch(&pilot.latest)
        .payment_preview
        .as_ref()
        .unwrap()
        .clone();
    assert!(preview.valid && preview.complete, "{preview:?}");
    activation.payment = preview.selection;
    activation.restricted_mana = preview.restricted_mana;
    pilot.command(17, Cmd::ActivateAbility(activation));
    assert_eq!(
        pilot.state().objects[&elf].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(pilot.state().players[0].graveyard.contains(&elf));
    assert_eq!(pilot.state().players[0].mana_pool.green, 1);
    assert!(pilot.state().stack.is_empty());
    pilot.finish();
}
