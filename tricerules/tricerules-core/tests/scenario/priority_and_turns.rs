use crate::helpers::*;

#[test]
fn primitive_yield_active_skips_double_pass_main1() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None, true).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    e.apply_command(0, &primitive_yield())
        .expect("active primitive");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
}

#[test]
fn empty_stack_double_pass_emits_ap_priority_in_new_phase() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None, true).expect("new");
    e.apply_command(0, &pass()).expect("p0 pass");
    let b = e.apply_command(1, &pass()).expect("p1 pass");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
    assert!(
        priority_changes_in(&b).contains(&0),
        "after phase advance, active player should explicitly regain priority"
    );
}

#[test]
fn mana_pools_empty_on_step_change() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None, true).expect("new");
    e.state.players[0].mana_pool.red = 2;
    e.state.players[1].mana_pool.green = 1;

    e.apply_command(0, &primitive_yield())
        .expect("active primitive");

    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
    assert_eq!(e.state.players[0].mana_pool.red, 0);
    assert_eq!(e.state.players[0].mana_pool.green, 0);
    assert_eq!(e.state.players[0].mana_pool.blue, 0);
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    assert_eq!(e.state.players[1].mana_pool.red, 0);
    assert_eq!(e.state.players[1].mana_pool.green, 0);
    assert_eq!(e.state.players[1].mana_pool.blue, 0);
    assert_eq!(e.state.players[1].mana_pool.colorless, 0);
}

#[test]
fn stack_resolution_emits_priority_to_active_player() {
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
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(13, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
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
    e.apply_command(0, &pass()).expect("caster pass");
    let resolved = e.apply_command(1, &pass()).expect("opponent pass");
    assert!(
        priority_changes_in(&resolved).contains(&0),
        "active player should regain priority after stack resolves"
    );
}

#[test]
fn cleanup_batch_discard_three_at_once() {
    let mut e = GameEngine::new(1002, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let ap_idx = e.state.player_idx(0).unwrap();
    for _ in 0..3 {
        let oid = e.state.players[ap_idx]
            .library
            .pop_front()
            .expect("library");
        e.state.players[ap_idx].hand.push(oid);
        e.state.objects.get_mut(&oid).expect("obj").zone = tricerules_core::Zone::Hand;
    }
    assert_eq!(e.state.players[ap_idx].hand.len(), 10);

    e.apply_command(0, &primitive_yield())
        .expect("main1->begin combat");
    // No eligible attackers: BeginCombat auto-skips to EndCombat.
    e.apply_command(0, &primitive_yield())
        .expect("begin combat->end combat");
    e.apply_command(0, &primitive_yield())
        .expect("end combat->main2");
    e.apply_command(0, &primitive_yield())
        .expect("main2->end step");
    e.apply_command(0, &primitive_yield())
        .expect("end step->cleanup");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Cleanup);

    e.apply_command(0, &discard_cleanup_batch(vec![9, 8, 7]))
        .expect("batch discard top three");
    assert_eq!(e.state.players[ap_idx].hand.len(), 7);
    assert_eq!(e.state.active_player_id(), 1);
}

#[test]
fn cleanup_step_opens_when_hand_exceeds_max_and_discard_finishes_turn() {
    let mut e = GameEngine::new(1001, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let ap_idx = e.state.player_idx(0).unwrap();
    let oid = e.state.players[ap_idx]
        .library
        .pop_front()
        .expect("library");
    e.state.players[ap_idx].hand.push(oid);
    e.state.objects.get_mut(&oid).expect("obj").zone = tricerules_core::Zone::Hand;
    assert!(e.state.players[ap_idx].hand.len() > 7);

    e.apply_command(0, &primitive_yield())
        .expect("main1->begin combat");
    // No eligible attackers: BeginCombat auto-skips to EndCombat.
    e.apply_command(0, &primitive_yield())
        .expect("begin combat->end combat");
    e.apply_command(0, &primitive_yield())
        .expect("end combat->main2");
    e.apply_command(0, &primitive_yield())
        .expect("main2->end step");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndStep);

    e.apply_command(0, &primitive_yield())
        .expect("end step->cleanup");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Cleanup);
    assert_eq!(e.state.cleanup_discard_player, Some(0));

    e.apply_command(0, &discard_cleanup(0))
        .expect("discard one");
    assert_eq!(e.state.players[ap_idx].hand.len(), 7);
    assert_eq!(e.state.active_player_id(), 1);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
}

#[test]
fn main2_double_pass_advances_to_end_step_stop() {
    let mut e = GameEngine::new(69, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    // No eligible attackers: BeginCombat auto-skips to EndCombat in one yield.
    e.apply_command(0, &primitive_yield())
        .expect("begin combat to end combat");
    e.apply_command(0, &primitive_yield())
        .expect("end combat to main2");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main2);
    e.apply_command(0, &pass()).expect("ap pass main2");
    let b = e.apply_command(1, &pass()).expect("nap pass main2");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndStep);
    assert!(
        priority_changes_in(&b).contains(&0),
        "end step should open a priority window for active player"
    );
}

#[test]
fn new_turn_stops_at_upkeep_then_draw_then_main1() {
    let mut e = GameEngine::new(70, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    end_active_turn(&mut e, 0);
    assert_eq!(e.state.active_player_id(), 1);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    e.apply_command(1, &pass()).expect("ap pass upkeep");
    let to_draw = e.apply_command(0, &pass()).expect("nap pass upkeep");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
    assert!(
        priority_changes_in(&to_draw).contains(&1),
        "draw step should open priority for the active player"
    );
    e.apply_command(1, &pass()).expect("ap pass draw");
    let to_main = e.apply_command(0, &pass()).expect("nap pass draw");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    assert!(
        priority_changes_in(&to_main).contains(&1),
        "main1 should open priority for the active player"
    );
}

/// CR 103.8: only the starting player skips their first draw. The duel `turn` counter can remain 1
/// for the second seat's first turn (it bumps when active wraps to seat 0), so skip logic must
/// key off who started, not `turn == 1` alone.
#[test]
fn second_seat_first_draw_draws_when_seat_zero_started() {
    let mut e = GameEngine::new(71, &[0, 1], 20, None, true).expect("new");
    assert_eq!(e.state.starting_player_idx, 0);
    advance_to_main1_from_game_start(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        7,
        "starting seat skipped first draw"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(e.state.active_player_id(), 1);
    assert_eq!(e.state.turn, 1);
    e.apply_command(1, &pass()).expect("ap pass upkeep");
    e.apply_command(0, &pass()).expect("nap pass upkeep");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
    assert_eq!(
        e.state.players[1].hand.len(),
        8,
        "second seat must draw on their first draw step"
    );
}

#[test]
fn untap_and_draw_happen_in_new_turn_sequence() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
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
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(88, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let hand_before_turn = e.state.players[0].hand.len();
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");

    let mountain_oid = battlefield_object_for_card(&e, 0, "mountain");
    // Tap the mountain to produce mana (simulating the client tapping land before casting).
    e.state
        .objects
        .get_mut(&mountain_oid)
        .expect("mountain object")
        .tapped = true;
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
        .expect("cast lightning bolt");
    e.apply_command(0, &pass()).expect("caster pass");
    e.apply_command(1, &pass())
        .expect("opponent pass to resolve");

    assert!(
        e.state
            .objects
            .get(&mountain_oid)
            .expect("mountain object")
            .tapped,
        "mountain is tapped after paying for bolt"
    );

    end_active_turn(&mut e, 0); // now active player 1, upkeep
    pass_both_players(&mut e); // upkeep -> draw
    pass_both_players(&mut e); // draw -> main1
    e.apply_command(1, &primitive_yield())
        .expect("p1 main1 to begin combat");
    // No eligible attackers: BeginCombat auto-skips to EndCombat on both-player pass.
    pass_both_players(&mut e); // begin combat -> end combat
    pass_both_players(&mut e); // end combat -> main2
    pass_both_players(&mut e); // main2 -> end step
    pass_both_players(&mut e); // end step -> cleanup or p0 upkeep
    resolve_cleanup_discards_if_any(&mut e);
    pass_both_players(&mut e); // upkeep -> draw
    pass_both_players(&mut e); // draw -> main1

    assert_eq!(e.state.active_player_id(), 0);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    assert!(
        !e.state
            .objects
            .get(&mountain_oid)
            .expect("mountain object")
            .tapped,
        "mountain untaps during the active player's untap phase"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_turn - 1,
        "player drew one card during draw phase after spending two cards"
    );
}

#[test]
fn zone_view_includes_battlefield_object_ids() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(404, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bears = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // ZoneViewSync is emitted as part of every batch via apply_command's tail.
    let b = e.apply_command(0, &pass()).expect("ap pass main1");
    let zone_view = b
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view in batch");
    let p0 = zone_view
        .per_player
        .iter()
        .find(|p| p.player_id == 0)
        .expect("p0 view");
    assert_eq!(p0.battlefield_object_id.len(), p0.battlefield.len());
    assert_eq!(p0.battlefield_power.len(), p0.battlefield.len());
    assert_eq!(p0.battlefield_toughness.len(), p0.battlefield.len());
    assert_eq!(p0.battlefield_damage.len(), p0.battlefield.len());
    assert_eq!(p0.battlefield_is_creature.len(), p0.battlefield.len());
    assert_eq!(p0.hand_object_id.len(), p0.hand.len());
    let pos = p0
        .battlefield
        .iter()
        .position(|c| c == "grizzly_bears")
        .expect("bears in view");
    assert_eq!(p0.battlefield_object_id[pos], bears);
    assert!(p0.battlefield_is_creature[pos]);
    assert_eq!(p0.battlefield_power[pos], 2);
    assert_eq!(p0.battlefield_toughness[pos], 2);
    assert_eq!(p0.battlefield_damage[pos], 0);
}

#[test]
fn second_sorcery_rejected_while_spell_on_stack_even_with_priority() {
    let p0_deck: Vec<String> = std::iter::repeat_n("island".into(), 25)
        .chain(std::iter::repeat_n("divination".into(), 5))
        .collect();
    let decks = Some(vec![p0_deck, vec!["forest".into(); 15]]);
    let mut e = GameEngine::new(904, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    while e.state.players[0]
        .hand
        .iter()
        .filter(|oid| e.state.objects.get(*oid).map(|o| o.card_id.as_str()) == Some("divination"))
        .count()
        < 2
    {
        take_card_from_library_to_hand(&mut e, 0, "divination");
    }
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let div0 = hand_index_for_card(&e, 0, "divination");
    e.apply_command(0, &cast_spell(div0, vec![]))
        .expect("first divination");
    assert_eq!(
        e.state.stack.len(),
        1,
        "first sorcery should sit on the stack while AP still has priority"
    );

    let div1 = hand_index_for_card(&e, 0, "divination");
    let err = e
        .apply_command(0, &cast_spell(div1, vec![]))
        .expect_err("second sorcery with stack nonempty");
    assert!(
        err.to_string().contains("sorcery speed"),
        "unexpected: {err}"
    );
}

/// Regression: flying/reach changes must not affect normal ground-vs-ground blocking.
#[test]
fn ground_creature_still_blockable_by_ground_blocker_regression() {
    let mut e = GameEngine::new(9005, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let merfolk = inject_creature_on_battlefield(&mut e, 0, "coral_merfolk");
    let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![merfolk]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: merfolk,
            blocker_id: bears,
        }]),
    )
    .expect("ground creature must still be able to block a ground attacker");
}

/// P2 color filter: Doom Blade ("Destroy target nonblack creature") accepts a nonblack target
/// and rejects a black one.
#[test]
fn doom_blade_targets_only_nonblack_creatures() {
    let mut e = anthem_engine(5007, "doom_blade");
    let green = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let black = inject_creature_on_battlefield(&mut e, 1, "acolyte_of_xathrid");

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "doom_blade");
    // Black creature is an illegal target (nonblack restriction).
    assert!(
        e.apply_command(
            0,
            &cast_spell(
                idx,
                vec![TargetRef {
                    object_id: black,
                    damage_amount: 0
                }]
            )
        )
        .is_err(),
        "Doom Blade cannot target a black creature"
    );
    // Nonblack creature is legal; it resolves and dies.
    let idx = hand_index_for_card(&e, 0, "doom_blade");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: green,
                damage_amount: 0,
            }],
        ),
    )
    .expect("Doom Blade targets a nonblack creature");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.objects.get(&green).map(|o| o.zone) != Some(tricerules_core::Zone::Battlefield),
        "the nonblack creature is destroyed"
    );
}

/// P2 counter spell-type filter: Essence Scatter counters a creature spell but cannot target a
/// noncreature spell; Negate is the mirror image.
#[test]
fn essence_scatter_and_negate_respect_spell_type() {
    let decks = Some(vec![
        vec![
            "grizzly_bears".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "essence_scatter".into(),
            "essence_scatter".into(),
            "negate".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(5009, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // P0 casts a creature spell (grizzly_bears) — it sits on the stack.
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let gb_idx = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(gb_idx, vec![]))
        .expect("cast bears");
    let bears_spell = e.state.stack.last().expect("bears on stack").id;
    e.apply_command(0, &pass())
        .expect("p0 pass to give p1 priority");

    // Negate (noncreature only) cannot target a creature spell (illegal cast — card not consumed).
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 6,
            ..Default::default()
        },
    );
    let neg_idx = hand_index_for_card(&e, 1, "negate");
    assert!(
        e.apply_command(
            1,
            &cast_spell(
                neg_idx,
                vec![TargetRef {
                    object_id: bears_spell,
                    damage_amount: 0,
                }]
            )
        )
        .is_err(),
        "Negate cannot counter a creature spell"
    );
    // Essence Scatter (creature only) can.
    let es_idx = hand_index_for_card(&e, 1, "essence_scatter");
    e.apply_command(
        1,
        &cast_spell(
            es_idx,
            vec![TargetRef {
                object_id: bears_spell,
                damage_amount: 0,
            }],
        ),
    )
    .expect("Essence Scatter counters a creature spell");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        count_card_id_in_graveyard(&e, 0, "grizzly_bears") == 1,
        "the creature spell is countered into its owner's graveyard"
    );

    // Now P0 casts a noncreature spell (lightning_bolt) and P1's Essence Scatter cannot hit it.
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
    let bolt_spell = e.state.stack.last().expect("bolt on stack").id;
    e.apply_command(0, &pass())
        .expect("p0 pass to give p1 priority");

    let es2 = hand_index_for_card(&e, 1, "essence_scatter");
    assert!(
        e.apply_command(
            1,
            &cast_spell(
                es2,
                vec![TargetRef {
                    object_id: bolt_spell,
                    damage_amount: 0,
                }]
            )
        )
        .is_err(),
        "Essence Scatter cannot counter a noncreature spell"
    );
    let neg2 = hand_index_for_card(&e, 1, "negate");
    e.apply_command(
        1,
        &cast_spell(
            neg2,
            vec![TargetRef {
                object_id: bolt_spell,
                damage_amount: 0,
            }],
        ),
    )
    .expect("Negate counters a noncreature spell");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        count_card_id_in_graveyard(&e, 0, "lightning_bolt") == 1,
        "the noncreature spell is countered"
    );
}
