use super::helpers::*;

use tricerules_proto::ruled::v1::TargetRef;

#[test]
fn life_goes_on_gains_eight_after_a_creature_dies() {
    let decks = Some(vec![
        deck_with("forest", &["life_goes_on", "murder"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(6101, &[0, 1], 20, decks, true).expect("new");
    ensure_in_hand(&mut e, 0, "murder");
    ensure_in_hand(&mut e, 0, "life_goes_on");
    let bear = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(&e, 0, "murder");
    e.apply_command(
        0,
        &cast_spell(
            murder,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Murder");
    resolve_entire_stack_two_player(&mut e);

    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let life_goes_on = hand_index_for_card(&e, 0, "life_goes_on");
    e.apply_command(0, &cast_spell(life_goes_on, vec![]))
        .expect("cast Life Goes On");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.players[0].life, 28);
    assert_eq!(e.state.turn_history.current.creatures_died, 1);
    assert_eq!(e.state.turn_history.current.spells_cast, 2);
}

#[test]
fn life_goes_on_gains_four_when_no_creature_died() {
    let decks = Some(vec![
        deck_with("forest", &["life_goes_on"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6102, &[0, 1], 20, decks, true).expect("new");
    ensure_in_hand(&mut e, 0, "life_goes_on");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    let life_goes_on = hand_index_for_card(&e, 0, "life_goes_on");
    e.apply_command(0, &cast_spell(life_goes_on, vec![]))
        .expect("cast Life Goes On");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.players[0].life, 24);
    assert_eq!(e.state.turn_history.current.creatures_died, 0);
}

#[test]
fn conditional_amount_is_evaluated_when_the_effect_resolves() {
    let decks = Some(vec![
        deck_with("forest", &["life_goes_on"]),
        deck_with("swamp", &["murder", "grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(6103, &[0, 1], 20, decks, true).expect("new");
    ensure_in_hand(&mut e, 0, "life_goes_on");
    ensure_in_hand(&mut e, 1, "murder");
    let bear = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut e,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );

    let life_goes_on = hand_index_for_card(&e, 0, "life_goes_on");
    e.apply_command(0, &cast_spell(life_goes_on, vec![]))
        .expect("cast Life Goes On before any creature has died");
    e.apply_command(0, &pass()).expect("pass priority");
    let murder = hand_index_for_card(&e, 1, "murder");
    e.apply_command(
        1,
        &cast_spell(
            murder,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("respond with Murder");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.players[0].life, 28);
    assert_eq!(e.state.turn_history.current.creatures_died, 1);
}

#[test]
fn the_same_creature_can_die_more_than_once_in_a_turn() {
    let decks = Some(vec![
        deck_with("swamp", &["murder", "reanimate", "murder"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(6104, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
    ensure_in_hand(&mut e, 0, "murder");
    ensure_in_hand(&mut e, 0, "reanimate");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 5,
            c: 2,
            ..Default::default()
        },
    );

    let first_murder = hand_index_for_card(&e, 0, "murder");
    e.apply_command(
        0,
        &cast_spell(
            first_murder,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast first Murder");
    resolve_entire_stack_two_player(&mut e);

    let reanimate = hand_index_for_card(&e, 0, "reanimate");
    e.apply_command(
        0,
        &cast_spell(
            reanimate,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("reanimate the Bear");
    resolve_entire_stack_two_player(&mut e);

    ensure_in_hand(&mut e, 0, "murder");
    let second_murder = hand_index_for_card(&e, 0, "murder");
    e.apply_command(
        0,
        &cast_spell(
            second_murder,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast second Murder");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.turn_history.current.creatures_died, 2);
}

#[test]
fn noncreature_deaths_do_not_increment_the_creature_count() {
    let decks = Some(vec![
        deck_with("mountain", &["shatterstorm", "short_sword"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6105, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "short_sword", false);
    ensure_in_hand(&mut e, 0, "shatterstorm");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );

    let shatterstorm = hand_index_for_card(&e, 0, "shatterstorm");
    e.apply_command(0, &cast_spell(shatterstorm, vec![]))
        .expect("cast Shatterstorm");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.turn_history.current.creatures_died, 0);
}

#[test]
fn cleanup_rolls_current_history_to_previous_and_resets_current() {
    let decks = Some(vec![
        deck_with("swamp", &["murder"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(6106, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "murder");
    let bear = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(&e, 0, "murder");
    e.apply_command(
        0,
        &cast_spell(
            murder,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Murder");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.turn_history.current.creatures_died, 1);
    assert_eq!(e.state.turn_history.current.spells_cast, 1);

    end_active_turn(&mut e, 0);

    assert_eq!(e.state.turn_history.current.creatures_died, 0);
    assert_eq!(e.state.turn_history.current.spells_cast, 0);
    assert_eq!(e.state.turn_history.previous.creatures_died, 1);
    assert_eq!(e.state.turn_history.previous.spells_cast, 1);
}

#[test]
fn rejected_casts_do_not_enter_turn_history() {
    let decks = Some(vec![
        deck_with("forest", &["life_goes_on"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6107, &[0, 1], 20, decks, true).expect("new");
    ensure_in_hand(&mut e, 0, "life_goes_on");
    let life_goes_on = hand_index_for_card(&e, 0, "life_goes_on");

    e.apply_command(0, &cast_spell(life_goes_on, vec![]))
        .expect_err("casting without green mana is rejected");

    assert_eq!(e.state.turn_history.current.spells_cast, 0);
    assert_eq!(hand_index_for_card(&e, 0, "life_goes_on"), life_goes_on);
}
