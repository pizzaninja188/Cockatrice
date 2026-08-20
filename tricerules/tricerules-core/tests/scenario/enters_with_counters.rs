use crate::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

fn engine_with_p0_cards(seed: u64, cards: &[&str]) -> GameEngine {
    let mut p0: Vec<String> = cards.iter().map(|card| (*card).to_string()).collect();
    while p0.len() < 7 {
        p0.push("mountain".to_string());
    }
    let decks = Some(vec![p0, vec!["forest".to_string(); 7]]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn plus_one_counters(engine: &GameEngine, object_id: u32) -> u32 {
    engine.state.objects[&object_id]
        .counters
        .get(&CounterKind::PlusOnePlusOne)
        .copied()
        .unwrap_or(0)
}

#[test]
fn endless_one_uses_the_resolving_spells_chosen_x_for_entry_counters() {
    let mut engine = engine_with_p0_cards(51_001, &["endless_one"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );

    let endless_one = hand_index_for_card(&engine, 0, "endless_one");
    engine
        .apply_command(0, &cast_spell_x(endless_one, vec![], 3))
        .expect("cast Endless One with X=3");
    pass_both_players(&mut engine);

    let object_id = battlefield_object_for_card(&engine, 0, "endless_one");
    assert_eq!(plus_one_counters(&engine, object_id), 3);
    assert_eq!(engine.effective_power(object_id), Some(3));
    assert_eq!(engine.effective_toughness(object_id), Some(3));
}

#[test]
fn zero_count_is_applied_once_before_endless_one_dies_to_state_based_actions() {
    let mut engine = engine_with_p0_cards(51_002, &["endless_one"]);

    let endless_one = hand_index_for_card(&engine, 0, "endless_one");
    let object_id = engine.state.players[0].hand[endless_one];
    engine
        .apply_command(0, &cast_spell_x(endless_one, vec![], 0))
        .expect("cast Endless One with X=0");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&object_id].zone, Zone::Graveyard);
    assert_eq!(engine.state.turn_history.current.creatures_died, 1);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn squad_captain_counts_only_other_creatures_its_controller_controls() {
    let mut engine = engine_with_p0_cards(51_003, &["squad_captain"]);
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 0, "walking_corpse");
    inject_creature_on_battlefield(&mut engine, 1, "alpine_grizzly");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
    );

    let captain = hand_index_for_card(&engine, 0, "squad_captain");
    engine
        .apply_command(0, &cast_spell(captain, vec![]))
        .expect("cast Squad Captain");
    pass_both_players(&mut engine);

    let object_id = battlefield_object_for_card(&engine, 0, "squad_captain");
    assert_eq!(plus_one_counters(&engine, object_id), 2);
    assert_eq!(engine.effective_power(object_id), Some(4));
}

#[test]
fn squad_captain_count_is_generic_over_every_seat_in_game_state() {
    let mut engine = engine_with_p0_cards(51_008, &["squad_captain"]);
    // Game construction remains two-seat until combat gains per-attacker defender identity. Add a
    // third state seat here only to prove that the shared amount evaluator does not use two-player
    // arithmetic when it scans battlefield controllers.
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 1, "walking_corpse");
    inject_creature_on_battlefield(&mut engine, 2, "alpine_grizzly");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
    );

    let captain = hand_index_for_card(&engine, 0, "squad_captain");
    engine
        .apply_command(0, &cast_spell(captain, vec![]))
        .expect("cast Squad Captain");
    for _ in 0..engine.state.players.len() {
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("each seat passes priority");
    }

    let object_id = battlefield_object_for_card(&engine, 0, "squad_captain");
    assert_eq!(plus_one_counters(&engine, object_id), 1);
}

#[test]
fn squad_captain_records_a_zero_count_without_reapplying_the_effect() {
    let mut engine = engine_with_p0_cards(51_004, &["squad_captain"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
    );

    let captain = hand_index_for_card(&engine, 0, "squad_captain");
    engine
        .apply_command(0, &cast_spell(captain, vec![]))
        .expect("cast Squad Captain");
    pass_both_players(&mut engine);

    let object_id = battlefield_object_for_card(&engine, 0, "squad_captain");
    assert_eq!(plus_one_counters(&engine, object_id), 0);
    assert_eq!(engine.effective_power(object_id), Some(2));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn bloodcrazed_paladin_uses_the_shared_current_turn_death_count() {
    let mut engine = engine_with_p0_cards(51_005, &["bloodcrazed_paladin"]);
    engine.state.turn_history.current.creatures_died = 3;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    let paladin = hand_index_for_card(&engine, 0, "bloodcrazed_paladin");
    engine
        .apply_command(0, &cast_spell(paladin, vec![]))
        .expect("cast Bloodcrazed Paladin");
    pass_both_players(&mut engine);

    let object_id = battlefield_object_for_card(&engine, 0, "bloodcrazed_paladin");
    assert_eq!(plus_one_counters(&engine, object_id), 3);
    assert_eq!(engine.effective_power(object_id), Some(4));
}

#[test]
fn counter_and_tapped_entry_replacements_are_order_independent() {
    for (seed, chosen_candidate) in [(51_006, 0_usize), (51_007, 1_usize)] {
        let mut engine = engine_with_p0_cards(seed, &["orb_of_dreams", "endless_one"]);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 6,
                ..Default::default()
            },
        );

        let orb = hand_index_for_card(&engine, 0, "orb_of_dreams");
        engine
            .apply_command(0, &cast_spell(orb, vec![]))
            .expect("cast Orb of Dreams");
        pass_both_players(&mut engine);

        let endless_one = hand_index_for_card(&engine, 0, "endless_one");
        engine
            .apply_command(0, &cast_spell_x(endless_one, vec![], 3))
            .expect("cast Endless One");
        pass_both_players(&mut engine);

        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("both entry replacements require ordering");
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ReplacementEffect
        );
        assert_eq!(pending.presentation.candidates.len(), 2);
        let application = pending.presentation.candidates[chosen_candidate];
        engine
            .apply_command(0, &submit_resolution_choice(vec![application]))
            .expect("choose first entry replacement");

        let object_id = battlefield_object_for_card(&engine, 0, "endless_one");
        assert!(engine.state.objects[&object_id].tapped);
        assert_eq!(plus_one_counters(&engine, object_id), 3);
        assert!(engine.state.pending_resolution.is_none());
    }
}
