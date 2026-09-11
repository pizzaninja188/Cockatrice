//! Issue #253 — exact tapped multicolor-land recipes reuse existing replacement and mana paths.
//!
//! Oracle and rulings checked 2026-09-11. CR 614.1d and 614.12 govern the entry
//! replacement, while CR 605.1a and 605.3b make the targetless mana abilities resolve
//! immediately without using the stack.

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

#[test]
fn generated_guildgate_enters_tapped_and_cannot_activate_immediately() {
    let decks = Some(vec![
        vec!["rakdos_guildgate".into(); 7],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(253_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let hand_index = hand_index_for_card(&engine, 0, "rakdos_guildgate");
    engine
        .apply_command(0, &play_land(hand_index))
        .expect("play Rakdos Guildgate");
    let guildgate = battlefield_object_for_card(&engine, 0, "rakdos_guildgate");
    assert!(engine.state.objects[&guildgate].tapped);

    engine
        .apply_command(0, &activate_ability_for(&engine, guildgate, 0, vec![]))
        .expect_err("a tapped land cannot pay its tap cost");
    let pool = &engine.state.players[0].mana_pool;
    assert_eq!((pool.black, pool.red), (0, 0));
}

#[test]
fn generated_two_and_three_color_lands_offer_each_printed_option_in_order() {
    for (seed, card_id, option, expected) in [
        (253_100, "rakdos_guildgate", 0, (1, 0, 0, 0, 0)),
        (253_101, "rakdos_guildgate", 1, (0, 1, 0, 0, 0)),
        (253_102, "nomad_outpost", 0, (0, 1, 0, 0, 0)),
        (253_103, "nomad_outpost", 1, (0, 0, 1, 0, 0)),
        (253_104, "nomad_outpost", 2, (1, 0, 0, 0, 0)),
    ] {
        let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let land = inject_permanent_on_battlefield(&mut engine, 0, card_id);
        let mut command = activate_ability_for(&engine, land, 0, vec![]);
        let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
            unreachable!()
        };
        ability.mana_option_index = option;

        engine
            .apply_command(0, &command)
            .expect("activate selected mana option");
        let pool = &engine.state.players[0].mana_pool;
        assert_eq!(
            (pool.black, pool.red, pool.white, pool.blue, pool.green),
            expected,
            "{card_id} option {option}"
        );
        assert!(engine.state.objects[&land].tapped);
        assert!(
            engine.state.stack.is_empty(),
            "mana ability resolves immediately"
        );
    }
}
