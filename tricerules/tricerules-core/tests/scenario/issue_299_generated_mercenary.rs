//! Issue #299 — the reviewed dies-to-Mercenary cohort.
//!
//! CR 603.6c/603.10 and 700.4 cover dies/LKI; CR 111.2-111.4 cover token
//! ownership and characteristics; CR 602.5d and 514.2 cover the token's
//! sorcery-speed activation and end-of-turn pump cleanup.

use super::helpers::*;
use tricerules_core::Zone;

fn three_player_main1(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    while engine.state.turn_step != tricerules_core::TurnStep::Main1 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("advance three-player turn");
    }
    engine
}

fn resolve_entire_stack_three_player(engine: &mut GameEngine) {
    loop {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            break;
        }
        for _ in 0..engine.state.players.len() {
            if engine.state.stack.is_empty() {
                break;
            }
            let player = engine.state.priority_player_id();
            engine
                .apply_command(player, &pass())
                .expect("three-player priority pass");
        }
    }
}

#[test]
fn issue_299_nezumi_death_creates_one_untapped_controller_owned_mercenary_after_lki() {
    let mut engine = GameEngine::new(299_001, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    // P1 owns the permanent while P0 controls it.  The death trigger must use
    // the last-known controller, then resolve after the source is in its graveyard.
    let source = inject_creature_under_foreign_control(&mut engine, 1, 0, "nezumi_linkbreaker");
    engine.state.objects.get_mut(&source).unwrap().damage = 1;
    engine.apply_command(0, &pass()).expect("state-based death");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1, "one pending dies trigger");
    assert!(battlefield_token_oids(&engine, 0, "mercenary_r_1_1").is_empty());

    resolve_entire_stack_two_player(&mut engine);
    let mercenaries = battlefield_token_oids(&engine, 0, "mercenary_r_1_1");
    assert_eq!(mercenaries.len(), 1, "the trigger must create one token");
    let [mercenary] = mercenaries.as_slice() else {
        panic!("one Mercenary should resolve from the pending LKI trigger");
    };
    let token = &engine.state.objects[mercenary];
    assert_eq!(token.owner, 0, "token owner is the creating controller");
    assert_eq!(token.controller, 0);
    assert!(!token.tapped, "recipe creates one untapped token");
    assert!(token.summoning_sick, "the new token is summoning sick");
}

#[test]
fn issue_299_sacrifice_death_creates_exactly_one_mercenary() {
    let decks = Some(vec![
        deck_with("swamp", &["village_rites", "nezumi_linkbreaker"]),
        deck_with("mountain", &[]),
    ]);
    let mut engine = GameEngine::new(299_007, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "village_rites");
    let source = relocate_to_battlefield(&mut engine, 0, "nezumi_linkbreaker", false);
    let rites = hand_index_for_card(&engine, 0, "village_rites");
    engine.state.players[0].mana_pool.black = 1;
    engine
        .apply_command(
            0,
            &cast_spell_with_costs(rites, vec![], vec![permanent_cost_selection(0, source)]),
        )
        .expect("sacrifice Nezumi as an additional cost");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "dies trigger plus Village Rites"
    );

    resolve_entire_stack_two_player(&mut engine);
    let mercenaries = battlefield_token_oids(&engine, 0, "mercenary_r_1_1");
    assert_eq!(mercenaries.len(), 1, "sacrifice death creates one token");
    let [mercenary] = mercenaries.as_slice() else {
        panic!("sacrifice death should create one Mercenary");
    };
    assert_eq!(engine.state.objects[mercenary].owner, 0);
    assert!(!engine.state.objects[mercenary].tapped);
}

#[test]
fn issue_299_bounce_and_exile_are_not_dies() {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                "nezumi_linkbreaker",
                "nezumi_linkbreaker",
                "unsummon",
                "wander_off",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(299_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let first = relocate_to_battlefield(&mut engine, 0, "nezumi_linkbreaker", false);
    relocate_to_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(first)))
        .expect("bounce Nezumi");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&first].zone, Zone::Hand);
    assert!(battlefield_token_oids(&engine, 0, "mercenary_r_1_1").is_empty());

    let second = relocate_to_battlefield(&mut engine, 0, "nezumi_linkbreaker", false);
    relocate_to_hand(&mut engine, 0, "wander_off");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let wander_off = hand_index_for_card(&engine, 0, "wander_off");
    engine
        .apply_command(0, &cast_spell(wander_off, target_object(second)))
        .expect("exile Nezumi");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&second].zone, Zone::Exile);
    assert!(battlefield_token_oids(&engine, 0, "mercenary_r_1_1").is_empty());
}

#[test]
fn issue_299_hand_to_graveyard_discard_is_not_dies() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &[
                "thrill_of_possibility",
                "nezumi_linkbreaker",
                "forest",
                "forest",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(299_008, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "thrill_of_possibility");
    ensure_in_hand(&mut engine, 0, "nezumi_linkbreaker");
    let spell_slot = hand_index_for_card(&engine, 0, "thrill_of_possibility");
    let discard_slot = hand_index_for_card(&engine, 0, "nezumi_linkbreaker");
    let discarded = engine.state.players[0].hand[discard_slot];
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_command(
            0,
            &cast_spell_with_costs(
                spell_slot,
                vec![],
                vec![hand_cost_selection(0, discard_slot as u32)],
            ),
        )
        .expect("discard Nezumi from hand as a spell cost");
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert!(battlefield_token_oids(&engine, 0, "mercenary_r_1_1").is_empty());
    resolve_entire_stack_two_player(&mut engine);
    assert!(battlefield_token_oids(&engine, 0, "mercenary_r_1_1").is_empty());
}

#[test]
fn issue_299_simultaneous_deaths_create_one_token_per_trigger_controller() {
    let mut engine = GameEngine::new(299_003, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let first = inject_creature_under_foreign_control(&mut engine, 0, 0, "nezumi_linkbreaker");
    let second = inject_creature_under_foreign_control(&mut engine, 1, 1, "nezumi_linkbreaker");
    engine.state.objects.get_mut(&first).unwrap().damage = 1;
    engine.state.objects.get_mut(&second).unwrap().damage = 1;

    engine
        .apply_command(0, &pass())
        .expect("simultaneous death SBA");
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 2, "one trigger per dying source");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        battlefield_token_oids(&engine, 0, "mercenary_r_1_1").len(),
        1
    );
    assert_eq!(
        battlefield_token_oids(&engine, 1, "mercenary_r_1_1").len(),
        1
    );
}

#[test]
fn issue_299_three_player_foreign_owner_token_stays_with_last_controller() {
    let mut engine = three_player_main1(299_004);
    let source = inject_creature_under_foreign_control(&mut engine, 1, 2, "nezumi_linkbreaker");
    engine.state.objects.get_mut(&source).unwrap().damage = 1;
    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("three-player death SBA");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_three_player(&mut engine);
    assert!(battlefield_token_oids(&engine, 0, "mercenary_r_1_1").is_empty());
    assert!(battlefield_token_oids(&engine, 1, "mercenary_r_1_1").is_empty());
    assert_eq!(
        battlefield_token_oids(&engine, 2, "mercenary_r_1_1").len(),
        1
    );
}

#[test]
fn issue_299_mercenary_activation_is_controller_only_sorcery_speed_and_eot_limited() {
    let decks = Some(vec![
        deck_with("island", &["nezumi_linkbreaker", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(299_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "nezumi_linkbreaker", false);
    let friendly = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let opposing = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine.state.objects.get_mut(&source).unwrap().damage = 1;
    engine.apply_command(0, &pass()).expect("death SBA");
    resolve_entire_stack_two_player(&mut engine);
    let mercenaries = battlefield_token_oids(&engine, 0, "mercenary_r_1_1");
    let [mercenary] = mercenaries.as_slice() else {
        panic!("death creates one Mercenary");
    };

    // The Mercenary is a creature token, so let the controller's next untap
    // step clear summoning sickness before exercising its activated ability.
    end_active_turn(&mut engine, 0);
    for _ in 0..20 {
        if engine.state.active_player_id() == 0
            && engine.state.turn_step == tricerules_core::TurnStep::Main1
        {
            break;
        }
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &primitive_yield())
            .expect("opponent yields back to controller");
        resolve_cleanup_discards_if_any(&mut engine);
    }
    assert_eq!(engine.state.active_player_id(), 0);
    assert!(!engine.state.objects[mercenary].summoning_sick);

    let before = engine.state.players[0].mana_pool;
    assert!(apply_ability(&mut engine, 0, *mercenary, 0, target_object(opposing)).is_err());
    assert_eq!(engine.state.players[0].mana_pool, before);
    assert!(!engine.state.objects[mercenary].tapped);

    assert!(apply_ability(&mut engine, 1, *mercenary, 0, target_object(friendly)).is_err());
    assert!(!engine.state.objects[mercenary].tapped);

    apply_ability(&mut engine, 0, *mercenary, 0, target_object(friendly))
        .expect("controller activates Mercenary at sorcery speed");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(friendly), Some(3));
    assert_eq!(engine.effective_power(opposing), Some(2));
    assert!(engine.state.objects[mercenary].tapped);

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(friendly),
        Some(2),
        "pump expires at cleanup"
    );
}

#[test]
fn issue_299_mercenary_cannot_activate_outside_controller_sorcery_window() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(299_006, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_under_foreign_control(&mut engine, 1, 0, "nezumi_linkbreaker");
    engine.state.objects.get_mut(&source).unwrap().damage = 1;
    engine.apply_command(0, &pass()).expect("death SBA");
    resolve_entire_stack_two_player(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let mercenaries = battlefield_token_oids(&engine, 0, "mercenary_r_1_1");
    let [mercenary] = mercenaries.as_slice() else {
        panic!("death creates one Mercenary");
    };

    // Reach the controller's next main phase so summoning sickness and priority
    // are no longer confounders for the sorcery-speed check.
    end_active_turn(&mut engine, 0);
    for _ in 0..20 {
        if engine.state.active_player_id() == 0
            && engine.state.turn_step == tricerules_core::TurnStep::Main1
        {
            break;
        }
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &primitive_yield())
            .expect("advance to controller main phase");
        resolve_cleanup_discards_if_any(&mut engine);
    }
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    assert!(!engine.state.objects[mercenary].summoning_sick);

    inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    grant_pool(&mut engine, 1);
    engine
        .apply_command(0, &pass())
        .expect("pass priority to P1");
    let bolt = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_command(1, &cast_spell(bolt, target_player(0)))
        .expect("P1 casts an instant while P0's turn is active");
    assert_eq!(engine.state.stack.len(), 1);
    engine
        .apply_command(1, &pass())
        .expect("P1 passes priority");
    assert_eq!(engine.state.priority_player_id(), 0);

    let before_power = engine.effective_power(target);
    assert!(
        apply_ability(&mut engine, 0, *mercenary, 0, target_object(target)).is_err(),
        "the validly targeted Mercenary activation is rejected only by sorcery timing"
    );
    assert!(!engine.state.objects[mercenary].tapped);
    assert_eq!(engine.effective_power(target), before_power);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "failed activation leaves the stack unchanged"
    );
}
