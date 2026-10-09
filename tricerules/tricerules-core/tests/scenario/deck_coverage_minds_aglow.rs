//! Actual-card coverage for Minds Aglow's sequential Join Forces payments and shared draw count.
//!
//! The contribution prompt must accept the exact selected mana, including zero, and resume the
//! same resolving spell until every player has contributed. The existing EachPlayer draw
//! instruction owns the final draw order and replacement handling.

use crate::helpers::*;
use prost::Message;
use tricerules_cards::{AbilityPresentation, ChoiceId, ManaAmount, ManaSpendingRestriction};
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, CostObjectRef, ManaSpendSelection, PaymentMana, PaymentSelection,
    ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

const PLAYERS: [i32; 3] = [4, 9, 27];
const MINDS_AGLOW: &str = "minds_aglow";

fn resolution_payment(engine: &GameEngine, mana: PaymentMana) -> SubmitResolutionChoice {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Minds Aglow contribution is pending");
    let source_object_id = pending.presentation.source_object_id;
    SubmitResolutionChoice {
        decision: ResolutionChoiceDecision::PayMana as i32,
        payment: Some(PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(CostObjectRef {
                object_id: source_object_id,
                zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&source_object_id)
                    .copied()
                    .unwrap_or(0),
            }),
            mana: Some(mana),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn contribute(engine: &mut GameEngine, player: i32, mana: PaymentMana) {
    let answer = resolution_payment(engine, mana);
    engine
        .apply_command(
            player,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(answer)),
            },
        )
        .expect("the current player accepts this exact mana contribution");
}

#[test]
fn minds_aglow_collects_sequential_contributions_and_each_player_draws_the_total() {
    let decks = Some(vec![vec!["island".into(); 20]; PLAYERS.len()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_701,
        &PLAYERS,
        20,
        decks,
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);

    let third_player_mountain = inject_permanent_on_battlefield(&mut engine, 2, "mountain");
    inject_card_into_hand(&mut engine, 0, MINDS_AGLOW);
    give_mana(
        &mut engine,
        PLAYERS[0],
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        PLAYERS[1],
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let library_sizes_before = engine
        .state
        .players
        .iter()
        .map(|player| player.library.len())
        .collect::<Vec<_>>();

    let hand_index = hand_index_for_card(&engine, 0, MINDS_AGLOW);
    engine
        .apply_command(PLAYERS[0], &cast_spell(hand_index, vec![]))
        .expect("cast Minds Aglow");
    pass_priority_round(&mut engine);

    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("first contribution prompt")
            .deciding_player,
        PLAYERS[0]
    );
    let command_index_before_wrong_payer = engine.state.command_index;
    let wrong_payer = resolution_payment(
        &engine,
        PaymentMana {
            c: 2,
            ..Default::default()
        },
    );
    assert!(engine
        .apply_command(
            PLAYERS[1],
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(wrong_payer)),
            },
        )
        .is_err());
    assert_eq!(engine.state.command_index, command_index_before_wrong_payer);
    assert_eq!(engine.state.players[1].mana_pool.colorless, 2);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("wrong payer keeps the current prompt")
            .deciding_player,
        PLAYERS[0]
    );

    let command_index_before_bad_payment = engine.state.command_index;
    let excessive_payment = resolution_payment(
        &engine,
        PaymentMana {
            c: 2,
            ..Default::default()
        },
    );
    assert!(engine
        .apply_command(
            PLAYERS[0],
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(excessive_payment)),
            },
        )
        .is_err());
    assert_eq!(engine.state.command_index, command_index_before_bad_payment);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("rejected payment keeps the current prompt")
            .deciding_player,
        PLAYERS[0]
    );
    contribute(
        &mut engine,
        PLAYERS[0],
        PaymentMana {
            c: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("second contribution prompt")
            .deciding_player,
        PLAYERS[1]
    );
    contribute(
        &mut engine,
        PLAYERS[1],
        PaymentMana {
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("zero contribution prompt")
            .deciding_player,
        PLAYERS[2]
    );
    let tap_mountain = activate_ability_for(&engine, third_player_mountain, 0, vec![]);
    engine
        .apply_command(PLAYERS[2], &tap_mountain)
        .expect("the current payer may activate a mana ability during resolution");
    assert_eq!(engine.state.players[2].mana_pool.red, 1);
    assert!(engine.state.objects[&third_player_mountain].tapped);
    assert_eq!(engine.state.undoable_mana_abilities.len(), 1);
    contribute(&mut engine, PLAYERS[2], PaymentMana::default());

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[2].mana_pool.red, 1,
        "accepted zero leaves floated mana available"
    );
    assert!(engine.state.objects[&third_player_mountain].tapped);
    assert!(
        engine.state.undoable_mana_abilities.is_empty(),
        "the accepted zero makes the mana ability consequential"
    );
    for (index, player) in engine.state.players.iter().enumerate() {
        assert_eq!(
            library_sizes_before[index] - player.library.len(),
            3,
            "each player draws the total of one plus two mana"
        );
    }
}

#[test]
fn minds_aglow_contribution_command_log_replays_the_same_batches_and_state() {
    const REPLAY_PLAYERS: [i32; 2] = [4, 9];
    let make_engine = || {
        let decks = Some(vec![vec!["island".into(); 20]; REPLAY_PLAYERS.len()]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            202_610_703,
            &REPLAY_PLAYERS,
            20,
            decks,
            true,
        )
        .expect("new two-player replay game");
        advance_to_main1_from_game_start(&mut engine);
        inject_card_into_hand(&mut engine, 0, MINDS_AGLOW);
        give_mana(
            &mut engine,
            REPLAY_PLAYERS[0],
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        );
        give_mana(
            &mut engine,
            REPLAY_PLAYERS[1],
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        engine
    };

    fn apply_and_log(
        engine: &mut GameEngine,
        log: &mut Vec<(i32, RuledCommand, Vec<u8>)>,
        actor: i32,
        command: RuledCommand,
    ) {
        let batch = engine
            .apply_command(actor, &command)
            .expect("recorded Minds Aglow command is accepted");
        log.push((actor, command, batch.encode_to_vec()));
    }

    let mut engine = make_engine();
    let mut replay = make_engine();
    let mut command_log = Vec::new();
    let cast = cast_spell(hand_index_for_card(&engine, 0, MINDS_AGLOW), vec![]);
    apply_and_log(&mut engine, &mut command_log, REPLAY_PLAYERS[0], cast);
    for _ in 0..REPLAY_PLAYERS.len() {
        let actor = engine.state.priority_player_id();
        apply_and_log(&mut engine, &mut command_log, actor, pass());
    }
    let first_contribution = resolution_payment(
        &engine,
        PaymentMana {
            c: 1,
            ..Default::default()
        },
    );
    apply_and_log(
        &mut engine,
        &mut command_log,
        REPLAY_PLAYERS[0],
        RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(first_contribution)),
        },
    );
    let second_contribution = resolution_payment(
        &engine,
        PaymentMana {
            c: 2,
            ..Default::default()
        },
    );
    apply_and_log(
        &mut engine,
        &mut command_log,
        REPLAY_PLAYERS[1],
        RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(second_contribution)),
        },
    );

    for (actor, command, expected_batch) in command_log {
        let actual_batch = replay
            .apply_command(actor, &command)
            .expect("accepted command replays from the matching seed and pre-state");
        assert_eq!(actual_batch.encode_to_vec(), expected_batch);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap(),
        "the Join Forces continuation and equal draws remain command-log deterministic"
    );
}

#[test]
fn minds_aglow_resumes_when_its_controller_concedes_during_join_forces() {
    const FOUR_PLAYERS: [i32; 4] = [4, 9, 27, 31];
    let decks = Some(vec![vec!["island".into(); 20]; FOUR_PLAYERS.len()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_702,
        &FOUR_PLAYERS,
        20,
        decks,
        true,
    )
    .expect("new four-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, MINDS_AGLOW);
    give_mana(
        &mut engine,
        FOUR_PLAYERS[0],
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        FOUR_PLAYERS[1],
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        FOUR_PLAYERS[2],
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let library_sizes_before = engine
        .state
        .players
        .iter()
        .map(|player| player.library.len())
        .collect::<Vec<_>>();

    let hand_index = hand_index_for_card(&engine, 0, MINDS_AGLOW);
    engine
        .apply_command(FOUR_PLAYERS[0], &cast_spell(hand_index, vec![]))
        .expect("cast Minds Aglow");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("first contribution prompt")
            .deciding_player,
        FOUR_PLAYERS[0]
    );

    engine
        .apply_command(FOUR_PLAYERS[0], &concede())
        .expect("the spell controller may concede during the first contribution prompt");
    assert!(engine.state.players[0].has_lost);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the resolving spell survives its controller's departure")
            .deciding_player,
        FOUR_PLAYERS[1]
    );

    contribute(
        &mut engine,
        FOUR_PLAYERS[1],
        PaymentMana {
            c: 1,
            ..Default::default()
        },
    );
    contribute(
        &mut engine,
        FOUR_PLAYERS[2],
        PaymentMana {
            c: 2,
            ..Default::default()
        },
    );
    contribute(&mut engine, FOUR_PLAYERS[3], PaymentMana::default());

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    for (index, player) in engine.state.players.iter().enumerate().skip(1) {
        assert_eq!(
            library_sizes_before[index] - player.library.len(),
            3,
            "each surviving player draws the total of one plus two mana"
        );
    }
}

#[test]
fn minds_aglow_accepts_only_restricted_mana_eligible_for_resolution_payments() {
    let decks = Some(vec![vec!["island".into(); 20]; PLAYERS.len()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_703,
        &PLAYERS,
        20,
        decks,
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    engine
        .state
        .mana_restrictions
        .push(ManaSpendingRestriction {
            restriction_id: ChoiceId::new("minds_aglow_resolution_only").unwrap(),
            presentation: AbilityPresentation::Fallback,
            unrestricted: false,
            cast_spell: vec![],
            activate_ability: vec![],
            activate_any_ability: false,
            all_nonspell_costs: true,
            special_actions: vec![],
            spending_effects: vec![],
        });
    engine.state.players[0].restricted_mana.push(
        tricerules_core::state::RestrictedManaContribution {
            restriction_group_id: 1,
            amount: ManaAmount {
                c: 1,
                ..Default::default()
            },
        },
    );
    inject_card_into_hand(&mut engine, 0, MINDS_AGLOW);
    give_mana(
        &mut engine,
        PLAYERS[0],
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let library_sizes_before = engine
        .state
        .players
        .iter()
        .map(|player| player.library.len())
        .collect::<Vec<_>>();
    let hand_index = hand_index_for_card(&engine, 0, MINDS_AGLOW);
    engine
        .apply_command(PLAYERS[0], &cast_spell(hand_index, vec![]))
        .expect("cast Minds Aglow");
    pass_priority_round(&mut engine);

    let mut unavailable = resolution_payment(&engine, PaymentMana::default());
    unavailable.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: 2,
        c: 1,
        ..Default::default()
    });
    let command_index_before_rejection = engine.state.command_index;
    assert!(engine
        .apply_command(
            PLAYERS[0],
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(unavailable)),
            },
        )
        .is_err());
    assert_eq!(engine.state.command_index, command_index_before_rejection);
    assert_eq!(engine.state.players[0].restricted_mana[0].amount.c, 1);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("invalid restriction keeps the contribution prompt")
            .deciding_player,
        PLAYERS[0]
    );

    let mut eligible = resolution_payment(&engine, PaymentMana::default());
    eligible.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: 1,
        c: 1,
        ..Default::default()
    });
    engine
        .apply_command(
            PLAYERS[0],
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(eligible)),
            },
        )
        .expect("eligible restricted mana counts toward the contribution");
    assert!(engine.state.players[0].restricted_mana.is_empty());
    contribute(&mut engine, PLAYERS[1], PaymentMana::default());
    contribute(&mut engine, PLAYERS[2], PaymentMana::default());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    for (index, player) in engine.state.players.iter().enumerate() {
        assert_eq!(
            library_sizes_before[index] - player.library.len(),
            1,
            "the eligible restricted contribution is included in the total"
        );
    }
}
