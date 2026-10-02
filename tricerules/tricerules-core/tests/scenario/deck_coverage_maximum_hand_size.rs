//! Exact-card scenarios for the Kami maximum-hand-size cohort.

use super::helpers::*;
use tricerules_cards::{primitives::CounterKind, CardRegistry};
use tricerules_core::{GameEngine, TurnStep};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ExecutePermanentAction, PermanentActionKind, RuledCommand,
};

#[test]
fn folio_pays_double_x_draws_every_player_and_mills_each_opponents_own_hand() {
    let decks = Some(vec![
        deck_with("island", &[]),
        deck_with("forest", &[]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(20_260_935, &[0, 1, 2], 20, decks, true).expect("new game");
    engine.state.turn_step = TurnStep::Main1;
    let folio = inject_permanent_on_battlefield(&mut engine, 0, "folio_of_fancies");
    let before = [0, 1, 2].map(|player| engine.state.players[player].hand.len());
    engine.state.players[0].mana_pool.colorless = 4;
    let mut draw = activate_ability_for(&engine, folio, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = draw.cmd.as_mut() else {
        unreachable!()
    };
    ability.x_value = 2;
    engine
        .apply_command(0, &draw)
        .expect("pay X twice and tap Folio");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    for _ in 0..3 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("resolve shared draw");
    }
    for (player, old_hand_size) in before.iter().enumerate() {
        assert_eq!(engine.state.players[player].hand.len(), *old_hand_size + 2);
    }
    assert!(
        engine
            .apply_command(0, &activate_ability_for(&engine, folio, 1, vec![]))
            .is_err(),
        "the second tap ability needs an intervening untap"
    );

    engine.state.objects.get_mut(&folio).unwrap().tapped = false;
    inject_card_into_hand(&mut engine, 2, "plains");
    let hand_sizes = [1, 2].map(|player| engine.state.players[player].hand.len());
    let libraries = [0, 1, 2].map(|player| engine.state.players[player].library.len());
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    apply_ability(&mut engine, 0, folio, 1, vec![]).expect("activate Folio mill");
    for _ in 0..3 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("resolve Folio mill");
    }
    assert_eq!(engine.state.players[0].library.len(), libraries[0]);
    assert_eq!(
        engine.state.players[1].library.len(),
        libraries[1] - hand_sizes[0]
    );
    assert_eq!(
        engine.state.players[2].library.len(),
        libraries[2] - hand_sizes[1]
    );
}

#[test]
fn folio_draws_full_x_then_an_opponent_with_too_few_library_cards_loses() {
    let decks = Some(vec![
        deck_with("island", &[]),
        deck_with("forest", &[]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(20_260_956, &[0, 1, 2], 20, decks, true).expect("new game");
    engine.state.turn_step = TurnStep::Main1;
    let folio = inject_permanent_on_battlefield(&mut engine, 0, "folio_of_fancies");
    engine.state.players[1].library.clear();
    inject_library_card(&mut engine, 1, "forest");
    engine.state.players[0].mana_pool.colorless = 4;
    let mut draw = activate_ability_for(&engine, folio, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = draw.cmd.as_mut() else {
        unreachable!()
    };
    ability.x_value = 2;
    engine
        .apply_command(0, &draw)
        .expect("activate Folio at X = 2");
    for _ in 0..3 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("resolve Folio draw");
    }
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.players[0].has_lost);
    assert!(!engine.state.players[2].has_lost);
}

#[test]
fn folio_and_toad_apply_the_later_hand_limit_only_during_cleanup() {
    for folio_enters_last in [false, true] {
        let decks = Some(vec![
            deck_with("island", &["folio_of_fancies", "twenty-toed_toad"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(
            20_260_936 + folio_enters_last as u64,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new game");
        advance_to_main1_from_game_start(&mut engine);
        let (first, second) = if folio_enters_last {
            ("twenty-toed_toad", "folio_of_fancies")
        } else {
            ("folio_of_fancies", "twenty-toed_toad")
        };
        move_ready_to_battlefield(&mut engine, 0, first);
        move_ready_to_battlefield(&mut engine, 0, second);
        while engine.state.players[0].hand.len() < 21 {
            inject_card_into_hand(&mut engine, 0, "island");
        }
        assert_eq!(engine.state.cleanup_discard_player, None);
        assert_eq!(engine.state.players[0].hand.len(), 21);
        engine.state.turn_step = TurnStep::EndStep;
        engine
            .apply_command(0, &pass())
            .expect("active player passes end step");
        engine
            .apply_command(1, &pass())
            .expect("opponent passes end step");
        if folio_enters_last {
            assert_eq!(engine.state.cleanup_discard_player, None);
            assert_eq!(engine.state.players[0].hand.len(), 21);
        } else {
            assert_eq!(engine.state.cleanup_discard_player, Some(0));
            engine
                .apply_command(0, &discard_cleanup_batch(vec![0]))
                .expect("discard one card to the later twenty-card limit");
            assert_eq!(engine.state.players[0].hand.len(), 20);
        }
    }
}

#[test]
fn turning_toad_face_up_gives_its_hand_limit_a_new_timestamp() {
    let decks = Some(vec![
        deck_with("island", &["twenty-toed_toad", "folio_of_fancies"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(20_260_957, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let toad = move_ready_to_battlefield(&mut engine, 0, "twenty-toed_toad");
    engine.state.objects.get_mut(&toad).unwrap().face_down = true;
    move_ready_to_battlefield(&mut engine, 0, "folio_of_fancies");
    while engine.state.players[0].hand.len() < 21 {
        inject_card_into_hand(&mut engine, 0, "island");
    }
    engine.state.players[0].mana_pool.colorless = 3;
    engine.state.players[0].mana_pool.blue = 1;
    let generation = engine
        .state
        .zone_change_generation
        .get(&toad)
        .copied()
        .unwrap_or(0);
    execute_permanent_action_with_payment(
        &mut engine,
        0,
        RuledCommand {
            cmd: Some(Cmd::ExecutePermanentAction(ExecutePermanentAction {
                kind: PermanentActionKind::TurnFaceUp as i32,
                object_id: toad,
                expected_zone_change_generation: generation,
                ..Default::default()
            })),
        },
    )
    .expect("turn Toad face up with its printed mana cost");
    assert!(!engine.state.objects[&toad].face_down);
    engine.state.turn_step = TurnStep::EndStep;
    engine
        .apply_command(0, &pass())
        .expect("active player passes end step");
    engine
        .apply_command(1, &pass())
        .expect("opponent passes end step");
    assert_eq!(engine.state.cleanup_discard_player, Some(0));
}

#[test]
fn triskaidekaphile_checks_thirteen_at_upkeep_start_and_again_on_resolution() {
    for starting_hand in [12, 13] {
        let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
        let mut engine = GameEngine::new(20_260_938 + starting_hand, &[0, 1], 20, decks, true)
            .expect("new game");
        advance_to_main1_from_game_start(&mut engine);
        inject_permanent_on_battlefield(&mut engine, 0, "triskaidekaphile");
        while engine.state.players[0].hand.len() < starting_hand as usize {
            inject_card_into_hand(&mut engine, 0, "island");
        }
        engine.state.active_player_idx = 1;
        engine.state.priority_idx = 1;
        engine.state.turn_step = TurnStep::EndStep;
        engine
            .apply_command(1, &pass())
            .expect("opponent ends turn");
        engine.apply_command(0, &pass()).expect("controller passes");
        assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
        assert_eq!(engine.state.stack.len(), usize::from(starting_hand == 13));
        if starting_hand == 13 {
            inject_card_into_hand(&mut engine, 0, "island");
            resolve_entire_stack_two_player(&mut engine);
            assert_eq!(
                engine.state.winner(),
                None,
                "intervening if rechecks at resolution"
            );
        }
    }

    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(20_260_952, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let triska = inject_permanent_on_battlefield(&mut engine, 0, "triskaidekaphile");
    engine.state.players[0].mana_pool.colorless = 3;
    engine.state.players[0].mana_pool.blue = 1;
    let before = engine.state.players[0].hand.len();
    apply_ability(&mut engine, 0, triska, 0, vec![]).expect("activate draw ability");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
}

#[test]
fn toad_two_attacker_trigger_adds_counter_and_draws_without_winning() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(20_260_953, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut engine);
    let bear = *engine.state.players[0]
        .battlefield
        .iter()
        .find(|oid| engine.state.objects.get(oid).unwrap().card_id == "grizzly_bears")
        .expect("eligible companion attacker");
    let toad = inject_permanent_on_battlefield(&mut engine, 0, "twenty-toed_toad");
    let before = engine.state.players[0].hand.len();
    engine
        .apply_command(0, &declare_attackers(vec![toad, bear]))
        .expect("attack with Toad and companion");
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 2);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&toad].counters[&CounterKind::PlusOnePlusOne],
        1
    );
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
    assert_eq!(engine.state.winner(), None);
}

#[test]
fn toad_self_attack_win_uses_all_counter_kinds_or_resolution_time_hand_count() {
    for win_by_hand in [false, true] {
        let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
        let mut engine = GameEngine::new(20_260_954 + win_by_hand as u64, &[0, 1], 20, decks, true)
            .expect("new game");
        advance_to_declare_attackers(&mut engine);
        let toad = inject_permanent_on_battlefield(&mut engine, 0, "twenty-toed_toad");
        if win_by_hand {
            while engine.state.players[0].hand.len() < 19 {
                inject_card_into_hand(&mut engine, 0, "island");
            }
        } else {
            let object = engine.state.objects.get_mut(&toad).unwrap();
            object.add_counters(CounterKind::PlusOnePlusOne, 12, engine.state.command_index);
            object.add_counters(CounterKind::Charge, 8, engine.state.command_index);
        }
        engine
            .apply_command(0, &declare_attackers(vec![toad]))
            .expect("attack with Toad alone");
        assert_eq!(
            engine.state.stack.len(),
            1,
            "self-attack trigger always occurs"
        );
        if win_by_hand {
            inject_card_into_hand(&mut engine, 0, "island");
        }
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.winner(), Some(0));
    }
}

#[test]
fn maximum_hand_cohort_registry_preserves_exact_faces_and_clauses() {
    for (card_id, face_id, static_count, trigger_count, activated_count) in [
        ("folio_of_fancies", "folio_of_fancies", 1, 0, 2),
        ("triskaidekaphile", "triskaidekaphile", 1, 1, 1),
        ("twenty-toed_toad", "twenty_toed_toad", 1, 2, 0),
    ] {
        let card = CardRegistry::global()
            .get(card_id)
            .expect("exact card registered");
        let face = card.primary_face();
        assert_eq!(face.face_id.as_str(), face_id);
        assert_eq!(face.static_abilities.len(), static_count);
        assert_eq!(face.triggered_abilities.len(), trigger_count);
        assert_eq!(face.activated_abilities.len(), activated_count);
    }
}
