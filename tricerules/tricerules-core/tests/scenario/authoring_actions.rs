use super::helpers::*;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, AbilitySourceZone};

#[test]
fn graveyard_activation_carries_source_zone_and_generation() {
    let deck = deck_with("forest", &["fanatic_of_rhonas"]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        702,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    let source = take_oid_from_library_or_hand(&mut e, 0, "fanatic_of_rhonas");
    e.state.players[0].graveyard.push(source);
    e.state.objects.get_mut(&source).unwrap().zone = Zone::Graveyard;
    e.state.zone_change_generation.insert(source, 7);
    e.state.players[0].mana_pool.green = 2;
    e.state.players[0].mana_pool.colorless = 2;
    let command = authoring_actions::activation(&mut e, 0, source, 2).unwrap();
    let Some(Cmd::ActivateAbility(ability)) = &command.cmd else {
        panic!("activation")
    };
    assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard as i32);
    assert_eq!(ability.expected_zone_change_generation, 7);
    e.state.zone_change_generation.insert(source, 8);
    let before = format!("{:?}", e.state);
    assert!(e.apply_command(0, &command).is_err());
    assert_eq!(format!("{:?}", e.state), before);
    let command = authoring_actions::activation(&mut e, 0, source, 2).unwrap();
    e.apply_command(0, &command).unwrap();
    assert_eq!(e.state.objects[&source].zone, Zone::Exile);
    assert_eq!(e.state.players[0].mana_pool, Default::default());
    resolve_entire_stack_two_player(&mut e);
    let tokens = battlefield_token_oids(&e, 0, "fanatic_of_rhonas_b_4_4_zombie_eternalized");
    assert_eq!(tokens.len(), 1);
    assert_eq!(e.effective_power(tokens[0]), Some(4));
    assert_eq!(e.effective_toughness(tokens[0]), Some(4));
}

#[test]
fn main_phase_fixture_passes_all_three_players() {
    let deck = deck_with("forest", &[]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        703,
        &[10, 20, 30],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    assert_eq!(e.state.turn_step, TurnStep::Main1);
    assert_eq!(e.state.priority_player_id(), 10);
}

#[test]
fn priority_round_finishes_existing_passes_without_crossing_next_window() {
    let deck = deck_with("forest", &[]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        711,
        &[10, 20, 30],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .unwrap();
    e.apply_command(10, &pass()).unwrap();
    assert_eq!(e.state.turn_step, TurnStep::Upkeep);
    pass_priority_round(&mut e);
    assert_eq!(e.state.turn_step, TurnStep::Draw);
    assert_eq!(e.state.priority_player_id(), 10);
    assert_eq!(e.state.passes_since_stack_change, 0);
}

#[test]
fn offered_mana_activation_rejects_wrong_actor_and_tapped_source() {
    let mut e = authoring_fixture::game(704, &[10, 20, 30], "sol_ring", Some(0));
    let source = authoring_fixture::ability_source(&mut e, 0, "sol_ring", 0, 0);
    e.state.players[0].mana_pool = Default::default();
    let before = format!("{:?}", e.state);
    assert!(authoring_actions::activation(&mut e, 20, source, 0).is_err());
    assert!(authoring_actions::activation(&mut e, 999, source, 0).is_err());
    assert!(authoring_actions::activation(&mut e, 10, source, 99).is_err());
    assert_eq!(format!("{:?}", e.state), before);
    let command = authoring_actions::activation(&mut e, 10, source, 0).unwrap();
    e.apply_command(10, &command).unwrap();
    assert_eq!(e.state.players[0].mana_pool.colorless, 2);
    assert!(e.state.objects[&source].tapped);
    assert!(e.state.stack.is_empty());
    let before = format!("{:?}", e.state);
    assert!(authoring_actions::activation(&mut e, 10, source, 0).is_err());
    assert_eq!(format!("{:?}", e.state), before);
}

#[test]
fn shared_ferocious_resources_enable_exact_four_green() {
    let mut e = authoring_fixture::game(705, &[0, 1], "fanatic_of_rhonas", Some(0));
    let source = authoring_fixture::ability_source(&mut e, 0, "fanatic_of_rhonas", 0, 1);
    let before = format!("{:?}", e.state);
    assert!(authoring_actions::activation(&mut e, 0, source, 1).is_err());
    assert_eq!(format!("{:?}", e.state), before);

    let mut e = authoring_fixture::game(706, &[0, 1], "fanatic_of_rhonas", Some(1));
    let source = authoring_fixture::ability_source(&mut e, 0, "fanatic_of_rhonas", 0, 1);
    e.state.players[0].mana_pool = Default::default();
    let command = authoring_actions::activation(&mut e, 0, source, 1).unwrap();
    e.apply_command(0, &command).unwrap();
    assert_eq!(e.state.players[0].mana_pool.green, 4);
    assert!(e.state.objects[&source].tapped);
    assert!(e.state.stack.is_empty());
}

#[test]
fn offered_draw_activation_pays_exact_mana_and_sacrifices_source() {
    let mut e = authoring_fixture::game(707, &[0, 1], "mind_stone", Some(1));
    let source = authoring_fixture::ability_source(&mut e, 0, "mind_stone", 0, 1);
    e.state.players[0].mana_pool = Default::default();
    let before = format!("{:?}", e.state);
    assert!(authoring_actions::activation(&mut e, 0, source, 1).is_err());
    assert_eq!(format!("{:?}", e.state), before);
    e.state.players[0].mana_pool.colorless = 1;
    let hand = e.state.players[0].hand.len();
    let generation = e
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let command = authoring_actions::activation(&mut e, 0, source, 1).unwrap();
    e.apply_command(0, &command).unwrap();
    assert_eq!(e.state.players[0].mana_pool, Default::default());
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(e.state.zone_change_generation[&source], generation + 1);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand + 1);
}

#[test]
fn shared_conformance_setup_seeds_search_recovery_and_loyalty_resources() {
    let mut e = authoring_fixture::game(708, &[0, 1], "inventors_fair", Some(1));
    let source = authoring_fixture::ability_source(&mut e, 0, "inventors_fair", 0, 1);
    assert_eq!(
        e.state.players[0]
            .battlefield
            .iter()
            .filter(|oid| e.state.objects[oid].card_id == "sol_ring")
            .count(),
        2
    );
    assert!(authoring_actions::activation(&mut e, 0, source, 1).is_ok());
    let e = authoring_fixture::game(709, &[0, 1], "trash_for_treasure", None);
    assert!(e.state.players[0]
        .graveyard
        .iter()
        .any(|oid| e.state.objects[oid].card_id == "mind_stone"));
    let mut e = authoring_fixture::game(710, &[0, 1], "chandra,_novice_pyromancer", Some(2));
    let source = authoring_fixture::ability_source(&mut e, 0, "chandra,_novice_pyromancer", 0, 2);
    let command = authoring_actions::activation(&mut e, 0, source, 2).unwrap();
    e.apply_command(0, &command).unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&source].counter_count(tricerules_cards::CounterKind::Loyalty),
        3
    );
}
