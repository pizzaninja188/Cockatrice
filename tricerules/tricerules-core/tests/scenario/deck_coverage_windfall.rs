use super::helpers::*;
use prost::Message;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

fn assert_terminal_draw(engine: &mut GameEngine, batch: &rv1::RuledEventBatch) {
    assert_eq!(
        engine.state.outcome,
        Some(tricerules_core::state::GameOutcome::Draw)
    );
    assert!(engine.state.is_terminal());
    assert_eq!(engine.state.winner(), None);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
    assert!(batch.legal_by_player.is_empty());
    assert_eq!(batch.events.iter().filter(|event|matches!(&event.ev,Some(rv1::ruled_event::Ev::Log(log)) if log.text == "Game over. Draw.")).count(),1);
    assert!(!batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(
            rv1::ruled_event::Ev::PriorityChanged(_)
                | rv1::ruled_event::Ev::ResolutionChoiceRequired(_)
                | rv1::ruled_event::Ev::TriggerOrderRequired(_)
        )
    )));
    let snapshot = engine.diagnostic_snapshot().unwrap();
    for actor in engine
        .state
        .players
        .iter()
        .map(|p| p.id)
        .collect::<Vec<_>>()
    {
        assert!(engine.apply_command(actor, &pass()).is_err());
        assert!(engine
            .apply_command(actor, &submit_resolution_choice(vec![0]))
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), snapshot);
    }
    assert_eq!(snapshot["state"]["outcome"], "Draw");
}

fn assert_full_terminal_snapshot(engine: &mut GameEngine) {
    let initial = engine.initial_response_batch();
    assert!(initial.legal_by_player.is_empty());
    assert!(!initial.events.iter().any(|event| matches!(
        &event.ev,
        Some(
            rv1::ruled_event::Ev::PriorityChanged(_)
                | rv1::ruled_event::Ev::ResolutionChoiceRequired(_)
                | rv1::ruled_event::Ev::TriggerOrderRequired(_)
        )
    )));
    let views: Vec<_> = initial
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .collect();
    assert_eq!(views.len(), 1);
    let view = views[0];
    assert!(
        !view.battlefields_unchanged,
        "terminal startup must contain a full battlefield snapshot"
    );
    for player in &engine.state.players {
        let actual = view
            .per_player
            .iter()
            .find(|p| p.player_id == player.id)
            .unwrap();
        assert!(
            !actual.private_zones_unchanged,
            "terminal startup must seed hand and library even after caches were primed"
        );
        assert_eq!(
            actual
                .hand_cards
                .iter()
                .map(|c| (c.object_id, c.card_id.clone()))
                .collect::<Vec<_>>(),
            player
                .hand
                .iter()
                .map(|id| (*id, engine.state.objects[id].card_id.clone()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            actual
                .library_cards
                .iter()
                .map(|c| (c.object_id, c.card_id.clone()))
                .collect::<Vec<_>>(),
            player
                .library
                .iter()
                .map(|id| (*id, engine.state.objects[id].card_id.clone()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            actual
                .battlefield_objects
                .iter()
                .map(|o| o.object_id)
                .collect::<Vec<_>>(),
            player.battlefield
        );
    }
}

fn fixture(seed: u64, ids: &[i32], sizes: &[usize]) -> (GameEngine, Vec<Vec<u32>>) {
    assert!(
        tricerules_cards::CardRegistry::global()
            .get("windfall")
            .is_some(),
        "Windfall must be completely registered"
    );
    let mut engine = GameEngine::new(seed, ids, 20, None, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    for player in &mut engine.state.players {
        for oid in std::mem::take(&mut player.hand) {
            engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Library;
            player.library.push_back(oid);
        }
    }
    let hands = sizes
        .iter()
        .enumerate()
        .map(|(seat, size)| {
            (0..*size)
                .map(|_| inject_card_into_hand(&mut engine, seat, "mountain"))
                .collect()
        })
        .collect();
    (engine, hands)
}

fn cast(engine: &mut GameEngine) -> u32 {
    let active = engine.state.active_player_id();
    let seat = engine
        .state
        .players
        .iter()
        .position(|p| p.id == active)
        .unwrap();
    let spell = inject_card_into_hand(engine, seat, "windfall");
    give_mana(
        engine,
        active,
        ManaGift {
            c: 2,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, seat, "windfall");
    semantic::accepted(engine, active, &cast_spell(slot, vec![]));
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    assert_eq!(engine.state.players[seat].mana_pool.blue, 0);
    assert_eq!(engine.state.players[seat].mana_pool.colorless, 0);
    spell
}

#[test]
fn windfall_paid_cast_uses_greatest_discard_cohort_not_sum_or_caster_hand() {
    let (mut engine, hands) = fixture(50501, &[0, 1, 2], &[2, 5, 3]);
    let expected: Vec<Vec<u32>> = engine
        .state
        .players
        .iter()
        .map(|p| p.library.iter().take(5).copied().collect())
        .collect();
    let spell = cast(&mut engine);
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    for (seat, hand) in hands.iter().enumerate() {
        assert_eq!(engine.state.players[seat].hand, expected[seat]);
        assert!(hand
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Graveyard));
    }
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn windfall_tied_maximum_and_empty_hands_finish_without_choices() {
    for (case, sizes) in [[3, 3], [0, 0]].into_iter().enumerate() {
        let (mut engine, hands) = fixture(50510 + case as u64, &[0, 1], &sizes);
        let before: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|p| p.library.len())
            .collect();
        cast(&mut engine);
        pass_priority_round(&mut engine);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        for (seat, hand) in hands.iter().enumerate() {
            assert_eq!(engine.state.players[seat].hand.len(), sizes[0]);
            assert_eq!(
                engine.state.players[seat].library.len(),
                before[seat] - sizes[0]
            );
            assert!(hand
                .iter()
                .all(|oid| engine.state.objects[oid].zone == Zone::Graveyard));
        }
    }
}

#[test]
fn windfall_four_nonconsecutive_seats_draw_apnap_and_exclude_departed_players() {
    for departed in [false, true] {
        let (mut engine, hands) =
            fixture(50515 + u64::from(departed), &[4, 9, 27, 52], &[1, 4, 2, 0]);
        engine.state.active_player_idx = 2;
        engine.state.priority_idx = 2;
        if departed {
            engine.state.players[1].has_lost = true;
            engine.state.players[1].life = 0;
        }
        let count = if departed { 2 } else { 4 };
        cast(&mut engine);
        let mut logs = Vec::new();
        for _ in 0..if departed { 3 } else { 4 } {
            let actor = engine.state.priority_player_id();
            let batch = engine.apply_command(actor, &pass()).unwrap();
            logs.extend(batch.events.into_iter().filter_map(|event| match event.ev {
                Some(rv1::ruled_event::Ev::Log(log))
                    if log.text.ends_with("(Windfall).") && log.text.contains(" draws ") =>
                {
                    Some(log.text)
                }
                _ => None,
            }));
        }
        let order = if departed {
            vec![27, 52, 4]
        } else {
            vec![27, 52, 4, 9]
        };
        assert_eq!(
            logs,
            order
                .iter()
                .map(|id| format!("P{id} draws {count} cards (Windfall)."))
                .collect::<Vec<_>>()
        );
        for (seat, hand) in hands.iter().enumerate() {
            if departed && seat == 1 {
                continue;
            }
            assert_eq!(engine.state.players[seat].hand.len(), count);
            assert!(hand
                .iter()
                .all(|oid| engine.state.objects[oid].zone == Zone::Graveyard));
        }
    }
}

#[test]
fn windfall_leng_destinations_and_order_finish_before_every_draw() {
    let (mut engine, hands) = fixture(50520, &[0, 1, 2], &[3, 2, 1]);
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    let before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|p| p.library.len())
        .collect();
    let spell = cast(&mut engine);
    pass_priority_round(&mut engine);
    let original = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![1]))
        .is_err());
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![99]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), original);
    for destination in [1, 1, 0] {
        engine
            .apply_command(0, &submit_resolution_choice(vec![destination]))
            .unwrap();
        for (seat, hand) in hands.iter().enumerate() {
            assert_eq!(
                &engine.state.players[seat].hand, hand,
                "all hands retained until destination ordering completes"
            );
            assert_eq!(engine.state.players[seat].library.len(), before[seat]);
        }
        assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    }
    let before_order = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![hands[0][0], hands[0][0]]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_order);
    engine
        .apply_command(0, &submit_resolution_choice(vec![hands[0][1], hands[0][0]]))
        .unwrap();
    assert_eq!(
        &engine.state.players[0].hand[..2],
        &[hands[0][1], hands[0][0]]
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        3,
        "library-destination discards contribute to maximum"
    );
    assert_eq!(engine.state.zone_change_generation[&hands[0][0]], 2);
    for seat in 1..3 {
        assert_eq!(engine.state.players[seat].hand.len(), 3);
    }
    assert_eq!(engine.state.objects[&hands[0][2]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn windfall_madness_exile_counts_and_offer_waits_until_wheel_finishes() {
    let (mut engine, _) = fixture(50530, &[0, 1], &[0, 1]);
    let madness = inject_card_into_hand(&mut engine, 0, "fiery_temper");
    inject_card_into_hand(&mut engine, 0, "mountain");
    let spell = cast(&mut engine);
    pass_priority_round(&mut engine);
    assert!(
        engine.state.pending_resolution.is_none(),
        "madness casting waits until Windfall finishes"
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&madness].zone, Zone::Exile);
    assert!(engine.state.players.iter().all(|p| p.hand.len() == 2));
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.source_permanent_id == Some(madness)));
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_command(
            0,
            &submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline),
        )
        .unwrap();
    assert_eq!(engine.state.objects[&madness].zone, Zone::Graveyard);
}

#[test]
fn windfall_early_deckout_waits_for_later_parked_draw_replacement() {
    let (mut engine, _) = fixture(50540, &[4, 9, 27], &[2, 1, 0]);
    engine.state.players[0].library.clear();
    for _ in 0..2 {
        inject_permanent_on_battlefield(&mut engine, 1, "thought_reflection");
    }
    let spell = cast(&mut engine);
    let mut choice = None;
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        let batch = engine.apply_command(actor, &pass()).unwrap();
        choice = find_resolution_choice(&batch).or(choice);
    }
    assert!(engine.state.players[0].pending_library_loss);
    assert!(!engine.state.players[0].has_lost);
    assert!(
        engine.state.players.iter().all(|p| p.hand.is_empty()),
        "all discards precede the later player's first draw choice"
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    for _ in 0..24 {
        if engine.state.pending_resolution.is_none() {
            break;
        }
        let current = choice.take().expect("parked draw replacement");
        assert_eq!(current.deciding_player_id, 9);
        assert_eq!(current.choice_kind(), rv1::ChoiceKind::ReplacementEffect);
        let application = current.replacement_options[0].application_id;
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(27, &submit_resolution_choice(vec![application]))
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        let batch = engine
            .apply_command(9, &submit_resolution_choice(vec![application]))
            .unwrap();
        choice = find_resolution_choice(&batch);
    }
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[0].has_lost);
    assert_eq!(
        engine.state.players[1].hand.len(),
        8,
        "two fixed draws each replaced twice"
    );
    assert_eq!(
        engine.state.players[2].hand.len(),
        2,
        "later recipient still draws frozen maximum"
    );
    assert!(
        !engine.state.objects.contains_key(&spell),
        "departed caster's owned spell leaves the game after resolution"
    );
}

#[test]
fn windfall_all_remaining_players_deckout_ends_in_draw() {
    let (mut engine, _) = fixture(50550, &[0, 1, 2], &[2, 1, 0]);
    for player in &mut engine.state.players {
        player.library.clear();
    }
    let spell = cast(&mut engine);
    let mut events = Vec::new();
    let mut terminal = None;
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        let command = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CanonicalGameplay(
                rv1::CanonicalGameplayCommand {
                    command: pass().encode_to_vec(),
                    auto_pass_policies: engine
                        .state
                        .players
                        .iter()
                        .map(|player| rv1::AutoPassPolicy {
                            player_id: player.id,
                            stop_on_own_turn: vec![],
                            stop_on_opponent_turn: vec![],
                        })
                        .collect(),
                },
            )),
        };
        let batch = engine.apply_command(actor, &command).unwrap();
        events.extend(batch.events.clone());
        terminal = Some(batch);
    }
    assert!(engine.state.players.iter().all(|p| p.has_lost));
    assert!(events.iter().any(|event|matches!(&event.ev,Some(rv1::ruled_event::Ev::Log(log)) if log.text == "Game over. Draw.")),"all simultaneous library losses must publish a terminal draw");
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_terminal_draw(&mut engine, &terminal.unwrap());
    let initial = engine.initial_response_batch();
    assert_terminal_draw(&mut engine, &initial);
}

#[test]
fn risky_shortcut_simultaneous_life_loss_ends_in_draw_after_draw_instruction() {
    let (mut engine, _) = fixture(50551, &[4, 9, 27], &[1, 1, 1]);
    for player in &mut engine.state.players {
        player.life = 2;
    }
    inject_card_into_hand(&mut engine, 0, "risky_shortcut");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            c: 2,
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "risky_shortcut");
    // Populate omission caches before the terminal transaction.
    engine.initial_response_batch();
    semantic::accepted(&mut engine, 4, &cast_spell(slot, vec![]));
    let mut events = Vec::new();
    let mut terminal = None;
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        let batch = engine.apply_command(actor, &pass()).unwrap();
        events.extend(batch.events.clone());
        terminal = Some(batch);
    }
    assert!(events.iter().any(|event|matches!(&event.ev,Some(rv1::ruled_event::Ev::Log(log)) if log.text == "P4 draws 2 cards (Risky Shortcut).")),"mandatory draw finishes before simultaneous life losses");
    assert!(engine
        .state
        .players
        .iter()
        .all(|p| p.has_lost && p.life == 0));
    assert!(events.iter().any(|event|matches!(&event.ev,Some(rv1::ruled_event::Ev::Log(log)) if log.text == "Game over. Draw.")),"all simultaneous life losses must publish a terminal draw");
    assert_terminal_draw(&mut engine, &terminal.unwrap());
    let initial = engine.initial_response_batch();
    assert_terminal_draw(&mut engine, &initial);
    assert_full_terminal_snapshot(&mut engine);
}

#[test]
fn windfall_terminal_draw_replays_exact_accepted_commands_and_batches() {
    let fresh = || {
        let (mut engine, _) = fixture(50552, &[4, 9, 27], &[1, 2, 0]);
        for player in &mut engine.state.players {
            player.library.clear();
        }
        cast(&mut engine);
        engine
    };
    let mut engine = fresh();
    let mut recorded = Vec::new();
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        let command = pass();
        let batch = engine.apply_command(actor, &command).unwrap();
        recorded.push((actor, command, batch));
    }
    let mut replay = fresh();
    for (actor, command, batch) in recorded {
        let decoded = rv1::RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), batch);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap()
    );
    let initial = engine.initial_response_batch();
    assert_terminal_draw(&mut engine, &initial);
}

#[test]
fn windfall_two_deckout_losses_leave_the_nonconsecutive_survivor_as_winner() {
    let (mut engine, _) = fixture(50553, &[4, 9, 27], &[1, 2, 0]);
    let permanent = inject_permanent_on_battlefield(&mut engine, 2, "mountain");
    engine.initial_response_batch();
    engine.state.players[0].library.clear();
    engine.state.players[1].library.clear();
    cast(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.outcome,
        Some(tricerules_core::state::GameOutcome::Winner(27))
    );
    assert_eq!(engine.state.winner(), Some(27));
    assert_eq!(engine.state.players[2].hand.len(), 2);
    assert!(engine.state.players[..2].iter().all(|p| p.has_lost));
    assert!(!engine.state.players[2].library.is_empty());
    assert_eq!(engine.state.players[2].battlefield, [permanent]);
    assert_full_terminal_snapshot(&mut engine);
}

#[test]
fn windfall_exact_identity_face_and_complete_typed_effects() {
    use tricerules_cards::primitives::{
        Amount, CardResultAction, CardResultSource, CountExpression, DiscardQuantity,
        PlayerRecipient, RelativePlayerSet, SpellEffectKind,
    };
    let definition = tricerules_cards::CardRegistry::global()
        .get("windfall")
        .unwrap();
    assert_eq!(definition.name, "Windfall");
    assert_eq!(definition.layout, tricerules_cards::Layout::Normal);
    assert_eq!(definition.faces_iter().count(), 1);
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![tricerules_cards::Color::Blue]);
    assert_eq!((face.power, face.toughness), (None, None));
    assert_eq!(face.spell_effect.len(), 2);
    assert!(matches!(
        &face.spell_effect[0],
        SpellEffectKind::Discard {
            who: PlayerRecipient::EachPlayer,
            quantity: DiscardQuantity::All
        }
    ));
    assert!(
        matches!(&face.spell_effect[1],SpellEffectKind::Draw { who: PlayerRecipient::EachPlayer, count: Amount::Count(CountExpression::MaximumCardsMatchingResult { filter }) } if filter.source == CardResultSource::PreviousEffect && filter.action == CardResultAction::Discard && filter.players == RelativePlayerSet::All && filter.card_type.is_none())
    );
}
