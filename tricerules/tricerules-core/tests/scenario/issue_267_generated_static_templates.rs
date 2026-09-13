//! Issue #267 — the current-Standard static and compact multi-template cohort is wired through
//! existing typed engine primitives.  Oracle and rulings were checked 2026-09-13.  CR 305.2,
//! 400.7, 509.1b, 514.1, 603, 604, 611.3, 613, 614.1d, and 701.18 govern land-play limits,
//! new-object identity, blocking legality, cleanup, trigger staging, static effects, continuous
//! effects, effect layers, entry replacements, and library searches.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{self as rv1, ChoiceKind};

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    engine
        .apply_command(first, &pass())
        .expect("first priority pass");
    let second = engine.state.priority_player_id();
    engine
        .apply_command(second, &pass())
        .expect("second priority pass resolves stack item")
}

fn three_player_main1(seed: u64) -> GameEngine {
    // The constructor remains two-player for the production harness.  This fixture extends the
    // state only to exercise EachOpponent's seat-generic recipient expansion.
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    while engine.state.turn_step != TurnStep::Main1 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("advance three-player turn");
    }
    engine
}

fn resolve_three_player_stack(engine: &mut GameEngine) {
    while !engine.state.stack.is_empty() {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            break;
        }
        let player_count = engine.state.players.len();
        for _ in 0..player_count {
            let player = engine.state.priority_player_id();
            engine
                .apply_command(player, &pass())
                .expect("three-player priority pass");
            if engine.state.stack.is_empty() {
                break;
            }
        }
    }
}

fn play_graveyard_land(object_id: u32, generation: u64) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PlayLand(rv1::PlayLand {
            source: Some(rv1::LandSource {
                location: Some(rv1::land_source::Location::GraveyardObjectId(object_id)),
                expected_zone_change_generation: Some(generation),
            }),
            face_index: 0,
        })),
    }
}

#[test]
fn issue_267_static_conditions_re_evaluate_and_entry_replacement_is_immediate() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "bearer_of_glory",
                "anthem_of_champions",
                "daring_thunder-thief",
            ],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(267_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let bearer = move_ready_to_battlefield(&mut engine, 0, "bearer_of_glory");
    assert!(engine.effective_has_keyword(bearer, Keyword::FirstStrike));

    let friendly = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut engine, 0, "anthem_of_champions");
    assert_eq!(engine.effective_power(friendly), Some(3));
    assert_eq!(engine.effective_toughness(friendly), Some(3));
    assert_eq!(engine.effective_power(opposing), Some(2));

    // Daring Thunder-Thief enters through the normal replacement path, so it is already tapped
    // while any entry-trigger staging would still be pending.
    let daring = move_ready_to_battlefield(&mut engine, 0, "daring_thunder-thief");
    assert!(engine.state.objects[&daring].tapped);

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.active_player_idx, 1);
    assert!(!engine.effective_has_keyword(bearer, Keyword::FirstStrike));
    assert_eq!(engine.effective_power(friendly), Some(3));
}

#[test]
fn issue_267_self_restriction_is_published_and_rejects_a_block() {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("swamp", &["vampire_interloper"]),
    ]);
    let mut engine = GameEngine::new(267_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let interloper = move_ready_to_battlefield(&mut engine, 1, "vampire_interloper");
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, interloper),
        vec!["Can't block"]
    );

    engine
        .apply_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_both_players(&mut engine);
    let legal = &engine.initial_response_batch().legal_by_player[&1];
    assert!(!legal
        .legal_block_pairs
        .iter()
        .any(|pair| pair.blocker_id == interloper));
    assert!(engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: interloper,
            }]),
        )
        .is_err());
}

#[test]
fn issue_267_icetill_grants_two_land_plays_and_mills_after_landfall() {
    let decks = Some(vec![
        deck_with("forest", &["icetill_explorer"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(267_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "icetill_explorer");
    ensure_in_hand(&mut engine, 0, "forest");
    ensure_in_hand(&mut engine, 0, "forest");

    let library_before = engine.state.players[0].library.len();
    let first = hand_index_for_card(&engine, 0, "forest");
    engine
        .apply_command(0, &play_land(first))
        .expect("first land play");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.lands_played_this_turn, 1);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);

    let second = hand_index_for_card(&engine, 0, "forest");
    engine
        .apply_command(0, &play_land(second))
        .expect("second land play from Icetill Explorer");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.lands_played_this_turn, 2);

    let third = hand_index_for_card(&engine, 0, "forest");
    engine
        .apply_command(0, &play_land(third))
        .expect_err("third land exceeds one extra play");
}

#[test]
fn issue_267_icetill_reuses_generation_bound_graveyard_land_permission() {
    let decks = Some(vec![
        deck_with("forest", &["icetill_explorer"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(267_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "icetill_explorer");
    let forest = inject_graveyard_card(&mut engine, 0, "forest");
    let generation = engine
        .state
        .zone_change_generation
        .get(&forest)
        .copied()
        .unwrap_or(0);
    let legal = &engine.initial_response_batch().legal_by_player[&0].zone_land_actions;
    assert!(legal.iter().any(|action| {
        action.object_id == forest && action.zone_change_generation == generation
    }));

    engine
        .apply_command(0, &play_graveyard_land(forest, generation))
        .expect("play a Forest from the graveyard");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert_eq!(engine.state.lands_played_this_turn, 1);
    assert!(engine
        .apply_command(0, &play_graveyard_land(forest, generation))
        .is_err());
}

#[test]
fn issue_267_adventure_search_is_filtered_and_rat_tokens_keep_their_restriction() {
    let decks = Some(vec![
        deck_with("plains", &["the_arkenstone_seek_the_heart"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(267_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let legendary = inject_library_card(&mut engine, 0, "jasmine_boreal");
    let ordinary = inject_library_card(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "the_arkenstone_seek_the_heart");
    grant_pool(&mut engine, 0);
    let arkenstone = hand_index_for_card(&engine, 0, "the_arkenstone_seek_the_heart");
    engine
        .apply_command(0, &cast_spell_face(arkenstone, Vec::new(), 1))
        .expect("cast Seek the Heart Adventure face");
    let search_batch = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&search_batch).expect("legendary creature search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!(choice.candidate_object_ids, vec![legendary]);
    assert!(!choice.candidate_object_ids.contains(&ordinary));
    assert!(choice.public_reveal.is_none());
    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![legendary]))
        .expect("choose the private search candidate");
    assert_eq!(engine.state.objects[&legendary].zone, Zone::Hand);
    assert!(completion.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("reveals Jasmine Boreal"))
    }));
    assert!(completion.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("shuffles their library"))
    }));

    let mut tokens = GameEngine::new(
        267_006,
        &[0, 1],
        20,
        Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("token engine");
    advance_to_main1_from_game_start(&mut tokens);
    inject_card_into_hand(&mut tokens, 0, "ratcatcher_trainee_pest_problem");
    grant_pool(&mut tokens, 0);
    let slot = hand_index_for_card(&tokens, 0, "ratcatcher_trainee_pest_problem");
    tokens
        .apply_command(0, &cast_spell_face(slot, Vec::new(), 1))
        .expect("cast Pest Problem Adventure face");
    let token_batch = resolve_top_stack(&mut tokens);
    let rats = battlefield_token_oids(&tokens, 0, "rat_b_1_1_cant_block");
    assert_eq!(rats.len(), 2);
    assert_eq!(token_created_events(&token_batch).len(), 2);
    for rat in rats {
        assert_eq!(tokens.state.objects[&rat].owner, 0);
        assert_eq!(
            (tokens.effective_power(rat), tokens.effective_toughness(rat)),
            (Some(1), Some(1))
        );
        assert_eq!(
            zone_view_rules_annotation_labels(&mut tokens, 0, rat),
            vec!["Can't block"]
        );
    }
}

#[test]
fn issue_267_warleaders_call_damages_each_opponent_in_multiplayer() {
    let mut engine = three_player_main1(267_007);
    inject_card_into_hand(&mut engine, 0, "warleaders_call");
    inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let warleader = move_ready_to_battlefield(&mut engine, 0, "warleaders_call");
    let creature = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(engine.effective_power(creature), Some(3));
    assert_eq!(engine.state.stack.len(), 1);
    resolve_three_player_stack(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
    assert_eq!(engine.state.objects[&warleader].zone, Zone::Battlefield);
}
