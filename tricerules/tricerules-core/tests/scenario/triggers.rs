use crate::helpers::*;

/// Regression test: combat damage triggers must land on the stack and require both players to
/// pass priority before resolving.  The fix is that PhaseChanged is emitted before StackPushed
/// so the C++ client doesn't clear ruledStackObjectIds and mistakenly auto-passes.
#[test]
fn combat_damage_trigger_lands_on_stack_and_requires_priority() {
    // 10-card decks: skip_opening draws 7 as opening hand, leaving 3 in library
    // so the Scroll Thief trigger can draw without hitting an empty library.
    let p0_deck: Vec<String> = vec![
        "island".into(),
        "island".into(),
        "scroll_thief".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
    ];
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 10).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(77, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Cheat Scroll Thief onto P0's battlefield without summoning sickness.
    let thief_hand_idx = hand_index_for_card(&e, 0, "scroll_thief");
    let thief_oid = e.state.players[0].hand.remove(thief_hand_idx);
    e.state.players[0].battlefield.push(thief_oid);
    if let Some(obj) = e.state.objects.get_mut(&thief_oid) {
        obj.zone = tricerules_core::Zone::Battlefield;
        obj.summoning_sick = false;
    }

    let p0_hand_before = e.state.players[0].hand.len();

    // Enter combat and attack with Scroll Thief.
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap begin combat pass");
    e.apply_command(1, &pass()).expect("nap begin combat pass");
    e.apply_command(0, &declare_attackers(vec![thief_oid]))
        .expect("declare attackers");
    e.apply_command(0, &pass())
        .expect("ap pass declare attackers");
    e.apply_command(1, &pass())
        .expect("nap pass declare attackers");
    // P1 has no blockers; engine auto-declares empty blockers.
    e.apply_command(0, &pass())
        .expect("ap pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("nap pass declare blockers -> combat damage");

    // After both players pass in DeclareBlockers, combat damage resolves and Scroll Thief's
    // trigger fires.  The trigger must be on the stack awaiting priority.
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::CombatDamage,
        "should be in CombatDamage step"
    );
    assert_eq!(
        e.state.stack.len(),
        1,
        "combat damage trigger should be on the stack"
    );

    // Verify that in the returned batch PhaseChanged arrives before StackPushed.
    // This is the ordering fix: C++ clears its ruledStackObjectIds on PhaseChanged; the
    // trigger must follow so it is visible when the auto-pass timer fires.
    let event_order: Vec<&str> = b
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::PhaseChanged(_)) => Some("phase"),
            Some(Ev::StackPushed(_)) => Some("stack_pushed"),
            _ => None,
        })
        .collect();
    let phase_pos = event_order.iter().position(|&e| e == "phase");
    let pushed_pos = event_order.iter().position(|&e| e == "stack_pushed");
    assert!(
        phase_pos.is_some() && pushed_pos.is_some(),
        "batch must contain both PhaseChanged and StackPushed"
    );
    assert!(
        phase_pos.unwrap() < pushed_pos.unwrap(),
        "PhaseChanged must precede StackPushed in the combat damage batch"
    );

    // Abilities have no physical card on the stack: StackPushed.card_id stays empty
    // (spells carry their engine card id for catalog-based relay binding).
    let trigger_push = b
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("trigger StackPushed");
    assert!(trigger_push.card_id.is_empty());

    // Both players passing resolves the trigger; Scroll Thief draws a card for P0.
    pass_both_players(&mut e);
    assert!(
        e.state.stack.is_empty(),
        "stack should be empty after trigger resolves"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        p0_hand_before + 1,
        "Scroll Thief trigger should have drawn a card for P0"
    );
}

/// CR 603.3b: when multiple triggered abilities fire from the same event (simultaneous triggers),
/// all of them must go on the stack — not just the first one.  Regression test for the old
/// `Option<PendingTrigger>` design that silently dropped any trigger after the first.
///
/// Scenario: two Scroll Thieves (WheneverSelfDealsCombatDamageToPlayer → DrawCards(1)) both
/// attack an unblocked player. Both triggers must fire, resulting in two cards drawn.
#[test]
fn simultaneous_combat_damage_triggers_both_fire() {
    // 12-card library so drawing 7 opening hand + 2 triggers still has cards remaining.
    let p0_deck: Vec<String> = vec![
        "scroll_thief".into(),
        "scroll_thief".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
    ];
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 12).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(8001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Cheat both Scroll Thieves onto P0's battlefield without summoning sickness.
    let thief1_hand_idx = hand_index_for_card(&e, 0, "scroll_thief");
    let thief1_oid = e.state.players[0].hand.remove(thief1_hand_idx);
    e.state.players[0].battlefield.push(thief1_oid);
    if let Some(obj) = e.state.objects.get_mut(&thief1_oid) {
        obj.zone = tricerules_core::Zone::Battlefield;
        obj.summoning_sick = false;
    }

    let thief2_hand_idx = hand_index_for_card(&e, 0, "scroll_thief");
    let thief2_oid = e.state.players[0].hand.remove(thief2_hand_idx);
    e.state.players[0].battlefield.push(thief2_oid);
    if let Some(obj) = e.state.objects.get_mut(&thief2_oid) {
        obj.zone = tricerules_core::Zone::Battlefield;
        obj.summoning_sick = false;
    }

    let p0_hand_before = e.state.players[0].hand.len();

    // Enter combat and attack with both Scroll Thieves.
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap begin combat pass");
    e.apply_command(1, &pass()).expect("nap begin combat pass");
    e.apply_command(0, &declare_attackers(vec![thief1_oid, thief2_oid]))
        .expect("declare attackers");
    e.apply_command(0, &pass())
        .expect("ap pass after attackers");
    e.apply_command(1, &pass())
        .expect("nap pass after attackers");
    // P1 has no creatures; engine auto-declares empty blockers.
    e.apply_command(0, &pass())
        .expect("ap pass declare blockers");
    e.apply_command(1, &pass())
        .expect("nap pass declare blockers -> combat damage");

    // Both Scroll Thieves dealt combat damage to P1, so both triggers must be on the stack.
    assert_eq!(
        e.state.stack.len(),
        2,
        "both Scroll Thief triggers must be on the stack simultaneously"
    );

    // Resolving both triggers draws 2 cards for P0.
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.stack.is_empty(),
        "stack must be empty after both triggers resolve"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        p0_hand_before + 2,
        "two Scroll Thief triggers must draw two cards total"
    );
}

/// Regression: trigger queue serializes correctly when a single targeted trigger needs target
/// selection. After `choose_trigger_target` the queue should be empty and priority returned.
/// This guards against regressions in the queue-based `choose_trigger_target` path.
#[test]
fn targeted_trigger_resolves_after_target_chosen() {
    // Thieving Magpie: WheneverSelfDealsCombatDamageToPlayer → DrawCards(1) (no target needed).
    // Use a card whose trigger DOES need a target so choose_trigger_target is exercised.
    // Flametongue Kavu: WhenSelfEntersBattlefield → DamageTarget(4) — targeted ETB trigger.
    let p0_deck: Vec<String> = vec![
        "flametongue_kavu".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
    ];
    let p1_deck: Vec<String> = vec![
        "grizzly_bears".into(),
        "forest".into(),
        "forest".into(),
        "forest".into(),
        "forest".into(),
        "forest".into(),
        "forest".into(),
        "forest".into(),
        "forest".into(),
        "forest".into(),
        "forest".into(),
    ];
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(8003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Get Grizzly Bears onto P1's battlefield (cheat it in).
    let bears_oid = {
        let pos = e.state.players[1]
            .hand
            .iter()
            .position(|oid| {
                e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("grizzly_bears")
            })
            .expect("grizzly_bears in P1 hand");
        let oid = e.state.players[1].hand.remove(pos);
        e.state.players[1].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("obj").zone = tricerules_core::Zone::Battlefield;
        oid
    };

    // Give P0 enough mana to cast Flametongue Kavu (3R = 3 colorless + 1 red).
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );

    // Cast Flametongue Kavu (no target at cast time for its ETB trigger — target chosen later).
    let ftk_idx = hand_index_for_card(&e, 0, "flametongue_kavu");
    e.apply_command(0, &cast_spell(ftk_idx, vec![]))
        .expect("cast FTK");

    // Let it resolve (both players pass).
    pass_both_players(&mut e);

    // ETB trigger fires: DamageTarget(4) needs a target → pending_triggers should have 1 entry.
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "FTK ETB trigger must be queued for target selection"
    );

    // P0 chooses Grizzly Bears as the target.
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: bears_oid,
            })),
        },
    )
    .expect("choose trigger target");

    // Queue must be empty and trigger on the stack.
    assert!(
        e.state.pending_triggers.is_empty(),
        "pending_triggers queue must be empty after target chosen"
    );
    assert_eq!(e.state.stack.len(), 1, "trigger must be on the stack");

    // Resolve the trigger: Grizzly Bears (2/2) takes 4 damage → dies.
    pass_both_players(&mut e);
    assert_eq!(
        e.state.objects.get(&bears_oid).expect("bears").zone,
        tricerules_core::Zone::Graveyard,
        "Grizzly Bears must die to Flametongue Kavu's ETB trigger"
    );
}

/// CR 603.2 + Argothian Enchantress: casting an enchantment spell triggers a draw.
/// Scenario: P0 controls an Argothian Enchantress; P0 casts Exploration (a green
/// enchantment). The trigger fires and puts a Draw(1) on the stack, which resolves
/// and increases P0's hand size by one.
#[test]
fn argothian_enchantress_triggers_on_enchantment_cast() {
    // 14-card decks so library is never empty after the opening hand + draw step.
    let p0_deck: Vec<String> = std::iter::once("exploration".into())
        .chain(std::iter::repeat_n("forest".into(), 13))
        .collect();
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 14).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(9004, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Argothian Enchantress onto P0's battlefield (no summoning sickness needed).
    inject_creature_with_stats(&mut e, 0, "argothian_enchantress", 0, 1);

    // Play a Forest for mana.
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

    // Ensure Exploration is in P0's hand.
    if !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("exploration"))
    {
        take_card_from_library_to_hand(&mut e, 0, "exploration");
    }

    let hand_before = e.state.players[0].hand.len();

    let expl_idx = hand_index_for_card(&e, 0, "exploration");
    e.apply_command(0, &cast_spell(expl_idx, vec![]))
        .expect("cast Exploration");

    // Both Exploration and the enchantress draw trigger must be on the stack.
    assert!(
        e.state.stack.len() >= 2,
        "Exploration and its triggered Draw must both be on the stack"
    );

    // Resolve the draw trigger (top of stack) — P0 draws one card.
    // Net hand change: -1 for casting Exploration, +1 for trigger draw = 0 vs hand_before.
    pass_both_players(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "Argothian Enchantress trigger must draw exactly one card on enchantment cast (net hand = hand_before)"
    );

    // Resolve Exploration itself.
    pass_both_players(&mut e);
}

#[test]
fn soul_warden_gains_life_when_another_creature_enters() {
    let decks = Some(vec![
        deck_with("forest", &["soul_warden", "grizzly_bears"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(7400, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_hand(&mut e, 0, "soul_warden");
    relocate_to_hand(&mut e, 0, "grizzly_bears");
    assert_eq!(e.state.players[0].life, 20);

    // Casting Soul Warden itself does NOT gain life (the "another creature" / exclude_self clause).
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let warden_idx = hand_index_for_card(&e, 0, "soul_warden");
    e.apply_command(0, &cast_spell(warden_idx, vec![]))
        .expect("cast soul warden");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].life, 20,
        "Soul Warden's own ETB must not trigger itself"
    );

    // Another creature entering the battlefield triggers Soul Warden: +1 life.
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let bears_idx = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(bears_idx, vec![]))
        .expect("cast grizzly bears");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].life, 21,
        "another creature entering gains Soul Warden's controller 1 life"
    );
}
