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
