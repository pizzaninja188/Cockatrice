//! Issue #270 — generated utility permanents reuse engine-owned ETB triggers, private scry,
//! atomic activation costs, target revalidation, source identity, tokens, and selectable mana.
//!
//! Oracle and rulings checked 2026-09-13. CR 115, 117, 118, 400.7, 602, 603, 605, 608.2b,
//! 701.7, 701.8, 701.21, and 701.22 govern targets, timing, costs, new-object identity,
//! activated and triggered abilities, mana abilities, resolution, tokens, destruction,
//! sacrifice, and scry.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let objects = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|candidate| !objects.contains(candidate));
    for object in objects.iter().rev() {
        engine.state.players[player].library.push_front(*object);
    }
    objects
}

fn resolve_top_two_player(engine: &mut GameEngine) -> RuledEventBatch {
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("first pass");
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("second pass resolves")
}

fn activate_with_mana_option(
    engine: &GameEngine,
    source: u32,
    ability_index: u32,
    option: u32,
) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, ability_index, Vec::new());
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn candy_trail_scry_is_private_then_sacrifice_gains_and_draws_in_order() {
    let decks = Some(vec![
        deck_with("island", &["candy_trail"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(270_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let top = seat_on_top(&mut engine, 0, &["storm_crow", "grizzly_bears"]);

    let candy = move_ready_to_battlefield(&mut engine, 0, "candy_trail");
    let choice_batch = resolve_top_two_player(&mut engine);
    let choice = find_resolution_choice(&choice_batch).expect("private scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, top);
    assert!(choice.public_reveal.is_none());
    engine
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("bottom one card and keep one on top");

    engine.state.players[0].life = 17;
    let hand_before = engine.state.players[0].hand.len();
    let generation_before = engine.state.zone_change_generation[&candy];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, candy, 0, Vec::new())
        .expect("pay, tap, and sacrifice atomically");
    assert_eq!(engine.state.objects[&candy].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&candy],
        generation_before + 1
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
}

#[test]
fn bear_trap_rejects_bad_payments_and_stale_targets_then_attributes_damage() {
    let mut engine = anthem_engine(270_002, "bear_trap");
    let bear_trap = relocate_to_battlefield(&mut engine, 0, "bear_trap", false);
    let stale_target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");

    apply_ability(&mut engine, 0, bear_trap, 0, target_object(stale_target))
        .expect_err("underfunded activation is illegal");
    assert_eq!(engine.state.objects[&bear_trap].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&bear_trap].tapped);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&bear_trap)
        .copied()
        .unwrap_or(0);
    apply_ability(&mut engine, 0, bear_trap, 0, target_object(stale_target))
        .expect("activate Bear Trap");
    assert_eq!(engine.state.objects[&bear_trap].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&bear_trap],
        generation_before + 1
    );
    *engine
        .state
        .zone_change_generation
        .entry(stale_target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&stale_target].damage, 0);

    let second_trap = inject_permanent_on_battlefield(&mut engine, 0, "bear_trap");
    let legal_target = inject_creature_on_battlefield(&mut engine, 1, "colossal_dreadmaw");
    engine
        .state
        .objects
        .get_mut(&legal_target)
        .unwrap()
        .toughness = Some(6);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, second_trap, 0, target_object(legal_target))
        .expect("activate the second Bear Trap");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&legal_target].damage, 3);
}

#[test]
fn giant_boulder_destroys_exact_target_after_sacrificing() {
    let mut engine = anthem_engine(270_003, "giants_boulder");
    let boulder = relocate_to_battlefield(&mut engine, 0, "giants_boulder", false);
    let target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );

    apply_ability(&mut engine, 0, boulder, 1, target_object(target))
        .expect("activate Giant's Boulder");
    assert_eq!(engine.state.objects[&boulder].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn hot_dog_cart_and_omni_cheese_use_tokens_and_selectable_mana_paths() {
    let decks = Some(vec![
        deck_with("mountain", &["hot_dog_cart", "omni-cheese_pizza"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(270_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let cart = move_ready_to_battlefield(&mut engine, 0, "hot_dog_cart");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);

    engine
        .apply_command(0, &activate_with_mana_option(&engine, cart, 0, 1))
        .expect("Hot Dog Cart produces the selected blue mana");
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);

    let pizza = relocate_to_battlefield(&mut engine, 0, "omni-cheese_pizza", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_with_mana_option(&engine, pizza, 0, 3))
        .expect("Omni-Cheese Pizza pays and sacrifices before making red mana");
    assert_eq!(engine.state.objects[&pizza].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.red, 1);
}
