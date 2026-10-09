//! Exact deck-corpus coverage for Zur's Weirding's global draw replacement.

use super::helpers::*;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn engine_with_zur(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["zurs_weirding"]),
        deck_with("island", &["divination"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_battlefield(&mut engine, 0, "zurs_weirding", false);
    engine
}

fn pass_until_resolution_choice(
    engine: &mut GameEngine,
) -> (
    rv1::ResolutionChoiceRequired,
    tricerules_proto::ruled::v1::RuledEventBatch,
) {
    for _ in 0..16 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("pass until the resolving draw asks for input");
        if let Some(choice) = find_resolution_choice(&batch) {
            return (choice, batch);
        }
    }
    panic!("no Zur's Weirding payer choice");
}

fn revealed_object_id(batch: &tricerules_proto::ruled::v1::RuledEventBatch) -> u32 {
    batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => reveal.cards.first().map(|card| card.object_id),
            _ => None,
        })
        .expect("separate public CardsRevealed event")
}

fn advance_to_opponent_main1_with_draw_declined(engine: &mut GameEngine) {
    end_active_turn(engine, 0);
    for _ in 0..40 {
        if engine.state.turn_step == TurnStep::Main1 && engine.state.active_player_id() == 1 {
            return;
        }
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            assert_eq!(pending.deciding_player, 0);
            assert_eq!(
                pending.presentation.choice_kind,
                rv1::ChoiceKind::ResolutionBranch
            );
            engine
                .apply_command(0, &select_branch(1))
                .expect("decline the normal draw's Zur payment");
            continue;
        }
        let actor = engine.state.priority_player_id();
        engine
            .apply_command(actor, &pass())
            .expect("pass through the opponent's turn start");
    }
    panic!("opponent did not reach main phase");
}

#[test]
fn zurs_weirding_replaces_each_opponent_draw_in_divination() {
    let mut engine = engine_with_zur(20_261_008);
    let zone_view = engine
        .initial_response_batch()
        .events
        .into_iter()
        .find_map(|event| match event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("authoritative zone view");
    for player_id in [0, 1] {
        let player_view = zone_view
            .per_player
            .iter()
            .find(|view| view.player_id == player_id)
            .expect("player view");
        let public_hand = player_view
            .public_hand
            .as_ref()
            .expect("active Zur's Weirding reveals every hand");
        let player = &engine.state.players[engine.state.player_idx(player_id).unwrap()];
        assert_eq!(public_hand.cards.len(), player.hand.len());
        for (card, oid) in public_hand.cards.iter().zip(&player.hand) {
            assert_eq!(card.object_id, *oid);
            assert_eq!(card.card_id, engine.state.objects[oid].card_id);
            assert!(!card.card_name.is_empty());
        }
    }

    advance_to_opponent_main1_with_draw_declined(&mut engine);
    ensure_in_hand(&mut engine, 1, "divination");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let hand_before = engine.state.players[1].hand.len();
    let first_draw = engine.state.players[1].library[0];
    let second_draw = engine.state.players[1].library[1];
    let third_draw = engine.state.players[1].library[2];
    let divination = hand_index_for_card(&engine, 1, "divination");
    engine
        .apply_command(1, &cast_spell(divination, Vec::new()))
        .expect("cast Divination");

    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("caster passes");
    let first_resolution = engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("Divination resolves into a Zur choice");
    let first_choice = find_resolution_choice(&first_resolution).expect("first payer choice");
    assert_eq!(first_choice.deciding_player_id, 0);
    assert!(first_choice.public_reveal.is_none());
    assert_eq!(revealed_object_id(&first_resolution), first_draw);
    assert_eq!(first_choice.resolution_branches.len(), 2);
    assert!(first_choice.resolution_branches[0].selectable);
    assert_eq!(first_choice.resolution_branches[1].label, "Decline");

    let second_resolution = engine
        .apply_command(0, &select_branch(0))
        .expect("pay to put the first revealed card in its owner's graveyard");
    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.objects[&first_draw].zone, Zone::Graveyard);
    assert!(engine.state.players[1].graveyard.contains(&first_draw));
    assert_eq!(engine.state.players[1].hand.len(), hand_before - 1);
    let second_choice =
        find_resolution_choice(&second_resolution).expect("second draw gets its own choice");
    assert_eq!(second_choice.deciding_player_id, 0);
    assert!(second_choice.public_reveal.is_none());
    assert_eq!(revealed_object_id(&second_resolution), second_draw);

    let final_batch = engine
        .apply_command(0, &select_branch(1))
        .expect("decline the second draw's payment");
    assert_eq!(engine.state.objects[&second_draw].zone, Zone::Hand);
    assert!(engine.state.players[1].hand.contains(&second_draw));
    assert_eq!(engine.state.players[1].hand.len(), hand_before);
    assert_eq!(engine.state.players[1].library.front(), Some(&third_draw));
    assert!(engine.state.pending_resolution.is_none());
    assert!(!final_batch
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::ResolutionChoiceRequired(_)))));
}

#[test]
fn zurs_weirding_collects_four_player_apnap_choices_before_debiting_life() {
    let decks = Some(vec![
        deck_with("island", &["zurs_weirding", "divination"]),
        deck_with("island", &[]),
        deck_with("island", &[]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_261_009,
        &[0, 1, 2, 3],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_battlefield(&mut engine, 0, "zurs_weirding", false);
    ensure_in_hand(&mut engine, 0, "divination");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let hand_before_cast = engine.state.players[0].hand.len();
    let first_draw = engine.state.players[0].library[0];
    let second_draw = engine.state.players[0].library[1];
    let third_draw = engine.state.players[0].library[2];
    let divination = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(divination, Vec::new()))
        .expect("cast Divination");

    let (first_choice, first_batch) = pass_until_resolution_choice(&mut engine);
    assert_eq!(first_choice.deciding_player_id, 1);
    assert!(first_choice.public_reveal.is_none());
    assert_eq!(revealed_object_id(&first_batch), first_draw);
    let p1_pays = engine
        .apply_command(1, &select_branch(0))
        .expect("first APNAP payer chooses to pay");
    assert!(p1_pays.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text.contains("P1 chooses to pay")
    )));
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>(),
        vec![20, 20, 20, 20],
        "the first intent does not debit life before every payer answers"
    );
    let second_payer = find_resolution_choice(&p1_pays).expect("second payer choice");
    assert_eq!(second_payer.deciding_player_id, 2);

    let p2_pays = engine
        .apply_command(2, &select_branch(0))
        .expect("second APNAP payer chooses to pay");
    assert!(p2_pays.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text.contains("P2 chooses to pay")
    )));
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>(),
        vec![20, 20, 20, 20]
    );
    assert_eq!(
        find_resolution_choice(&p2_pays).unwrap().deciding_player_id,
        3
    );

    let p3_declines = engine
        .apply_command(3, &select_branch(1))
        .expect("last APNAP payer declines");
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>(),
        vec![20, 18, 18, 20],
        "both accepted payments apply together after the final choice"
    );
    assert_eq!(engine.state.objects[&first_draw].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&first_draw));
    assert_eq!(engine.state.players[0].hand.len(), hand_before_cast - 1);
    let second_draw_choice = find_resolution_choice(&p3_declines).expect("second Divination draw");
    assert_eq!(second_draw_choice.deciding_player_id, 1);
    assert!(second_draw_choice.public_reveal.is_none());
    assert_eq!(revealed_object_id(&p3_declines), second_draw);

    let mut final_batch = None;
    for payer in [1, 2, 3] {
        let batch = engine
            .apply_command(payer, &select_branch(1))
            .expect("decline payment for the second individual draw");
        if payer < 3 {
            assert_eq!(
                find_resolution_choice(&batch).unwrap().deciding_player_id,
                payer + 1
            );
        } else {
            final_batch = Some(batch);
        }
    }
    assert_eq!(engine.state.objects[&second_draw].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&second_draw));
    assert_eq!(engine.state.players[0].hand.len(), hand_before_cast);
    assert_eq!(engine.state.players[0].library.front(), Some(&third_draw));
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>(),
        vec![20, 18, 18, 20]
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(!final_batch
        .unwrap()
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::ResolutionChoiceRequired(_)))));
}

#[test]
fn song_of_the_dryads_removes_zurs_weirding_public_hand_and_draw_replacement() {
    let decks = Some(vec![
        deck_with("island", &["zurs_weirding", "song_of_the_dryads"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_261_010,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let zur = relocate_to_battlefield(&mut engine, 0, "zurs_weirding", false);
    let before = engine.initial_response_batch();
    let player_view = before
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("initial zone view");
    assert!(player_view
        .per_player
        .iter()
        .all(|view| view.public_hand.is_some()));

    ensure_in_hand(&mut engine, 0, "song_of_the_dryads");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let song_slot = hand_index_for_card(&engine, 0, "song_of_the_dryads");
    engine
        .apply_command(0, &cast_spell(song_slot, target_object(zur)))
        .expect("cast Song of the Dryads on Zur's Weirding");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine
        .apply_command(1, &pass())
        .expect("Song resolves onto Zur's Weirding");
    let song = battlefield_object_for_card(&engine, 0, "song_of_the_dryads");
    assert_eq!(
        engine.state.objects[&song].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(zur))
    );
    assert!(engine
        .characteristics(zur)
        .expect("enchanted permanent characteristics")
        .has_type("Land"));

    let after_song = engine.initial_response_batch();
    let player_view = after_song
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("zone view after Song resolves");
    assert!(player_view
        .per_player
        .iter()
        .all(|view| view.public_hand.is_none()));

    let expected_draw = engine.state.players[engine.state.player_idx(1).unwrap()].library[0];
    let hand_before = engine.state.players[engine.state.player_idx(1).unwrap()]
        .hand
        .len();
    end_active_turn(&mut engine, 0);
    for _ in 0..24 {
        if engine.state.turn_step == TurnStep::Main1 && engine.state.active_player_id() == 1 {
            break;
        }
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("advance through the opponent's draw step");
        assert!(find_resolution_choice(&batch).is_none());
    }
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.objects[&expected_draw].zone, Zone::Hand);
    assert!(engine.state.players[engine.state.player_idx(1).unwrap()]
        .hand
        .contains(&expected_draw));
    assert_eq!(
        engine.state.players[engine.state.player_idx(1).unwrap()]
            .hand
            .len(),
        hand_before + 1,
        "the normal draw is not replaced by Zur's Weirding"
    );
}
