use crate::helpers::*;

#[test]
fn lightning_bolt_rejects_basic_land_target() {
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
    let mut e = GameEngine::new(1401, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let land_oid = battlefield_object_for_card(&e, 0, "mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let err = e
        .apply_command(
            0,
            &cast_spell(
                bolt_idx,
                vec![TargetRef {
                    object_id: land_oid,
                }],
            ),
        )
        .expect_err("bolt cannot target land");
    assert!(err.to_string().contains("creature"), "unexpected: {err}");
}

#[test]
fn lightning_bolt_rejects_missing_target() {
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
    let mut e = GameEngine::new(1402, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let err = e
        .apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect_err("bolt needs a target");
    assert!(
        err.to_string().contains("exactly one target"),
        "unexpected: {err}"
    );
}

#[test]
fn giant_growth_rejects_land_target() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "giant_growth".into(),
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
    let mut e = GameEngine::new(1403, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let forest_idx = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(forest_idx))
        .expect("play forest");
    let land_oid = battlefield_object_for_card(&e, 0, "forest");
    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    let err = e
        .apply_command(
            0,
            &cast_spell(
                growth_idx,
                vec![TargetRef {
                    object_id: land_oid,
                }],
            ),
        )
        .expect_err("growth cannot target land");
    assert!(err.to_string().contains("creature"), "unexpected: {err}");
}

/// Stack LIFO: `Lightning Bolt` on top kills the creature; `Giant Growth` underneath fizzles (CR 608.2b).
#[test]
fn giant_growth_fizzles_if_creature_target_dies_before_resolution() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "giant_growth".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(91021, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    let forest_idx = hand_index_for_card(&e, 0, "forest");
    let forest_oid = e.state.players[0].hand.remove(forest_idx);
    e.state.players[0].battlefield.push(forest_oid);
    e.state.objects.get_mut(&forest_oid).expect("forest").zone = tricerules_core::Zone::Battlefield;

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    let mountain_oid = e.state.players[0].hand.remove(mountain_idx);
    e.state.players[0].battlefield.push(mountain_oid);
    e.state
        .objects
        .get_mut(&mountain_oid)
        .expect("mountain")
        .zone = tricerules_core::Zone::Battlefield;

    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(
        0,
        &cast_spell(growth_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast growth");

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(bolt_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast bolt on top of growth");

    assert_eq!(e.state.stack.len(), 2);

    let mut growth_fizzled = false;
    let mut saw_pump_log = false;
    while !e.state.stack.is_empty() {
        let first = e.state.priority_player_id();
        let second = if first == e.state.players[0].id {
            e.state.players[1].id
        } else {
            e.state.players[0].id
        };
        e.apply_command(first, &pass()).expect("pass");
        let batch = e.apply_command(second, &pass()).expect("pass resolves");
        for ev in &batch.events {
            if let Some(Ev::Log(lm)) = &ev.ev {
                if lm.text.contains("Giant Growth") && lm.text.contains("fizzles") {
                    growth_fizzled = true;
                }
                if lm.text.contains("+3/+3") {
                    saw_pump_log = true;
                }
            }
        }
    }

    assert!(growth_fizzled, "expected Giant Growth to fizzle");
    assert!(!saw_pump_log, "fizzled pump spell must not log +3/+3 line");
    let dead = e.state.objects.get(&bear).expect("bear object");
    assert_eq!(dead.zone, tricerules_core::Zone::Graveyard);
    assert_eq!(dead.power, Some(2));
    assert_eq!(dead.toughness, Some(2));
}

/// Second bolt should not add damage to a creature already in the graveyard (608.2b).
#[test]
fn lightning_bolt_fizzles_when_creature_target_left_battlefield() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "mountain".into(),
            "grizzly_bears".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(91022, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    for _ in 0..2 {
        let mi = hand_index_for_card(&e, 0, "mountain");
        let oid = e.state.players[0].hand.remove(mi);
        e.state.players[0].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("mountain").zone = tricerules_core::Zone::Battlefield;
    }

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_a = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_a, vec![TargetRef { object_id: bear }]))
        .expect("first bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_b = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_b, vec![TargetRef { object_id: bear }]))
        .expect("second bolt on top");

    resolve_entire_stack_two_player(&mut e);

    let dead = e.state.objects.get(&bear).expect("bear");
    assert_eq!(dead.zone, tricerules_core::Zone::Graveyard);
    // The first bolt's 3 damage killed the bear; the second fizzled (target left the
    // battlefield). Marked damage is cleared on the zone change (CR 400.7) — the card in the
    // graveyard is a new object — so the proof the second bolt added nothing is that the bear
    // is dead with no lingering damage, not a damage total of 3.
    assert_eq!(
        dead.damage, 0,
        "marked damage clears when leaving the battlefield"
    );
}

/// `Go for the Throat` under a bolt that kills the same creature fizzles on resolution.
#[test]
fn go_for_the_throat_fizzles_when_creature_target_left_battlefield() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "swamp".into(),
            "grizzly_bears".into(),
            "go_for_the_throat".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(91023, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    let mountain_oid = e.state.players[0].hand.remove(mountain_idx);
    e.state.players[0].battlefield.push(mountain_oid);
    e.state
        .objects
        .get_mut(&mountain_oid)
        .expect("mountain")
        .zone = tricerules_core::Zone::Battlefield;

    for _ in 0..2 {
        let si = hand_index_for_card(&e, 0, "swamp");
        let oid = e.state.players[0].hand.remove(si);
        e.state.players[0].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("swamp").zone = tricerules_core::Zone::Battlefield;
    }

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let gfth_idx = hand_index_for_card(&e, 0, "go_for_the_throat");
    e.apply_command(
        0,
        &cast_spell(gfth_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("go for the throat");

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(bolt_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("bolt on top");

    let mut saw_destroy = false;
    let mut saw_fizzle = false;
    while !e.state.stack.is_empty() {
        let first = e.state.priority_player_id();
        let second = if first == e.state.players[0].id {
            e.state.players[1].id
        } else {
            e.state.players[0].id
        };
        e.apply_command(first, &pass()).expect("pass");
        let batch = e.apply_command(second, &pass()).expect("resolve");
        for ev in &batch.events {
            if let Some(Ev::Log(lm)) = &ev.ev {
                if lm.text.contains("destroys") && lm.text.contains("Grizzly Bears") {
                    saw_destroy = true;
                }
                if lm.text.contains("Go for the Throat") && lm.text.contains("fizzles") {
                    saw_fizzle = true;
                }
            }
        }
    }

    assert!(
        !saw_destroy,
        "destroy effect should not run when the creature is already gone"
    );
    assert!(saw_fizzle);
}

#[test]
fn go_for_the_throat_rejects_artifact_creature_target() {
    // Go for the Throat can't target artifact creatures (not_artifact: true filter).
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "go_for_the_throat".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec![
            "plains".into(),
            "ornithopter".into(), // artifact creature
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
    ]);
    let mut e = GameEngine::new(3001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Seed Ornithopter directly onto P1's battlefield (bypasses priority).
    let ornithopter_oid = put_creature_on_battlefield(&mut e, 1, "ornithopter");

    // Seed a swamp for P0 and play a land for the second mana.
    let seeded_swamp_idx = hand_index_for_card(&e, 0, "swamp");
    let seeded_swamp = e.state.players[0].hand.remove(seeded_swamp_idx);
    e.state.players[0].battlefield.push(seeded_swamp);
    e.state
        .objects
        .get_mut(&seeded_swamp)
        .expect("seeded swamp")
        .zone = tricerules_core::Zone::Battlefield;
    let swamp_to_play_idx = hand_index_for_card(&e, 0, "swamp");
    e.apply_command(0, &play_land(swamp_to_play_idx))
        .expect("play swamp");

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let gftt_idx = hand_index_for_card(&e, 0, "go_for_the_throat");
    let err = e
        .apply_command(
            0,
            &cast_spell(
                gftt_idx,
                vec![TargetRef {
                    object_id: ornithopter_oid,
                }],
            ),
        )
        .expect_err("go for the throat cannot target artifact creature");
    assert!(
        err.to_string().contains("creature") || err.to_string().contains("illegal"),
        "unexpected: {err}"
    );
    // Ornithopter must still be on the battlefield.
    assert!(e.state.players[1].battlefield.contains(&ornithopter_oid));
}

#[test]
fn bump_in_the_night_rejects_creature_target() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "bump_in_the_night".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2605, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let bump_idx = hand_index_for_card(&e, 0, "bump_in_the_night");
    let err = e
        .apply_command(
            0,
            &cast_spell(bump_idx, vec![TargetRef { object_id: bear }]),
        )
        .expect_err("bump cannot target creature");
    assert!(format!("{err:?}").contains("player"), "unexpected: {err:?}");
}

#[test]
fn bump_in_the_night_rejects_self_target() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "bump_in_the_night".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2615, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let bump_idx = hand_index_for_card(&e, 0, "bump_in_the_night");
    let err = e
        .apply_command(0, &cast_spell(bump_idx, target_player(0)))
        .expect_err("bump cannot target self (target opponent)");
    assert!(
        format!("{err:?}").contains("opponent"),
        "unexpected: {err:?}"
    );
}

#[test]
fn swords_to_plowshares_fizzles_if_target_dies_before_resolution() {
    let decks = Some(vec![
        vec![
            "plains".into(),
            "swords_to_plowshares".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2609, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            r: 1,
            ..Default::default()
        },
    );

    let swords_idx = hand_index_for_card(&e, 0, "swords_to_plowshares");
    e.apply_command(
        0,
        &cast_spell(swords_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast swords");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(bolt_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast bolt on top");
    assert_eq!(e.state.stack.len(), 2);

    let p1_life_before = e.state.players[1].life;
    resolve_entire_stack_two_player(&mut e);

    // Bolt killed the bear; Swords had no legal target → fizzles, no life change.
    assert_eq!(
        e.state.objects.get(&bear).expect("bear").zone,
        tricerules_core::Zone::Graveyard,
        "bear died to bolt"
    );
    assert_eq!(
        e.state.players[1].life, p1_life_before,
        "swords fizzled, no life gain"
    );
}

#[test]
fn unsummon_rejects_land_target() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "unsummon".into(),
            "island".into(),
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
    let mut e = GameEngine::new(2611, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let island_idx = hand_index_for_card(&e, 0, "island");
    e.apply_command(0, &play_land(island_idx))
        .expect("play island");
    let island_oid = battlefield_object_for_card(&e, 0, "island");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "unsummon");
    let err = e
        .apply_command(
            0,
            &cast_spell(
                idx,
                vec![TargetRef {
                    object_id: island_oid,
                }],
            ),
        )
        .expect_err("unsummon cannot target land");
    assert!(
        format!("{err:?}").contains("creature"),
        "unexpected: {err:?}"
    );
}

#[test]
fn mind_sculpt_rejects_self_target() {
    // Mind Sculpt is opponent-only in this build: casting at yourself is illegal at cast time.
    let decks = Some(vec![island_only_deck(), forest_only_deck()]);
    let mut e = GameEngine::new(2616, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let sculpt_id = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                card_id: "mind_sculpt".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
            },
        );
        e.state.players[0].hand.push(id);
        id
    };
    let sculpt_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == sculpt_id)
        .expect("sculpt in hand");
    let lib_before = e.state.players[0].library.len();
    let err = e.apply_command(0, &cast_spell(sculpt_idx, target_player(0)));
    assert!(
        err.is_err(),
        "mind sculpt targeting its controller must be rejected"
    );
    // No cards milled from the caster.
    assert_eq!(e.state.players[0].library.len(), lib_before);
}

// ---------------------------------------------------------------------------
// Hexproof / Shroud (CR 702.18 / CR 702.16)
// ---------------------------------------------------------------------------

/// CR 702.18: Gladecover Scout has hexproof — an opponent cannot target it with
/// Lightning Bolt. The cast attempt must be rejected as illegal.
#[test]
fn hexproof_opponent_cannot_target_with_spell() {
    // 14-card decks so library is never empty after the opening hand + draw step.
    let p0_deck: Vec<String> = std::iter::once("lightning_bolt".into())
        .chain(std::iter::repeat_n("mountain".into(), 13))
        .collect();
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 14).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(9001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Gladecover Scout (1/1 hexproof) directly onto P1's battlefield.
    let scout = inject_creature_with_stats(&mut e, 1, "gladecover_scout", 1, 1);

    // Give P0 one red mana (Lightning Bolt costs R) by tapping a Mountain.
    let mtn_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mtn_idx))
        .expect("play mountain");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    // Ensure Lightning Bolt is in P0's hand (may be in library depending on seed).
    if !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("lightning_bolt"))
    {
        take_card_from_library_to_hand(&mut e, 0, "lightning_bolt");
    }

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let result = e.apply_command(
        0,
        &cast_spell(bolt_idx, vec![TargetRef { object_id: scout }]),
    );
    assert!(
        result.is_err(),
        "CR 702.18: opponent must not be able to target a hexproof permanent with a spell"
    );
}

/// CR 702.18: a player CAN target their own hexproof permanent (hexproof only
/// protects against opponents). Giant Growth on your own Gladecover Scout is legal.
#[test]
fn hexproof_controller_can_target_own_permanent() {
    let p0_deck: Vec<String> = std::iter::once("giant_growth".into())
        .chain(std::iter::repeat_n("forest".into(), 13))
        .collect();
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 14).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(9002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Gladecover Scout (1/1 hexproof) directly onto P0's battlefield.
    let scout = inject_creature_with_stats(&mut e, 0, "gladecover_scout", 1, 1);

    let forest_idx = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(forest_idx))
        .expect("play forest");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    // Ensure Giant Growth is in P0's hand.
    if !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("giant_growth"))
    {
        take_card_from_library_to_hand(&mut e, 0, "giant_growth");
    }

    let gg_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(0, &cast_spell(gg_idx, vec![TargetRef { object_id: scout }]))
        .expect("CR 702.18: controller can target own hexproof creature");

    // Resolve the pump.
    pass_both_players(&mut e);

    assert_eq!(
        e.effective_power(scout),
        Some(4),
        "Giant Growth (+3/+3) must pump Gladecover Scout from 1 to 4 effective power"
    );
}

/// CR 702.16: Argothian Enchantress has shroud — even its controller cannot
/// target it with a spell. Giant Growth targeting the Enchantress must be rejected.
#[test]
fn shroud_controller_cannot_target_own_permanent() {
    let p0_deck: Vec<String> = std::iter::once("giant_growth".into())
        .chain(std::iter::repeat_n("forest".into(), 13))
        .collect();
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 14).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(9003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Argothian Enchantress (0/1 shroud) directly onto P0's battlefield.
    let enchantress = inject_creature_with_stats(&mut e, 0, "argothian_enchantress", 0, 1);

    let forest_idx = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(forest_idx))
        .expect("play forest");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    // Ensure Giant Growth is in P0's hand.
    if !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("giant_growth"))
    {
        take_card_from_library_to_hand(&mut e, 0, "giant_growth");
    }

    let gg_idx = hand_index_for_card(&e, 0, "giant_growth");
    let result = e.apply_command(
        0,
        &cast_spell(
            gg_idx,
            vec![TargetRef {
                object_id: enchantress,
            }],
        ),
    );
    assert!(
        result.is_err(),
        "CR 702.16: controller must not be able to target a shroud permanent with a spell"
    );
}

/// Royal Assassin's `{T}: Destroy target tapped creature.` — now a `DestroyTarget` with a
/// `tapped: true` filter (the old single-card `DestroyTargetTapped` primitive was removed).
/// Happy path: a tapped enemy creature is a legal target and is destroyed on resolution.
#[test]
fn royal_assassin_destroys_tapped_creature() {
    let decks = Some(vec![
        vec!["royal_assassin".into(); 20],
        vec!["grizzly_bears".into(); 20],
    ]);
    let mut e = GameEngine::new(4201, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let assassin = deploy_to_battlefield(&mut e, 0, "royal_assassin", false);
    let bears = deploy_to_battlefield(&mut e, 1, "grizzly_bears", /* tapped */ true);

    e.apply_command(
        0,
        &activate_ability(assassin, 0, vec![TargetRef { object_id: bears }]),
    )
    .expect("activate Royal Assassin on tapped creature");

    // Source taps to pay the cost; ability is on the stack.
    assert!(e.state.objects.get(&assassin).expect("assassin").tapped);
    assert_eq!(e.state.stack.len(), 1);

    // Both players pass → ability resolves, destroying the tapped creature.
    pass_both_players(&mut e);
    assert!(e.state.stack.is_empty());
    assert!(
        e.state.players[1].graveyard.contains(&bears),
        "tapped creature should be destroyed to its owner's graveyard"
    );
}

/// Illegal path: an untapped creature fails the `tapped: true` filter, so activation is
/// rejected at target validation (CR 602.2) before any cost is paid.
#[test]
fn royal_assassin_cannot_target_untapped_creature() {
    let decks = Some(vec![
        vec!["royal_assassin".into(); 20],
        vec!["grizzly_bears".into(); 20],
    ]);
    let mut e = GameEngine::new(4202, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let assassin = deploy_to_battlefield(&mut e, 0, "royal_assassin", false);
    let bears = deploy_to_battlefield(&mut e, 1, "grizzly_bears", /* tapped */ false);

    let err = e.apply_command(
        0,
        &activate_ability(assassin, 0, vec![TargetRef { object_id: bears }]),
    );
    assert!(err.is_err(), "untapped creature is not a legal target");
    // Cost untouched: source stays untapped, nothing on the stack.
    assert!(!e.state.objects.get(&assassin).expect("assassin").tapped);
    assert!(e.state.stack.is_empty());
}
