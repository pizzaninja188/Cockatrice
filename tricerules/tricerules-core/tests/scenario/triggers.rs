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
                decline: false,
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

// ---------------------------------------------------------------------------
// Blood Artist — WheneverCreatureDies observer trigger + DrainTarget effect
// ---------------------------------------------------------------------------

/// Blood Artist's trigger fires when a creature controlled by ANY player dies.
/// Scenario: P0 has Blood Artist; P1's Grizzly Bears die via lethal damage (SBA).
/// P0 chooses P1 as the drain target → P1 loses 1 life, P0 gains 1 life.
#[test]
fn blood_artist_triggers_on_opponent_creature_dying() {
    let decks = Some(vec![
        deck_with("swamp", &["blood_artist"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(8100, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let artist_oid = relocate_to_battlefield(&mut e, 0, "blood_artist", false);
    let bears_oid = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);

    // Record life totals before the kill.
    let p0_life_before = e.state.players[0].life;
    let p1_life_before = e.state.players[1].life;

    // Mark lethal damage on bears (toughness 2); SBAs will destroy them on next priority check.
    e.state.objects.get_mut(&bears_oid).expect("bears").damage = 2;

    // Pass priority — SBA runs, Bears die, Blood Artist trigger fires (needs target).
    let batch = e.apply_command(0, &pass()).expect("pass triggers SBA");
    let _ = batch; // SBA fires and pending_trigger is queued

    assert_eq!(
        e.state.objects.get(&bears_oid).expect("bears").zone,
        tricerules_core::Zone::Graveyard,
        "Grizzly Bears must be in graveyard after lethal-damage SBA"
    );
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "Blood Artist WheneverCreatureDies trigger must be pending target selection"
    );

    // P0 (Blood Artist controller) chooses P1 as the drain target.
    let p1_id = e.state.players[1].id;
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: p1_id as u32,
                decline: false,
            })),
        },
    )
    .expect("choose trigger target: P1");

    assert!(
        e.state.pending_triggers.is_empty(),
        "pending_triggers must be empty after target chosen"
    );
    assert_eq!(e.state.stack.len(), 1, "Blood Artist trigger on the stack");

    // Resolve the trigger: P1 had priority after P0's pass; both pass to resolve the drain.
    pass_both_players(&mut e);

    assert_eq!(
        e.state.players[0].life,
        p0_life_before + 1,
        "Blood Artist controller must gain 1 life"
    );
    assert_eq!(
        e.state.players[1].life,
        p1_life_before - 1,
        "drained player must lose 1 life"
    );
    let _ = artist_oid; // suppress unused warning
}

/// Blood Artist triggers on its own death (exclude_self: false) and still drains.
/// Blood Artist (0/1) takes 1 damage → lethal → Blood Artist dies → trigger fires.
#[test]
fn blood_artist_triggers_on_own_death() {
    let decks = Some(vec![
        deck_with("swamp", &["blood_artist"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(8101, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let artist_oid = relocate_to_battlefield(&mut e, 0, "blood_artist", false);
    let p0_life_before = e.state.players[0].life;
    let p1_life_before = e.state.players[1].life;

    // Blood Artist has toughness 1; 1 damage kills it.
    e.state.objects.get_mut(&artist_oid).expect("artist").damage = 1;

    // Pass priority triggers SBA → Blood Artist dies.
    e.apply_command(0, &pass()).expect("pass triggers SBA");

    assert_eq!(
        e.state.objects.get(&artist_oid).expect("artist").zone,
        tricerules_core::Zone::Graveyard,
        "Blood Artist must be dead"
    );
    // The trigger fires from the graveyard context (card data still in registry), needs target.
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "Blood Artist must queue its own-death trigger"
    );

    // P0 chooses P1 as drain target.
    let p1_id = e.state.players[1].id;
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: p1_id as u32,
                decline: false,
            })),
        },
    )
    .expect("choose trigger target: P1");

    // After P0's pass triggered the SBA, priority is now P1's; pass both to resolve.
    pass_both_players(&mut e);

    assert_eq!(
        e.state.players[0].life,
        p0_life_before + 1,
        "Blood Artist gains 1 life on its own death"
    );
    assert_eq!(
        e.state.players[1].life,
        p1_life_before - 1,
        "P1 loses 1 life from Blood Artist drain on self-death"
    );
}

/// Blood Artist triggers on its controller's own creature dying (not just opponents').
/// P0 has Blood Artist + Grizzly Bears; Bears take lethal damage → drain fires.
#[test]
fn blood_artist_triggers_on_own_creature_dying() {
    let decks = Some(vec![
        deck_with("swamp", &["blood_artist", "grizzly_bears"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(8102, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let _artist = relocate_to_battlefield(&mut e, 0, "blood_artist", false);
    let bears_oid = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let p0_life_before = e.state.players[0].life;
    let p1_life_before = e.state.players[1].life;

    // Grizzly Bears (2/2) takes 2 lethal damage.
    e.state.objects.get_mut(&bears_oid).expect("bears").damage = 2;

    e.apply_command(0, &pass()).expect("SBA pass");

    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "Blood Artist must trigger on controller's own creature dying"
    );

    let p1_id = e.state.players[1].id;
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: p1_id as u32,
                decline: false,
            })),
        },
    )
    .expect("choose P1 as drain target");

    pass_both_players(&mut e);

    assert_eq!(e.state.players[0].life, p0_life_before + 1);
    assert_eq!(e.state.players[1].life, p1_life_before - 1);
}

/// Two Blood Artists both trigger on the same creature dying — simultaneous triggers,
/// both drain independently (net: target loses 2 life, P0 gains 2 life).
#[test]
fn two_blood_artists_both_trigger_on_one_death() {
    let decks = Some(vec![
        deck_with("swamp", &["blood_artist", "blood_artist"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(8103, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    relocate_to_battlefield(&mut e, 0, "blood_artist", false);
    relocate_to_battlefield(&mut e, 0, "blood_artist", false);
    let bears_oid = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let p0_life_before = e.state.players[0].life;
    let p1_life_before = e.state.players[1].life;

    e.state.objects.get_mut(&bears_oid).expect("bears").damage = 2;

    e.apply_command(0, &pass()).expect("SBA pass");

    assert_eq!(
        e.state.pending_triggers.len(),
        2,
        "both Blood Artists must queue a drain trigger on one creature death"
    );

    let p1_id = e.state.players[1].id;
    // Choose target for the first pending trigger.
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: p1_id as u32,
                decline: false,
            })),
        },
    )
    .expect("target 1st trigger");
    // Choose target for the second pending trigger.
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: p1_id as u32,
                decline: false,
            })),
        },
    )
    .expect("target 2nd trigger");

    assert_eq!(e.state.stack.len(), 2, "both drain triggers on stack");

    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.players[0].life,
        p0_life_before + 2,
        "P0 gains 2 life from two Blood Artist drains"
    );
    assert_eq!(
        e.state.players[1].life,
        p1_life_before - 2,
        "P1 loses 2 life from two Blood Artist drains"
    );
}

// ===========================================================================
// "Whenever you gain life" (CR 118.3) — Ajani's Pridemate, Bloodthirsty Aerialist
// ===========================================================================

/// The gain funnel fires the trigger once per life-gain event, whatever the amount:
/// Angel's Mercy gains 7 life and grows Ajani's Pridemate by exactly one counter.
#[test]
fn pridemate_grows_on_spell_life_gain() {
    let mut e = anthem_engine(9101, "angels_mercy");
    let pridemate = inject_creature_on_battlefield(&mut e, 0, "ajanis_pridemate");
    let life_before = e.state.players[0].life;

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "angels_mercy");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    resolve_entire_stack_two_player(&mut e); // the spell, then its trigger

    assert_eq!(e.state.players[0].life, life_before + 7, "Angel's Mercy");
    assert_eq!(e.effective_power(pridemate), Some(3), "2/2 + one counter");
    assert_eq!(e.effective_toughness(pridemate), Some(3));
}

/// Two life-gain events are two triggers — the ability watches events, not totals.
#[test]
fn two_life_gain_events_trigger_separately() {
    let decks = Some(vec![
        deck_with("plains", &["angels_mercy", "angels_mercy"]),
        island_only_deck(),
    ]);
    let mut e = GameEngine::new(9102, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let pridemate = inject_creature_on_battlefield(&mut e, 0, "ajanis_pridemate");

    for _ in 0..2 {
        grant_pool(&mut e, 0);
        ensure_in_hand(&mut e, 0, "angels_mercy");
        let idx = hand_index_for_card(&e, 0, "angels_mercy");
        e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
        resolve_entire_stack_two_player(&mut e);
    }

    assert_eq!(e.effective_power(pridemate), Some(4), "two counters");
}

/// CR 702.15b: lifelink life gain is an ordinary life-gain event, and each lifelinker's damage is
/// its own event — two attacking Children of Night grow the Pridemate twice, not once.
#[test]
fn pridemate_grows_once_per_lifelink_creature() {
    let decks = Some(vec![forest_only_deck(), island_only_deck()]);
    let mut e = GameEngine::new(9103, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let pridemate = inject_creature_on_battlefield(&mut e, 0, "ajanis_pridemate");
    let vamp_a = inject_creature_on_battlefield(&mut e, 0, "child_of_night");
    let vamp_b = inject_creature_on_battlefield(&mut e, 0, "child_of_night");
    let life_before = e.state.players[0].life;

    e.apply_command(0, &declare_attackers(vec![vamp_a, vamp_b]))
        .expect("declare attackers");
    pass_both_players(&mut e); // declare attackers -> declare blockers
    pass_both_players(&mut e); // no blockers -> combat damage
    resolve_entire_stack_two_player(&mut e); // the two lifegain triggers

    assert_eq!(
        e.state.players[0].life,
        life_before + 4,
        "two 2-power lifelinkers"
    );
    assert_eq!(
        e.effective_power(pridemate),
        Some(4),
        "one counter per lifelink gain event, not one for the combined 4 life"
    );
}

/// CR 118.4: gaining 0 life is not a life-gain event. Swords to Plowshares on a 0-power creature
/// exiles it and gains nothing, so the Pridemate stays 2/2.
#[test]
fn zero_life_gain_does_not_trigger() {
    let mut e = anthem_engine(9104, "swords_to_plowshares");
    let pridemate = inject_creature_on_battlefield(&mut e, 0, "ajanis_pridemate");
    let wall = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 0, 4);
    let life_before = e.state.players[0].life;

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "swords_to_plowshares");
    e.apply_command(0, &cast_spell(idx, targets_with_damage(vec![(wall, 0)])))
        .expect("cast swords");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.players[0].life, life_before, "0 power, 0 life");
    assert!(e.state.stack.is_empty(), "no trigger was put on the stack");
    assert_eq!(e.effective_power(pridemate), Some(2), "still 2/2");
}

/// "Whenever *you* gain life" (`CastTriggerPlayer::Controller`): an opponent's Pridemate does not
/// grow when P0 gains life.
#[test]
fn opponents_pridemate_does_not_grow_on_your_life_gain() {
    let mut e = anthem_engine(9105, "angels_mercy");
    let mine = inject_creature_on_battlefield(&mut e, 0, "ajanis_pridemate");
    let theirs = inject_creature_on_battlefield(&mut e, 1, "bloodthirsty_aerialist");

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "angels_mercy");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.effective_power(mine),
        Some(3),
        "controller's payoff grows"
    );
    assert_eq!(
        e.effective_power(theirs),
        Some(2),
        "opponent's payoff is untouched by P0's life gain"
    );
}

// ===========================================================================
// "At the beginning of each player's draw step" (CR 504.2) — Howling Mine,
// Kami of the Crescent Moon. The card goes to the player whose draw step it is
// (CR 603.7d "that player"), not to the source's controller, and Howling Mine's
// "if this artifact is untapped" is a CR 603.4 intervening-"if" clause.
// ===========================================================================

/// Two 20-card decks at Main 1 of P0's turn. Deliberately not `anthem_engine`, whose 7-card
/// decks are entirely consumed by the opening hand — these tests count *library* movement.
fn draw_step_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e
}

/// Advance from Main 1 of the active player's turn through the *next* player's draw step,
/// resolving whatever the draw step put on the stack. Returns the library sizes
/// `(P0, P1)` captured immediately before the draw step began.
fn pass_turn_through_next_draw_step(e: &mut GameEngine, active: i32) -> (usize, usize) {
    // Called again after a previous draw step, the turn is still in Draw; `end_active_turn`
    // starts counting from Main 1.
    if e.state.turn_step == tricerules_core::TurnStep::Draw {
        pass_both_players(e);
    }
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    // Two draws a turn overshoots the CR 514.1 maximum hand size fast; discard down by hand so
    // the turn ends without a cleanup prompt (the shared helper discards one card at a time).
    for player in 0..2 {
        while e.state.players[player].hand.len() > 5 {
            let oid = e.state.players[player].hand.pop().expect("nonempty hand");
            e.state.players[player].graveyard.push(oid);
            e.state.objects.get_mut(&oid).expect("object").zone = tricerules_core::Zone::Graveyard;
        }
    }
    end_active_turn(e, active);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    let before = (
        e.state.players[0].library.len(),
        e.state.players[1].library.len(),
    );
    pass_both_players(e); // upkeep -> draw step: turn-based draw, then CR 504.2 triggers
    resolve_entire_stack_two_player(e);
    before
}

fn drawn_by(e: &GameEngine, player: usize, before: (usize, usize)) -> usize {
    let was = if player == 0 { before.0 } else { before.1 };
    was - e.state.players[player].library.len()
}

/// P0's untapped Howling Mine draws an extra card for **whoever's** draw step it is — the
/// opponent's included (`CastTriggerPlayer::AnyPlayer` + the all-players battlefield scan).
#[test]
fn howling_mine_draws_an_extra_card_on_each_players_draw_step() {
    let mut e = draw_step_engine(9301);
    inject_permanent_on_battlefield(&mut e, 0, "howling_mine");

    // P1's draw step: P1 draws the turn-based card plus the Mine's extra one.
    let before = pass_turn_through_next_draw_step(&mut e, 0);
    assert_eq!(e.state.active_player_id(), 1, "P1's turn");
    assert_eq!(drawn_by(&e, 1, before), 2, "P1 draws 1 + 1 from P0's Mine");
    assert_eq!(
        drawn_by(&e, 0, before),
        0,
        "the Mine's controller draws nothing"
    );

    // Back around to P0's draw step: now its controller is the drawing player.
    let before = pass_turn_through_next_draw_step(&mut e, 1);
    assert_eq!(e.state.active_player_id(), 0, "P0's turn");
    assert_eq!(drawn_by(&e, 0, before), 2, "P0 draws 1 + 1");
    assert_eq!(drawn_by(&e, 1, before), 0);
}

/// CR 603.4, first check: a tapped Howling Mine never triggers at all — no stack object,
/// so nothing for either player to respond to.
#[test]
fn tapped_howling_mine_never_triggers() {
    let mut e = draw_step_engine(9302);
    let mine = inject_permanent_on_battlefield(&mut e, 0, "howling_mine");
    e.state.objects.get_mut(&mine).expect("mine").tapped = true;

    end_active_turn(&mut e, 0);
    let before = (
        e.state.players[0].library.len(),
        e.state.players[1].library.len(),
    );
    pass_both_players(&mut e); // upkeep -> draw step

    assert!(
        e.state.stack.is_empty(),
        "tapped Mine puts no trigger on the stack"
    );
    assert_eq!(drawn_by(&e, 1, before), 1, "only the turn-based draw");
}

/// CR 603.4, second check: the clause is re-evaluated on resolution. Tapping the Mine while its
/// own trigger is on the stack makes the ability do nothing.
#[test]
fn howling_mine_tapped_in_response_does_nothing() {
    let mut e = draw_step_engine(9303);
    let mine = inject_permanent_on_battlefield(&mut e, 0, "howling_mine");

    end_active_turn(&mut e, 0);
    let before = (
        e.state.players[0].library.len(),
        e.state.players[1].library.len(),
    );
    pass_both_players(&mut e); // upkeep -> draw step
    assert_eq!(e.state.stack.len(), 1, "the Mine's trigger is on the stack");

    e.state.objects.get_mut(&mine).expect("mine").tapped = true;
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        drawn_by(&e, 1, before),
        1,
        "turn-based draw only; the trigger fizzled"
    );
}

/// CR 504.1/504.2: the turn-based draw is not on the stack and happens *first* — by the time the
/// trigger is waiting for priority the normal card has already been drawn.
#[test]
fn turn_based_draw_precedes_the_draw_step_trigger() {
    let mut e = draw_step_engine(9304);
    inject_permanent_on_battlefield(&mut e, 0, "howling_mine");

    end_active_turn(&mut e, 0);
    let before = (
        e.state.players[0].library.len(),
        e.state.players[1].library.len(),
    );
    pass_both_players(&mut e); // upkeep -> draw step

    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
    assert_eq!(
        drawn_by(&e, 1, before),
        1,
        "turn-based draw already resolved"
    );
    assert_eq!(
        e.state.stack.len(),
        1,
        "the extra draw is still a stack object"
    );

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(drawn_by(&e, 1, before), 2);
}

/// The same trigger without an intervening-"if", on a creature: Kami of the Crescent Moon draws
/// for its *opponent* on their draw step, which only works because the beneficiary is the
/// trigger's player and not the ability's controller.
#[test]
fn kami_of_the_crescent_moon_draws_for_the_opponent() {
    let mut e = draw_step_engine(9305);
    inject_creature_on_battlefield(&mut e, 0, "kami_of_the_crescent_moon");

    let before = pass_turn_through_next_draw_step(&mut e, 0);
    assert_eq!(drawn_by(&e, 1, before), 2, "P1 draws 1 + 1 from P0's Kami");
    assert_eq!(drawn_by(&e, 0, before), 0);
}

/// CR 104.3c/120.3: the extra draw from an empty library does not fail the ability — the player
/// loses instead, and resolution completes cleanly.
#[test]
fn draw_step_trigger_can_deck_the_drawing_player() {
    let mut e = draw_step_engine(9306);
    inject_permanent_on_battlefield(&mut e, 0, "howling_mine");

    end_active_turn(&mut e, 0);
    // Exactly one card left: the turn-based draw takes it, the Mine's extra draw finds nothing.
    while e.state.players[1].library.len() > 1 {
        e.state.players[1].library.pop_back();
    }
    pass_both_players(&mut e); // upkeep -> draw step
    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.players[1].library.is_empty());
    assert!(e.state.players[1].has_lost, "decked out on the extra draw");
}

/// CR 608.2h / 113.7a (Howling Mine ruling, 2004-10-04: "if Howling Mine leaves the battlefield
/// before it resolves, then the last known tap or untap state of the card is used for
/// resolution"). Bouncing the *untapped* Mine in response does not stop the extra draw — the
/// intervening-"if" is re-checked against last known information, not against a vanished object.
#[test]
fn howling_mine_bounced_while_untapped_still_draws() {
    let decks = Some(vec![
        deck_with("plains", &[]),
        deck_with("island", &["boomerang"]),
    ]);
    let mut e = GameEngine::new(9307, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mine = inject_permanent_on_battlefield(&mut e, 0, "howling_mine");

    end_active_turn(&mut e, 0);
    ensure_in_hand(&mut e, 1, "boomerang");
    let before = (
        e.state.players[0].library.len(),
        e.state.players[1].library.len(),
    );
    pass_both_players(&mut e); // upkeep -> draw step
    assert_eq!(e.state.stack.len(), 1, "the Mine's trigger is on the stack");

    grant_pool(&mut e, 1);
    let idx = hand_index_for_card(&e, 1, "boomerang");
    e.apply_command(1, &cast_spell(idx, targets_with_damage(vec![(mine, 0)])))
        .expect("bounce the Mine in response");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.objects.get(&mine).expect("mine object").zone,
        tricerules_core::Zone::Hand,
        "the Mine really left the battlefield"
    );
    assert_eq!(
        drawn_by(&e, 1, before),
        2,
        "turn-based draw plus the extra one: LKI says the Mine was untapped"
    );
}

/// The other half of the same rule, and the reason live state can't be read: CR 400.7 clears
/// `tapped` on the way out, so a Mine that was tapped when it left would *look* untapped. Last
/// known information says tapped, so the ability still does nothing.
#[test]
fn howling_mine_bounced_while_tapped_does_nothing() {
    let decks = Some(vec![
        deck_with("plains", &[]),
        deck_with("island", &["boomerang"]),
    ]);
    let mut e = GameEngine::new(9308, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mine = inject_permanent_on_battlefield(&mut e, 0, "howling_mine");

    end_active_turn(&mut e, 0);
    ensure_in_hand(&mut e, 1, "boomerang");
    let before = (
        e.state.players[0].library.len(),
        e.state.players[1].library.len(),
    );
    pass_both_players(&mut e); // upkeep -> draw step; it triggered while untapped

    e.state.objects.get_mut(&mine).expect("mine").tapped = true;
    grant_pool(&mut e, 1);
    let idx = hand_index_for_card(&e, 1, "boomerang");
    e.apply_command(1, &cast_spell(idx, targets_with_damage(vec![(mine, 0)])))
        .expect("bounce the tapped Mine in response");
    resolve_entire_stack_two_player(&mut e);

    assert!(
        !e.state.objects.get(&mine).expect("mine object").tapped,
        "CR 400.7 reset the tap status the LKI check must not read"
    );
    assert_eq!(
        drawn_by(&e, 1, before),
        1,
        "turn-based draw only: LKI says the Mine was tapped"
    );
}

// ===========================================================================
// "At the beginning of ... upkeep" (CR 503.1a) — Sulfuric Vortex (each player),
// Phyrexian Arena (your upkeep). Every player's battlefield is scanned in APNAP
// order (CR 603.3b), so a NONACTIVE player's permanent triggers on the active
// player's upkeep — the bug docs/FINDINGS.md logged against this arm, which
// scanned only the active player's battlefield.
// ===========================================================================

/// Same two 20-card decks the draw-step block uses; named for the block it serves.
fn upkeep_engine(seed: u64) -> GameEngine {
    draw_step_engine(seed)
}

/// From Main 1 of `active`'s turn to the *next* player's upkeep, stopping with whatever the
/// upkeep put on the stack still unresolved so a test can inspect it.
///
/// Deliberately not `pass_turn_through_next_draw_step`: that helper passes straight through the
/// upkeep, which with an upkeep trigger in play would resolve the trigger instead of advancing
/// to the draw step.
fn pass_turn_to_next_upkeep(e: &mut GameEngine, active: i32) {
    // Called again after a previous upkeep, the turn is still in Upkeep (or Draw, if the caller
    // advanced); `end_active_turn` counts from Main 1, so walk there first.
    while e.state.turn_step != tricerules_core::TurnStep::Main1 {
        pass_both_players(e);
    }
    // Keep hands under the CR 514.1 maximum so the turn ends without a cleanup prompt.
    for player in 0..2 {
        while e.state.players[player].hand.len() > 5 {
            let oid = e.state.players[player].hand.pop().expect("nonempty hand");
            e.state.players[player].graveyard.push(oid);
            e.state.objects.get_mut(&oid).expect("object").zone = tricerules_core::Zone::Graveyard;
        }
    }
    end_active_turn(e, active);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
}

fn life(e: &GameEngine, player: usize) -> i32 {
    e.state.players[player].life
}

/// **The regression test for the APNAP scan.** P0 controls the Vortex; on P1's upkeep the trigger
/// must be on the stack even though P0 is the *nonactive* player. The old arm scanned only the
/// active player's battlefield, so it produced nothing here at all.
#[test]
fn sulfuric_vortex_triggers_on_the_nonactive_controllers_upkeep() {
    let mut e = upkeep_engine(4401);
    inject_permanent_on_battlefield(&mut e, 0, "sulfuric_vortex");

    pass_turn_to_next_upkeep(&mut e, 0); // -> P1's turn, P1's upkeep

    assert_eq!(e.state.stack.len(), 1, "one upkeep trigger on the stack");
    assert_eq!(
        e.state.stack[0].controller, 0,
        "the trigger belongs to the Vortex's controller (CR 603.3a), not the active player"
    );

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(life(&e, 1), 18, "2 damage to the player whose upkeep it is");
    assert_eq!(life(&e, 0), 20, "the Vortex's controller is untouched");
}

/// `AnyPlayer` in both directions, and proof that `who: AffectedPlayer` resolves to the upkeep's
/// player rather than the source's controller: P0's own upkeep hits P0.
#[test]
fn sulfuric_vortex_damages_that_player_on_every_upkeep() {
    let mut e = upkeep_engine(4402);
    inject_permanent_on_battlefield(&mut e, 0, "sulfuric_vortex");

    pass_turn_to_next_upkeep(&mut e, 0); // P1's upkeep
    resolve_entire_stack_two_player(&mut e);
    assert_eq!((life(&e, 0), life(&e, 1)), (20, 18));

    pass_turn_to_next_upkeep(&mut e, 1); // back around to P0's upkeep
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (life(&e, 0), life(&e, 1)),
        (18, 18),
        "the Vortex's own controller takes it on their upkeep too"
    );
}

/// CR 603.3b / 101.4: with a Vortex on each side, both triggers are on the stack and the *active*
/// player's went on first — so the nonactive player's resolves first (LIFO).
#[test]
fn apnap_puts_the_active_players_upkeep_trigger_on_the_stack_first() {
    let mut e = upkeep_engine(4403);
    inject_permanent_on_battlefield(&mut e, 0, "sulfuric_vortex");
    inject_permanent_on_battlefield(&mut e, 1, "sulfuric_vortex");

    pass_turn_to_next_upkeep(&mut e, 0); // P1's upkeep; P1 is active

    assert_eq!(e.state.stack.len(), 2, "both Vortexes trigger");
    assert_eq!(
        e.state.stack[0].controller, 1,
        "active player's trigger goes on the stack first (CR 603.3b)"
    );
    assert_eq!(
        e.state.stack[1].controller, 0,
        "then the nonactive player's"
    );

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(life(&e, 1), 16, "both triggers hit the upkeep's player");
    assert_eq!(life(&e, 0), 20);
}

/// **The scope test.** A `Controller`-scoped upkeep trigger must not fire on the opponent's
/// upkeep. The widened scan now *sees* P0's Arena during P1's upkeep; the filter is what rejects
/// it. Catches a wrong serde default or an inverted `Controller`/`Opponent` comparison.
#[test]
fn phyrexian_arena_does_not_trigger_on_the_opponents_upkeep() {
    let mut e = upkeep_engine(4404);
    inject_permanent_on_battlefield(&mut e, 0, "phyrexian_arena");

    pass_turn_to_next_upkeep(&mut e, 0); // P1's upkeep
    let hand_before = e.state.players[0].hand.len();

    assert!(
        e.state.stack.is_empty(),
        "an \"at the beginning of your upkeep\" trigger sits out the opponent's upkeep"
    );
    assert_eq!((life(&e, 0), life(&e, 1)), (20, 20));
    assert_eq!(e.state.players[0].hand.len(), hand_before);
}

/// **The multi-effect test** (CR 608.2): one trigger, two effects, resolved in written order —
/// draw the card, then lose the life. Only expressible since abilities took an effect list.
#[test]
fn phyrexian_arena_draws_then_loses_life_on_its_controllers_upkeep() {
    let mut e = upkeep_engine(4405);
    inject_permanent_on_battlefield(&mut e, 0, "phyrexian_arena");

    pass_turn_to_next_upkeep(&mut e, 0); // P1's upkeep — Arena is silent
    assert!(e.state.stack.is_empty());
    pass_turn_to_next_upkeep(&mut e, 1); // P0's upkeep — Arena fires

    assert_eq!(e.state.stack.len(), 1);
    assert_eq!(e.state.stack[0].controller, 0);

    let hand_before = e.state.players[0].hand.len();
    let library_before = e.state.players[0].library.len();
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "drew a card from the first effect"
    );
    assert_eq!(e.state.players[0].library.len(), library_before - 1);
    assert_eq!(life(&e, 0), 19, "and lost 1 life from the second");
    assert_eq!(life(&e, 1), 20, "the opponent is untouched");
}

/// CR 500.2 / 503.1a: the upkeep trigger resolves *within* the upkeep step — the step does not
/// end until the stack is empty and both players pass. Guards against anyone "simplifying" the
/// step machine by draining the upkeep stack implicitly.
#[test]
fn upkeep_trigger_resolves_before_the_draw_step() {
    let mut e = upkeep_engine(4406);
    inject_permanent_on_battlefield(&mut e, 0, "sulfuric_vortex");

    pass_turn_to_next_upkeep(&mut e, 0);
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::Upkeep,
        "still in upkeep after the trigger resolves"
    );
    pass_both_players(&mut e);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
}

/// Illegal path: playing a land is a sorcery-speed action, so it is rejected while the upkeep
/// trigger is still on the stack.
#[test]
fn upkeep_trigger_on_the_stack_blocks_sorcery_speed_actions() {
    let mut e = upkeep_engine(4407);
    inject_permanent_on_battlefield(&mut e, 0, "sulfuric_vortex");

    pass_turn_to_next_upkeep(&mut e, 0); // P1's upkeep, trigger on the stack
    assert_eq!(e.state.stack.len(), 1);

    let land_slot = e.state.players[1]
        .hand
        .iter()
        .position(|oid| e.state.objects[oid].card_id == "island")
        .expect("P1 holds an island");
    let err = e.apply_command(1, &play_land(land_slot));
    assert!(
        err.is_err(),
        "no land drops while a trigger is on the upkeep stack"
    );
}

/// CR 704.5a: the upkeep damage goes through the real life/SBA path, so it can end the game.
#[test]
fn sulfuric_vortex_upkeep_damage_can_kill() {
    let mut e = upkeep_engine(4408);
    inject_permanent_on_battlefield(&mut e, 0, "sulfuric_vortex");
    e.state.players[1].life = 2;

    pass_turn_to_next_upkeep(&mut e, 0); // P1's upkeep
    resolve_entire_stack_two_player(&mut e);

    assert!(
        e.state.players[1].has_lost,
        "0 or less life loses (CR 704.5a)"
    );
    assert_eq!(e.state.winner, Some(0));
}

// ---------------------------------------------------------------------------
// Death triggers: controller (not owner), sacrifice costs, and simultaneity.
// These cover CR 603.3a / 603.6 behaviours that regressed once each.
// ---------------------------------------------------------------------------

/// CR 603.3a: a dies trigger's "controller" is whoever controlled the permanent as it died, not
/// whoever owns the card. P1's Blood Artist watching P0's *borrowed* creature die must see the
/// death as P1's own, so an "opponent controls" style relation reads the right seat. Blood Artist
/// takes AnyPlayer, so the observable assertion is that the trigger fires exactly once and that
/// the death is attributed to the controlling seat in the batch.
#[test]
fn dies_trigger_uses_last_controller_not_owner() {
    let decks = Some(vec![
        deck_with("swamp", &["blood_artist"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(9301, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let artist_oid = relocate_to_battlefield(&mut e, 0, "blood_artist", false);
    // Owned by P1, controlled by P0 — a stolen creature.
    let borrowed = inject_creature_under_foreign_control(&mut e, 1, 0, "grizzly_bears");
    e.state.objects.get_mut(&borrowed).expect("borrowed").damage = 99;

    e.apply_command(0, &pass()).expect("pass triggers SBA");

    assert_eq!(
        e.state.objects.get(&borrowed).expect("borrowed").zone,
        tricerules_core::Zone::Graveyard,
        "the borrowed creature must have died"
    );
    // The card goes to its *owner's* graveyard even though its controller was P0.
    assert!(
        e.state.players[1].graveyard.contains(&borrowed),
        "a dead card returns to its owner's graveyard (CR 404.3)"
    );
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "Blood Artist sees the borrowed creature die exactly once"
    );
    let _ = artist_oid;
}

/// CR 603.6a: a permanent sacrificed to pay an activation cost still dies, so Blood Artist sees
/// it. Bottle Gnomes sacrifices itself for its own ability; the drain must still happen.
#[test]
fn sacrifice_cost_fires_dies_triggers() {
    let decks = Some(vec![
        deck_with("swamp", &["blood_artist", "bottle_gnomes"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(9302, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    relocate_to_battlefield(&mut e, 0, "blood_artist", false);
    let gnomes = relocate_to_battlefield(&mut e, 0, "bottle_gnomes", false);

    e.apply_command(0, &activate_ability(gnomes, 0, vec![]))
        .expect("sacrifice Bottle Gnomes for its ability");

    assert_eq!(
        e.state.objects.get(&gnomes).expect("gnomes").zone,
        tricerules_core::Zone::Graveyard,
        "the sacrificed creature is in the graveyard"
    );
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "Blood Artist must trigger on a creature sacrificed as a cost"
    );
}

/// CR 603.6/603.10: creatures destroyed by one spell die simultaneously, so a Blood Artist that
/// dies in the wipe still observes every *other* creature dying — the pre-fix engine moved all
/// victims off the battlefield before scanning, which lost those triggers entirely.
#[test]
fn blood_artist_dying_in_a_wipe_still_sees_the_other_deaths() {
    let decks = Some(vec![
        deck_with("plains", &["wrath_of_god", "blood_artist"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(9303, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    relocate_to_battlefield(&mut e, 0, "blood_artist", false);
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    ensure_in_hand(&mut e, 0, "wrath_of_god");
    let wrath_idx = hand_index_for_card(&e, 0, "wrath_of_god");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            c: 2,
            ..Default::default()
        },
    );
    e.apply_command(0, &cast_spell(wrath_idx, vec![]))
        .expect("cast Wrath of God");
    pass_both_players(&mut e);

    // Blood Artist itself + both Grizzly Bears all died in the same event, so its ability
    // triggers three times (it is not "another creature" — exclude_self is false).
    assert_eq!(
        e.state.pending_triggers.len(),
        3,
        "Blood Artist sees its own death and both Bears dying simultaneously"
    );
}

/// The Bottle Gnomes softlock: a trigger queued while paying an activation cost must be emitted
/// *after* the ability's own StackPushed. The client treats an ability arriving on the stack as
/// "the pending trigger target was just answered", so emitting the prompt first made it discard
/// the prompt and strand the player with a trigger the engine was still waiting on.
#[test]
fn sacrifice_cost_trigger_prompt_follows_the_ability_on_the_stack() {
    let decks = Some(vec![
        deck_with("swamp", &["blood_artist", "bottle_gnomes"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(9304, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "blood_artist", false);
    let gnomes = relocate_to_battlefield(&mut e, 0, "bottle_gnomes", false);

    let batch = e
        .apply_command(0, &activate_ability(gnomes, 0, vec![]))
        .expect("sacrifice Bottle Gnomes");

    let order: Vec<&str> = batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(_)) => Some("ability"),
            Some(Ev::TriggerNeedsTarget(_)) => Some("prompt"),
            _ => None,
        })
        .collect();
    let ability_pos = order.iter().position(|&x| x == "ability");
    let prompt_pos = order.iter().position(|&x| x == "prompt");
    assert!(
        ability_pos.is_some() && prompt_pos.is_some(),
        "batch must contain both the ability and the trigger prompt: {order:?}"
    );
    assert!(
        ability_pos.unwrap() < prompt_pos.unwrap(),
        "the ability must reach the stack before its cost's death trigger prompts: {order:?}"
    );
}

/// An *activated* ability must not be marked `is_triggered`: that flag is the client's only way to
/// tell the two apart (both have an empty card_id), and it uses it to decide whether a pending
/// trigger-target prompt has been answered.
#[test]
fn stack_pushed_distinguishes_triggered_from_activated_abilities() {
    let decks = Some(vec![
        deck_with("swamp", &["blood_artist", "bottle_gnomes"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(9305, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "blood_artist", false);
    let gnomes = relocate_to_battlefield(&mut e, 0, "bottle_gnomes", false);

    let batch = e
        .apply_command(0, &activate_ability(gnomes, 0, vec![]))
        .expect("sacrifice Bottle Gnomes");
    let pushed: Vec<bool> = batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s.is_triggered),
            _ => None,
        })
        .collect();
    assert_eq!(
        pushed,
        vec![false],
        "the activated ability is not a trigger"
    );

    // Answering the trigger puts the triggered ability on the stack, flagged.
    let p1_id = e.state.players[1].id;
    let batch = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    target_object_id: p1_id as u32,
                    decline: false,
                })),
            },
        )
        .expect("choose drain target");
    let pushed: Vec<bool> = batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s.is_triggered),
            _ => None,
        })
        .collect();
    assert_eq!(pushed, vec![true], "Blood Artist's ability is a trigger");
}

/// A rejected trigger target must leave the trigger pending. It was popped off the queue before
/// validation, so a rejection destroyed it while the client still displayed its prompt — and the
/// follow-up Decline then failed with "no pending trigger", leaving the player with a prompt
/// nothing could answer. Reachable by answering the wrong prompt when two triggers are queued.
#[test]
fn rejected_trigger_target_leaves_the_trigger_pending() {
    let decks = Some(vec![
        deck_with("swamp", &["gravedigger"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(9306, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bears = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 0, "gravedigger");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "gravedigger");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast Gravedigger");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "Gravedigger's ETB trigger is awaiting a target"
    );

    // A permanent on the battlefield is not a creature *card in a graveyard*.
    let on_battlefield = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let err = e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: on_battlefield,
                decline: false,
            })),
        },
    );
    assert!(err.is_err(), "a battlefield permanent is an illegal target");
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "the rejected choice must leave the trigger pending, not consume it"
    );

    // Both ways out of the prompt still work after a rejection.
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: 0,
                decline: true,
            })),
        },
    )
    .expect("declining an optional trigger still works after a rejected target");
    assert!(e.state.pending_triggers.is_empty());
    assert!(
        e.state.players[0].graveyard.contains(&bears),
        "declining leaves the card in the graveyard"
    );
}

/// The same prompt must still be answerable normally after a rejection (the retry path).
#[test]
fn trigger_target_can_be_retried_after_a_rejection() {
    let decks = Some(vec![
        deck_with("swamp", &["gravedigger"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(9307, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bears = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 0, "gravedigger");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "gravedigger");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast Gravedigger");
    resolve_entire_stack_two_player(&mut e);

    let on_battlefield = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let _ = e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: on_battlefield,
                decline: false,
            })),
        },
    );
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: bears,
                decline: false,
            })),
        },
    )
    .expect("retrying with a legal target works");
    pass_both_players(&mut e);
    assert!(
        e.state.players[0].hand.contains(&bears),
        "the retried trigger returned the card to hand"
    );
}
