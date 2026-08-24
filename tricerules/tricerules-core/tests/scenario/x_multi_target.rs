//! Scenario tests for `DamageTargets` (Fireball / Fire // Ice).
//! Oracle: Fireball deals X damage divided evenly, rounded down, among any number of targets.
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
                cast_method: tricerules_proto::ruled::v1::CastMethod::Normal as i32,
                source: Some(hand_cast_source(idx)),
                targets: vec![TargetRef {
                    object_id: 1,
                    damage_amount: 5,
                    group_index: 0,
                    kind: 0,
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

/// Fireball with X=5 split evenly between two targets: 2 to each player (rounded down).
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
                cast_method: tricerules_proto::ruled::v1::CastMethod::Normal as i32,
                source: Some(hand_cast_source(idx)),
                targets: vec![
                    TargetRef {
                        object_id: 1,
                        damage_amount: 0,
                        group_index: 0,
                        kind: 0,
                    },
                    TargetRef {
                        object_id: 0,
                        damage_amount: 0,
                        group_index: 0,
                        kind: 0,
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

    assert_eq!(e.state.players[1].life, 18, "2 damage to opponent");
    assert_eq!(e.state.players[0].life, 18, "2 damage to self");
}

/// Fireball ignores caller-supplied allocation amounts because division happens on resolution.
#[test]
fn fireball_does_not_accept_cast_time_allocation() {
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
    // X=3; the stale client-side allocation is ignored and the spell still casts.
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                cast_method: tricerules_proto::ruled::v1::CastMethod::Normal as i32,
                source: Some(hand_cast_source(idx)),
                targets: vec![
                    TargetRef {
                        object_id: 1,
                        damage_amount: 4,
                        group_index: 0,
                        kind: 0,
                    },
                    TargetRef {
                        object_id: 0,
                        damage_amount: 2,
                        group_index: 0,
                        kind: 0,
                    },
                ],
                x_value: 3,
                ..Default::default()
            })),
        },
    )
    .expect("Fireball does not use cast-time allocation");
}

/// Current Fireball rulings allow a zero-target cast; it simply deals no damage.
#[test]
fn fireball_allows_zero_targets() {
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
    e.apply_command(0, &cast_spell_x(idx, vec![], 3))
        .expect("zero-target Fireball is legal");
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
                cast_method: tricerules_proto::ruled::v1::CastMethod::Normal as i32,
                source: Some(hand_cast_source(idx)),
                targets: vec![
                    TargetRef {
                        object_id: 1,
                        damage_amount: 2,
                        group_index: 0,
                        kind: 0,
                    },
                    TargetRef {
                        object_id: 0,
                        damage_amount: 1,
                        group_index: 0,
                        kind: 0,
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

/// Fireball ruling (2017-11-17): "The division involves only targets that are still legal as
/// Fireball resolves." X=4 split between a player and a creature is 2 each — but if the creature
/// is gone by resolution, the remaining legal target takes the *recomputed* 4, not 2.
#[test]
fn fireball_divides_evenly_among_targets_still_legal_at_resolution() {
    let decks = Some(vec![
        deck_with("mountain", &["fireball", "lightning_bolt"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(1610, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    ensure_in_hand(&mut e, 0, "fireball");
    ensure_in_hand(&mut e, 0, "lightning_bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 9,
            c: 9,
            ..Default::default()
        },
    );

    // Fireball X=4 targeting P1 and the Bears (surcharge {1} for the second target).
    let fireball_idx = hand_index_for_card(&e, 0, "fireball");
    e.apply_command(
        0,
        &cast_spell_x(
            fireball_idx,
            targets_with_damage(vec![(1, 0), (bears, 0)]),
            4,
        ),
    )
    .expect("cast Fireball at two targets");

    // In response, kill the Bears so only P1 is a legal target when Fireball resolves.
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(
            bolt_idx,
            vec![TargetRef {
                object_id: bears,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("bolt the Bears in response");

    let p1_life = e.state.players[1].life;
    while !e.state.stack.is_empty() {
        pass_both_players(&mut e);
    }

    assert_eq!(
        e.state.objects.get(&bears).expect("bears").zone,
        tricerules_core::Zone::Graveyard,
        "the Bears died to the Bolt before Fireball resolved"
    );
    assert_eq!(
        e.state.players[1].life,
        p1_life - 4,
        "with one legal target left, all 4 damage goes there (not the cast-time 2)"
    );
}

/// Fireball ruling (2017-11-17): "if the number of legal targets at the time Fireball resolves is
/// greater than X, none of them will be dealt any damage" — 1 damage divided among 2 targets
/// rounds down to 0 each.
#[test]
fn fireball_with_more_targets_than_damage_deals_nothing() {
    let mut e = GameEngine::new(1611, &[0, 1], 20, fireball_deck(), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 9,
            c: 9,
            ..Default::default()
        },
    );

    let p0_life = e.state.players[0].life;
    let p1_life = e.state.players[1].life;
    let idx = hand_index_for_card(&e, 0, "fireball");
    e.apply_command(
        0,
        &cast_spell_x(idx, targets_with_damage(vec![(1, 0), (0, 0)]), 1),
    )
    .expect("cast Fireball X=1 at both players");
    pass_both_players(&mut e);

    assert_eq!(
        e.state.players[1].life, p1_life,
        "1 / 2 targets rounds to 0"
    );
    assert_eq!(
        e.state.players[0].life, p0_life,
        "1 / 2 targets rounds to 0"
    );
}
