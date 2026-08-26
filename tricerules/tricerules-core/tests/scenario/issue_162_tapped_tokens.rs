//! Issue #162: ordinary token creation may author tapped initial entry status without a later
//! tap action. The same simultaneous entry and public TokenCreated path remains authoritative.

use crate::helpers::*;

fn assert_one_tapped_robot(engine: &GameEngine, batch: &RuledEventBatch) {
    let robots = battlefield_token_oids(engine, 0, "robot_c_2_2");
    assert_eq!(robots.len(), 1, "one Robot token entered");
    let robot = robots[0];
    assert!(engine.state.objects[&robot].tapped, "Robot entered tapped");
    assert!(token_created_events(batch).iter().any(|created| {
        created.object_id == robot && created.card_id == "robot_c_2_2" && created.enters_tapped
    }));
}

#[test]
fn gravpack_monoist_death_trigger_creates_a_tapped_robot() {
    let decks = Some(vec![
        deck_with("swamp", &["gravpack_monoist", "lightning_bolt"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(162_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let monoist = relocate_to_battlefield(&mut engine, 0, "gravpack_monoist", false);
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    grant_pool(&mut engine, 0);

    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt, target_object(monoist)))
        .expect("cast Lightning Bolt");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&monoist].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(engine.state.stack.len(), 1, "death trigger is on the stack");

    engine.apply_command(0, &pass()).expect("controller pass");
    let resolved = engine
        .apply_command(1, &pass())
        .expect("resolve death trigger");
    assert_one_tapped_robot(&engine, &resolved);
}

#[test]
fn melded_moxite_sacrifices_then_creates_a_tapped_robot() {
    let decks = Some(vec![
        deck_with("mountain", &["melded_moxite"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(162_002, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let moxite = relocate_to_battlefield(&mut engine, 0, "melded_moxite", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(moxite, 0, vec![]))
        .expect("activate Melded Moxite");
    assert_eq!(
        engine.state.objects[&moxite].zone,
        tricerules_core::Zone::Graveyard,
        "the source is sacrificed as an activation cost"
    );
    assert!(
        battlefield_token_oids(&engine, 0, "robot_c_2_2").is_empty(),
        "the token waits for the ability to resolve"
    );

    engine.apply_command(0, &pass()).expect("controller pass");
    let resolved = engine
        .apply_command(1, &pass())
        .expect("resolve activation");
    assert_one_tapped_robot(&engine, &resolved);
}

#[test]
fn melded_moxite_cannot_be_sacrificed_when_its_mana_cost_is_unaffordable() {
    let decks = Some(vec![
        deck_with("mountain", &["melded_moxite"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(162_003, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let moxite = relocate_to_battlefield(&mut engine, 0, "melded_moxite", false);

    engine
        .apply_command(0, &activate_ability(moxite, 0, vec![]))
        .expect_err("activation must reject an unpaid {3} cost");
    assert_eq!(
        engine.state.objects[&moxite].zone,
        tricerules_core::Zone::Battlefield,
        "a rejected activation cannot sacrifice its source"
    );
    assert!(battlefield_token_oids(&engine, 0, "robot_c_2_2").is_empty());
}
