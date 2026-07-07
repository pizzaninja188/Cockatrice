use crate::helpers::*;

#[test]
fn two_player_passes_empty_stack_advances_toward_combat() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None, true).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    e.apply_command(0, &pass()).expect("p0");
    e.apply_command(1, &pass()).expect("p1");
    // After two passes, should leave upkeep to draw.
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
}

#[test]
fn declare_attackers_handoff_emits_defender_priority() {
    // Defender needs an eligible blocker so the engine enters DeclareBlockers with
    // the defender holding priority (rather than auto-declaring empty blockers).
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(66, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    // Put one creature and two forests on battlefield for attacker.
    for card in ["forest", "forest", "grizzly_bears"] {
        let idx = hand_index_for_card(&e, 0, card);
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
            obj.tapped = false;
        }
    }
    // Give defender an eligible blocker (untapped, not summoning-sick).
    {
        let idx = hand_index_for_card(&e, 1, "grizzly_bears");
        let oid = e.state.players[1].hand.remove(idx);
        e.state.players[1].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
            obj.tapped = false;
        }
    }

    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    let bears_oid = battlefield_object_for_card(&e, 0, "grizzly_bears");
    let b = e
        .apply_command(0, &declare_attackers(vec![bears_oid]))
        .expect("declare attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    assert!(
        priority_changes_in(&b).contains(&0),
        "after declaring attackers, active player keeps priority in declare attackers"
    );
    let to_defender = e
        .apply_command(0, &pass())
        .expect("active pass declare attackers");
    assert!(
        priority_changes_in(&to_defender).contains(&1),
        "defender should receive priority in declare attackers"
    );
    let to_blockers = e
        .apply_command(1, &pass())
        .expect("defender pass declare attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    assert!(
        priority_changes_in(&to_blockers).contains(&1),
        "on entering declare blockers, defender has priority"
    );
}

#[test]
fn no_attackers_skip_to_end_combat_emits_active_priority() {
    // No creatures on battlefield → BeginCombat auto-skips to EndCombat.
    let mut e = GameEngine::new(67, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    let b = e.apply_command(1, &pass()).expect("nap pass begin combat");
    // Engine must skip directly to EndCombat (no DeclareAttackers needed).
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndCombat);
    assert!(
        priority_changes_in(&b).contains(&0),
        "active player should hold priority in end_combat after auto-skip"
    );

    // EndCombat still has a full priority pass cycle before postcombat main.
    let to_nap = e.apply_command(0, &pass()).expect("ap pass end combat");
    assert!(
        priority_changes_in(&to_nap).contains(&1),
        "non-active player should receive priority in end combat"
    );
    e.apply_command(1, &pass()).expect("nap pass end combat");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main2);
}

#[test]
fn blockers_to_combat_damage_emits_priority_stop() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
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
    let mut e = GameEngine::new(68, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    for card in ["forest", "grizzly_bears"] {
        let idx = hand_index_for_card(&e, 0, card);
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
        }
    }
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    let bears_oid = battlefield_object_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![bears_oid]))
        .expect("declare attackers");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    // No eligible blockers for defender: engine auto-declares empty blockers,
    // active player gets priority in DeclareBlockers.
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "engine should auto-declare empty blockers and stay in DeclareBlockers"
    );
    assert!(
        e.state.combat.as_ref().is_some_and(|c| c.blockers_declared),
        "blockers_declared must be true after auto-skip"
    );
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::CombatDamage);
    assert!(
        priority_changes_in(&b).contains(&0),
        "combat damage should open a priority window for active player"
    );
}

#[test]
fn duplicate_attacker_ids_are_rejected() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
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
    let mut e = GameEngine::new(101, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    for card in ["forest", "grizzly_bears"] {
        let idx = hand_index_for_card(&e, 0, card);
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
            obj.tapped = false;
        }
    }
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    let bears_oid = battlefield_object_for_card(&e, 0, "grizzly_bears");

    let err = e
        .apply_command(0, &declare_attackers(vec![bears_oid, bears_oid]))
        .expect_err("duplicate attackers should fail");
    assert_eq!(err.to_string(), "illegal command: duplicate attacker");
}

#[test]
fn same_blocker_cannot_block_two_attackers() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(202, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    for card in ["forest", "forest", "grizzly_bears", "grizzly_bears"] {
        let idx = hand_index_for_card(&e, 0, card);
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
            obj.tapped = false;
        }
    }
    for card in ["forest", "grizzly_bears"] {
        let idx = hand_index_for_card(&e, 1, card);
        let oid = e.state.players[1].hand.remove(idx);
        e.state.players[1].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
            obj.tapped = false;
        }
    }
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");

    let attacker_a = battlefield_object_for_card(&e, 0, "grizzly_bears");
    let attacker_b = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| {
            *oid != attacker_a
                && e.state
                    .objects
                    .get(oid)
                    .map(|o| o.card_id == "grizzly_bears")
                    .unwrap_or(false)
        })
        .expect("second attacker");
    e.apply_command(0, &declare_attackers(vec![attacker_a, attacker_b]))
        .expect("declare two attackers");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    let blocker = battlefield_object_for_card(&e, 1, "grizzly_bears");

    let err = e
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: attacker_a,
                    blocker_id: blocker,
                },
                BlockPair {
                    attacker_id: attacker_b,
                    blocker_id: blocker,
                },
            ]),
        )
        .expect_err("same blocker twice should fail");
    assert_eq!(
        err.to_string(),
        "illegal command: blocker assigned more than once"
    );
}

#[test]
fn declare_attackers_emits_attackers_declared_event() {
    let mut e = GameEngine::new(505, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b = e
        .apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare attackers");
    let evs = attackers_declared_in(&b);
    assert_eq!(evs.len(), 1, "exactly one AttackersDeclared event");
    assert_eq!(evs[0].attacking_player_id, 0);
    assert_eq!(evs[0].attacker_object_ids, vec![bears]);
}

#[test]
fn preview_declare_attackers_is_rejected_by_engine() {
    let mut e = GameEngine::new(508, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let idx_before = e.state.command_index;
    let cmd = RuledCommand {
        cmd: Some(Cmd::PreviewDeclareAttackers(PreviewDeclareAttackers {
            creature_ids: vec![],
        })),
    };
    let err = e
        .apply_command(0, &cmd)
        .expect_err("preview must not apply");
    assert!(err.to_string().contains("preview"), "unexpected err: {err}");
    assert_eq!(e.state.command_index, idx_before);
}

#[test]
fn preview_declare_blockers_is_rejected_by_engine() {
    let mut e = GameEngine::new(507, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let idx_before = e.state.command_index;
    let cmd = RuledCommand {
        cmd: Some(Cmd::PreviewDeclareBlockers(PreviewDeclareBlockers {
            block_pairs: vec![],
        })),
    };
    let err = e
        .apply_command(0, &cmd)
        .expect_err("preview must not apply");
    assert!(err.to_string().contains("preview"), "unexpected err: {err}");
    assert_eq!(
        e.state.command_index, idx_before,
        "preview must not advance command_index"
    );
}

#[test]
fn declare_blockers_emits_blockers_declared_event() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "grizzly_bears".into(),
        ],
    ]);
    let mut e = GameEngine::new(506, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let atk = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blk = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![atk]))
        .expect("declare attackers");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    let b = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: atk,
                blocker_id: blk,
            }]),
        )
        .expect("declare blockers");
    let evs = blockers_declared_in(&b);
    assert_eq!(evs.len(), 1, "exactly one BlockersDeclared event");
    assert_eq!(evs[0].block_pairs.len(), 1);
    assert_eq!(evs[0].block_pairs[0].attacker_id, atk);
    assert_eq!(evs[0].block_pairs[0].blocker_id, blk);
}

#[test]
fn unblocked_combat_damage_emits_life_changed() {
    let mut e = GameEngine::new(606, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears_a = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bears_b = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![bears_a, bears_b]))
        .expect("two attackers");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    // No eligible blockers: engine auto-declares empty blockers, active player has priority.
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    let life = life_changes_in(&b);
    assert_eq!(life.len(), 1, "single LifeChanged event for defender");
    assert_eq!(life[0].player_id, 1);
    assert_eq!(life[0].delta, -4, "two 2/2s deal 4 damage");
    assert_eq!(life[0].new_total, 16);
    assert_eq!(e.state.players[1].life, 16);
}

#[test]
fn blocked_combat_kills_blocker_and_emits_permanent_moved() {
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
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(707, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // Defender needs a creature on the battlefield to block. Put a 2/2 too -> mutual destruction.
    let blocker = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    let declared = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare blocker");
    assert!(
        permanents_moved_in(&declared).is_empty(),
        "creatures should not die until combat damage step"
    );
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();
    assert!(
        dead_ids.contains(&attacker) && dead_ids.contains(&blocker),
        "both 2/2s die in mutual block, got {dead_ids:?}"
    );
    for pm in &dead {
        assert_eq!(
            pm.destination,
            tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32
        );
    }
    // No life loss on a mutual block.
    let life = life_changes_in(&b);
    assert!(life.is_empty(), "no life change on a fully blocked combat");
}

#[test]
fn full_combat_2v1_trade_and_life_loss() {
    // Active player has two 2/2 attackers; defender has one 2/2 blocker.
    // Active player attacks with both. Defender blocks attacker_a only.
    // Outcome: attacker_a + blocker trade (both move to graveyard); attacker_b
    // hits the defender for 2 unblocked damage.
    let decks = Some(vec![
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(808, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker_a = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let attacker_b = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    // Snapshot pre-combat state we care about.
    let attacker_a_pre_tapped = e
        .state
        .objects
        .get(&attacker_a)
        .map(|o| o.tapped)
        .unwrap_or(true);
    let attacker_b_pre_tapped = e
        .state
        .objects
        .get(&attacker_b)
        .map(|o| o.tapped)
        .unwrap_or(true);
    assert!(
        !attacker_a_pre_tapped,
        "attacker_a should be untapped pre-combat"
    );
    assert!(
        !attacker_b_pre_tapped,
        "attacker_b should be untapped pre-combat"
    );

    // Declare attackers.
    let attack_batch = e
        .apply_command(0, &declare_attackers(vec![attacker_a, attacker_b]))
        .expect("declare two attackers");
    let ad = attackers_declared_in(&attack_batch);
    assert_eq!(ad.len(), 1);
    assert_eq!(ad[0].attacking_player_id, 0);
    let mut declared_ids = ad[0].attacker_object_ids.clone();
    declared_ids.sort();
    let mut expected = vec![attacker_a, attacker_b];
    expected.sort();
    assert_eq!(declared_ids, expected, "both attackers reported");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers,
        "after attackers are declared, still in declare attackers until priority passes"
    );

    // Engine should auto-tap attackers.
    assert!(
        e.state
            .objects
            .get(&attacker_a)
            .map(|o| o.tapped)
            .unwrap_or(false),
        "attacker_a tapped on attack"
    );
    assert!(
        e.state
            .objects
            .get(&attacker_b)
            .map(|o| o.tapped)
            .unwrap_or(false),
        "attacker_b tapped on attack"
    );
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    // Declare blockers: only attacker_a is blocked.
    let declared_blockers_batch = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker_a,
                blocker_id: blocker,
            }]),
        )
        .expect("declare blocker");
    assert!(
        permanents_moved_in(&declared_blockers_batch).is_empty(),
        "no deaths during blocker declaration itself"
    );
    assert!(
        life_changes_in(&declared_blockers_batch).is_empty(),
        "no life loss during blocker declaration itself"
    );
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let block_batch = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");

    // Mutual destruction on the blocked pair -> both go to graveyard.
    let dead = permanents_moved_in(&block_batch);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();
    assert!(
        dead_ids.contains(&attacker_a),
        "attacker_a dies in trade, got {dead_ids:?}"
    );
    assert!(
        dead_ids.contains(&blocker),
        "blocker dies in trade, got {dead_ids:?}"
    );
    assert!(
        !dead_ids.contains(&attacker_b),
        "attacker_b survives, got {dead_ids:?}"
    );
    for pm in &dead {
        assert_eq!(
            pm.destination,
            tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32,
            "trade victims go to graveyard"
        );
    }

    // Defender takes 2 from attacker_b's unblocked damage.
    let life = life_changes_in(&block_batch);
    assert_eq!(life.len(), 1, "exactly one life change event");
    assert_eq!(life[0].player_id, 1);
    assert_eq!(life[0].delta, -2, "attacker_b deals 2 unblocked");
    assert_eq!(life[0].new_total, 18);
    assert_eq!(e.state.players[1].life, 18);
}

#[test]
fn giant_growth_changes_combat_outcome() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "giant_growth".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(902, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let p0_bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let p1_bear = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    let forest_idx = hand_index_for_card(&e, 0, "forest");
    let forest_oid = e.state.players[0].hand.remove(forest_idx);
    e.state.players[0].battlefield.push(forest_oid);
    e.state.objects.get_mut(&forest_oid).expect("forest").zone = tricerules_core::Zone::Battlefield;

    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    let growth_batch = e
        .apply_command(
            0,
            &cast_spell(
                growth_idx,
                vec![TargetRef {
                    object_id: p0_bear,
                    damage_amount: 0,
                }],
            ),
        )
        .expect("cast growth");
    let growth_push = growth_batch
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("growth stack pushed");
    assert_eq!(growth_push.targets.len(), 1);
    assert_eq!(growth_push.targets[0].object_id, p0_bear);
    e.apply_command(0, &pass()).expect("p0 pass growth");
    e.apply_command(1, &pass()).expect("p1 pass growth");

    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    e.apply_command(0, &declare_attackers(vec![p0_bear]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("ap pass declare attackers");
    e.apply_command(1, &pass())
        .expect("nap pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: p0_bear,
            blocker_id: p1_bear,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass())
        .expect("ap pass declare blockers");
    let damage_batch = e
        .apply_command(1, &pass())
        .expect("nap pass declare blockers");

    let moved_ids: Vec<u32> = permanents_moved_in(&damage_batch)
        .iter()
        .map(|p| p.object_id)
        .collect();
    assert!(moved_ids.contains(&p1_bear), "blocked bear should die");
    assert!(
        !moved_ids.contains(&p0_bear),
        "grown attacker should survive combat"
    );
}

#[test]
fn cannot_cast_spell_until_attackers_declared() {
    let mut e = GameEngine::new(9200, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let _bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    while !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("lightning_bolt"))
    {
        take_card_from_library_to_hand(&mut e, 0, "lightning_bolt");
    }
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let err = e
        .apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect_err("cast before attackers illegal");
    assert!(
        err.to_string()
            .contains("cannot cast until attack or block declaration is complete"),
        "unexpected: {err}"
    );

    let bear_oid = battlefield_object_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![bear_oid]))
        .expect("declare attackers");

    while !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("mountain"))
    {
        take_card_from_library_to_hand(&mut e, 0, "mountain");
    }
    let m_idx = hand_index_for_card(&e, 0, "mountain");
    let m_oid = e.state.players[0].hand.remove(m_idx);
    e.state.players[0].battlefield.push(m_oid);
    let o = e.state.objects.get_mut(&m_oid).expect("mountain");
    o.zone = tricerules_core::Zone::Battlefield;
    o.summoning_sick = false;
    o.tapped = false;

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx2 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx2, target_player(1)))
        .expect("instant legal after attackers committed");
    assert_eq!(e.state.stack.len(), 1);
}

#[test]
fn cannot_cast_spell_until_blockers_declared() {
    let mut e = GameEngine::new(9300, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // Inject an eligible blocker for the defender so the engine prompts them in DeclareBlockers.
    inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare");
    e.apply_command(0, &pass())
        .expect("ap pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers -> declare blockers");

    while !e.state.players[1]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("giant_growth"))
    {
        take_card_from_library_to_hand(&mut e, 1, "giant_growth");
    }
    while !e.state.players[1]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("forest"))
    {
        take_card_from_library_to_hand(&mut e, 1, "forest");
    }
    let f_idx = hand_index_for_card(&e, 1, "forest");
    let f_oid = e.state.players[1].hand.remove(f_idx);
    e.state.players[1].battlefield.push(f_oid);
    let fo = e.state.objects.get_mut(&f_oid).expect("forest");
    fo.zone = tricerules_core::Zone::Battlefield;
    fo.summoning_sick = false;
    fo.tapped = false;

    let growth_idx = hand_index_for_card(&e, 1, "giant_growth");
    let err = e
        .apply_command(
            1,
            &cast_spell(
                growth_idx,
                vec![TargetRef {
                    object_id: attacker,
                    damage_amount: 0,
                }],
            ),
        )
        .expect_err("cast before blockers illegal");
    assert!(
        err.to_string()
            .contains("cannot cast until attack or block declaration is complete"),
        "unexpected: {err}"
    );

    e.apply_command(1, &declare_blockers(vec![]))
        .expect("declare no blockers");
    e.apply_command(0, &pass())
        .expect("ap pass declare blockers");
    give_mana(
        &mut e,
        1,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let growth_idx2 = hand_index_for_card(&e, 1, "giant_growth");
    e.apply_command(
        1,
        &cast_spell(
            growth_idx2,
            vec![TargetRef {
                object_id: attacker,
                damage_amount: 0,
            }],
        ),
    )
    .expect("instant legal after blockers committed");
    assert_eq!(e.state.stack.len(), 1);
}

#[test]
fn two_blockers_damage_order_required_and_resolves() {
    // Attacker: grizzly_bears (2/2) = 2 power.
    // Blockers: savannah_lions (2/1) + grizzly_bears (2/2).
    // Assignment: lions 1, bears 1 (sum = attacker power).
    // Attacker receives 2+2=4 damage (toughness 2) → dies. No life loss.
    let decks = Some(vec![
        // P0: enough grizzly_bears to guarantee one in hand after draw step
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        // P1: equal mix so both are available in library after opening draw
        {
            let mut d: Vec<String> = std::iter::repeat_n("savannah_lions".to_string(), 5).collect();
            d.extend(std::iter::repeat_n("grizzly_bears".to_string(), 5));
            d
        },
    ]);
    let mut e = GameEngine::new(901, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 1, "savannah_lions");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker_lions = put_creature_on_battlefield(&mut e, 1, "savannah_lions");
    let blocker_bears = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    // Defender sends both blockers to the same attacker.
    let b = e
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker_lions,
                },
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker_bears,
                },
            ]),
        )
        .expect("declare two blockers");

    assert!(
        e.state.combat.as_ref().unwrap().damage_assignment_needed,
        "damage_assignment_needed must be true after multi-block"
    );
    assert!(
        !e.state.combat.as_ref().unwrap().assign_combat_damage_phase,
        "still in declare blockers priority before passes"
    );
    assert!(life_changes_in(&b).is_empty(), "no damage dealt yet");

    assert!(
        e.apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(blocker_lions, 1), (blocker_bears, 1)]),
        )
        .is_err(),
        "cannot assign combat damage before declare-blockers priority round"
    );

    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass())
        .expect("defender pass → assign combat damage step");
    assert!(
        e.state.combat.as_ref().unwrap().assign_combat_damage_phase,
        "assign_combat_damage_phase after both pass"
    );

    let b3 = e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(blocker_lions, 1), (blocker_bears, 1)]),
        )
        .expect("assign combat damage");

    let dead = permanents_moved_in(&b3);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();

    // Attacker (2/2) gets 2+2=4 total blocker damage → dies.
    assert!(dead_ids.contains(&attacker), "attacker dies: {dead_ids:?}");
    // Lions (2/1) gets 1 lethal damage first in order → dies.
    assert!(dead_ids.contains(&blocker_lions), "lions die: {dead_ids:?}");
    // Bears (2/2) gets remaining 1 damage (< toughness 2) → survives.
    assert!(
        !dead_ids.contains(&blocker_bears),
        "bears survive: {dead_ids:?}"
    );
    let bears_obj = e.state.objects.get(&blocker_bears).expect("bears object");
    assert_eq!(bears_obj.damage, 1, "bears has 1 marked damage");
    assert_eq!(bears_obj.zone, tricerules_core::Zone::Battlefield);
    assert!(
        life_changes_in(&b3).is_empty(),
        "no life change on fully-blocked combat"
    );
}

#[test]
fn two_blockers_insufficient_power_kills_only_first_in_order() {
    // Attacker: savannah_lions (2/1) = 2 power.
    // Blockers: coral_merfolk (2/1) + grizzly_bears (2/2).
    // merfolk 1 lethal, bears 1 partial.
    // Attacker receives 2+2=4 damage → dies. No life loss.
    let decks = Some(vec![
        {
            let mut d: Vec<String> = std::iter::repeat_n("savannah_lions".to_string(), 5).collect();
            d.extend(std::iter::repeat_n("grizzly_bears".to_string(), 5));
            d
        },
        {
            let mut d: Vec<String> = std::iter::repeat_n("coral_merfolk".to_string(), 5).collect();
            d.extend(std::iter::repeat_n("grizzly_bears".to_string(), 5));
            d
        },
    ]);
    let mut e = GameEngine::new(902, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "savannah_lions");
    ensure_in_hand(&mut e, 1, "coral_merfolk");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let blocker_merfolk = put_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    let blocker_bears = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");

    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair {
                attacker_id: attacker,
                blocker_id: blocker_merfolk,
            },
            BlockPair {
                attacker_id: attacker,
                blocker_id: blocker_bears,
            },
        ]),
    )
    .expect("two blockers");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass())
        .expect("defender pass → assign combat damage");
    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(blocker_merfolk, 1), (blocker_bears, 1)]),
        )
        .expect("assign combat damage");

    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();

    // Attacker (2/1) gets 2+2=4 damage → dies.
    assert!(
        dead_ids.contains(&attacker),
        "lions attacker dies: {dead_ids:?}"
    );
    // Merfolk (2/1) gets 1 lethal → dies.
    assert!(
        dead_ids.contains(&blocker_merfolk),
        "merfolk die: {dead_ids:?}"
    );
    // Bears (2/2) gets remaining 1 damage (< toughness 2) → survives.
    assert!(
        !dead_ids.contains(&blocker_bears),
        "bears survive: {dead_ids:?}"
    );
    assert!(
        life_changes_in(&b).is_empty(),
        "no life change (fully blocked)"
    );
}

#[test]
fn single_blocker_no_damage_order_needed() {
    // Regression: single blocker must not trigger damage_assignment_needed; combat proceeds normally.
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(903, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");

    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: blocker,
        }]),
    )
    .expect("declare single blocker");

    assert!(
        !e.state.combat.as_ref().unwrap().damage_assignment_needed,
        "damage_assignment_needed must be false for single-blocker combat"
    );

    // Combat resolves normally without any AssignCombatDamage step: both 2/2s die.
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e.apply_command(1, &pass()).expect("combat damage");
    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();
    assert!(
        dead_ids.contains(&attacker),
        "attacker dies in mutual block"
    );
    assert!(dead_ids.contains(&blocker), "blocker dies in mutual block");
    assert!(
        life_changes_in(&b).is_empty(),
        "no life loss on fully blocked combat"
    );
}

#[test]
fn assign_combat_damage_rejects_sum_mismatch() {
    let (mut e, attacker, a, b) = setup_two_blockers_assign_phase(910);
    assert!(e
        .apply_command(0, &assign_combat_damage_cmd(attacker, vec![(a, 1), (b, 0)]),)
        .is_err());
    assert!(!e
        .state
        .combat
        .as_ref()
        .unwrap()
        .damage_assignments
        .contains_key(&attacker));
}

#[test]
fn assign_combat_damage_accepts_split_with_two_nonlethal_hits() {
    // Two 2/2 blockers vs 2-power attacker: 1+1 is allowed (no lethal-first requirement).
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(911, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b1 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b2 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");
    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair {
                attacker_id: attacker,
                blocker_id: b1,
            },
            BlockPair {
                attacker_id: attacker,
                blocker_id: b2,
            },
        ]),
    )
    .expect("declare two blockers");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass");
    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(b1, 1), (b2, 1)]),
        )
        .expect("assign 1+1");
    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();
    assert!(
        dead_ids.contains(&attacker),
        "attacker dies from 2+2 blocker damage"
    );
    assert!(
        !dead_ids.contains(&b1) && !dead_ids.contains(&b2),
        "both blockers survive with 1 dmg"
    );
    assert_eq!(e.state.objects.get(&b1).unwrap().damage, 1);
    assert_eq!(e.state.objects.get(&b2).unwrap().damage, 1);
}

#[test]
fn assign_combat_damage_rejects_wrong_blocker_set() {
    let (mut e, attacker, a, _b) = setup_two_blockers_assign_phase(912);
    let other = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    assert!(e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(a, 1), (other, 1)]),
        )
        .is_err());
}

#[test]
fn assign_combat_damage_rejects_defender_player() {
    let (mut e, attacker, a, b) = setup_two_blockers_assign_phase(913);
    assert!(e
        .apply_command(1, &assign_combat_damage_cmd(attacker, vec![(a, 1), (b, 1)]),)
        .is_err());
}

#[test]
fn assign_combat_damage_rejects_sum_exceeds_power() {
    // 2-power attacker, two blockers: 1+2 sums to 3 > power. Must reject.
    let (mut e, attacker, a, b) = setup_two_blockers_assign_phase(914);
    assert!(e
        .apply_command(0, &assign_combat_damage_cmd(attacker, vec![(a, 1), (b, 2)]))
        .is_err());
    assert!(!e
        .state
        .combat
        .as_ref()
        .unwrap()
        .damage_assignments
        .contains_key(&attacker));
    // State stays in assign-damage phase so the AP can retry with a legal split.
    assert!(e.state.combat.as_ref().unwrap().assign_combat_damage_phase);
}

#[test]
fn assign_combat_damage_three_blockers_split_one_each() {
    // 3-power attacker (Balduvian Barbarians, 3/2) blocked by three 2/2 grizzly bears.
    // Split 1+1+1: every blocker takes 1 (survives); attacker takes 2+2+2=6 → dies.
    let decks = Some(vec![
        std::iter::repeat_n("balduvian_barbarians".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(915, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "balduvian_barbarians");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "balduvian_barbarians");
    let b1 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b2 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b3 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");
    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair {
                attacker_id: attacker,
                blocker_id: b1,
            },
            BlockPair {
                attacker_id: attacker,
                blocker_id: b2,
            },
            BlockPair {
                attacker_id: attacker,
                blocker_id: b3,
            },
        ]),
    )
    .expect("declare three blockers");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass");

    // Sum != power must still be rejected with N=3.
    assert!(e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(b1, 1), (b2, 1), (b3, 0)]),
        )
        .is_err());
    // Wrong blocker set (missing one) must be rejected.
    assert!(e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(b1, 2), (b2, 1)])
        )
        .is_err());

    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(b1, 1), (b2, 1), (b3, 1)]),
        )
        .expect("assign 1+1+1");
    let dead: Vec<u32> = permanents_moved_in(&b)
        .iter()
        .map(|p| p.object_id)
        .collect();
    assert!(
        dead.contains(&attacker),
        "attacker dies from 2+2+2 blocker damage: {dead:?}"
    );
    assert!(
        !dead.contains(&b1) && !dead.contains(&b2) && !dead.contains(&b3),
        "all three blockers survive at 1 marked damage: {dead:?}"
    );
    for bid in [b1, b2, b3] {
        let obj = e.state.objects.get(&bid).expect("blocker present");
        assert_eq!(obj.damage, 1);
        assert_eq!(obj.zone, tricerules_core::Zone::Battlefield);
    }
    // After resolution combat is cleared.
    assert!(e.state.combat.is_none());
}

#[test]
fn assign_combat_damage_two_multi_blocked_attackers_requires_both() {
    // Two grizzly_bears (2/2) attackers, each blocked by two coral_merfolk (2/1).
    // Engine must hold damage resolution until BOTH attackers receive assignments,
    // and resolution should only fire on the second assign call.
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("coral_merfolk".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(916, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 1, "coral_merfolk");
    let atk1 = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let atk2 = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b1a = put_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    let b1b = put_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    let b2a = put_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    let b2b = put_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    e.apply_command(0, &declare_attackers(vec![atk1, atk2]))
        .expect("declare two attackers");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");
    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair {
                attacker_id: atk1,
                blocker_id: b1a,
            },
            BlockPair {
                attacker_id: atk1,
                blocker_id: b1b,
            },
            BlockPair {
                attacker_id: atk2,
                blocker_id: b2a,
            },
            BlockPair {
                attacker_id: atk2,
                blocker_id: b2b,
            },
        ]),
    )
    .expect("declare blockers");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass");
    assert!(e.state.combat.as_ref().unwrap().assign_combat_damage_phase);

    // First assignment: combat must NOT yet resolve.
    let b_first = e
        .apply_command(0, &assign_combat_damage_cmd(atk1, vec![(b1a, 1), (b1b, 1)]))
        .expect("assign for atk1");
    assert!(
        permanents_moved_in(&b_first).is_empty(),
        "no permanents moved yet; second attacker still needs assignment"
    );
    assert!(
        e.state
            .combat
            .as_ref()
            .expect("combat still active")
            .damage_assignment_needed,
        "still waiting on atk2 assignment"
    );

    // Second assignment: combat resolves now.
    let b_second = e
        .apply_command(0, &assign_combat_damage_cmd(atk2, vec![(b2a, 1), (b2b, 1)]))
        .expect("assign for atk2");
    let dead: Vec<u32> = permanents_moved_in(&b_second)
        .iter()
        .map(|p| p.object_id)
        .collect();
    // Each 2/2 attacker takes 1+1=2 damage from its two 2/1 blockers → both attackers die.
    assert!(dead.contains(&atk1), "atk1 dies: {dead:?}");
    assert!(dead.contains(&atk2), "atk2 dies: {dead:?}");
    // Each 2/1 blocker takes 1 lethal damage → all blockers die.
    for bid in [b1a, b1b, b2a, b2b] {
        assert!(dead.contains(&bid), "blocker {bid} dies: {dead:?}");
    }
    assert!(e.state.combat.is_none(), "combat cleared after resolution");
}

// ── Combat eligibility skip tests ────────────────────────────────────────────

#[test]
fn begin_combat_skips_when_no_eligible_attackers() {
    // Default deck has no creatures on the battlefield.
    // BeginCombat must auto-skip directly to EndCombat.
    let mut e = GameEngine::new(4001, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    let b = e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::EndCombat,
        "no eligible attackers must skip to end_combat"
    );
    assert!(
        priority_changes_in(&b).contains(&0),
        "active player must hold priority in end_combat after auto-skip"
    );
}

#[test]
fn begin_combat_skips_when_all_creatures_summoning_sick() {
    let mut e = GameEngine::new(4002, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    // Inject a summoning-sick creature (cannot attack).
    let oid = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    if let Some(obj) = e.state.objects.get_mut(&oid) {
        obj.summoning_sick = true;
    }
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::EndCombat,
        "summoning-sick creature must not prevent skip to end_combat"
    );
}

#[test]
fn begin_combat_skips_when_all_creatures_tapped() {
    let mut e = GameEngine::new(4003, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    // Inject a tapped creature (cannot attack).
    let oid = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    if let Some(obj) = e.state.objects.get_mut(&oid) {
        obj.tapped = true;
    }
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::EndCombat,
        "tapped creature must not prevent skip to end_combat"
    );
}

#[test]
fn begin_combat_enters_declare_attackers_when_eligible_attacker_exists() {
    let mut e = GameEngine::new(4004, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers,
        "eligible attacker must cause engine to enter declare_attackers"
    );
}

#[test]
fn declare_attackers_skips_blockers_when_no_eligible_blockers() {
    // Active player has an attacker; defending player has no creatures.
    // After both pass priority in DeclareAttackers, engine auto-declares empty blockers.
    let mut e = GameEngine::new(4005, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    let bears = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    e.apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare attacker");
    // Both pass in DeclareAttackers.
    e.apply_command(0, &pass())
        .expect("ap pass declare_attackers");
    let b = e
        .apply_command(1, &pass())
        .expect("nap pass declare_attackers");
    // Engine lands in DeclareBlockers with blockers_declared = true and active player holding priority.
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    assert!(
        priority_changes_in(&b).contains(&0),
        "active player must hold priority when blockers auto-declared"
    );
    assert!(
        e.state.combat.as_ref().is_some_and(|c| c.blockers_declared),
        "blockers_declared must be true after auto-skip"
    );
}

#[test]
fn cannot_add_mana_while_declaring_attackers() {
    let mut e = GameEngine::new(4010, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    // Priority is locked until the active player declares attackers, so a mana ability (a
    // tapped land) cannot be activated yet (CR 605.3a; the engine rejects it).
    let land = inject_permanent_on_battlefield(&mut e, 0, "mountain");
    let err = e
        .apply_command(0, &activate_ability(land, 0, vec![]))
        .expect_err("mana ability must be illegal during declare attackers");
    assert!(
        format!("{err:?}").contains("attack or block declaration"),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        e.state.players[0].mana_pool.red, 0,
        "no mana produced while locked"
    );
}

#[test]
fn cannot_add_mana_while_declaring_blockers() {
    let mut e = GameEngine::new(4011, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attackers");
    e.apply_command(0, &pass())
        .expect("ap pass declare attackers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "should be in declare blockers"
    );
    assert!(
        priority_changes_in(&b).contains(&1),
        "defender must hold priority in declare blockers"
    );
    // Defender holds priority but priority is locked for blocker declaration, so the defender
    // cannot activate a mana ability (tap a land) yet (CR 605.3a).
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let err = e
        .apply_command(1, &activate_ability(land, 0, vec![]))
        .expect_err("mana ability must be illegal during declare blockers");
    assert!(
        format!("{err:?}").contains("attack or block declaration"),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        e.state.players[1].mana_pool.green, 0,
        "no mana produced while locked"
    );
}

/// CR 510.4: the per-player zone view exposes `first_strike_step_pending=true` between
/// declare-attackers and the end of the first-strike step, so the client can show the
/// "First Strike Damage" pass-priority button label.
#[test]
fn zone_view_signals_first_strike_step_pending() {
    let mut e = GameEngine::new(11_006, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let goblin = inject_creature_with_stats(&mut e, 0, "goblin_striker", 1, 1);

    let b = e
        .apply_command(0, &declare_attackers(vec![goblin]))
        .expect("declare attacker");
    let zv = b
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view present");
    assert!(
        zv.per_player.iter().all(|p| p.first_strike_step_pending),
        "first_strike_step_pending must be true while a FS attacker is in combat"
    );
}

/// CR 510.4: `first_strike_step_pending` must remain true after blockers are declared (still
/// pre-resolution), so the declare-blockers pass-priority button stays labeled
/// "First Strike Damage" up until the substep actually resolves.
#[test]
fn zone_view_signals_pending_after_blockers_declared() {
    let mut e = GameEngine::new(11_007, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let goblin = inject_creature_with_stats(&mut e, 0, "goblin_striker", 1, 1);
    let corpse = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);

    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    let b = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: goblin,
                blocker_id: corpse,
            }]),
        )
        .expect("declare blockers");
    let zv = b
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view present");
    assert!(
        zv.per_player.iter().all(|p| p.first_strike_step_pending),
        "pending must stay true after blockers declared (mixed FS attacker + vanilla blocker)"
    );

    // And it must flip to false once the FS substep resolves.
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    let b2 = e.apply_command(1, &pass()).expect("def pass dec blk");
    let zv2 = b2
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view present");
    assert!(
        zv2.per_player.iter().all(|p| !p.first_strike_step_pending),
        "pending must flip false once the first-strike substep has resolved"
    );
}

/// CR 510.4: when no FS/DS creature is in combat, `first_strike_step_pending` is never true.
#[test]
fn zone_view_does_not_signal_pending_for_vanilla_combat() {
    let mut e = GameEngine::new(11_008, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let b = e
        .apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare attacker");
    let zv = b
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view present");
    assert!(
        zv.per_player.iter().all(|p| !p.first_strike_step_pending),
        "pending must stay false in vanilla combat (no FS/DS combatants)"
    );
}

/// P2 combat filter: Divine Verdict ("Destroy target attacking or blocking creature") is legal
/// against a declared attacker and illegal against a creature not in combat.
#[test]
fn divine_verdict_targets_only_combatants() {
    let decks = Some(vec![
        vec![
            "divine_verdict".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec![
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(5008, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    // The active player holds priority in DeclareAttackers after declaring; it casts the instant.
    assert_eq!(
        e.state.priority_player_id(),
        0,
        "active player has priority after declaration"
    );

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 4,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "divine_verdict");
    // A creature not in combat is an illegal target.
    assert!(
        e.apply_command(
            0,
            &cast_spell(
                idx,
                vec![TargetRef {
                    object_id: bystander,
                    damage_amount: 0,
                }]
            )
        )
        .is_err(),
        "Divine Verdict cannot target a creature that is not attacking or blocking"
    );
    let idx = hand_index_for_card(&e, 0, "divine_verdict");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: attacker,
                damage_amount: 0,
            }],
        ),
    )
    .expect("Divine Verdict targets the attacker");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.objects.get(&attacker).map(|o| o.zone) != Some(tricerules_core::Zone::Battlefield),
        "the attacking creature is destroyed"
    );
}

// ── Must-attack enforcement (CR 508.1d) ──────────────────────────────────────

/// Happy path: a must-attack creature (Crazed Goblin) declared as an attacker is accepted.
#[test]
fn must_attack_creature_declared_as_attacker_is_legal() {
    let mut e = GameEngine::new(5500, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    // Inject a must-attack creature (mirrors Crazed Goblin — attacks each combat if able).
    let goblin = inject_creature_on_battlefield(&mut e, 0, "crazed_goblin");
    e.state
        .objects
        .get_mut(&goblin)
        .unwrap()
        .must_attack_if_able = true;
    // Declaring it as an attacker must succeed.
    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("must-attack creature can be declared as attacker");
    assert!(
        e.state.combat.as_ref().unwrap().attacking.contains(&goblin),
        "Crazed Goblin is attacking"
    );
}

/// Illegal path: omitting a must-attack creature when it could legally attack returns Illegal.
#[test]
fn must_attack_creature_omitted_from_attackers_is_illegal() {
    let mut e = GameEngine::new(5501, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    // Inject a must-attack creature.
    let goblin = inject_creature_on_battlefield(&mut e, 0, "crazed_goblin");
    e.state
        .objects
        .get_mut(&goblin)
        .unwrap()
        .must_attack_if_able = true;
    // Tap the grizzly_bears that advance_to_declare_attackers injected so it can't cause noise,
    // but since bears doesn't have must_attack, it doesn't matter — the goblin is the only
    // must-attack creature. Declaring empty attackers must fail.
    let result = e.apply_command(0, &declare_attackers(vec![]));
    assert!(
        result.is_err(),
        "omitting must-attack creature from attackers should be illegal"
    );
}

/// CR 508.1d "if able": a must-attack creature that is summoning-sick is NOT required to attack.
#[test]
fn must_attack_creature_summoning_sick_may_skip() {
    let mut e = GameEngine::new(5502, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    // Inject a must-attack creature that is summoning-sick — it is not a legal attacker.
    let goblin = inject_creature_on_battlefield(&mut e, 0, "crazed_goblin");
    {
        let obj = e.state.objects.get_mut(&goblin).unwrap();
        obj.must_attack_if_able = true;
        obj.summoning_sick = true;
    }
    // The grizzly_bears from advance_to_declare_attackers doesn't have must_attack, so
    // declaring no attackers is legal (no eligible must-attack creature exists).
    e.apply_command(0, &declare_attackers(vec![]))
        .expect("summoning-sick must-attack creature does not force an attack");
}

/// CR 508.1d "if able": a must-attack creature that is tapped cannot legally attack, so skip is OK.
#[test]
fn must_attack_creature_tapped_may_skip() {
    let mut e = GameEngine::new(5503, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    // Inject a must-attack creature that is tapped — it is not a legal attacker.
    let goblin = inject_creature_on_battlefield(&mut e, 0, "crazed_goblin");
    {
        let obj = e.state.objects.get_mut(&goblin).unwrap();
        obj.must_attack_if_able = true;
        obj.tapped = true;
    }
    e.apply_command(0, &declare_attackers(vec![]))
        .expect("tapped must-attack creature does not force an attack");
}

/// CR 509.1c: a must-block creature that omits a legal block returns Illegal.
#[test]
fn must_block_creature_omitted_from_blockers_is_illegal() {
    let mut e = GameEngine::new(5504, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    // The grizzly_bears from advance_to_declare_attackers is the attacker.
    let attacker = battlefield_object_for_card(&e, 0, "grizzly_bears");
    // Inject a must-block creature on the defender's side.
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state
        .objects
        .get_mut(&blocker)
        .unwrap()
        .must_block_if_able = true;
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass attackers");
    e.apply_command(1, &pass())
        .expect("defender pass attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    // Declaring no blockers while must-block creature can block must be illegal.
    let result = e.apply_command(1, &declare_blockers(vec![]));
    assert!(
        result.is_err(),
        "omitting must-block creature while it can legally block should be illegal"
    );
}

/// CR 508.1d: the active player's LegalActions must surface the must-attack creature id so the
/// client can gate its confirm-attackers control identically to the engine's set_attackers check.
#[test]
fn legal_actions_surface_required_attacker_to_active_player() {
    let mut e = GameEngine::new(5510, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    // An eligible ordinary attacker so BeginCombat enters DeclareAttackers.
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // A must-attack creature that is itself a legal attacker (untapped, not summoning-sick).
    let goblin = inject_creature_on_battlefield(&mut e, 0, "crazed_goblin");
    e.state
        .objects
        .get_mut(&goblin)
        .unwrap()
        .must_attack_if_able = true;
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    let batch = e.apply_command(1, &pass()).expect("nap pass begin combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    let legal = batch.legal_by_player.get(&0).expect("legal for P0");
    assert!(
        legal.required_attacker_ids.contains(&goblin),
        "active player's LegalActions must list the must-attack Crazed Goblin"
    );
    // The non-active player is never asked to declare attackers.
    let legal_nap = batch.legal_by_player.get(&1).expect("legal for P1");
    assert!(
        legal_nap.required_attacker_ids.is_empty(),
        "defender has no required attackers"
    );
}

/// CR 509.1c: the defending player's LegalActions must surface the must-block creature id so the
/// client can gate its confirm-blockers control identically to the engine's set_blockers check.
#[test]
fn legal_actions_surface_required_blocker_to_defender() {
    let mut e = GameEngine::new(5511, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = battlefield_object_for_card(&e, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state
        .objects
        .get_mut(&blocker)
        .unwrap()
        .must_block_if_able = true;
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass attackers");
    let batch = e
        .apply_command(1, &pass())
        .expect("defender pass attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    let legal = batch.legal_by_player.get(&1).expect("legal for P1");
    assert!(
        legal.required_blocker_ids.contains(&blocker),
        "defender's LegalActions must list the must-block creature"
    );
    // The active player is never asked to declare blockers.
    let legal_ap = batch.legal_by_player.get(&0).expect("legal for P0");
    assert!(
        legal_ap.required_blocker_ids.is_empty(),
        "active player has no required blockers"
    );
}
