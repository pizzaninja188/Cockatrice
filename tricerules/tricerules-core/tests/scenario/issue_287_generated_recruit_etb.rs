//! Issue #287: generated ETB Recruit (CR 701.70) draws, takes a private mandatory discard, and
//! creates exactly one 1/1 white Human Soldier token only when the authoritative discard result
//! contains a nonland card. The implementation reuses the typed `DrawDiscard` instruction plus the
//! result-carrying `CardResultCount` resolution branch; these scenarios prove the observable
//! behavior, the edge cases, the CR 701.9c unrevealed hidden-zone replacement, and the
//! choosing-player boundaries.

use crate::helpers::*;
use tricerules_core::Zone;

const RECRUIT_SOLDIER: &str = "human_soldier_w_1_1";

/// Park one ETB Recruit at its private discard choice. Returns the engine and the id of the known
/// nonland card that was moved into the recruiting player's hand.
fn park_recruit_choice(card_id: &str, seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("plains", &[card_id, "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("recruit engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    let discard = hand_object_for_card(&engine, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut engine, 0, card_id);
    pass_both_players(&mut engine);
    (engine, discard)
}

fn hand_object_for_card(engine: &GameEngine, player: usize, card_id: &str) -> u32 {
    let index = hand_index_for_card(engine, player, card_id);
    engine.state.players[player].hand[index]
}

#[test]
fn issue_287_nonland_discard_creates_exactly_one_recruit_soldier() {
    for (card_id, seed) in [
        ("long_lake_nuisance", 287_001),
        ("patient_instructor", 287_002),
    ] {
        let (mut engine, nonland) = park_recruit_choice(card_id, seed);
        let choice = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Recruit private discard choice");
        assert_eq!(choice.deciding_player, 0);
        assert_eq!((choice.presentation.min, choice.presentation.max), (1, 1));
        assert!(
            choice.presentation.candidates.contains(&nonland),
            "{card_id}: the known nonland card is a legal discard candidate"
        );

        engine
            .apply_command(0, &submit_resolution_choice(vec![nonland]))
            .expect("discard the nonland card");
        assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            battlefield_token_oids(&engine, 0, RECRUIT_SOLDIER).len(),
            1,
            "{card_id}: a nonland discard creates exactly one Human Soldier"
        );
        assert!(
            battlefield_token_oids(&engine, 1, RECRUIT_SOLDIER).is_empty(),
            "{card_id}: the token is controlled by the recruiting player only"
        );
    }
}

#[test]
fn issue_287_land_discard_creates_no_recruit_soldier() {
    for (card_id, seed) in [
        ("long_lake_nuisance", 287_003),
        ("patient_instructor", 287_004),
    ] {
        let decks = Some(vec![
            deck_with("plains", &[card_id, "grizzly_bears"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("recruit engine");
        advance_to_main1_from_game_start(&mut engine);
        ensure_in_hand(&mut engine, 0, "plains");
        let land = hand_object_for_card(&engine, 0, "plains");
        move_ready_to_battlefield(&mut engine, 0, card_id);
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &submit_resolution_choice(vec![land]))
            .expect("discard the land");
        assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
        assert!(
            battlefield_token_oids(&engine, 0, RECRUIT_SOLDIER).is_empty(),
            "{card_id}: a discarded land creates no Human Soldier"
        );
    }
}

#[test]
fn issue_287_choice_is_private_and_scoped_to_the_recruiting_player() {
    let (mut engine, nonland) = park_recruit_choice("long_lake_nuisance", 287_005);
    let foreign = inject_card_into_hand(&mut engine, 1, "grizzly_bears");

    let opponent_attempt = engine.apply_command(1, &submit_resolution_choice(vec![nonland]));
    assert!(
        matches!(
            opponent_attempt,
            Err(tricerules_core::EngineError::Illegal(_))
        ),
        "only the recruiting player may answer the private discard choice"
    );
    let empty_attempt = engine.apply_command(0, &submit_resolution_choice(vec![]));
    assert!(
        matches!(empty_attempt, Err(tricerules_core::EngineError::Illegal(_))),
        "the mandatory discard requires exactly one card"
    );
    let foreign_attempt = engine.apply_command(0, &submit_resolution_choice(vec![foreign]));
    assert!(
        matches!(
            foreign_attempt,
            Err(tricerules_core::EngineError::Illegal(_))
        ),
        "a card outside the candidate cohort is rejected"
    );

    let generation = engine
        .state
        .zone_change_generation
        .get(&nonland)
        .copied()
        .unwrap_or(0);
    *engine
        .state
        .zone_change_generation
        .entry(nonland)
        .or_default() += 1;
    let stale_attempt = engine.apply_command(0, &submit_resolution_choice(vec![nonland]));
    assert!(
        matches!(stale_attempt, Err(tricerules_core::EngineError::Illegal(_))),
        "a stale object generation cannot satisfy the discard choice"
    );
    *engine
        .state
        .zone_change_generation
        .entry(nonland)
        .or_default() = generation;

    engine
        .apply_command(0, &submit_resolution_choice(vec![nonland]))
        .expect("the recruiting player completes the discard");
    assert_eq!(battlefield_token_oids(&engine, 0, RECRUIT_SOLDIER).len(), 1);
}

#[test]
fn issue_287_empty_library_and_hand_discards_nothing_and_creates_no_soldier() {
    let decks = Some(vec![
        deck_with("plains", &["long_lake_nuisance"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(287_006, &[0, 1], 20, decks, true).expect("recruit engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "long_lake_nuisance");
    while let Some(oid) = engine.state.players[0].hand.pop() {
        engine
            .state
            .objects
            .get_mut(&oid)
            .expect("hand object")
            .zone = Zone::Graveyard;
    }
    engine.state.players[0].library.clear();

    pass_both_players(&mut engine);
    assert!(
        engine.state.pending_resolution.is_none(),
        "an empty hand parks no discard choice"
    );
    assert!(
        battlefield_token_oids(&engine, 0, RECRUIT_SOLDIER).is_empty(),
        "no card was actually discarded, so Recruit creates no token"
    );
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[0].pending_library_loss);
    assert_eq!(engine.state.winner, Some(1));
}

#[test]
fn issue_287_library_of_leng_replacement_observes_the_committed_discard_destination() {
    use tricerules_proto::ruled::v1::ruled_event::Ev;

    // CR 701.9c: when a discard is replaced into a hidden zone without being revealed, all
    // characteristic values of the discarded card are undefined. The replacement destination is
    // chosen through the resumable private-replacement continuation, and the result the Recruit
    // branch reads must be the committed destination, not the proposed discard.
    fn park_replaced_recruit(seed: u64) -> (GameEngine, u32) {
        let decks = Some(vec![
            deck_with("plains", &["long_lake_nuisance", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("recruit engine");
        advance_to_main1_from_game_start(&mut engine);
        inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
        ensure_in_hand(&mut engine, 0, "grizzly_bears");
        let nonland = hand_object_for_card(&engine, 0, "grizzly_bears");
        move_ready_to_battlefield(&mut engine, 0, "long_lake_nuisance");
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &submit_resolution_choice(vec![nonland]))
            .expect("choose the nonland card for the replaced discard");
        let destination = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Library of Leng replacement destination choice");
        assert_eq!(destination.deciding_player, 0);
        assert_eq!(
            destination.presentation.choice_kind,
            ChoiceKind::PrivateReplacement,
            "the replacement destination is a private choice for the recruiting player"
        );
        (engine, nonland)
    }

    // 0 = discard to the graveyard (no replacement), 1 = discard to the top of the library.
    for (destination_index, expect_token, expect_named_log) in
        [(0_u32, true, true), (1, false, false)]
    {
        let (mut engine, nonland) = park_replaced_recruit(287_008 + u64::from(destination_index));
        let resolved = engine
            .apply_command(0, &submit_resolution_choice(vec![destination_index]))
            .expect("commit the replacement destination");
        assert!(engine.state.pending_resolution.is_none());

        if expect_token {
            assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
            assert_eq!(
                battlefield_token_oids(&engine, 0, RECRUIT_SOLDIER).len(),
                1,
                "a graveyard discard is a real discard, so Recruit creates the Soldier"
            );
        } else {
            assert_eq!(engine.state.objects[&nonland].zone, Zone::Library);
            assert_eq!(engine.state.players[0].library.front(), Some(&nonland));
            assert!(
                battlefield_token_oids(&engine, 0, RECRUIT_SOLDIER).is_empty(),
                "CR 701.9c leaves the unrevealed library discard's characteristics undefined, \
                 so the nonland result filter cannot match"
            );
        }

        let logs = resolved
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(Ev::Log(log)) => Some(log.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if expect_named_log {
            assert!(
                logs.contains(&"P0 discards Grizzly Bears."),
                "the public graveyard discard names the card: {logs:?}"
            );
        } else {
            assert!(
                logs.contains(&"P0 discards a card to the top of their library."),
                "the unrevealed discard must state the destination without naming the card: {logs:?}"
            );
            assert!(
                logs.iter().all(|text| !text.contains("Grizzly Bears")),
                "the unrevealed discard must not publish the card's identity in log text: {logs:?}"
            );
            assert!(
                resolved
                    .events
                    .iter()
                    .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))),
                "an unrevealed library discard must not be published as a reveal"
            );
        }
    }
}

#[test]
fn issue_287_choice_and_conditional_token_replay_deterministically() {
    fn setup() -> GameEngine {
        let decks = Some(vec![
            deck_with("plains", &["long_lake_nuisance", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]);
        let mut engine =
            GameEngine::new(287_007, &[0, 1], 20, decks, true).expect("recruit engine");
        advance_to_main1_from_game_start(&mut engine);
        engine
    }

    fn drive(mut engine: GameEngine, mut replay: Option<&mut GameEngine>) -> GameEngine {
        ensure_in_hand(&mut engine, 0, "grizzly_bears");
        if let Some(replay) = replay.as_deref_mut() {
            ensure_in_hand(replay, 0, "grizzly_bears");
        }
        let permanent = move_ready_to_battlefield(&mut engine, 0, "long_lake_nuisance");
        if let Some(replay) = replay.as_deref_mut() {
            assert_eq!(
                move_ready_to_battlefield(replay, 0, "long_lake_nuisance"),
                permanent
            );
        }
        for _ in 0..4 {
            let holder = engine.state.priority_player_id();
            let batch = engine.apply_command(holder, &pass()).expect("pass");
            if let Some(replay) = replay.as_deref_mut() {
                assert_eq!(holder, replay.state.priority_player_id());
                assert_eq!(
                    replay.apply_command(holder, &pass()).expect("replay pass"),
                    batch,
                    "the same seed and command sequence must produce the same batch"
                );
            }
            if find_resolution_choice(&batch).is_some() {
                return engine;
            }
        }
        panic!("the Recruit discard choice never parked");
    }

    let mut replay = setup();
    let mut engine = drive(setup(), Some(&mut replay));
    let nonland = hand_object_for_card(&engine, 0, "grizzly_bears");
    assert_eq!(nonland, hand_object_for_card(&replay, 0, "grizzly_bears"));
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        format!("{:?}", replay.state.pending_resolution)
    );

    let command = submit_resolution_choice(vec![nonland]);
    let batch = engine.apply_command(0, &command).expect("player discards");
    assert_eq!(
        replay.apply_command(0, &command).expect("replay discards"),
        batch
    );
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&engine, 0, RECRUIT_SOLDIER).len(), 1);
    assert_eq!(
        replay.state.objects[&nonland].zone,
        Zone::Graveyard,
        "the replay reaches the same committed discard"
    );
    assert_eq!(battlefield_token_oids(&replay, 0, RECRUIT_SOLDIER).len(), 1);
    assert_eq!(
        engine.state.players[0].hand.len(),
        replay.state.players[0].hand.len()
    );
}
