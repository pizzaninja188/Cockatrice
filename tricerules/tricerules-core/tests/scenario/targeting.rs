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
                    damage_amount: 0,
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
                    damage_amount: 0,
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
        &cast_spell(
            growth_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
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
        &cast_spell(
            bolt_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
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
    e.apply_command(
        0,
        &cast_spell(
            bolt_a,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
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
    e.apply_command(
        0,
        &cast_spell(
            bolt_b,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
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
        &cast_spell(
            gfth_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
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
        &cast_spell(
            bolt_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
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
                    damage_amount: 0,
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
            &cast_spell(
                bump_idx,
                vec![TargetRef {
                    object_id: bear,
                    damage_amount: 0,
                }],
            ),
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
fn bump_in_the_night_can_be_cast_from_graveyard_with_flashback() {
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
    let mut e = GameEngine::new(2625, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bump_oid = e.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|oid| {
            e.state
                .objects
                .get(oid)
                .is_some_and(|o| o.card_id == "bump_in_the_night")
        })
        .expect("bump in hand");
    e.state.players[0].hand.retain(|&oid| oid != bump_oid);
    e.state.players[0].graveyard.push(bump_oid);
    e.state
        .objects
        .get_mut(&bump_oid)
        .expect("bump object")
        .zone = tricerules_core::Zone::Graveyard;
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            r: 1,
            c: 5,
            ..Default::default()
        },
    );
    let target = target_player(1);
    let cast = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    targets: target,
                    source: Some(graveyard_cast_source(bump_oid)),
                    ..Default::default()
                })),
            },
        )
        .expect("flashback cast");
    // CR 702.34: the stack card is labelled, because nothing on the face distinguishes a flashback
    // cast from a normal one — and this one is exiled rather than buried when it leaves the stack.
    let annotation = cast
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(pushed)) => Some(pushed.ability_annotation.clone()),
            _ => None,
        })
        .expect("spell pushed to the stack");
    assert_eq!(annotation, "Flashback");
    e.apply_command(0, &pass()).expect("p0 pass");
    let batch = e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(e.state.players[1].life, 17);
    assert!(e.state.players[0].exile.contains(&bump_oid));
    assert!(!e.state.players[0].graveyard.contains(&bump_oid));
    assert!(batch.events.iter().any(|event| {
        matches!(
            event.ev,
            Some(Ev::StackResolved(ref resolved))
                if resolved.destination
                    == tricerules_proto::ruled::v1::StackResolveDestination::Exile as i32
        )
    }));
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
        &cast_spell(
            swords_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast swords");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(
            bolt_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
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
                    damage_amount: 0,
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
                base_controller: 0,
                controller: 0,
                card_id: "mind_sculpt".into(),
                copiable_values: None,
                copy_revision: 0,
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
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                adventure_cast_permission: None,
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
        &cast_spell(
            bolt_idx,
            vec![TargetRef {
                object_id: scout,
                damage_amount: 0,
            }],
        ),
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
    e.apply_command(
        0,
        &cast_spell(
            gg_idx,
            vec![TargetRef {
                object_id: scout,
                damage_amount: 0,
            }],
        ),
    )
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
                damage_amount: 0,
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
        &activate_ability(
            assassin,
            0,
            vec![TargetRef {
                object_id: bears,
                damage_amount: 0,
            }],
        ),
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
        &activate_ability(
            assassin,
            0,
            vec![TargetRef {
                object_id: bears,
                damage_amount: 0,
            }],
        ),
    );
    assert!(err.is_err(), "untapped creature is not a legal target");
    // Cost untouched: source stays untapped, nothing on the stack.
    assert!(!e.state.objects.get(&assassin).expect("assassin").tapped);
    assert!(e.state.stack.is_empty());
}

/// Oracle: "Tap target artifact, creature, or land." An Aura on the battlefield is a permanent but
/// not one of those three types, so it is not a legal target — the filter used to be a bare
/// "any permanent" and accepted enchantments.
#[test]
fn icy_manipulator_cannot_target_an_enchantment() {
    let decks = Some(vec![
        deck_with("island", &["icy_manipulator", "holy_strength"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(2811, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let icy = relocate_to_battlefield(&mut e, 0, "icy_manipulator", false);
    // An Aura attached to a creature: a permanent, but not artifact/creature/land.
    let bears = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut e, 0, "holy_strength");
    e.state.objects.get_mut(&aura).expect("aura").attached_to = Some(bears);

    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let err = e.apply_command(
        0,
        &activate_ability(
            icy,
            0,
            vec![TargetRef {
                object_id: aura,
                damage_amount: 0,
            }],
        ),
    );
    assert!(err.is_err(), "an enchantment is not artifact/creature/land");

    // The same ability still accepts a creature.
    e.apply_command(
        0,
        &activate_ability(
            icy,
            0,
            vec![TargetRef {
                object_id: bears,
                damage_amount: 0,
            }],
        ),
    )
    .expect("a creature is a legal Icy Manipulator target");
}

/// Oracle: "Destroy target non-Elf creature." The excluded-subtype restriction is enforced at
/// target selection, so an Elf cannot be chosen at all (CR 601.2c).
#[test]
fn eyeblights_ending_cannot_target_an_elf() {
    let decks = Some(vec![
        deck_with("swamp", &["eyeblights_ending"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(2812, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let elf = inject_creature_on_battlefield(&mut e, 1, "cylian_elf");
    let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    ensure_in_hand(&mut e, 0, "eyeblights_ending");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "eyeblights_ending");
    let err = e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: elf,
                damage_amount: 0,
            }],
        ),
    );
    assert!(err.is_err(), "an Elf is not a legal target");

    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: bears,
                damage_amount: 0,
            }],
        ),
    )
    .expect("a non-Elf creature is a legal target");
}

/// The same excluded-subtype filter reached through an activated ability rather than a spell:
/// Avacynian Priest ("{1}, {T}: Tap target non-Human creature") cannot tap a fellow Human.
#[test]
fn avacynian_priest_taps_only_non_humans() {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(2813, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let priest = inject_creature_on_battlefield(&mut e, 0, "avacynian_priest");
    let human = inject_creature_on_battlefield(&mut e, 1, "fencing_ace");
    let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    assert!(
        e.apply_command(
            0,
            &activate_ability(
                priest,
                0,
                vec![TargetRef {
                    object_id: human,
                    damage_amount: 0
                }]
            )
        )
        .is_err(),
        "a Human creature is not a legal target"
    );
    // The rejected activation paid nothing: the Priest is still untapped and can try again.
    assert!(
        !e.state.objects.get(&priest).expect("priest").tapped,
        "an illegal activation does not pay the tap cost"
    );

    e.apply_command(
        0,
        &activate_ability(
            priest,
            0,
            vec![TargetRef {
                object_id: bears,
                damage_amount: 0,
            }],
        ),
    )
    .expect("a non-Human creature is a legal target");
    pass_both_players(&mut e);
    assert!(
        e.state.objects.get(&bears).expect("bears").tapped,
        "the non-Human creature is tapped"
    );
}

#[test]
fn published_zone_targets_follow_apnap_and_zone_order() {
    let decks = Some(vec![
        deck_with("swamp", &["lightning_bolt", "reanimate"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(2814, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let p0_battlefield = [
        inject_creature_on_battlefield(&mut e, 0, "grizzly_bears"),
        inject_creature_on_battlefield(&mut e, 0, "savannah_lions"),
    ];
    let p1_battlefield = [
        inject_creature_on_battlefield(&mut e, 1, "cylian_elf"),
        inject_creature_on_battlefield(&mut e, 1, "fencing_ace"),
    ];
    let p0_graveyard = [
        inject_graveyard_card(&mut e, 0, "grizzly_bears"),
        inject_graveyard_card(&mut e, 0, "savannah_lions"),
    ];
    let p1_graveyard = [
        inject_graveyard_card(&mut e, 1, "cylian_elf"),
        inject_graveyard_card(&mut e, 1, "fencing_ace"),
    ];

    let bolt = relocate_to_hand(&mut e, 0, "lightning_bolt");
    let reanimate = relocate_to_hand(&mut e, 0, "reanimate");
    let bolt_slot = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == bolt)
        .expect("Lightning Bolt in hand") as u32;
    let reanimate_slot = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == reanimate)
        .expect("Reanimate in hand") as u32;

    let assert_targets =
        |engine: &mut GameEngine, expected_battlefield: Vec<u32>, expected_graveyard: Vec<u32>| {
            let batch = engine.initial_response_batch();
            let legal = batch.legal_by_player.get(&0).expect("legal actions for P0");
            assert_eq!(
                legal.valid_targets_by_hand_slot[&(bolt_slot << 8)].valid_permanent_ids,
                expected_battlefield,
                "battlefield targets must be grouped APNAP and retain battlefield order"
            );
            assert_eq!(
                legal.valid_targets_by_hand_slot[&(reanimate_slot << 8)].valid_graveyard_ids,
                expected_graveyard,
                "graveyard targets must be grouped APNAP and retain graveyard order"
            );
        };

    assert_targets(
        &mut e,
        p0_battlefield.into_iter().chain(p1_battlefield).collect(),
        p0_graveyard.into_iter().chain(p1_graveyard).collect(),
    );

    e.state.active_player_idx = 1;
    assert_targets(
        &mut e,
        p1_battlefield.into_iter().chain(p0_battlefield).collect(),
        p1_graveyard.into_iter().chain(p0_graveyard).collect(),
    );
}

/// Issue #42. Oracle: "Target creature gains deathtouch until end of turn." Targeted `GrantKeywords`
/// was missing from the spell-side validator, so its catch-all advertised every object — graveyard
/// cards, players and stack spells included (CR 115.1: only battlefield creatures are legal here).
#[test]
fn bladebrand_target_tables_exclude_objects_outside_the_battlefield() {
    let decks = Some(vec![
        deck_with("swamp", &["bladebrand", "lightning_bolt"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(2816, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let buried = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let bladebrand = relocate_to_hand(&mut e, 0, "bladebrand");
    ensure_in_hand(&mut e, 0, "lightning_bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            r: 1,
            c: 1,
            ..Default::default()
        },
    );

    // A spell on the stack, so `valid_stack_ids` has something it could wrongly offer.
    let bolt_slot = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(
            bolt_slot,
            vec![TargetRef {
                object_id: 1,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Lightning Bolt at P1");
    let bolt_on_stack = e.state.stack.last().expect("Bolt on stack").id;

    let bladebrand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == bladebrand)
        .expect("Bladebrand in hand") as u32;
    let batch = e.initial_response_batch();
    let legal = batch.legal_by_player.get(&0).expect("legal actions for P0");
    let targets = &legal.valid_targets_by_hand_slot[&(bladebrand_slot << 8)];

    assert_eq!(
        targets.valid_permanent_ids,
        vec![bears],
        "the battlefield creature is the only legal target"
    );
    assert!(
        targets.valid_graveyard_ids.is_empty(),
        "a graveyard card is not a creature on the battlefield: {:?}",
        targets.valid_graveyard_ids
    );
    assert!(
        targets.valid_stack_ids.is_empty(),
        "a spell on the stack cannot gain deathtouch: {:?}",
        targets.valid_stack_ids
    );
    assert!(!targets.can_target_self, "a player is not a creature");
    assert!(!targets.can_target_opponent, "a player is not a creature");

    // The ids exist — the tables exclude them on legality, not because they are absent.
    assert!(e.state.objects.contains_key(&buried));
    assert!(e.state.stack.iter().any(|s| s.id == bolt_on_stack));
}

/// The same defect from the command side: casting must reject an illegal target outright (CR
/// 601.2c) rather than letting the spell resolve and fizzle after the mana is spent (CR 608.2b).
#[test]
fn bladebrand_rejects_cast_targets_outside_the_battlefield() {
    let decks = Some(vec![
        deck_with("swamp", &["bladebrand", "lightning_bolt"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(2817, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let buried = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 0, "bladebrand");
    ensure_in_hand(&mut e, 0, "lightning_bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            r: 1,
            c: 1,
            ..Default::default()
        },
    );

    let bolt_slot = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(
            bolt_slot,
            vec![TargetRef {
                object_id: 1,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Lightning Bolt at P1");
    let bolt_on_stack = e.state.stack.last().expect("Bolt on stack").id;

    for (label, object_id) in [
        ("a graveyard card", buried),
        ("a player", 1u32),
        ("a spell on the stack", bolt_on_stack),
    ] {
        let idx = hand_index_for_card(&e, 0, "bladebrand");
        let result = e.apply_command(
            0,
            &cast_spell(
                idx,
                vec![TargetRef {
                    object_id,
                    damage_amount: 0,
                }],
            ),
        );
        assert!(
            result.is_err(),
            "{label} must not be a legal Bladebrand target"
        );
        assert!(
            e.state.players[0]
                .hand
                .iter()
                .any(|&oid| e.state.objects[&oid].card_id == "bladebrand"),
            "the rejected cast must leave Bladebrand in hand ({label})"
        );
    }

    let idx = hand_index_for_card(&e, 0, "bladebrand");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: bears,
                damage_amount: 0,
            }],
        ),
    )
    .expect("a battlefield creature is still a legal target");
}

#[test]
fn published_stack_targets_include_copies_in_bottom_to_top_order() {
    let decks = Some(vec![
        deck_with("island", &["lightning_bolt", "counterspell"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(2815, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let counterspell = relocate_to_hand(&mut e, 0, "counterspell");
    ensure_in_hand(&mut e, 0, "lightning_bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_slot = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(
            bolt_slot,
            vec![TargetRef {
                object_id: 1,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Lightning Bolt");

    let physical_spell = e.state.stack.last().expect("Bolt on stack").clone();
    let copy_id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let mut copied_spell = physical_spell.clone();
    copied_spell.id = copy_id;
    copied_spell.is_copy = true;
    e.state.stack.push(copied_spell);

    let ability_id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let mut ability = physical_spell.clone();
    ability.id = ability_id;
    ability.ability_text = Some("Triggered ability".to_string());
    ability.is_triggered = true;
    e.state.stack.push(ability);

    let counterspell_slot = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == counterspell)
        .expect("Counterspell in hand") as u32;
    let batch = e.initial_response_batch();
    let legal = batch.legal_by_player.get(&0).expect("legal actions for P0");
    let targets = &legal.valid_targets_by_hand_slot[&(counterspell_slot << 8)];
    assert_eq!(
        targets.valid_stack_ids,
        vec![physical_spell.id, copy_id],
        "spell targets must include copies bottom-to-top and exclude abilities"
    );
    assert!(!e.state.objects.contains_key(&copy_id));
    assert!(!targets.valid_stack_ids.contains(&ability_id));
}
