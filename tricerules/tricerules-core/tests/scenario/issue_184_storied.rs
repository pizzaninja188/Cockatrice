use super::helpers::*;
use tricerules_cards::Keyword;

fn advance_to_active_player_upkeep(engine: &mut GameEngine, player: i32) {
    for _ in 0..60 {
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let player_index = engine
                    .state
                    .player_idx(cleanup_player)
                    .expect("cleanup player");
                let excess = engine.state.players[player_index].hand.len() - 7;
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_command(actor, &command)
            .expect("pass toward requested upkeep");
        if engine.state.active_player_id() == player
            && engine.state.turn_step == tricerules_core::TurnStep::Upkeep
        {
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}

#[test]
fn storied_latches_once_and_is_published() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["oin_the_brave", "bottle_gnomes", "bottle_gnomes"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(184_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    move_ready_to_battlefield(&mut engine, 0, "oin_the_brave");
    move_ready_to_battlefield(&mut engine, 0, "bottle_gnomes");
    assert!(!engine.state.players[0].has_enduring_story);

    move_ready_to_battlefield(&mut engine, 0, "bottle_gnomes");
    assert!(engine.state.players[0].has_enduring_story);

    let view = engine.initial_response_batch();
    let player_view = view
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view.per_player.iter().find(|player| player.player_id == 0),
            _ => None,
        })
        .expect("player zone view");
    assert!(player_view.has_enduring_story);

    engine.state.players[0].battlefield.pop();
    assert!(engine.state.players[0].has_enduring_story);
}

#[test]
fn oin_bonus_turns_on_at_the_designation_boundary() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["oin_the_brave", "bottle_gnomes", "bottle_gnomes"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(184_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let oin = move_ready_to_battlefield(&mut engine, 0, "oin_the_brave");
    move_ready_to_battlefield(&mut engine, 0, "bottle_gnomes");
    assert_eq!(engine.characteristics(oin).expect("Óin").power, Some(1));
    assert!(!engine.effective_has_keyword(oin, Keyword::Haste));

    move_ready_to_battlefield(&mut engine, 0, "bottle_gnomes");
    assert_eq!(engine.characteristics(oin).expect("Óin").power, Some(2));
    assert!(engine.effective_has_keyword(oin, Keyword::Haste));
}

#[test]
fn bifur_doubles_its_dwarf_trigger_when_entry_establishes_story() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["bifur,_melodic_rider", "bottle_gnomes", "bottle_gnomes"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(184_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "bottle_gnomes");
    move_ready_to_battlefield(&mut engine, 0, "bottle_gnomes");

    move_ready_to_battlefield(&mut engine, 0, "bifur,_melodic_rider");

    assert!(engine.state.players[0].has_enduring_story);
    assert_eq!(
        engine
            .state
            .pending_trigger_order
            .as_ref()
            .expect("two simultaneous Bifur triggers require ordering")
            .candidates
            .len(),
        2
    );
}

#[test]
fn bombur_stays_tapped_without_a_story_then_untaps_with_one() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["bombur,_gentle_dreamer", "bottle_gnomes", "bottle_gnomes"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(184_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let bombur = move_ready_to_battlefield(&mut engine, 0, "bombur,_gentle_dreamer");
    engine
        .state
        .objects
        .get_mut(&bombur)
        .expect("Bombur")
        .tapped = true;

    advance_to_active_player_upkeep(&mut engine, 0);
    assert!(engine.state.objects[&bombur].tapped);

    move_ready_to_battlefield(&mut engine, 0, "bottle_gnomes");
    move_ready_to_battlefield(&mut engine, 0, "bottle_gnomes");
    assert!(engine.state.players[0].has_enduring_story);
    advance_to_active_player_upkeep(&mut engine, 1);
    advance_to_active_player_upkeep(&mut engine, 0);
    assert!(!engine.state.objects[&bombur].tapped);
}

#[test]
fn thorin_grants_ward_to_controlled_artifacts_after_story_is_gained() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["thorin_oakenshield", "bottle_gnomes", "bottle_gnomes"],
        ),
        deck_with("island", &["unsummon"]),
    ]);
    let mut engine = GameEngine::new(184_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "thorin_oakenshield");
    let target = move_ready_to_battlefield(&mut engine, 0, "bottle_gnomes");
    move_ready_to_battlefield(&mut engine, 0, "bottle_gnomes");
    assert!(engine.state.players[0].has_enduring_story);

    ensure_in_hand(&mut engine, 1, "unsummon");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &pass()).expect("pass priority");
    let unsummon = hand_index_for_card(&engine, 1, "unsummon");
    engine
        .apply_command(1, &cast_spell(unsummon, target_object(target)))
        .expect("target Thorin-granted ward permanent");

    assert_eq!(engine.state.stack.len(), 2, "ward is above Unsummon");
    assert!(
        engine
            .state
            .stack
            .last()
            .expect("ward trigger")
            .is_triggered
    );
}
