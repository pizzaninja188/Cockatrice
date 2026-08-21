use crate::helpers::*;

#[test]
fn opening_choose_first_london_mulligan_then_start() {
    use tricerules_proto::ruled::v1::ruled_command::Cmd;
    use tricerules_proto::ruled::v1::{
        ChooseStartingPlayer, MulliganDecision, PutOpeningHandOnBottom, RuledCommand,
    };
    // seed 100 → chooser is player_ids[0] == 5
    let mut e = GameEngine::new(100, &[5, 6], 20, None, false).expect("new");
    let chooser = e.state.opening.as_ref().expect("opening").chooser;
    assert_eq!(chooser, 5);
    e.apply_command(
        chooser,
        &RuledCommand {
            cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                starting_player_id: 5,
            })),
        },
    )
    .expect("choose first");
    assert_eq!(e.state.players[0].hand.len(), 7);
    assert_eq!(e.state.players[1].hand.len(), 7);
    assert_eq!(e.state.turn_history.current.player(5).cards_drawn, 0);
    assert_eq!(e.state.turn_history.current.player(6).cards_drawn, 0);
    e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
        },
    )
    .expect("mulligan");
    assert_eq!(e.state.turn_history.current.player(5).cards_drawn, 0);
    assert_eq!(e.state.opening.as_ref().unwrap().mulligans_taken[0], 1);
    assert_eq!(
        e.state.opening.as_ref().unwrap().mulligan_actor,
        Some(6),
        "after a mulligan, the other player is offered a decision while they have not kept"
    );
    e.apply_command(
        6,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
        },
    )
    .expect("p6 keep (opponent locked in first)");
    assert!(e.state.opening.as_ref().unwrap().resolved[1]);
    assert_eq!(
        e.state.opening.as_ref().unwrap().mulligan_actor,
        Some(5),
        "once the opponent has kept, the mulliganing player acts again"
    );
    e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
        },
    )
    .expect("keep to bottom");
    let hi = 0u32;
    e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::PutOpeningHandOnBottom(PutOpeningHandOnBottom {
                hand_card_index: hi,
            })),
        },
    )
    .expect("bottom one");
    assert!(e.state.opening.is_none());
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert_eq!(e.state.turn_history.current.player(5).cards_drawn, 0);
    assert_eq!(e.state.turn_history.current.player(6).cards_drawn, 0);
}

#[test]
fn opening_mulligan_to_zero_auto_keeps_and_enters_bottom_phase() {
    use tricerules_proto::ruled::v1::ruled_command::Cmd;
    use tricerules_proto::ruled::v1::{
        ChooseStartingPlayer, MulliganDecision, PutOpeningHandOnBottom, RuledCommand,
    };
    let mut e = GameEngine::new(100, &[5, 6], 20, None, false).expect("new");
    let chooser = e.state.opening.as_ref().unwrap().chooser;
    e.apply_command(
        chooser,
        &RuledCommand {
            cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                starting_player_id: 5,
            })),
        },
    )
    .expect("choose first");

    // P5 (starting player) mulligans first; P6 keeps on their turn; then P5 mulligans 6 more times.
    e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
        },
    )
    .expect("p5 first mulligan");
    e.apply_command(
        6,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
        },
    )
    .expect("p6 keep");
    // P5 mulligans 6 more times (7 total → auto-keep at 0).
    for _ in 0..6 {
        e.apply_command(
            5,
            &RuledCommand {
                cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
            },
        )
        .expect("mulligan");
    }

    // After the 7th mulligan the engine must auto-keep: bottom phase active, no more keep/mulligan.
    let op = e
        .state
        .opening
        .as_ref()
        .expect("opening still active for bottom");
    assert_eq!(op.mulligans_taken[0], 7, "7 mulligans taken");
    assert!(op.bottom.is_some(), "bottom phase must be active");
    assert_eq!(op.bottom.unwrap().1, 7, "must place 7 cards on bottom");
    // mulligan_actor still points to P5 (they are bottoming).
    assert_eq!(op.mulligan_actor, Some(5));

    // P5 places all 7 cards on the bottom one by one.
    for _ in 0..7 {
        e.apply_command(
            5,
            &RuledCommand {
                cmd: Some(Cmd::PutOpeningHandOnBottom(PutOpeningHandOnBottom {
                    hand_card_index: 0,
                })),
            },
        )
        .expect("place on bottom");
    }

    // Opening complete; P5 has 0 cards in hand.
    assert!(e.state.opening.is_none(), "opening should be finished");
    assert_eq!(e.state.players[0].hand.len(), 0);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
}

#[test]
fn opening_mulligan_to_zero_cannot_mulligan_further() {
    use tricerules_proto::ruled::v1::ruled_command::Cmd;
    use tricerules_proto::ruled::v1::{ChooseStartingPlayer, MulliganDecision, RuledCommand};
    let mut e = GameEngine::new(100, &[5, 6], 20, None, false).expect("new");
    let chooser = e.state.opening.as_ref().unwrap().chooser;
    e.apply_command(
        chooser,
        &RuledCommand {
            cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                starting_player_id: 5,
            })),
        },
    )
    .expect("choose first");

    // P5 mulligans first, then P6 keeps, then P5 mulligans 6 more (7 total → auto-keep).
    e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
        },
    )
    .expect("p5 first mulligan");
    e.apply_command(
        6,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
        },
    )
    .expect("p6 keep");
    for _ in 0..6 {
        e.apply_command(
            5,
            &RuledCommand {
                cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
            },
        )
        .expect("mulligan");
    }

    // An 8th Mulligan { keep: false } must be rejected (bottom phase is active, not mulligan phase).
    let err = e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
        },
    );
    assert!(
        err.is_err(),
        "must reject further mulligan when bottom phase is active"
    );
}

// ── Concede availability ───────────────────────────────────────────────────────

#[test]
fn concede_is_legal_during_opening_sequence() {
    use prost::Message;

    // CR 104.3a: a player may concede at any time. Regression: during the choose-first / mulligan
    // opening sequence every non-opening command (including Concede) was rejected, so a player
    // could not bail out before the first turn.
    let mut e = GameEngine::new(11, &[0, 1], 20, None, false).expect("new");
    assert!(
        e.state.opening.is_some(),
        "engine is still in the opening/mulligan sequence"
    );
    let command_index_before = e.state.command_index;
    let response = e.player_command_ipc(0, &concede().encode_to_vec());
    assert!(response.ok, "a player may concede during opening");
    assert_eq!(
        e.state.winner,
        Some(1),
        "the opponent wins once the other player concedes"
    );
    assert_eq!(e.state.command_index, command_index_before + 1);

    let batch = response.batch.expect("successful concession batch");
    let logs: Vec<_> = batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::Log(log)) => Some(log.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(logs, ["P0 conceded", "Game over. Winner: 1"]);
    assert!(
        batch.legal_by_player.is_empty(),
        "a terminal batch must clear every legal action"
    );

    let rejected = e.player_command_ipc(1, &pass().encode_to_vec());
    assert!(!rejected.ok, "commands after game over are rejected");
    assert_eq!(rejected.error, "illegal command: game over");
    assert_eq!(
        e.state.command_index,
        command_index_before + 1,
        "a rejected post-game command is not replayed"
    );
}

/// The seat-count gate (`SUPPORTED_PLAYER_COUNT` in `engine/mod.rs`) is the engine's one remaining
/// hard 2-player assumption, kept because `DeclareAttackers` carries no per-attacker defender. It
/// must reject rather than build a game that would then fail somewhere inside combat.
#[test]
fn engine_rejects_any_player_count_but_two() {
    for player_ids in [vec![], vec![5], vec![5, 6, 7], vec![5, 6, 7, 8]] {
        let err = GameEngine::new(1, &player_ids, 20, None, true)
            .err()
            .unwrap_or_else(|| panic!("{} players must be rejected", player_ids.len()));
        assert!(
            format!("{err:?}").contains("exactly 2 players"),
            "unexpected error for {} players: {err:?}",
            player_ids.len()
        );
    }
    assert!(
        GameEngine::new(1, &[5, 6], 20, None, true).is_ok(),
        "two players is still accepted"
    );
}
