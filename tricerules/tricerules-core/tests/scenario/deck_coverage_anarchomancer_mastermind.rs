//! Exact-card cost reductions and opponent draw ordinals.
use super::helpers::*;
use tricerules_cards::{AbilityPresentation, CardRegistry, Keyword, Layout};
use tricerules_core::TurnStep;

fn pass_round(engine: &mut GameEngine) {
    for _ in 0..engine.state.players.len() {
        answer_trigger_order_in_engine_order(engine);
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
}

#[test]
fn exact_characteristics_and_presentation() {
    let registry = CardRegistry::global();
    let goblin = registry
        .get("goblin_anarchomancer")
        .expect("registered Goblin");
    assert_eq!(goblin.name, "Goblin Anarchomancer");
    assert_eq!(goblin.layout, Layout::Normal);
    assert_eq!(goblin.face_count(), 1);
    let face = goblin.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{R}{G}");
    assert_eq!(face.types, ["Creature", "Goblin", "Shaman"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let faerie = registry
        .get("faerie_mastermind")
        .expect("registered Faerie");
    assert_eq!(faerie.name, "Faerie Mastermind");
    assert_eq!(faerie.layout, Layout::Normal);
    assert_eq!(faerie.face_count(), 1);
    let face = faerie.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Creature", "Faerie", "Rogue"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    assert_eq!(face.keywords, [Keyword::Flash, Keyword::Flying]);
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(
        face.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        face.activated_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![4])
    );
}

fn reduction(engine: &mut GameEngine, player: usize, card: &str) -> u32 {
    let index = hand_index_for_card(engine, player, card) as u32;
    engine.initial_response_batch().legal_by_player[&(player as i32)]
        .hand_actions
        .iter()
        .find(|a| a.hand_index == index)
        .unwrap()
        .generic_cost_reduction
}

#[test]
fn anarchomancer_matches_each_color_once_only_for_its_controller_and_preserves_pips() {
    let mut engine = GameEngine::new(
        202609321,
        &[0, 1],
        20,
        Some(vec![island_only_deck(), island_only_deck()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_creature_on_battlefield(&mut engine, 0, "goblin_anarchomancer");
    for card in [
        "hill_giant",
        "grizzly_bears",
        "raging_kavu",
        "divination",
        "raging_goblin",
    ] {
        inject_card_into_hand(&mut engine, 0, card);
    }
    for (card, amount) in [
        ("hill_giant", 1),
        ("grizzly_bears", 1),
        ("raging_kavu", 1),
        ("divination", 0),
        ("raging_goblin", 1),
    ] {
        assert_eq!(reduction(&mut engine, 0, card), amount, "{card}");
    }
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            g: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "raging_kavu");
    engine
        .apply_command(0, &cast_spell(index, vec![]))
        .expect("both colored pips are still paid");
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
    resolve_entire_stack_two_player(&mut engine);
    let index = hand_index_for_card(&engine, 0, "raging_goblin");
    assert!(
        engine.apply_command(0, &cast_spell(index, vec![])).is_err(),
        "reduction cannot remove red pip at generic zero"
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(index, vec![]))
        .expect("generic floor zero with red paid");
    inject_card_into_hand(&mut engine, 1, "raging_kavu");
    engine.apply_command(0, &pass()).unwrap();
    assert_eq!(
        reduction(&mut engine, 1, "raging_kavu"),
        0,
        "opponent receives no reduction"
    );
}

#[test]
fn mastermind_enters_after_first_draw_and_tracks_each_opponents_second_not_third() {
    let mut engine = GameEngine::new(
        202609322,
        &[0, 1, 2],
        20,
        Some(vec![
            island_only_deck(),
            island_only_deck(),
            island_only_deck(),
        ]),
        true,
    )
    .unwrap();
    for _ in 0..3 {
        if engine.state.turn_step == TurnStep::Main1 {
            break;
        }
        pass_round(&mut engine);
    }
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    let jace = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
    apply_ability(&mut engine, 0, jace, 0, vec![]).unwrap();
    pass_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    for player in 1..3 {
        assert_eq!(
            engine.state.turn_history.current.player(player).cards_drawn,
            1
        );
    }
    let controller_draws = engine.state.turn_history.current.player(0).cards_drawn;
    let source = inject_creature_on_battlefield(&mut engine, 0, "faerie_mastermind");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 6,
            ..Default::default()
        },
    );
    let before: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
    let tops: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|p| *p.library.front().unwrap())
        .collect();
    apply_ability(&mut engine, 0, source, 0, vec![]).unwrap();
    assert!(!engine.state.objects[&source].tapped, "no tap cost");
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.hand.len())
            .collect::<Vec<_>>(),
        before,
        "draw waits for resolution"
    );
    pass_round(&mut engine);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "one mandatory trigger per opponent; controller excluded"
    );
    for player in 0..3 {
        assert_eq!(engine.state.players[player].hand.len(), before[player] + 1);
        assert!(engine.state.players[player].hand.contains(&tops[player]));
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(player as i32)
                .cards_drawn,
            if player == 0 { controller_draws + 1 } else { 2 }
        );
    }
    pass_round(&mut engine);
    pass_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].hand.len(), before[0] + 3);
    apply_ability(&mut engine, 0, source, 0, vec![]).unwrap();
    pass_round(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "third opponent draws do not trigger"
    );
    assert_eq!(engine.state.players[0].hand.len(), before[0] + 4);
    for (player, original) in before.iter().enumerate().skip(1) {
        assert_eq!(engine.state.players[player].hand.len(), original + 2);
    }
}

#[test]
fn mastermind_flash_casts_on_opponent_turn_and_draw_history_resets() {
    let mut engine = GameEngine::new(
        202609323,
        &[0, 1],
        20,
        Some(vec![island_only_deck(), island_only_deck()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 0);
    pass_both_players(&mut engine); // normal turn draw is opponent's first draw
    pass_both_players(&mut engine); // opponent main1
    engine.apply_command(1, &pass()).unwrap();
    let faerie = inject_card_into_hand(&mut engine, 0, "faerie_mastermind");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "faerie_mastermind");
    engine
        .apply_command(0, &cast_spell(index, vec![]))
        .expect("Flash permits opponent-turn cast");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&faerie].zone,
        tricerules_core::Zone::Battlefield
    );
    // Active opponent passes so controller can activate at normal timing.
    engine.apply_command(1, &pass()).unwrap();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let before = engine.state.players[0].hand.len();
    apply_ability(&mut engine, 0, faerie, 0, vec![]).unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), before + 2);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 2);
    end_active_turn(&mut engine, 1);
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        0,
        "new turn resets opponent ordinal"
    );
}
