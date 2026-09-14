use super::helpers::*;

#[test]
fn issue_278_raid_draw_requires_an_attack_this_turn() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(278_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    inject_card_into_hand(&mut engine, 0, "storm_fleet_spy");
    inject_library_card(&mut engine, 0, "island");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let storm_fleet_spy = hand_index_for_card(&engine, 0, "storm_fleet_spy");
    let drawn_before_no_attack = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_command(0, &cast_spell(storm_fleet_spy, vec![]))
        .expect("cast Storm Fleet Spy before attacking");
    engine.apply_command(0, &pass()).expect("caster pass");
    engine.apply_command(1, &pass()).expect("opponent pass");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        drawn_before_no_attack,
        "Raid must not draw when its controller did not attack this turn"
    );

    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    engine
        .apply_command(0, &pass())
        .expect("ap pass begin combat");
    engine
        .apply_command(1, &pass())
        .expect("nap pass begin combat");
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare an attacker");
    engine
        .apply_command(0, &pass())
        .expect("active pass declare attackers");
    engine
        .apply_command(1, &pass())
        .expect("defender pass declare attackers");
    engine
        .apply_command(0, &pass())
        .expect("active pass empty declare blockers");
    engine
        .apply_command(1, &pass())
        .expect("defender pass empty declare blockers");
    engine
        .apply_command(0, &pass())
        .expect("active pass combat damage");
    engine
        .apply_command(1, &pass())
        .expect("defender pass combat damage");
    engine
        .apply_command(0, &pass())
        .expect("active pass end combat");
    engine
        .apply_command(1, &pass())
        .expect("defender pass end combat");
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main2);

    inject_card_into_hand(&mut engine, 0, "skyship_buccaneer");
    inject_library_card(&mut engine, 0, "island");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
    );
    let skyship_buccaneer = hand_index_for_card(&engine, 0, "skyship_buccaneer");
    let drawn_before_attack = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_command(0, &cast_spell(skyship_buccaneer, vec![]))
        .expect("cast Skyship Buccaneer after attacking");
    engine.apply_command(0, &pass()).expect("caster pass");
    engine.apply_command(1, &pass()).expect("opponent pass");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        drawn_before_attack + 1,
        "Raid should draw exactly one card after the controller attacked"
    );
}
