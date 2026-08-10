use crate::helpers::*;

#[test]
fn dwarven_priest_counts_controlled_creatures_when_its_trigger_resolves() {
    let decks = Some(vec![
        deck_with("plains", &["dwarven_priest"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(51_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "dwarven_priest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );

    let priest = hand_index_for_card(&engine, 0, "dwarven_priest");
    engine
        .apply_command(0, &cast_spell(priest, vec![]))
        .expect("cast Dwarven Priest");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is waiting");

    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.players[0].life, 23,
        "the Priest and both creatures its controller owns are counted at resolution"
    );
}
