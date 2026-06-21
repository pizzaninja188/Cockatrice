use crate::helpers::*;

// Regression: a spell countered by the *opponent* must go to its OWNER's graveyard (CR 701.6a),
// not the counterer's. The engine emits a PermanentMoved stamped with the countered spell's owner
// so the relay can route the physical card off the shared stack to the right player — without any
// per-card name special-case. Here P0 owns the bolt and P1 counters it.
#[test]
fn countered_spell_moves_to_its_owners_graveyard() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "island".into(),
            "counterspell".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(144, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("p0 play mountain");

    for _ in 0..2 {
        let island_idx = hand_index_for_card(&e, 1, "island");
        let island_oid = e.state.players[1].hand.remove(island_idx);
        e.state.players[1].battlefield.push(island_oid);
        e.state
            .objects
            .get_mut(&island_oid)
            .expect("seeded island")
            .zone = tricerules_core::Zone::Battlefield;
    }

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("p0 cast bolt");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;
    e.apply_command(0, &pass())
        .expect("p0 pass to give p1 priority");

    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let counter_idx = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(
        1,
        &cast_spell(
            counter_idx,
            vec![TargetRef {
                object_id: bolt_oid,
            }],
        ),
    )
    .expect("p1 cast counterspell at bolt");
    let counterspell_oid = e.state.stack.last().expect("counterspell on stack").id;

    e.apply_command(1, &pass()).expect("p1 pass");
    let resolve_batch = e
        .apply_command(0, &pass())
        .expect("p0 pass resolves counter");

    // The decisive assertion: the engine routes the countered bolt to its OWNER (P0).
    let bolt_move = permanents_moved_in(&resolve_batch)
        .into_iter()
        .find(|pm| pm.object_id == bolt_oid)
        .expect("counter must emit a PermanentMoved for the bolt");
    assert_eq!(
        bolt_move.owner_player_id, 0,
        "countered bolt must route to its owner P0, not the counterer P1"
    );
    assert_eq!(
        bolt_move.destination,
        tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32
    );

    assert!(e.state.stack.is_empty(), "counter clears the stack");
    assert!(
        e.state.players[0].graveyard.contains(&bolt_oid),
        "bolt in its owner P0's graveyard"
    );
    assert!(
        !e.state.players[1].graveyard.contains(&bolt_oid),
        "bolt must NOT be in counterer P1's graveyard"
    );
    assert!(
        e.state.players[1].graveyard.contains(&counterspell_oid),
        "counterspell in its owner P1's graveyard"
    );
}

/// Top counterspell counters the bolt; the second counterspell's target is gone — it fizzles.
#[test]
fn counterspell_fizzles_when_original_target_already_left_stack() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "island".into(),
            "island".into(),
            "counterspell".into(),
            "island".into(),
            "island".into(),
            "counterspell".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(91024, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let m0 = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(m0)).expect("mountain");

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("bolt");
    e.apply_command(0, &pass())
        .expect("AP pass so NAP can respond");

    let bolt_oid = e
        .state
        .stack
        .iter()
        .find(|s| s.card_id == "lightning_bolt")
        .expect("bolt on stack")
        .id;

    for _ in 0..4 {
        let ii = hand_index_for_card(&e, 1, "island");
        let oid = e.state.players[1].hand.remove(ii);
        e.state.players[1].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("island").zone = tricerules_core::Zone::Battlefield;
    }

    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let cs1 = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(
        1,
        &cast_spell(
            cs1,
            vec![TargetRef {
                object_id: bolt_oid,
            }],
        ),
    )
    .expect("counter 1");

    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let cs2 = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(
        1,
        &cast_spell(
            cs2,
            vec![TargetRef {
                object_id: bolt_oid,
            }],
        ),
    )
    .expect("counter 2 on top");

    assert_eq!(e.state.stack.len(), 3);

    let mut fizzle_logs = 0usize;
    while !e.state.stack.is_empty() {
        let first = e.state.priority_player_id();
        let second = if first == e.state.players[0].id {
            e.state.players[1].id
        } else {
            e.state.players[0].id
        };
        e.apply_command(first, &pass()).expect("pass");
        let batch = e.apply_command(second, &pass()).expect("resolve");
        fizzle_logs += batch
            .events
            .iter()
            .filter(|ev| matches!(&ev.ev, Some(Ev::Log(l)) if l.text.contains("fizzles")))
            .count();
    }

    assert_eq!(fizzle_logs, 1, "only the second counterspell should fizzle");
    assert_eq!(e.state.players[1].life, 20, "bolt never dealt damage");
}

#[test]
fn counterspell_counters_a_spell_on_stack() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "island".into(),
            "island".into(),
            "lightning_bolt".into(),
            "counterspell".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(903, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");

    for _ in 0..2 {
        let island_idx = hand_index_for_card(&e, 0, "island");
        let island_oid = e.state.players[0].hand.remove(island_idx);
        e.state.players[0].battlefield.push(island_oid);
        e.state
            .objects
            .get_mut(&island_oid)
            .expect("seed island")
            .zone = tricerules_core::Zone::Battlefield;
    }

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("cast bolt");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;

    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let cs_idx = hand_index_for_card(&e, 0, "counterspell");
    let cs_batch = e
        .apply_command(
            0,
            &cast_spell(
                cs_idx,
                vec![TargetRef {
                    object_id: bolt_oid,
                }],
            ),
        )
        .expect("cast counterspell");
    let cs_push = cs_batch
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("counterspell stack pushed");
    assert_eq!(cs_push.targets.len(), 1);
    assert_eq!(cs_push.targets[0].object_id, bolt_oid);
    let counterspell_oid = e.state.stack.last().expect("counterspell on stack").id;

    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert!(e.state.stack.is_empty(), "counterspell should clear stack");
    assert!(e.state.players[0].graveyard.contains(&counterspell_oid));
    assert!(e.state.players[0].graveyard.contains(&bolt_oid));
}

/// Active casts two `Lightning Bolt` while holding priority, then non-active responds
/// with a third bolt. Stack resolves LIFO: NAP's bolt, then AP's second, then AP's first.
#[test]
fn three_bolts_stack_lifo_active_sequential_then_non_active_response() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(4401, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let m0a = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(m0a))
        .expect("p0 play mountain");
    let m0b = hand_index_for_card(&e, 0, "mountain");
    let m0b_oid = e.state.players[0].hand.remove(m0b);
    e.state.players[0].battlefield.push(m0b_oid);
    e.state
        .objects
        .get_mut(&m0b_oid)
        .expect("p0 second mountain")
        .zone = tricerules_core::Zone::Battlefield;

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_p0_first = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_p0_first, target_player(1)))
        .expect("p0 first bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_p0_second = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_p0_second, target_player(1)))
        .expect("p0 second bolt while holding priority");
    assert_eq!(
        e.state.stack.len(),
        2,
        "p0 should have stacked two bolts before passing"
    );
    assert_eq!(
        e.state.priority_player_id(),
        0,
        "active player keeps priority after sequential casts"
    );

    for _ in 0..2 {
        let mi = hand_index_for_card(&e, 1, "mountain");
        let oid = e.state.players[1].hand.remove(mi);
        e.state.players[1].battlefield.push(oid);
        e.state
            .objects
            .get_mut(&oid)
            .expect("p1 seeded mountain")
            .zone = tricerules_core::Zone::Battlefield;
    }
    let bolt_p1 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(0, &pass()).expect("p0 pass to NAP");
    give_mana(
        &mut e,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    e.apply_command(1, &cast_spell(bolt_p1, target_player(0)))
        .expect("p1 bolt on top of stack");

    assert_eq!(
        e.state
            .stack
            .iter()
            .map(|s| s.card_id.as_str())
            .collect::<Vec<_>>(),
        vec!["lightning_bolt", "lightning_bolt", "lightning_bolt"],
        "bottom-to-top: AP bolt, AP bolt, NAP bolt"
    );
    assert_eq!(e.state.priority_player_id(), 1);

    // Do not pass here alone: with `passes_since_stack_change == 0`, a lone NAP pass would
    // leave `passes_since == 1` and the next AP pass would resolve the top spell mid–`pass_both_players`.
    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.players[0].life, 17,
        "NAP bolt resolves first (3 to P0)"
    );
    assert_eq!(
        e.state.players[1].life, 14,
        "then both AP bolts (6 total to P1)"
    );
}

/// Five `Lightning Bolt`s on one stack (AP stacks three, passes; NAP stacks two). Covers the
/// Cockatrice/Servatrice case where resolved NAP spells must move from the canonical stack zone
/// (lowest player id) into the caster's graveyard — engine-only regression for LIFO + zone state.
#[test]
fn five_lightning_bolts_combined_stack_resolves_lifo_two_players() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(4405, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let m0a = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(m0a))
        .expect("p0 play first mountain");
    for _ in 0..2 {
        let mi = hand_index_for_card(&e, 0, "mountain");
        let oid = e.state.players[0].hand.remove(mi);
        e.state.players[0].battlefield.push(oid);
        e.state
            .objects
            .get_mut(&oid)
            .expect("p0 seeded mountain")
            .zone = tricerules_core::Zone::Battlefield;
    }

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let b0 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(b0, target_player(1)))
        .expect("p0 first bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let b1 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(b1, target_player(1)))
        .expect("p0 second bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let b2 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(b2, target_player(1)))
        .expect("p0 third bolt");
    assert_eq!(
        e.state.stack.len(),
        3,
        "AP should stack three bolts before passing"
    );
    assert_eq!(e.state.priority_player_id(), 0);

    e.apply_command(0, &pass())
        .expect("AP pass — priority to NAP");

    for _ in 0..2 {
        let mi = hand_index_for_card(&e, 1, "mountain");
        let oid = e.state.players[1].hand.remove(mi);
        e.state.players[1].battlefield.push(oid);
        e.state
            .objects
            .get_mut(&oid)
            .expect("p1 seeded mountain")
            .zone = tricerules_core::Zone::Battlefield;
    }
    give_mana(
        &mut e,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let b3 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b3, target_player(0)))
        .expect("p1 first bolt");
    give_mana(
        &mut e,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let b4 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b4, target_player(0)))
        .expect("p1 second bolt while holding priority");

    assert_eq!(
        e.state.stack.len(),
        5,
        "combined stack: three from AP (bottom) then two from NAP (top)"
    );
    assert_eq!(
        e.state
            .stack
            .iter()
            .map(|s| s.card_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "lightning_bolt",
            "lightning_bolt",
            "lightning_bolt",
            "lightning_bolt",
            "lightning_bolt"
        ]
    );
    assert_eq!(e.state.priority_player_id(), 1);

    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.players[0].life, 14,
        "NAP's two bolts resolve first (6 to P0)"
    );
    assert_eq!(
        e.state.players[1].life, 11,
        "then AP's three bolts (9 to P1)"
    );
    assert_eq!(
        count_card_id_in_graveyard(&e, 0, "lightning_bolt"),
        3,
        "AP's three bolts in AP graveyard"
    );
    assert_eq!(
        count_card_id_in_graveyard(&e, 1, "lightning_bolt"),
        2,
        "NAP's two bolts in NAP graveyard"
    );
}

/// NAP casts two bolts in a row while holding priority in response to AP's bolt.
#[test]
fn non_active_holds_priority_two_bolts_on_stack_above_active_bolt() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(4402, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let m0 = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(m0))
        .expect("p0 play mountain");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_ap = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_ap, target_player(1)))
        .expect("AP bolt targeting P1");
    e.apply_command(0, &pass())
        .expect("AP pass — priority to P1");

    for _ in 0..2 {
        let mi = hand_index_for_card(&e, 1, "mountain");
        let oid = e.state.players[1].hand.remove(mi);
        e.state.players[1].battlefield.push(oid);
        e.state
            .objects
            .get_mut(&oid)
            .expect("p1 seeded mountain")
            .zone = tricerules_core::Zone::Battlefield;
    }

    give_mana(
        &mut e,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let b1 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b1, target_player(0)))
        .expect("NAP first bolt");
    assert_eq!(e.state.priority_player_id(), 1);
    give_mana(
        &mut e,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let b2 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b2, target_player(0)))
        .expect("NAP second bolt while holding priority");
    assert_eq!(e.state.stack.len(), 3);

    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.players[0].life, 14,
        "two NAP bolts resolve before AP's"
    );
    assert_eq!(e.state.players[1].life, 17, "AP bolt still resolves last");
}

/// AP stacks two bolts, passes; NAP counters the top (second) bolt so only the first resolves.
#[test]
fn counterspell_on_top_bolt_fizzles_second_leaves_bottom_bolt() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "island".into(),
            "island".into(),
            "counterspell".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(4403, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let m0a = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(m0a))
        .expect("p0 play mountain");
    let m0b = hand_index_for_card(&e, 0, "mountain");
    let m0b_oid = e.state.players[0].hand.remove(m0b);
    e.state.players[0].battlefield.push(m0b_oid);
    e.state
        .objects
        .get_mut(&m0b_oid)
        .expect("p0 second mountain")
        .zone = tricerules_core::Zone::Battlefield;

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_bottom = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_bottom, target_player(1)))
        .expect("first bolt (stack bottom)");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_top = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_top, target_player(1)))
        .expect("second bolt while holding priority (stack top before counter)");
    let top_bolt_oid = e.state.stack.last().expect("top bolt").id;
    e.apply_command(0, &pass()).expect("AP pass");

    for _ in 0..2 {
        let ii = hand_index_for_card(&e, 1, "island");
        let oid = e.state.players[1].hand.remove(ii);
        e.state.players[1].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("p1 island").zone = tricerules_core::Zone::Battlefield;
    }
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let cs_idx = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(
        1,
        &cast_spell(
            cs_idx,
            vec![TargetRef {
                object_id: top_bolt_oid,
            }],
        ),
    )
    .expect("counterspell targets AP's second bolt");

    assert_eq!(
        e.state.stack.len(),
        3,
        "bottom bolt, top bolt, counterspell"
    );

    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.players[1].life, 17,
        "only the uncountered first bolt deals 3 damage"
    );
    assert_eq!(e.state.players[0].life, 20);
}

// ---------------------------------------------------------------------------
// CR 707.10 — copying a spell on the stack (Twincast)
// ---------------------------------------------------------------------------

/// Happy path: P0 bolts P1; P1 casts Twincast at the bolt. Twincast resolves, putting a *copy*
/// of the bolt on the stack (controlled by P1, retaining the original's target — P1). The copy
/// resolves first (3 to P1), then the original bolt (3 to P1): P1 goes 20 → 14. The copy is a
/// virtual stack item (CR 707.10): it has no backing object, leaves nothing in any graveyard,
/// and its StackPushed is flagged `is_copy`.
#[test]
fn twincast_copies_bolt_both_deal_damage() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "island".into(),
            "twincast".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(144, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("p0 play mountain");

    // Give P1 two untapped islands on the battlefield to pay {U}{U}.
    for _ in 0..2 {
        let island_idx = hand_index_for_card(&e, 1, "island");
        let island_oid = e.state.players[1].hand.remove(island_idx);
        e.state.players[1].battlefield.push(island_oid);
        e.state
            .objects
            .get_mut(&island_oid)
            .expect("seeded island")
            .zone = tricerules_core::Zone::Battlefield;
    }

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("p0 cast bolt at p1");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;
    e.apply_command(0, &pass()).expect("p0 pass priority to p1");

    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let twincast_idx = hand_index_for_card(&e, 1, "twincast");
    let cast = e
        .apply_command(
            1,
            &cast_spell(
                twincast_idx,
                vec![TargetRef {
                    object_id: bolt_oid,
                }],
            ),
        )
        .expect("p1 cast twincast at the bolt");
    // Twincast on the stack is a normal (non-copy) spell.
    let tw_push = cast
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) if s.card_id == "twincast" => Some(s),
            _ => None,
        })
        .expect("twincast stack push");
    assert!(!tw_push.is_copy, "the cast Twincast is not itself a copy");
    assert_eq!(e.state.stack.len(), 2, "bolt + twincast on stack");

    // Both pass: Twincast resolves and emits ResolutionChoiceRequired (CR 707.10c: copy
    // controller must choose new targets before the copy lands on the stack).
    e.apply_command(1, &pass()).expect("p1 pass after twincast");
    let resolve = e
        .apply_command(0, &pass())
        .expect("p0 pass resolves twincast");

    // No StackPushed yet — copy is parked pending target choice.
    assert!(
        resolve
            .events
            .iter()
            .all(|ev| !matches!(&ev.ev, Some(Ev::StackPushed(s)) if s.is_copy)),
        "copy is not on the stack until targets are chosen"
    );
    let rcr = find_resolution_choice(&resolve).expect("ResolutionChoiceRequired emitted");
    assert_eq!(
        rcr.deciding_player_id, 1,
        "P1 (Twincast controller) chooses"
    );
    assert!(
        rcr.candidate_object_ids.contains(&1u32),
        "P1 (object id 1) is a valid target for the copy"
    );
    // Stack still just has the original bolt; copy not yet pushed.
    assert_eq!(e.state.stack.len(), 1, "only the original bolt on stack");

    // P1 submits target choice: keep original target (P1 = player id 1).
    let target_batch = e
        .apply_command(1, &submit_resolution_choice(vec![1]))
        .expect("P1 submits copy target");

    let copy_push = target_batch
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) if s.is_copy => Some(s),
            _ => None,
        })
        .expect("copy stack push emitted after target choice");
    assert_eq!(copy_push.card_id, "lightning_bolt", "copy is of the bolt");
    assert_eq!(copy_push.ability_annotation, "(copy)");
    assert_eq!(
        copy_push.targets,
        vec![TargetRef { object_id: 1 }],
        "copy has the chosen target (P1)"
    );

    assert_eq!(e.state.stack.len(), 2, "original bolt + its copy on stack");
    let copy_item = e.state.stack.last().expect("copy on top");
    assert!(copy_item.is_copy, "top of stack is the copy");
    assert_eq!(
        copy_item.controller, 1,
        "copy controlled by Twincast's caster"
    );
    assert!(
        !e.state.objects.contains_key(&copy_item.id),
        "a copy has no backing GameObject (CR 707.10)"
    );

    // Copy resolves: 3 damage to P1, and it ceases to exist (no graveyard card).
    let copy_id = copy_item.id;
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass resolves copy");
    assert_eq!(e.state.players[1].life, 17, "copy dealt 3 to P1");
    assert!(
        !e.state.players[0].graveyard.contains(&copy_id)
            && !e.state.players[1].graveyard.contains(&copy_id),
        "the copy left no card in any graveyard"
    );

    // Original bolt resolves: another 3 to P1, and the real card goes to P0's graveyard.
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass resolves bolt");
    assert!(e.state.stack.is_empty(), "stack empty");
    assert_eq!(e.state.players[1].life, 14, "bolt dealt the remaining 3");
    assert!(
        e.state.players[0].graveyard.contains(&bolt_oid),
        "the real bolt card is in its owner's graveyard"
    );
}

/// Illegal path: Twincast must target a spell on the stack. Targeting a battlefield permanent
/// (a creature) is rejected at cast, leaving game state untouched.
#[test]
fn twincast_rejects_non_spell_target() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "twincast".into(),
            "grizzly_bears".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(77, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Put a creature on P0's battlefield to (illegally) target.
    let bears_idx = hand_index_for_card(&e, 0, "grizzly_bears");
    let bears_oid = e.state.players[0].hand.remove(bears_idx);
    e.state.players[0].battlefield.push(bears_oid);
    e.state
        .objects
        .get_mut(&bears_oid)
        .expect("seeded bears")
        .zone = tricerules_core::Zone::Battlefield;

    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let twincast_idx = hand_index_for_card(&e, 0, "twincast");
    let before = e.state.stack.len();
    let err = e.apply_command(
        0,
        &cast_spell(
            twincast_idx,
            vec![TargetRef {
                object_id: bears_oid,
            }],
        ),
    );
    assert!(err.is_err(), "targeting a permanent must be rejected");
    assert_eq!(e.state.stack.len(), before, "no spell put on the stack");
    assert!(
        e.state.players[0]
            .hand
            .iter()
            .any(|&o| e.state.objects[&o].card_id == "twincast"),
        "Twincast stays in hand after the illegal cast"
    );
}

/// Regression (CR 707.10d): a copy of a spell has no backing card, so countering it must simply
/// remove the copy from the stack — not try to move a nonexistent `GameObject` (which errored and
/// left the already-popped stack inconsistent). Here P1's Twincast copies P0's bolt, then P0
/// counters the copy.
#[test]
fn countering_a_spell_copy_removes_it_without_error() {
    let decks = Some(vec![
        vec![
            "lightning_bolt".into(),
            "counterspell".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "twincast".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(144, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // P0 casts Lightning Bolt at P1.
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("p0 cast bolt");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;
    e.apply_command(0, &pass()).expect("p0 pass to p1");

    // P1 casts Twincast at the bolt; both pass; Twincast resolves and prompts P1 for new targets.
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let twincast_idx = hand_index_for_card(&e, 1, "twincast");
    e.apply_command(
        1,
        &cast_spell(
            twincast_idx,
            vec![TargetRef {
                object_id: bolt_oid,
            }],
        ),
    )
    .expect("p1 cast twincast at the bolt");
    e.apply_command(1, &pass()).expect("p1 pass after twincast");
    e.apply_command(0, &pass())
        .expect("p0 pass resolves twincast");

    // P1 must choose targets for the copy (CR 707.10c) before it lands on the stack.
    assert!(
        e.state.pending_resolution.is_some(),
        "copy target choice is pending"
    );
    // P1 keeps the original target (P1 = player 1).
    e.apply_command(1, &submit_resolution_choice(vec![1]))
        .expect("P1 submits copy target");

    let copy_item = e.state.stack.last().expect("copy on stack");
    assert!(copy_item.is_copy, "top of stack is the bolt copy");
    let copy_id = copy_item.id;

    // P0 holds priority after the copy lands — counter the copy.
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let counter_idx = hand_index_for_card(&e, 0, "counterspell");
    e.apply_command(
        0,
        &cast_spell(counter_idx, vec![TargetRef { object_id: copy_id }]),
    )
    .expect("p0 counter the copy");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass resolves counterspell (must not error on the copy)");

    assert!(
        !e.state.stack.iter().any(|s| s.id == copy_id),
        "the copy is gone from the stack"
    );
    assert!(
        !e.state.players[0].graveyard.contains(&copy_id)
            && !e.state.players[1].graveyard.contains(&copy_id),
        "a countered copy leaves no card in any graveyard (CR 707.10d)"
    );
    assert!(
        e.state.stack.iter().any(|s| s.id == bolt_oid),
        "the original bolt is untouched on the stack"
    );
}

/// CR 707.10c: Twincast's controller may choose *different* targets for the copy. P0 bolts P1;
/// P1 casts Twincast. When Twincast resolves, P1 redirects the copy to target P0 instead. The
/// copy deals 3 to P0 and the original bolt deals 3 to P1 → each player takes 3.
#[test]
fn twincast_copy_controller_chooses_new_target() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "island".into(),
            "twincast".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(144, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("p0 play mountain");

    for _ in 0..2 {
        let island_idx = hand_index_for_card(&e, 1, "island");
        let island_oid = e.state.players[1].hand.remove(island_idx);
        e.state.players[1].battlefield.push(island_oid);
        e.state.objects.get_mut(&island_oid).unwrap().zone = tricerules_core::Zone::Battlefield;
    }

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    // P0 bolts P1 (player id 1).
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("p0 cast bolt at p1");
    let bolt_oid = e.state.stack.last().unwrap().id;
    e.apply_command(0, &pass()).expect("p0 pass");

    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let twincast_idx = hand_index_for_card(&e, 1, "twincast");
    e.apply_command(
        1,
        &cast_spell(
            twincast_idx,
            vec![TargetRef {
                object_id: bolt_oid,
            }],
        ),
    )
    .expect("p1 cast twincast");
    e.apply_command(1, &pass()).expect("p1 pass");
    let resolve = e
        .apply_command(0, &pass())
        .expect("p0 pass resolves twincast");

    // ResolutionChoiceRequired must be emitted (CR 707.10c).
    let rcr = find_resolution_choice(&resolve).expect("copy target choice required");
    assert_eq!(rcr.deciding_player_id, 1, "P1 chooses targets for the copy");
    // Both players should be valid targets for Lightning Bolt.
    assert!(
        rcr.candidate_object_ids.contains(&0u32),
        "P0 is a valid target"
    );
    assert!(
        rcr.candidate_object_ids.contains(&1u32),
        "P1 is a valid target"
    );

    // P1 redirects the copy to hit P0 instead.
    let target_batch = e
        .apply_command(1, &submit_resolution_choice(vec![0]))
        .expect("P1 chooses P0 as new target");

    let copy_push = target_batch
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) if s.is_copy => Some(s),
            _ => None,
        })
        .expect("copy StackPushed after target choice");
    assert_eq!(
        copy_push.targets,
        vec![TargetRef { object_id: 0 }],
        "copy targets P0"
    );

    // Copy resolves first: 3 to P0.
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass resolves copy");
    assert_eq!(e.state.players[0].life, 17, "copy dealt 3 to P0");
    assert_eq!(e.state.players[1].life, 20, "P1 untouched so far");

    // Original bolt resolves: 3 to P1.
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass resolves original bolt");
    assert_eq!(e.state.players[0].life, 17, "P0 at 17");
    assert_eq!(e.state.players[1].life, 17, "bolt dealt 3 to P1");
    assert!(e.state.stack.is_empty(), "stack empty");
    assert!(
        e.state.players[0].graveyard.contains(&bolt_oid),
        "real bolt in P0's graveyard"
    );
}
