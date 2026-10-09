use crate::helpers::*;

#[test]
fn two_player_passes_empty_stack_advances_toward_combat() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        99,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        66,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        67,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        68,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        101,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        202,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        505,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let bears = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b = e
        .apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare attackers");
    let evs = attackers_declared_in(&b);
    assert_eq!(evs.len(), 1, "exactly one AttackersDeclared event");
    assert_eq!(evs[0].attacking_player_id, 0);
    assert_eq!(
        evs[0]
            .assignments
            .iter()
            .map(|assignment| assignment.attacker_object_id)
            .collect::<Vec<_>>(),
        vec![bears]
    );
}

#[test]
fn preview_declare_attackers_is_rejected_by_engine() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        508,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    let idx_before = e.state.command_index;
    let cmd = RuledCommand {
        cmd: Some(Cmd::PreviewDeclareAttackers(PreviewDeclareAttackers {
            assignments: vec![],
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        507,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        506,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        606,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        707,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        808,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut declared_ids = ad[0]
        .assignments
        .iter()
        .map(|assignment| assignment.attacker_object_id)
        .collect::<Vec<_>>();
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        902,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
                    expected_zone_change_generation: None,
                    object_id: p0_bear,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        9200,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        9300,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
                    expected_zone_change_generation: None,
                    object_id: attacker,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
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
                expected_zone_change_generation: None,
                object_id: attacker,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        901,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        902,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        903,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        911,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        915,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    // Combat remains active through the end-of-combat step (CR 511.3).
    pass_both_players(&mut e);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndCombat);
    assert!(
        e.state.combat.is_some(),
        "combat remains through end combat"
    );

    // Combat ends as the end-of-combat step ends.
    pass_both_players(&mut e);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main2);
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        916,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
    // Combat remains active through the end-of-combat step (CR 511.3).
    pass_both_players(&mut e);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndCombat);
    assert!(
        e.state.combat.is_some(),
        "combat remains through end combat"
    );

    // Combat ends as the end-of-combat step ends.
    pass_both_players(&mut e);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main2);
    assert!(e.state.combat.is_none(), "combat cleared after end combat");
}

// ── Combat eligibility skip tests ────────────────────────────────────────────

#[test]
fn begin_combat_skips_when_no_eligible_attackers() {
    // Default deck has no creatures on the battlefield.
    // BeginCombat must auto-skip directly to EndCombat.
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4001,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4002,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4003,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4004,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4005,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
fn issue_464_combat_damage_records_the_source_incarnation() {
    let decks = Some(vec![
        vec!["grizzly_bears".into(); 7],
        vec!["island".into(); 7],
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        464_005,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let attacker = put_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let generation = engine
        .state
        .zone_change_generation
        .entry(attacker)
        .or_default();
    *generation += 1; // the fixture moves this real card from hand to the battlefield
    let generation = *generation;
    engine
        .apply_command(0, &primitive_yield())
        .expect("enter begin combat");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_both_players(&mut engine); // no eligible blockers; blockers are declared empty
    pass_both_players(&mut engine); // combat damage resolves

    assert_eq!(engine.state.players[1].life, 18);
    assert_eq!(
        engine.state.turn_history.current.dealt_damage_objects,
        vec![(attacker, generation)],
        "the combat fast path records the physical source incarnation; turn={}, step={:?}",
        engine.state.turn,
        engine.state.turn_step
    );
}

#[test]
fn cannot_add_mana_while_declaring_attackers() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4010,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
fn declared_attackers_keep_mana_until_declare_attackers_step_ends() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4012,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    // Model mana produced inside a legal declaration-payment window. Declare attackers does not
    // end the step; any unspent mana survives until the active player later passes the step.
    e.state.players[0].mana_pool.colorless = 1;
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attackers");

    assert_eq!(
        e.state.players[0].mana_pool.colorless, 1,
        "unused mana must remain through post-declaration priority"
    );

    e.apply_command(0, &pass())
        .expect("active pass attackers step");
    e.apply_command(1, &pass())
        .expect("defender pass attackers step");
    assert_eq!(
        e.state.players[0].mana_pool.colorless, 0,
        "unused mana clears when declare attackers actually ends"
    );
}

#[test]
fn propaganda_tax_is_cumulative_for_each_creature_attacking_a_player() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4022,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let first_attacker = e.state.players[0].battlefield[0];
    let second_attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");

    let batch = e
        .apply_command(0, &declare_attackers(vec![first_attacker, second_attacker]))
        .expect("both creatures can be selected before the combined tax is paid");
    let pending = batch.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("cumulative attack tax payment");
    assert_eq!(pending.generic_mana_cost, 8);
    assert!(e.state.objects[&first_attacker].tapped);
    assert!(e.state.objects[&second_attacker].tapped);
    assert!(!e.state.combat.as_ref().expect("combat").attackers_declared);
}

#[test]
fn propaganda_does_not_tax_a_creature_attacking_its_controllers_planeswalker() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4023,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let planeswalker = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    let assignment = tricerules_proto::ruled::v1::AttackAssignment {
        attacker_object_id: attacker,
        attacker_zone_change_generation: e
            .state
            .zone_change_generation
            .get(&attacker)
            .copied()
            .unwrap_or(0),
        defender: Some(tricerules_proto::ruled::v1::TargetRef {
            object_id: planeswalker,
            kind: tricerules_proto::ruled::v1::TargetRefKind::Permanent as i32,
            ..Default::default()
        }),
        defender_zone_change_generation: e
            .state
            .zone_change_generation
            .get(&planeswalker)
            .copied()
            .unwrap_or(0),
        defending_player_id: 1,
    };

    let batch = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(
                    tricerules_proto::ruled::v1::DeclareAttackers {
                        assignments: vec![assignment],
                    },
                )),
            },
        )
        .expect("Propaganda permits attacking its controller's planeswalker without a tax");
    assert!(e.state.pending_attack_declaration.is_none());
    assert!(e.state.combat.as_ref().expect("combat").attackers_declared);
    assert!(batch.events.iter().any(|event| matches!(
        event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::AttackersDeclared(_))
    )));
}

#[test]
fn propaganda_tax_uses_each_directly_attacked_player_in_multiplayer() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4024,
        &[0, 1, 2],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main phase to begin combat");
    let taxed_edge_attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let free_edge_attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    for _ in 0..3 {
        let player = e.state.priority_player_id();
        e.apply_command(player, &pass())
            .expect("pass beginning-of-combat priority");
    }
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    let taxed_defender = e.state.players[1].id;
    let free_defender = e.state.players[2].id;
    let assignment =
        |engine: &GameEngine, attacker, defender| tricerules_proto::ruled::v1::AttackAssignment {
            attacker_object_id: attacker,
            attacker_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&attacker)
                .copied()
                .unwrap_or(0),
            defender: Some(tricerules_proto::ruled::v1::TargetRef {
                object_id: defender as u32,
                kind: tricerules_proto::ruled::v1::TargetRefKind::Player as i32,
                ..Default::default()
            }),
            defending_player_id: defender,
            ..Default::default()
        };
    let batch = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(
                    tricerules_proto::ruled::v1::DeclareAttackers {
                        assignments: vec![
                            assignment(&e, taxed_edge_attacker, taxed_defender),
                            assignment(&e, free_edge_attacker, free_defender),
                        ],
                    },
                )),
            },
        )
        .expect("only the creature assigned to Propaganda's controller carries a tax");
    assert_eq!(
        batch.legal_by_player[&0]
            .pending_attack_declaration
            .as_ref()
            .expect("one taxed edge opens payment")
            .generic_mana_cost,
        2
    );
}

#[test]
fn propaganda_attack_declaration_waits_for_attack_tax_payment() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4013,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");

    let batch = e
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");

    assert!(
        !e.state.combat.as_ref().expect("combat").attackers_declared,
        "the chosen attacker must remain pending until its tax is paid"
    );
    assert!(
        e.state.objects.get(&attacker).expect("attacker").tapped,
        "the non-vigilance attacker is tapped at CR 508.1f before payment"
    );
    assert!(
        batch.events.iter().all(|event| !matches!(
            event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::AttackersDeclared(_))
        )),
        "the declaration event must wait for payment commit"
    );
    let pending = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("attack payment transaction");
    assert_eq!(pending.generic_mana_cost, 2, "Propaganda charges {{2}}");
    assert!(batch.events.iter().any(|event| matches!(
        event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::AttackPaymentRequired(_))
    )));
    let payer_offer = batch.legal_by_player.get(&0).expect("payer legal actions");
    let pending_offer = payer_offer
        .pending_attack_declaration
        .as_ref()
        .expect("payer payment offer");
    let preview = pending_offer
        .payment_preview
        .as_ref()
        .expect("source-less payment preview");
    assert!(preview.valid, "locked attack tax can be previewed");
    assert!(
        preview
            .selection
            .as_ref()
            .expect("payment selection")
            .source
            .is_none(),
        "attack tax payment is not bound to a fabricated card source"
    );
    assert!(batch
        .legal_by_player
        .get(&1)
        .expect("defender legal actions")
        .pending_attack_declaration
        .is_none());

    let pending = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack transaction")
        .clone();
    e.state.players[0].mana_pool.colorless = 1;
    let underpaid = e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CommitAttackDeclaration(
                tricerules_proto::ruled::v1::CommitAttackDeclaration {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    payment: Some(tricerules_proto::ruled::v1::PaymentSelection {
                        expected_state_revision: e.state.command_index,
                        mana: Some(tricerules_proto::ruled::v1::PaymentMana {
                            c: 2,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
        },
    );
    assert!(underpaid.is_err(), "partial attack-tax payment is rejected");
    assert_eq!(e.state.players[0].mana_pool.colorless, 1);
    assert!(e.state.pending_attack_declaration.is_some());
    assert!(!e.state.combat.as_ref().expect("combat").attackers_declared);

    e.state.players[0].mana_pool.colorless = 2;
    let committed = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitAttackDeclaration(
                    tricerules_proto::ruled::v1::CommitAttackDeclaration {
                        transaction_id: pending.transaction_id,
                        expected_revision: pending.revision,
                        payment: Some(tricerules_proto::ruled::v1::PaymentSelection {
                            expected_state_revision: e.state.command_index,
                            mana: Some(tricerules_proto::ruled::v1::PaymentMana {
                                c: 2,
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("pay locked attack tax");
    assert!(e.state.pending_attack_declaration.is_none());
    assert!(e.state.combat.as_ref().expect("combat").attackers_declared);
    assert!(committed.events.iter().any(|event| matches!(
        event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::AttackersDeclared(_))
    )));
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn propaganda_rejects_stale_or_wrong_payer_commits_without_spending_mana() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4029,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    e.state.players[0].mana_pool.colorless = 2;
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin attack tax payment");
    let pending = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack declaration")
        .clone();
    let initial_command_index = e.state.command_index;

    let commit = |transaction_id, expected_revision, expected_state_revision| RuledCommand {
        cmd: Some(Cmd::CommitAttackDeclaration(
            tricerules_proto::ruled::v1::CommitAttackDeclaration {
                transaction_id,
                expected_revision,
                payment: Some(tricerules_proto::ruled::v1::PaymentSelection {
                    expected_state_revision,
                    mana: Some(tricerules_proto::ruled::v1::PaymentMana {
                        c: 2,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )),
    };

    assert!(e
        .apply_command(
            1,
            &commit(
                pending.transaction_id,
                pending.revision,
                initial_command_index
            )
        )
        .is_err());
    assert!(e
        .apply_command(
            0,
            &commit(
                pending.transaction_id,
                pending.revision + 1,
                initial_command_index
            )
        )
        .is_err());
    assert!(e
        .apply_command(
            0,
            &commit(
                pending.transaction_id + 1,
                pending.revision,
                initial_command_index
            )
        )
        .is_err());
    assert!(e
        .apply_command(
            0,
            &commit(
                pending.transaction_id,
                pending.revision,
                initial_command_index - 1
            )
        )
        .is_err());

    assert_eq!(e.state.command_index, initial_command_index);
    assert_eq!(e.state.players[0].mana_pool.colorless, 2);
    assert_eq!(e.state.pending_attack_declaration.as_ref(), Some(&pending));
    assert!(!e.state.combat.as_ref().expect("combat").attackers_declared);

    e.apply_command(
        0,
        &commit(
            pending.transaction_id,
            pending.revision,
            e.state.command_index,
        ),
    )
    .expect("a valid retry remains available after every rejected commit");
    assert!(e.state.pending_attack_declaration.is_none());
    assert!(e.state.combat.as_ref().expect("combat").attackers_declared);
}

#[test]
fn propaganda_commit_omits_an_attacker_that_left_and_returned_during_payment() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4030,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin attack tax payment");
    let pending = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack declaration")
        .clone();
    let original_generation = e
        .state
        .zone_change_generation
        .get(&attacker)
        .copied()
        .unwrap_or(0);

    e.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != attacker);
    e.state.players[0].graveyard.push(attacker);
    {
        let object = e.state.objects.get_mut(&attacker).expect("attacker object");
        object.zone = tricerules_core::Zone::Graveyard;
        object.tapped = false;
    }
    e.state
        .zone_change_generation
        .insert(attacker, original_generation + 1);
    e.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != attacker);
    e.state.players[0].battlefield.push(attacker);
    {
        let object = e.state.objects.get_mut(&attacker).expect("returned object");
        object.zone = tricerules_core::Zone::Battlefield;
        object.tapped = false;
        object.summoning_sick = true;
    }
    e.state.players[0].mana_pool.colorless = 2;

    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CommitAttackDeclaration(
                tricerules_proto::ruled::v1::CommitAttackDeclaration {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    payment: Some(tricerules_proto::ruled::v1::PaymentSelection {
                        expected_state_revision: e.state.command_index,
                        mana: Some(tricerules_proto::ruled::v1::PaymentMana {
                            c: 2,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
        },
    )
    .expect("the locked tax remains payable after the original attacker leaves");

    let combat = e.state.combat.as_ref().expect("combat remains active");
    assert!(combat.attackers_declared);
    assert!(combat.attacking.is_empty());
    assert!(!combat.attack_assignments.contains_key(&attacker));
    assert_eq!(
        e.state.zone_change_generation[&attacker],
        original_generation + 1
    );
}

#[test]
fn propaganda_attack_payment_commands_replay_identically() {
    fn prepared() -> (GameEngine, u32, [u32; 3]) {
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            4031,
            &[0, 1],
            20,
            None,
            true,
        )
        .expect("new");
        advance_to_declare_attackers(&mut e);
        let attacker = e.state.players[0].battlefield[0];
        let forests = [
            inject_permanent_on_battlefield(&mut e, 0, "forest"),
            inject_permanent_on_battlefield(&mut e, 0, "forest"),
            inject_permanent_on_battlefield(&mut e, 0, "forest"),
        ];
        inject_permanent_on_battlefield(&mut e, 1, "propaganda");
        (e, attacker, forests)
    }

    let (mut original, attacker, forests) = prepared();
    let (mut replay, replay_attacker, replay_forests) = prepared();
    assert_eq!((attacker, forests), (replay_attacker, replay_forests));

    let declare = declare_attackers(vec![attacker]);
    let pending = original.apply_command(0, &declare).expect("begin payment");
    let replay_pending = replay
        .apply_command(0, &declare)
        .expect("replay begin payment");
    assert_eq!(pending, replay_pending);

    let mut last_activation_batch = None;
    for forest in forests {
        let activation = activate_ability_for(&original, forest, 0, vec![]);
        let expected = original
            .apply_command(0, &activation)
            .expect("accepted mana ability");
        assert_eq!(
            replay
                .apply_command(0, &activation)
                .expect("replay mana ability"),
            expected
        );
        last_activation_batch = Some(expected);
    }

    let payment_window = original
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending payment")
        .clone();
    let payer_pending = last_activation_batch
        .as_ref()
        .expect("last activation batch")
        .legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("payer-visible pending payment");
    let first_receipt_index = payer_pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == forests[0])
        .expect("first mana receipt")
        .activation_command_index;
    let undo_first = RuledCommand {
        cmd: Some(Cmd::UndoManaAbility(
            tricerules_proto::ruled::v1::UndoManaAbility {
                attack_transaction_id: payment_window.transaction_id,
                activation_command_index: first_receipt_index,
            },
        )),
    };
    let undo_batch = original
        .apply_command(0, &undo_first)
        .expect("undo one mana activation while retaining two others");
    assert_eq!(
        replay
            .apply_command(0, &undo_first)
            .expect("replay selective mana undo"),
        undo_batch
    );

    let payment_window = original
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending payment")
        .clone();
    let commit = RuledCommand {
        cmd: Some(Cmd::CommitAttackDeclaration(
            tricerules_proto::ruled::v1::CommitAttackDeclaration {
                transaction_id: payment_window.transaction_id,
                expected_revision: payment_window.revision,
                payment: Some(tricerules_proto::ruled::v1::PaymentSelection {
                    expected_state_revision: original.state.command_index,
                    mana: Some(tricerules_proto::ruled::v1::PaymentMana {
                        g: 2,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )),
    };
    let expected = original
        .apply_command(0, &commit)
        .expect("commit the locked attack tax");
    assert_eq!(
        replay
            .apply_command(0, &commit)
            .expect("replay attack commit"),
        expected
    );
    assert_eq!(
        replay.diagnostic_snapshot().expect("replay snapshot"),
        original.diagnostic_snapshot().expect("original snapshot")
    );
}

fn grant_sacrificing_fixture_mana(engine: &mut GameEngine, source: u32, death_trigger: bool) {
    use tricerules_cards::{ContinuousEffectKind, EffectDuration};
    use tricerules_core::state::{AffectedScope, ContinuousEffect};

    let fixture = r#"(id: "propaganda_receipt_fixture", name: "Propaganda Receipt Fixture",
        face_id: "propaganda_receipt_fixture", types: ["Creature"], power: 1, toughness: 1,
        activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
            costs: [SacrificeSelf], effect: [ProduceMana(options: [(c: 1)])])],
        triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
            trigger: WheneverCreatureDies(controller: AnyPlayer, filter: (exclude_source: false)),
            effect: [GainLife(amount: 1)])])"#;
    let registry = tricerules_cards::CardRegistry::from_chunks_and_tokens(&[fixture], &[])
        .expect("scenario receipt fixture is valid card data");
    let face = registry
        .get("propaganda_receipt_fixture")
        .expect("scenario receipt fixture is registered")
        .primary_face();
    engine.state.add_activated_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantActivatedAbility(Box::new(
            face.activated_abilities[0].clone(),
        )),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    if death_trigger {
        engine.state.add_triggered_ability_grant(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(
                face.triggered_abilities[0].clone(),
            )),
            condition: None,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp: engine.state.command_index,
        });
    }
    engine.initial_response_batch();
}

fn grant_once_per_turn_tap_observer(engine: &mut GameEngine, source: u32) {
    use tricerules_cards::{ContinuousEffectKind, EffectDuration};
    use tricerules_core::state::{AffectedScope, ContinuousEffect};

    let fixture = r#"(id: "propaganda_tap_observer_fixture", name: "Propaganda Tap Observer",
        face_id: "propaganda_tap_observer_fixture", types: ["Creature"], power: 1, toughness: 1,
        triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
            trigger: WheneverPlayerTapsCreature(player: AnyPlayer, controllers: All,
                cardinality: OneOrMorePerAction), max_triggers_per_turn: Some(1),
            effect: [GainLife(amount: 1)])])"#;
    let registry = tricerules_cards::CardRegistry::from_chunks_and_tokens(&[fixture], &[])
        .expect("scenario tap observer fixture is valid card data");
    let ability = registry
        .get("propaganda_tap_observer_fixture")
        .expect("scenario tap observer fixture is registered")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    engine.initial_response_batch();
}

fn grant_life_gain_observer(engine: &mut GameEngine, source: u32) {
    use tricerules_cards::ContinuousEffectKind;
    use tricerules_core::state::{AffectedScope, ContinuousEffect};

    let fixture = r#"(id: "propaganda_life_gain_observer", name: "Propaganda Life Gain Observer",
        face_id: "propaganda_life_gain_observer", types: ["Artifact"],
        triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
            trigger: WheneverPlayerGainsLife(player: Controller),
            effect: [GainLife(amount: 1)])])"#;
    let registry = tricerules_cards::CardRegistry::from_chunks_and_tokens(&[fixture], &[])
        .expect("scenario life-gain observer fixture is valid card data");
    let ability = registry
        .get("propaganda_life_gain_observer")
        .expect("scenario life-gain observer fixture is registered")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: tricerules_cards::EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    engine.initial_response_batch();
}

fn grant_lifelink(engine: &mut GameEngine, source: u32) {
    use tricerules_cards::{ContinuousEffectKind, EffectDuration, Keyword};
    use tricerules_core::state::{AffectedScope, ContinuousEffect};

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    engine.initial_response_batch();
}

#[test]
fn propaganda_reapply_does_not_collect_a_newly_restored_death_observer() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4026,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let observer = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let later_source = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    grant_sacrificing_fixture_mana(&mut e, observer, true);
    grant_sacrificing_fixture_mana(&mut e, later_source, false);

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    e.apply_command(0, &activate_ability_for(&e, observer, 1, vec![]))
        .expect("the observer sacrifices itself and sees its own death");
    let later_batch = e
        .apply_command(0, &activate_ability_for(&e, later_source, 1, vec![]))
        .expect("the later source sacrifices after the observer has left");
    assert_eq!(e.state.staged_trigger_groups.len(), 1);

    let pending = later_batch.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("payer attack payment projection");
    let receipt = pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == observer)
        .expect("observer activation receipt");
    assert!(receipt.reversible);

    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::UndoManaAbility(
                tricerules_proto::ruled::v1::UndoManaAbility {
                    attack_transaction_id: pending.transaction_id,
                    activation_command_index: receipt.activation_command_index,
                },
            )),
        },
    )
    .expect("undo the observer while retaining the later mana ability");

    assert!(
        e.state.staged_trigger_groups.is_empty(),
        "the restored observer was not present when the later sacrifice happened"
    );
}

#[test]
fn propaganda_reapply_preserves_event_time_lifelink_triggers() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4028,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let petal = inject_permanent_on_battlefield(&mut e, 0, "lotus_petal");
    let talisman = inject_permanent_on_battlefield(&mut e, 0, "talisman_of_impulse");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    grant_life_gain_observer(&mut e, petal);
    grant_lifelink(&mut e, talisman);

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    let transaction_id = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack payment")
        .transaction_id;
    let mut petal_activation = activate_ability_for(&e, petal, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = petal_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 4;
    e.apply_command(0, &petal_activation)
        .expect("Lotus Petal sacrifices itself for one mana");

    let mut talisman_activation = activate_ability_for(&e, talisman, 1, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = talisman_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 0;
    let talisman_batch = e
        .apply_command(0, &talisman_activation)
        .expect("Talisman damage and lifelink resolve during payment");
    assert_eq!(e.state.players[0].life, 20);
    assert!(e.state.staged_trigger_groups.is_empty());

    let petal_receipt = talisman_batch.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("pending payment")
        .mana_ability_undo_options
        .iter()
        .find(|receipt| receipt.source_object_id == petal)
        .expect("reversible Lotus Petal receipt");
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::UndoManaAbility(
                tricerules_proto::ruled::v1::UndoManaAbility {
                    attack_transaction_id: transaction_id,
                    activation_command_index: petal_receipt.activation_command_index,
                },
            )),
        },
    )
    .expect("undo Lotus Petal while retaining the independent Talisman receipt");

    assert_eq!(e.state.players[0].life, 20);
    assert!(
        e.state.staged_trigger_groups.is_empty(),
        "the restored observer did not see the earlier Talisman lifelink event"
    );
    let pending = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("attack payment remains pending")
        .clone();
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CancelAttackDeclaration(
                tricerules_proto::ruled::v1::CancelAttackDeclaration {
                    transaction_id,
                    expected_revision: pending.revision,
                },
            )),
        },
    )
    .expect("cancel the declaration without recollecting Talisman's damage triggers");
    assert!(e.state.pending_attack_declaration.is_none());
    assert_eq!(e.state.players[0].life, 20);
    assert!(e.state.staged_trigger_groups.is_empty());
}

#[test]
fn propaganda_attack_tap_reserves_once_per_turn_trigger_before_payment_mana_abilities() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4025,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let mana_creature = inject_creature_on_battlefield(&mut e, 0, "llanowar_elves");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    let observer = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    grant_once_per_turn_tap_observer(&mut e, observer);

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("the attack tap is reserved while the attack payment waits");

    assert_eq!(
        e.state
            .trigger_uses_this_turn
            .values()
            .copied()
            .sum::<u32>(),
        1,
        "Sharae's once-per-turn trigger belongs to the earlier attack-tap event"
    );

    e.apply_command(0, &activate_ability_for(&e, mana_creature, 0, vec![]))
        .expect("a later mana ability remains legal during the payment window");
    assert_eq!(
        e.state
            .trigger_uses_this_turn
            .values()
            .copied()
            .sum::<u32>(),
        1,
        "the later Forest tap cannot take the trigger cap from the earlier attack tap"
    );
}

#[test]
fn propaganda_payer_concession_clears_pending_attack_for_survivors() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4027,
        &[0, 1, 2],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    e.apply_command(0, &primitive_yield())
        .expect("main phase to begin combat");
    for _ in 0..3 {
        let player = e.state.priority_player_id();
        e.apply_command(player, &pass())
            .expect("pass beginning-of-combat priority");
    }
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    let assignment = e.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|assignment| {
            assignment.attacker_object_id == attacker && assignment.defending_player_id == 1
        })
        .copied()
        .expect("legal attack against the taxing player");
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::DeclareAttackers(
                tricerules_proto::ruled::v1::DeclareAttackers {
                    assignments: vec![assignment],
                },
            )),
        },
    )
    .expect("open attack tax payment");
    assert!(e.state.pending_attack_declaration.is_some());

    let departure = e
        .apply_command(0, &concede())
        .expect("the payer may concede during attack payment");
    assert!(e.state.pending_attack_declaration.is_none());
    assert!(e.state.players[1..].iter().all(|player| !player.has_lost));
    assert_ne!(e.state.priority_player_id(), 0);
    assert!(!departure.legal_by_player.contains_key(&0));
    assert!(e
        .apply_command(e.state.priority_player_id(), &pass())
        .is_ok());
}

#[test]
fn propaganda_mana_receipt_reports_later_spend_dependency() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4020,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let capital_city = inject_permanent_on_battlefield(&mut e, 0, "capital_city");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    let transaction_id = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack transaction")
        .transaction_id;

    e.apply_command(0, &activate_ability_for(&e, forest, 0, vec![]))
        .expect("Forest produces the green mana spent by Capital City");
    let city_batch = e
        .apply_command(0, &activate_ability_for(&e, capital_city, 1, vec![]))
        .expect("Capital City spends green and produces white mana");
    let pending = city_batch.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack payment");
    let forest_receipt = pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == forest)
        .expect("Forest receipt");
    assert!(!forest_receipt.reversible);
    assert!(forest_receipt
        .unavailable_reason
        .contains("spent on a later mana ability"));

    let city_command_index = pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == capital_city)
        .expect("Capital City receipt")
        .activation_command_index;
    let after_city_undo = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::UndoManaAbility(
                    tricerules_proto::ruled::v1::UndoManaAbility {
                        attack_transaction_id: transaction_id,
                        activation_command_index: city_command_index,
                    },
                )),
            },
        )
        .expect("undo Capital City first");
    assert_eq!(e.state.players[0].mana_pool.green, 1);
    let forest_receipt = after_city_undo.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack payment")
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == forest)
        .expect("Forest receipt becomes reversible after its dependent receipt is removed");
    assert!(forest_receipt.reversible);
    assert!(forest_receipt.unavailable_reason.is_empty());
}

#[test]
fn propaganda_background_mana_is_attributed_before_receipt_mana() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4021,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let capital_city = inject_permanent_on_battlefield(&mut e, 0, "capital_city");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    e.state.players[0].mana_pool.green = 1;
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    let transaction_id = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack transaction")
        .transaction_id;

    e.apply_command(0, &activate_ability_for(&e, forest, 0, vec![]))
        .expect("Forest produces another green mana");
    let city_batch = e
        .apply_command(0, &activate_ability_for(&e, capital_city, 1, vec![]))
        .expect("Capital City spends one of the two green mana");
    let pending = city_batch.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack payment");
    let forest_command_index = pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == forest)
        .expect("Forest receipt")
        .activation_command_index;
    let forest_receipt = pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == forest)
        .expect("Forest receipt");
    assert!(forest_receipt.reversible);
    assert!(forest_receipt.unavailable_reason.is_empty());

    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::UndoManaAbility(
                tricerules_proto::ruled::v1::UndoManaAbility {
                    attack_transaction_id: transaction_id,
                    activation_command_index: forest_command_index,
                },
            )),
        },
    )
    .expect("background green mana pays the retained Capital City receipt");
    assert_eq!(e.state.players[0].mana_pool.green, 0);
    assert_eq!(e.state.players[0].mana_pool.white, 1);
    assert!(!e.state.objects.get(&forest).expect("Forest").tapped);
    assert!(
        e.state
            .objects
            .get(&capital_city)
            .expect("Capital City")
            .tapped
    );
    assert!(e.state.pending_attack_declaration.is_some());
}

#[test]
fn propaganda_rebases_mana_lineage_after_undoing_an_intermediate_receipt() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4032,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let first_city = inject_permanent_on_battlefield(&mut e, 0, "capital_city");
    let second_city = inject_permanent_on_battlefield(&mut e, 0, "capital_city");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    e.state.players[0].mana_pool.green = 1;
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    let transaction_id = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack transaction")
        .transaction_id;

    e.apply_command(0, &activate_ability_for(&e, forest, 0, vec![]))
        .expect("Forest A adds green to the payment pool");
    let mut first_city_activation = activate_ability_for(&e, first_city, 1, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = first_city_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 0;
    activation.payment = Some(tricerules_proto::ruled::v1::PaymentSelection {
        expected_state_revision: e.state.command_index,
        source: Some(tricerules_proto::ruled::v1::CostObjectRef {
            object_id: first_city,
            zone_change_generation: e
                .state
                .zone_change_generation
                .get(&first_city)
                .copied()
                .unwrap_or(0),
        }),
        mana: Some(tricerules_proto::ruled::v1::PaymentMana {
            g: 1,
            ..Default::default()
        }),
        ..Default::default()
    });
    e.apply_command(0, &first_city_activation)
        .expect("Capital City B spends the pre-window green mana");
    assert_eq!(e.state.players[0].mana_pool.green, 1);
    assert_eq!(e.state.players[0].mana_pool.white, 1);
    let mut second_city_activation = activate_ability_for(&e, second_city, 1, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = second_city_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 0;
    activation.payment = Some(tricerules_proto::ruled::v1::PaymentSelection {
        expected_state_revision: e.state.command_index,
        source: Some(tricerules_proto::ruled::v1::CostObjectRef {
            object_id: second_city,
            zone_change_generation: e
                .state
                .zone_change_generation
                .get(&second_city)
                .copied()
                .unwrap_or(0),
        }),
        mana: Some(tricerules_proto::ruled::v1::PaymentMana {
            g: 1,
            ..Default::default()
        }),
        ..Default::default()
    });
    let third_activation = e
        .apply_command(0, &second_city_activation)
        .expect("Capital City C spends Forest A's green mana");
    assert_eq!(e.state.players[0].mana_pool.green, 0);
    assert_eq!(e.state.players[0].mana_pool.white, 2);
    let pending = third_activation.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("pending payment");
    let forest_option = pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == forest)
        .expect("Forest A receipt");
    assert!(
        !forest_option.reversible,
        "pre-rebase options: {:?}",
        pending.mana_ability_undo_options
    );
    let first_city_index = pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == first_city)
        .expect("Capital City B receipt")
        .activation_command_index;

    let city_undo = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::UndoManaAbility(
                    tricerules_proto::ruled::v1::UndoManaAbility {
                        attack_transaction_id: transaction_id,
                        activation_command_index: first_city_index,
                    },
                )),
            },
        )
        .expect("undo Capital City B while rebasing retained Capital City C");

    let pending = city_undo.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("pending payment remains open");
    let forest_option = pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == forest)
        .expect("Forest A receipt after rebase");
    assert!(
        forest_option.reversible,
        "rebased Capital City C now spends background green, not Forest A's output"
    );
    let forest_index = forest_option.activation_command_index;
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::UndoManaAbility(
                tricerules_proto::ruled::v1::UndoManaAbility {
                    attack_transaction_id: transaction_id,
                    activation_command_index: forest_index,
                },
            )),
        },
    )
    .expect("undo Forest A after its output is no longer spent");
    assert!(!e.state.objects.get(&forest).expect("Forest").tapped);
    assert!(!e.state.objects.get(&first_city).expect("City B").tapped);
    assert!(e.state.objects.get(&second_city).expect("City C").tapped);
    assert_eq!(e.state.players[0].mana_pool.green, 0);
    assert_eq!(e.state.players[0].mana_pool.white, 1);
}

#[test]
fn propaganda_attack_payment_allows_and_undoes_mana_ability() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4014,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    let transaction_id = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack transaction")
        .transaction_id;

    let activation_batch = e
        .apply_command(0, &activate_ability_for(&e, forest, 0, vec![]))
        .expect("mana ability is allowed for the locked attack tax");
    assert_eq!(e.state.players[0].mana_pool.green, 1);
    assert!(e.state.objects.get(&forest).expect("forest").tapped);
    let activation_command_index = e.state.command_index - 1;
    assert_eq!(
        activation_batch.legal_by_player[&0].undoable_mana_abilities, 1,
        "the payer's Undo control must be available for the reversible attack receipt"
    );
    assert_eq!(
        activation_batch.legal_by_player[&0]
            .pending_attack_declaration
            .as_ref()
            .expect("still-pending payment")
            .mana_ability_undo_options
            .iter()
            .map(|option| option.activation_command_index)
            .collect::<Vec<_>>(),
        vec![activation_command_index]
    );

    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::UndoManaAbility(
                tricerules_proto::ruled::v1::UndoManaAbility {
                    attack_transaction_id: transaction_id,
                    activation_command_index,
                },
            )),
        },
    )
    .expect("undo the float while the declaration payment is pending");
    assert_eq!(e.state.players[0].mana_pool.green, 0);
    assert!(!e.state.objects.get(&forest).expect("forest").tapped);
    assert!(e.state.pending_attack_declaration.is_some());
}

#[test]
fn propaganda_undoes_earlier_cost_triggering_mana_ability_and_keeps_later_float() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4016,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let petal = inject_permanent_on_battlefield(&mut e, 0, "lotus_petal");
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    let initial_hand_size = e.state.players[0].hand.len();
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    let transaction_id = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack transaction")
        .transaction_id;

    let mut petal_activation = activate_ability_for(&e, petal, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = petal_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 4;
    e.apply_command(0, &petal_activation)
        .expect("Lotus Petal produces green mana and its death trigger is parked");
    let forest_batch = e
        .apply_command(0, &activate_ability_for(&e, forest, 0, vec![]))
        .expect("later Forest activation remains independent");
    let pending = forest_batch.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("still-pending attack payment");
    let petal_receipt = pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == petal)
        .expect("Lotus Petal activation receipt");
    assert!(
        petal_receipt.reversible,
        "a cost-triggering, non-library mana ability can be reversed while a later independent receipt remains"
    );

    let undone = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::UndoManaAbility(
                    tricerules_proto::ruled::v1::UndoManaAbility {
                        attack_transaction_id: transaction_id,
                        activation_command_index: petal_receipt.activation_command_index,
                    },
                )),
            },
        )
        .expect("reverse Lotus Petal without replaying Forest");

    assert_eq!(e.state.players[0].mana_pool.green, 1);
    assert_eq!(e.state.players[0].hand.len(), initial_hand_size);
    assert!(!e.state.objects.get(&petal).expect("petal object").tapped);
    assert_eq!(
        e.state.objects.get(&petal).expect("petal object").zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(e.state.objects.get(&forest).expect("forest").tapped);
    assert!(e.state.pending_attack_declaration.is_some());
    assert!(e.state.staged_trigger_groups.is_empty());
    assert!(e.state.pending_triggers.is_empty());
    assert!(undone.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(Ev::PermanentMoved(moved))
            if moved.object_id == petal
                && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield as i32
    )));
}

#[test]
fn propaganda_undoes_earlier_sacrifice_mana_ability_and_keeps_later_sacrifice_activation() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4017,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let first_petal = inject_permanent_on_battlefield(&mut e, 0, "lotus_petal");
    let later_petal = inject_permanent_on_battlefield(&mut e, 0, "lotus_petal");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    let transaction_id = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack transaction")
        .transaction_id;

    let mut first_activation = activate_ability_for(&e, first_petal, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = first_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 4;
    e.apply_command(0, &first_activation)
        .expect("first Lotus Petal activation");
    let mut later_activation = activate_ability_for(&e, later_petal, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = later_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 4;
    let later_batch = e
        .apply_command(0, &later_activation)
        .expect("later independent Lotus Petal activation");

    let pending = later_batch.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("still-pending attack payment");
    let first_receipt = pending
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == first_petal)
        .expect("first Lotus Petal activation receipt");
    assert!(
        first_receipt.reversible,
        "the earlier receipt remains reversible when a later independent mana ability paid its own sacrifice cost"
    );
    let first_command_index = first_receipt.activation_command_index;

    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::UndoManaAbility(
                tricerules_proto::ruled::v1::UndoManaAbility {
                    attack_transaction_id: transaction_id,
                    activation_command_index: first_command_index,
                },
            )),
        },
    )
    .expect("undo first Petal while retaining second Petal");

    assert_eq!(e.state.players[0].mana_pool.green, 1);
    assert_eq!(
        e.state.objects.get(&first_petal).expect("first Petal").zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(
        !e.state
            .objects
            .get(&first_petal)
            .expect("first Petal")
            .tapped
    );
    assert_eq!(
        e.state.objects.get(&later_petal).expect("later Petal").zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(e.state.pending_attack_declaration.is_some());
}

#[test]
fn propaganda_undo_preserves_later_mana_ability_damage_prevention_result() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4018,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let first_coast = inject_permanent_on_battlefield(&mut e, 0, "yavimaya_coast");
    let later_coast = inject_permanent_on_battlefield(&mut e, 0, "yavimaya_coast");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    let transaction_id = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack transaction")
        .transaction_id;
    e.state.add_damage_prevention_shield(0, 1);

    let mut first_activation = activate_ability_for(&e, first_coast, 1, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = first_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 0;
    e.apply_command(0, &first_activation)
        .expect("first Coast ability is prevented");
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.remaining_damage_prevention(0), 0);

    let mut later_activation = activate_ability_for(&e, later_coast, 1, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = later_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 0;
    let later_batch = e
        .apply_command(0, &later_activation)
        .expect("later Coast ability resolves against the exhausted shield");
    assert_eq!(e.state.players[0].life, 19);

    let first_command_index = later_batch.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("still-pending attack payment")
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == first_coast)
        .expect("first Coast receipt")
        .activation_command_index;
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::UndoManaAbility(
                tricerules_proto::ruled::v1::UndoManaAbility {
                    attack_transaction_id: transaction_id,
                    activation_command_index: first_command_index,
                },
            )),
        },
    )
    .expect("undo the earlier activation while retaining the later receipt");

    assert_eq!(
        e.state.players[0].life,
        19,
        "the retained ability keeps its original damage result instead of consuming a restored shield"
    );
    assert_eq!(e.state.remaining_damage_prevention(0), 1);
    assert_eq!(e.state.players[0].mana_pool.green, 1);
    assert!(
        !e.state
            .objects
            .get(&first_coast)
            .expect("first Coast")
            .tapped
    );
    assert!(
        e.state
            .objects
            .get(&later_coast)
            .expect("later Coast")
            .tapped
    );
}

#[test]
fn propaganda_undo_reuses_later_prevention_choice_and_finite_shield_receipt() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4019,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let first_coast = inject_permanent_on_battlefield(&mut e, 0, "yavimaya_coast");
    let later_coast = inject_permanent_on_battlefield(&mut e, 0, "yavimaya_coast");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    let transaction_id = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack transaction")
        .transaction_id;
    for _ in 0..3 {
        e.state.add_damage_prevention_shield(0, 1);
    }
    let original_effect_ids = e
        .state
        .damage_prevention_effects
        .iter()
        .map(|effect| effect.id)
        .collect::<std::collections::BTreeSet<_>>();

    let mut first_activation = activate_ability_for(&e, first_coast, 1, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = first_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 0;
    e.apply_command(0, &first_activation)
        .expect("first Coast damage asks for prevention ordering");
    let first_choice = e
        .state
        .pending_resolution
        .as_ref()
        .expect("first prevention choice")
        .presentation
        .candidates[0];
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(
                tricerules_proto::ruled::v1::SubmitResolutionChoice {
                    chosen_object_ids: vec![first_choice],
                    ..Default::default()
                },
            )),
        },
    )
    .expect("complete first damage prevention choice");
    let after_first_effect_ids = e
        .state
        .damage_prevention_effects
        .iter()
        .map(|effect| effect.id)
        .collect::<std::collections::BTreeSet<_>>();

    let mut later_activation = activate_ability_for(&e, later_coast, 1, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = later_activation.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 0;
    e.apply_command(0, &later_activation)
        .expect("later Coast damage asks for its own prevention ordering");
    let later_choice = e
        .state
        .pending_resolution
        .as_ref()
        .expect("later prevention choice")
        .presentation
        .candidates[0];
    let later_resolved = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(
                    tricerules_proto::ruled::v1::SubmitResolutionChoice {
                        chosen_object_ids: vec![later_choice],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("complete later damage prevention choice");
    assert!(e.state.pending_resolution.is_none());
    let after_later_effect_ids = e
        .state
        .damage_prevention_effects
        .iter()
        .map(|effect| effect.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(after_first_effect_ids.len(), 2);
    assert_eq!(after_later_effect_ids.len(), 1);

    let first_command_index = later_resolved.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("still-pending attack payment")
        .mana_ability_undo_options
        .iter()
        .find(|option| option.source_object_id == first_coast)
        .expect("first Coast receipt")
        .activation_command_index;
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::UndoManaAbility(
                tricerules_proto::ruled::v1::UndoManaAbility {
                    attack_transaction_id: transaction_id,
                    activation_command_index: first_command_index,
                },
            )),
        },
    )
    .expect("undo first activation while preserving the later chosen result");

    let consumed_by_later = after_first_effect_ids
        .difference(&after_later_effect_ids)
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let expected_effect_ids = original_effect_ids
        .difference(&consumed_by_later)
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let actual_effect_ids = e
        .state
        .damage_prevention_effects
        .iter()
        .map(|effect| effect.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual_effect_ids, expected_effect_ids);
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[0].mana_pool.green, 1);
    assert!(e.state.pending_resolution.is_none());
    assert!(
        !e.state
            .objects
            .get(&first_coast)
            .expect("first Coast")
            .tapped
    );
    assert!(
        e.state
            .objects
            .get(&later_coast)
            .expect("later Coast")
            .tapped
    );
}

#[test]
fn propaganda_cancellation_reverses_attack_tap_but_keeps_mana_ability() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4015,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = e.state.players[0].battlefield[0];
    let mana_creature = inject_creature_on_battlefield(&mut e, 0, "llanowar_elves");
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    let observer = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    grant_once_per_turn_tap_observer(&mut e, observer);
    let begin = e
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("begin declaration payment");
    let pending = e
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack declaration")
        .clone();
    e.apply_command(0, &activate_ability_for(&e, mana_creature, 0, vec![]))
        .expect("mana ability resolves");

    let canceled = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CancelAttackDeclaration(
                    tricerules_proto::ruled::v1::CancelAttackDeclaration {
                        transaction_id: pending.transaction_id,
                        expected_revision: pending.revision,
                    },
                )),
            },
        )
        .expect("cancel attack declaration");
    assert_eq!(e.state.players[0].mana_pool.green, 1);
    assert!(
        e.state
            .objects
            .get(&mana_creature)
            .expect("mana creature")
            .tapped
    );
    assert!(!e.state.objects.get(&attacker).expect("attacker").tapped);
    assert!(e.state.pending_attack_declaration.is_none());
    assert!(!e.state.combat.as_ref().expect("combat").attackers_declared);
    assert!(begin
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::AttackPaymentRequired(_)))));
    assert!(canceled
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::PermanentsUntapped(_)))));
    assert_eq!(
        e.state
            .trigger_uses_this_turn
            .values()
            .copied()
            .sum::<u32>(),
        1,
        "only the retained mana-creature tap consumes the once-per-turn trigger use after cancellation"
    );
    assert_eq!(e.state.stack.len(), 1);
    assert_eq!(e.state.stack[0].source_permanent_id, Some(observer));
}

#[test]
fn cannot_add_mana_while_declaring_blockers() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4011,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        11_006,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        11_007,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        11_008,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5008,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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
                    expected_zone_change_generation: None,
                    object_id: bystander,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
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
                expected_zone_change_generation: None,
                object_id: attacker,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5500,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5501,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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

/// CR 508.1d: a player is not required to pay an attack cost merely to obey a must-attack
/// requirement when every legal attack edge requires that payment.
#[test]
fn must_attack_creature_with_only_taxed_attack_edge_may_skip() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5505,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    let goblin = inject_creature_on_battlefield(&mut e, 0, "crazed_goblin");
    e.state
        .objects
        .get_mut(&goblin)
        .unwrap()
        .must_attack_if_able = true;
    inject_permanent_on_battlefield(&mut e, 1, "propaganda");

    e.apply_command(0, &declare_attackers(vec![]))
        .expect("a costed attack is optional even for a must-attack creature");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndCombat);
}

/// CR 508.1d "if able": a must-attack creature that is summoning-sick is NOT required to attack.
#[test]
fn must_attack_creature_summoning_sick_may_skip() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5502,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5503,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5504,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5510,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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
        legal.attack_requirement_ids.contains(&goblin),
        "active player's LegalActions must list the must-attack Crazed Goblin"
    );
    // The non-active player is never asked to declare attackers.
    let legal_nap = batch.legal_by_player.get(&1).expect("legal for P1");
    assert!(
        legal_nap.attack_requirement_ids.is_empty(),
        "defender has no required attackers"
    );
}

/// CR 509.1c: the defending player's LegalActions must surface the must-block creature id so the
/// client can gate its confirm-blockers control identically to the engine's set_blockers check.
#[test]
fn legal_actions_surface_required_blocker_to_defender() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5511,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
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

/// CR 615.1 / 702.15b: combat damage is dealt simultaneously, but prevention is applied per
/// *source* and lifelink counts the damage that source actually dealt. With a 3-point shield on
/// the attacker and two 2-power blockers, the shield fully absorbs the first blocker's damage —
/// so a lifelink blocker whose damage was entirely prevented gains its controller nothing.
/// The pre-fix engine summed blocker power, applied one shield to the total, and then credited
/// lifelink with each blocker's full printed power regardless of what was prevented.
#[test]
fn prevented_lifelink_blocker_damage_gains_no_life() {
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        {
            let mut d: Vec<String> = std::iter::repeat_n("child_of_night".to_string(), 5).collect();
            d.extend(std::iter::repeat_n("grizzly_bears".to_string(), 5));
            d
        },
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        9401,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 1, "child_of_night");
    ensure_in_hand(&mut e, 1, "grizzly_bears");

    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // Child of Night (2/1 lifelink) is declared first, so it is the first source the shield sees.
    let lifelinker = put_creature_on_battlefield(&mut e, 1, "child_of_night");
    let plain_blocker = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    // A 3-point prevention shield on the attacker: enough to absorb all of the lifelinker's 2
    // damage and 1 of the other blocker's.
    e.state.add_damage_prevention_shield(attacker, 3);

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");
    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair {
                attacker_id: attacker,
                blocker_id: lifelinker,
            },
            BlockPair {
                attacker_id: attacker,
                blocker_id: plain_blocker,
            },
        ]),
    )
    .expect("declare two blockers");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass");

    let p1_life = e.state.players[1].life;
    e.apply_command(
        0,
        &assign_combat_damage_cmd(attacker, vec![(lifelinker, 1), (plain_blocker, 1)]),
    )
    .expect("assign 1+1");
    let lifelinker_application = e
        .state
        .pending_resolution
        .as_ref()
        .expect("finite shield allocation choice")
        .presentation
        .candidates[0];
    e.apply_command(0, &submit_resolution_choice(vec![lifelinker_application]))
        .expect("allocate the shield to Child of Night first");

    assert_eq!(
        e.state.players[1].life, p1_life,
        "all of the lifelink blocker's damage was prevented, so it gains no life"
    );
    assert_eq!(
        e.state.objects.get(&attacker).expect("attacker").damage,
        1,
        "the 3-point shield leaves only 1 of the 4 combined blocker damage"
    );
}

/// Combat damage used to `unwrap()` the defending player; it now returns `EngineError::Illegal`.
///
/// The engine cannot be driven into `resolve_combat_damage` with no defender through public
/// commands — the moment a player is flagged `has_lost`, `sweep_life` names a winner and every
/// later command is rejected as "game over" — and `resolve_combat_damage` is `pub(super)`, so an
/// integration test cannot call it directly either. What is testable, and is the exact precondition
/// the guard handles, is that the lookup yields `None` rather than a bogus seat. The value of the
/// change is that reaching it costs a rejected command instead of taking down the sidecar task.
#[test]
fn no_defending_player_once_the_opponent_has_lost() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        70,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new");
    assert_eq!(
        e.state.sole_defending_player_id(),
        Some(1),
        "the opponent of the active player defends"
    );
    e.state.players[1].has_lost = true;
    assert_eq!(
        e.state.sole_defending_player_id(),
        None,
        "a player who has lost is not a defending player"
    );
    assert!(!e.state.is_defending_player(1));
    assert!(e.state.defending_player_ids().is_empty());
}
