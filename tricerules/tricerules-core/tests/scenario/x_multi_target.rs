//! Scenario tests for `DamageTargets` (Fireball / Fire // Ice).
//! Oracle: Fireball deals X damage divided as you choose among any number of targets.
//!   It costs {1} more to cast for each target beyond the first. (CR 601.2d/f)
//! Oracle: Fire deals 2 damage divided as you choose among one or two targets.

use crate::helpers::*;

fn fireball_deck() -> Option<Vec<Vec<String>>> {
    Some(vec![
        vec![
            "fireball".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec!["forest".into(); 7],
    ])
}

/// Fireball with X=5 and one target deals all 5 damage to that target.
#[test]
fn fireball_single_target_all_damage() {
    let mut e = GameEngine::new(1601, &[0, 1], 20, fireball_deck(), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 6,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "fireball");
    // {X}{R} with X=5: pay 5 generic + 1 red = 6 mana; single target → no surcharge.
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                hand_card_index: idx as u32,
                targets: vec![TargetRef {
                    object_id: 1,
                    damage_amount: 5,
                }],
                x_value: 5,
                ..Default::default()
            })),
        },
    )
    .expect("cast Fireball X=5 single target");

    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    assert_eq!(e.state.players[1].life, 15, "5 damage to opponent");
}

/// Fireball with X=5 split between two targets: 3 to player 1, 2 to player 0.
/// Costs {1} extra for the second target: total mana = {X=5}{R}{1} = 7.
#[test]
fn fireball_split_between_two_targets() {
    let mut e = GameEngine::new(1602, &[0, 1], 20, fireball_deck(), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 7,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "fireball");
    // X=5, 2 targets: cost = 5 + 1 (R) + 1 (surcharge) = 7 total.
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                hand_card_index: idx as u32,
                targets: vec![
                    TargetRef {
                        object_id: 1,
                        damage_amount: 3,
                    },
                    TargetRef {
                        object_id: 0,
                        damage_amount: 2,
                    },
                ],
                x_value: 5,
                ..Default::default()
            })),
        },
    )
    .expect("cast Fireball split");

    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    assert_eq!(e.state.players[1].life, 17, "3 damage to opponent");
    assert_eq!(e.state.players[0].life, 18, "2 damage to self");
}

/// Fireball damage amounts that don't sum to X are rejected at cast time.
#[test]
fn fireball_over_allocation_rejected() {
    let mut e = GameEngine::new(1603, &[0, 1], 20, fireball_deck(), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 8,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "fireball");
    // X=3 but allocating 4 + 2 = 6: rejected.
    let err = e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                hand_card_index: idx as u32,
                targets: vec![
                    TargetRef {
                        object_id: 1,
                        damage_amount: 4,
                    },
                    TargetRef {
                        object_id: 0,
                        damage_amount: 2,
                    },
                ],
                x_value: 3,
                ..Default::default()
            })),
        },
    );
    assert!(err.is_err(), "over-allocation must be rejected");
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("sum"), "error should mention sum: {msg}");
}

/// Fireball with zero targets is illegal.
#[test]
fn fireball_requires_at_least_one_target() {
    let mut e = GameEngine::new(1604, &[0, 1], 20, fireball_deck(), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 4,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "fireball");
    let err = e.apply_command(0, &cast_spell_x(idx, vec![], 3));
    assert!(err.is_err(), "Fireball needs at least one target");
}

/// Fire // Ice: Fire deals 2 to a single target (no surcharge, no split needed).
#[test]
fn fire_single_target_deals_two() {
    let decks = Some(vec![
        vec![
            "fire_ice".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(1605, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "fire_ice");
    // Fire face (index 0): all 2 damage to opponent.
    e.apply_command(0, &cast_spell_face(idx, target_player_damage(1, 2), 0))
        .expect("cast Fire");

    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    assert_eq!(e.state.players[1].life, 18, "Fire deals 2");
}

/// Fire // Ice: Fire split 1+1 between two players.
#[test]
fn fire_split_between_two_targets() {
    let decks = Some(vec![
        vec![
            "fire_ice".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(1606, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "fire_ice");
    // Fire face (index 0): split 1 to opponent, 1 to self; no surcharge.
    e.apply_command(
        0,
        &cast_spell_face(idx, targets_with_damage(vec![(1, 1), (0, 1)]), 0),
    )
    .expect("cast Fire split");

    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    assert_eq!(e.state.players[1].life, 19, "1 damage to opponent");
    assert_eq!(e.state.players[0].life, 19, "1 damage to self");
}

/// Fireball extra-target surcharge is enforced: two targets need 1 extra mana.
#[test]
fn fireball_insufficient_mana_for_surcharge_rejected() {
    let mut e = GameEngine::new(1607, &[0, 1], 20, fireball_deck(), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    // X=3 + {R} + 1 surcharge = 5 total; only 5 paid but pattern is {X}{R} + 1 = 6 needed.
    // Give only 5: {X=3}{R} = 4 for the base + need 1 more for second target = 5 total.
    // Actually: base {X=3}{R} = 4. Surcharge for second target = 1. Total = 5. Give 4 → fail.
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 4,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "fireball");
    let err = e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                hand_card_index: idx as u32,
                targets: vec![
                    TargetRef {
                        object_id: 1,
                        damage_amount: 2,
                    },
                    TargetRef {
                        object_id: 0,
                        damage_amount: 1,
                    },
                ],
                x_value: 3,
                ..Default::default()
            })),
        },
    );
    assert!(
        err.is_err(),
        "insufficient mana for surcharge must be rejected"
    );
}
