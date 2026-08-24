use crate::helpers::*;
use tricerules_cards::{ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::state::{AffectedScope, ContinuousEffect};

#[test]
fn summoning_sick_creature_can_block() {
    // CR 302.6: summoning sickness does NOT prevent blocking.
    // Defender has a summoning-sick but untapped creature → engine must enter DeclareBlockers
    // with the defender holding priority.
    let mut e = GameEngine::new(4006, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // Defender's creature is summoning-sick but untapped → eligible blocker.
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    if let Some(obj) = e.state.objects.get_mut(&blocker) {
        obj.summoning_sick = true;
        obj.tapped = false;
    }
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("ap pass declare_attackers");
    let b = e
        .apply_command(1, &pass())
        .expect("nap pass declare_attackers");
    // Defender has an eligible (summoning-sick) blocker → must get priority in DeclareBlockers.
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    assert!(
        priority_changes_in(&b).contains(&1),
        "defender must hold priority in declare_blockers when they have a summoning-sick blocker"
    );
}

// ── Flying & Reach Keyword Tests ─────────────────────────────────────────────
//
// Tests for CR 702.9b (flying) and CR 702.17b (reach) blocking restrictions.

/// A ground creature (no flying, no reach) attempting to block a flying attacker
/// must be rejected by set_blockers with an Illegal error mentioning "flying".
///
/// Setup: P1 has *both* coral_merfolk and a storm_crow so `defending_player_has_eligible_blockers`
/// returns true (storm_crow can block the flyer) and P1 actually reaches manual declaration.
/// We then try to block with the ground-only merfolk — that must fail.
#[test]
fn flying_creature_blocked_by_ground_creature_is_illegal() {
    let mut e = GameEngine::new(9001, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let crow_atk = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    // P1 has a ground creature and a flying creature; engine won't auto-skip.
    let merfolk = inject_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    let _crow_blk = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    e.apply_command(0, &declare_attackers(vec![crow_atk]))
        .expect("declare storm crow attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    // P1 tries to block with coral_merfolk (no flying/reach) — must be rejected.
    let err = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: crow_atk,
                blocker_id: merfolk,
            }]),
        )
        .expect_err("ground creature blocking a flyer should be illegal");
    assert!(
        err.to_string().contains("evasion"),
        "error should mention evasion, got: {err}"
    );
}

/// A creature with flying can block another creature with flying (CR 702.9b).
#[test]
fn flying_creature_can_be_blocked_by_flying_creature() {
    let mut e = GameEngine::new(9002, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let crow_atk = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    let crow_blk = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    e.apply_command(0, &declare_attackers(vec![crow_atk]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: crow_atk,
            blocker_id: crow_blk,
        }]),
    )
    .expect("flying creature must be able to block another flying creature");
}

/// A creature with reach can block a creature with flying (CR 702.17b).
#[test]
fn flying_creature_can_be_blocked_by_reach_creature() {
    let mut e = GameEngine::new(9003, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let crow = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    let spider = inject_creature_on_battlefield(&mut e, 1, "giant_spider");
    e.apply_command(0, &declare_attackers(vec![crow]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: crow,
            blocker_id: spider,
        }]),
    )
    .expect("reach creature must be able to block a flying creature");
}

/// When the only attacker has flying and the defender has only ground creatures,
/// `defending_player_has_eligible_blockers` returns false and the engine emits
/// BlockersDeclared (empty) automatically — no manual declaration needed.
#[test]
fn flying_auto_skips_blockers_when_no_reach_or_flyers() {
    let mut e = GameEngine::new(9004, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let crow = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    // P1 has only a ground creature — cannot legally block the flying crow.
    let _bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![crow]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    // After P1's pass the engine should auto-skip blockers (grizzly_bears can't block flyer).
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare attackers -> auto-skip");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "should enter DeclareBlockers after auto-skip"
    );
    // Auto-skip emits BlockersDeclared with empty pairs.
    let bd = blockers_declared_in(&b);
    assert_eq!(
        bd.len(),
        1,
        "exactly one BlockersDeclared event from auto-skip"
    );
    assert!(bd[0].block_pairs.is_empty(), "no block pairs in auto-skip");
    // The blockers_declared flag is set so combat can proceed without manual declaration.
    assert!(
        e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared flag must be set after auto-skip"
    );
}

// ── Landwalk Evasion Tests ──────────────────────────────────────────────
//
// CR 702.14c: basic landwalk prevents blocks while the defending player controls a land
// with the specified land subtype. CR 509.1b: every applicable blocking restriction must hold.

fn advance_landwalk_attack_to_manual_blocks(e: &mut GameEngine, landwalker_id: &str) -> (u32, u32) {
    advance_to_declare_attackers(e);
    let landwalker = inject_creature_on_battlefield(e, 0, landwalker_id);
    let vanilla_attacker = inject_creature_on_battlefield(e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![landwalker, vanilla_attacker]))
        .expect("declare landwalker and vanilla attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass reaches manual blockers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    assert!(!e.state.combat.as_ref().unwrap().blockers_declared);
    (landwalker, vanilla_attacker)
}

#[test]
fn islandwalk_cannot_be_blocked_when_defender_controls_island() {
    let mut e = GameEngine::new(9050, &[0, 1], 20, None, true).expect("new");
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_permanent_on_battlefield(&mut e, 1, "island");
    let (boa, _) = advance_landwalk_attack_to_manual_blocks(&mut e, "river_boa");

    let err = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: boa,
                blocker_id: blocker,
            }]),
        )
        .expect_err("islandwalk attacker must reject a blocker while defender controls Island");
    assert!(
        err.to_string().contains("evasion"),
        "unexpected error: {err}"
    );
}

#[test]
fn islandwalk_can_be_blocked_when_defender_controls_no_land() {
    let mut e = GameEngine::new(9051, &[0, 1], 20, None, true).expect("new");
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let (boa, _) = advance_landwalk_attack_to_manual_blocks(&mut e, "river_boa");

    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: boa,
            blocker_id: blocker,
        }]),
    )
    .expect("islandwalk is inactive without a matching land");
}

#[test]
fn nonmatching_land_subtype_does_not_enable_islandwalk() {
    let mut e = GameEngine::new(9052, &[0, 1], 20, None, true).expect("new");
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_permanent_on_battlefield(&mut e, 1, "forest");
    let (boa, _) = advance_landwalk_attack_to_manual_blocks(&mut e, "river_boa");

    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: boa,
            blocker_id: blocker,
        }]),
    )
    .expect("Forest must not enable Islandwalk");
}

#[test]
fn inactive_landwalk_still_obeys_flying_restriction() {
    let mut e = GameEngine::new(9056, &[0, 1], 20, None, true).expect("new");
    let ground_blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    inject_permanent_on_battlefield(&mut e, 1, "forest");
    let (boa, _) = advance_landwalk_attack_to_manual_blocks(&mut e, "river_boa");
    e.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(boa),
        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Flying),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: e.state.command_index,
    });

    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: boa,
            blocker_id: ground_blocker,
        }]),
    )
    .expect_err("inactive Islandwalk must not bypass the attacker's flying restriction");
}

#[test]
fn forestwalk_uses_the_same_land_subtype_evasion() {
    let mut e = GameEngine::new(9053, &[0, 1], 20, None, true).expect("new");
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_permanent_on_battlefield(&mut e, 1, "forest");
    let (dryads, _) = advance_landwalk_attack_to_manual_blocks(&mut e, "shanodin_dryads");

    let err = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: dryads,
                blocker_id: blocker,
            }]),
        )
        .expect_err("forestwalk attacker must reject a blocker while defender controls Forest");
    assert!(
        err.to_string().contains("evasion"),
        "unexpected error: {err}"
    );
}

#[test]
fn landwalk_uses_defending_land_controller_not_owner() {
    let mut e = GameEngine::new(9054, &[0, 1], 20, None, true).expect("new");
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let island = inject_permanent_on_battlefield(&mut e, 1, "island");
    e.state.objects.get_mut(&island).unwrap().owner = e.state.players[0].id;
    let (boa, _) = advance_landwalk_attack_to_manual_blocks(&mut e, "river_boa");

    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: boa,
            blocker_id: blocker,
        }]),
    )
    .expect_err("the defending player's controlled Island enables Islandwalk despite ownership");
}

#[test]
fn landwalk_auto_skips_when_every_available_blocker_is_illegal() {
    let mut e = GameEngine::new(9055, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let boa = inject_creature_on_battlefield(&mut e, 0, "river_boa");
    inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_permanent_on_battlefield(&mut e, 1, "island");
    e.apply_command(0, &declare_attackers(vec![boa]))
        .expect("declare islandwalk attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    let batch = e
        .apply_command(1, &pass())
        .expect("defender pass auto-skips illegal blockers");

    let declared = blockers_declared_in(&batch);
    assert_eq!(declared.len(), 1);
    assert!(declared[0].block_pairs.is_empty());
    assert!(e.state.combat.as_ref().unwrap().blockers_declared);
}

// ── Intimidate Keyword Tests ─────────────────────────────────────────────────
//
// Tests for CR 702.13b: a creature with intimidate can only be blocked by
// artifact creatures and/or creatures that share a color with it.

/// A non-artifact creature of a different color cannot block an intimidate creature.
/// Accursed Spirit (Black) vs Grizzly Bears (Green) — no shared color, not artifact.
#[test]
fn intimidate_blocked_by_different_color_non_artifact_is_illegal() {
    let mut e = GameEngine::new(9010, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let spirit = inject_creature_on_battlefield(&mut e, 0, "accursed_spirit");
    // P1 has Grizzly Bears (Green) AND a black creature so engine won't auto-skip.
    let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let _corpse = inject_creature_on_battlefield(&mut e, 1, "walking_corpse"); // black — keeps blockers open
    e.apply_command(0, &declare_attackers(vec![spirit]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    // Grizzly Bears is green, non-artifact — cannot block a black intimidate creature.
    let err = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: spirit,
                blocker_id: bears,
            }]),
        )
        .expect_err("different-color non-artifact blocker must be rejected");
    assert!(
        err.to_string().contains("evasion"),
        "error should mention evasion, got: {err}"
    );
}

/// A creature that shares a color with the intimidate creature can block it.
/// Accursed Spirit (Black) vs Walking Corpse (Black) — same color.
#[test]
fn intimidate_blocked_by_same_color_creature_is_legal() {
    let mut e = GameEngine::new(9011, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let spirit = inject_creature_on_battlefield(&mut e, 0, "accursed_spirit");
    // Walking Corpse costs 1B — it is a Black creature, shares color with the Spirit.
    let corpse = inject_creature_on_battlefield(&mut e, 1, "walking_corpse");
    e.apply_command(0, &declare_attackers(vec![spirit]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: spirit,
            blocker_id: corpse,
        }]),
    )
    .expect("same-color creature must be able to block an intimidate creature");
}

/// An artifact creature can always block an intimidate creature regardless of color.
/// Accursed Spirit (Black) vs Ornithopter (Colorless artifact) — no shared color, but artifact.
#[test]
fn intimidate_blocked_by_artifact_creature_is_legal() {
    let mut e = GameEngine::new(9012, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let spirit = inject_creature_on_battlefield(&mut e, 0, "accursed_spirit");
    // Ornithopter is a colorless artifact creature — qualifies despite no shared color.
    let thopter = inject_creature_on_battlefield(&mut e, 1, "ornithopter");
    e.apply_command(0, &declare_attackers(vec![spirit]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: spirit,
            blocker_id: thopter,
        }]),
    )
    .expect("artifact creature must be able to block an intimidate creature");
}

// ── Vigilance Keyword Tests ───────────────────────────────────────────────────
//
// Tests for CR 702.20b: a creature with vigilance doesn't tap when it attacks.

/// A creature with Vigilance remains untapped after being declared as an attacker.
/// Alpine Watchdog attacks — it should still be untapped after declaration.
#[test]
fn vigilance_attacker_does_not_tap() {
    let mut e = GameEngine::new(9020, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let watchdog = inject_creature_on_battlefield(&mut e, 0, "alpine_watchdog");
    e.apply_command(0, &declare_attackers(vec![watchdog]))
        .expect("declare vigilance attacker");
    let obj = e.state.objects.get(&watchdog).expect("watchdog object");
    assert!(
        !obj.tapped,
        "CR 702.20b: Alpine Watchdog (Vigilance) must NOT be tapped after attacking"
    );
}

/// Regression: a creature without Vigilance still taps when attacking.
/// Grizzly Bears attacks — it should be tapped after declaration.
#[test]
fn non_vigilance_attacker_still_taps() {
    let mut e = GameEngine::new(9021, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // Need a blocker on P1's side so the engine doesn't auto-skip to end combat.
    let _blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare normal attacker");
    let obj = e.state.objects.get(&bears).expect("bears object");
    assert!(
        obj.tapped,
        "a creature without Vigilance must be tapped after attacking"
    );
}

// ── Lifelink Keyword Tests ────────────────────────────────────────────────────
//
// Tests for CR 702.15b: damage dealt by a lifelink permanent also causes its
// controller to gain that much life.

/// An unblocked lifelink attacker deals damage to the defending player AND its
/// controller gains that much life simultaneously (CR 702.15b).
/// Child of Night (2/1 Lifelink) attacks unblocked — P1 loses 2 life, P0 gains 2.
#[test]
fn lifelink_unblocked_attacker_gains_life() {
    let mut e = GameEngine::new(9030, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let con = inject_creature_on_battlefield(&mut e, 0, "child_of_night");
    // No blockers for P1 → auto-skip.
    e.apply_command(0, &declare_attackers(vec![con]))
        .expect("declare lifelink attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    // Auto-empty blockers declared; active player has priority in DeclareBlockers.
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    let life = life_changes_in(&b);
    // Expect two events: defender loses 2, attacker controller gains 2.
    let defender_ev = life
        .iter()
        .find(|lc| lc.player_id == 1)
        .expect("defender LifeChanged");
    let attacker_ev = life
        .iter()
        .find(|lc| lc.player_id == 0)
        .expect("attacker controller LifeChanged (lifelink)");
    assert_eq!(defender_ev.delta, -2, "CR 702.15b: defender takes 2 damage");
    assert_eq!(defender_ev.new_total, 18);
    assert_eq!(attacker_ev.delta, 2, "CR 702.15b: lifelink gains 2 life");
    assert_eq!(attacker_ev.new_total, 22);
    assert_eq!(e.state.players[0].life, 22);
    assert_eq!(e.state.players[1].life, 18);
}

/// A lifelink creature gains its controller life when blocked — it deals damage to
/// the blocker (not the player), but lifelink still triggers (CR 702.15b).
#[test]
fn lifelink_blocked_attacker_still_gains_life() {
    let mut e = GameEngine::new(9031, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let con = inject_creature_on_battlefield(&mut e, 0, "child_of_night");
    // P1 has a blocker to intercept.
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![con]))
        .expect("declare lifelink attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: con,
            blocker_id: blocker,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    let life = life_changes_in(&b);
    // Defender player takes no direct damage (blocked).
    assert!(
        !life.iter().any(|lc| lc.player_id == 1 && lc.delta < 0),
        "defending player should not lose life when attack is blocked"
    );
    // Attacker controller should gain life equal to damage dealt to the blocker.
    let attacker_ev = life
        .iter()
        .find(|lc| lc.player_id == 0)
        .expect("attacker controller LifeChanged (lifelink)");
    assert_eq!(
        attacker_ev.delta, 2,
        "CR 702.15b: lifelink gains life even when blocked"
    );
    assert_eq!(e.state.players[0].life, 22);
}

/// A lifelink blocker gains its controller life for the damage it deals to the
/// attacker (CR 702.15b).
#[test]
fn lifelink_blocker_gains_life() {
    let mut e = GameEngine::new(9032, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    // P0 attacks with a plain Grizzly Bears (no lifelink).
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // P1 blocks with Child of Night (Lifelink, 2 power in injected state).
    let con = inject_creature_on_battlefield(&mut e, 1, "child_of_night");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: con,
        }]),
    )
    .expect("declare lifelink blocker");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    let life = life_changes_in(&b);
    // Defending player takes no direct damage (blocked).
    assert!(
        !life.iter().any(|lc| lc.player_id == 1 && lc.delta < 0),
        "defending player should not lose life (attack was blocked)"
    );
    // Blocker's controller (P1) gains life equal to damage the lifelink blocker dealt.
    let blocker_ev = life
        .iter()
        .find(|lc| lc.player_id == 1 && lc.delta > 0)
        .expect("blocker controller LifeChanged (lifelink)");
    assert_eq!(
        blocker_ev.delta, 2,
        "CR 702.15b: lifelink blocker gains life equal to damage it dealt"
    );
    assert_eq!(e.state.players[1].life, 22);
}

/// When all of the defender's creatures are ineligible to block an intimidate creature,
/// the engine auto-skips blocker declaration.
/// Accursed Spirit (Black) vs Grizzly Bears only (Green, non-artifact) → auto-skip.
#[test]
fn intimidate_auto_skips_blockers_when_no_eligible_creatures() {
    let mut e = GameEngine::new(9013, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let spirit = inject_creature_on_battlefield(&mut e, 0, "accursed_spirit");
    // P1 has only Grizzly Bears — Green, non-artifact, cannot block Black intimidate.
    let _bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![spirit]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    // After P1's pass the engine detects no eligible blockers and auto-skips.
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare attackers -> auto-skip");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "should enter DeclareBlockers after auto-skip"
    );
    let bd = blockers_declared_in(&b);
    assert_eq!(
        bd.len(),
        1,
        "exactly one BlockersDeclared event from auto-skip"
    );
    assert!(bd[0].block_pairs.is_empty(), "no block pairs in auto-skip");
    assert!(
        e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared flag must be set after auto-skip"
    );
}

// ---------------------------------------------------------------------------
// Haste (CR 702.10)
// ---------------------------------------------------------------------------

/// CR 702.10b: A creature with Haste can attack the same turn it enters the battlefield,
/// ignoring summoning sickness. Happy path: Raging Goblin is injected with summoning_sick=true
/// but is allowed to be declared as an attacker.
#[test]
fn haste_creature_can_attack_same_turn_it_enters() {
    let mut e = GameEngine::new(9020, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    // Inject Raging Goblin directly onto the battlefield *with* summoning sickness still set,
    // simulating the turn it just entered.
    let goblin = e.state.next_object_id;
    e.state.next_object_id += 1;
    let pid = e.state.players[0].id;
    e.state.objects.insert(
        goblin,
        tricerules_core::state::GameObject {
            id: goblin,
            owner: pid,
            base_controller: pid,
            controller: pid,
            card_id: "raging_goblin".to_string(),
            copiable_values: None,
            copy_revision: 0,
            zone: tricerules_core::Zone::Battlefield,
            tapped: false,
            summoning_sick: true, // still sick — haste should bypass this
            power: Some(1),
            toughness: Some(1),
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
            counter_timestamps: std::collections::BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            face_down: false,
        },
    );
    e.state.players[0].battlefield.push(goblin);

    // Declare the Goblin as an attacker — must succeed despite summoning_sick = true.
    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("haste creature should be allowed to attack same turn it entered");
    assert!(
        e.state.combat.as_ref().unwrap().attacking.contains(&goblin),
        "raging goblin must appear in the attacking list"
    );
}

/// CR 302.6 / 702.10: Without Haste, a summoning-sick creature may NOT be declared as an
/// attacker. Illegal path: Grizzly Bears with summoning_sick=true are rejected.
#[test]
fn non_haste_summoning_sick_creature_cannot_attack() {
    let mut e = GameEngine::new(9021, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    // Inject Bears directly with summoning sickness and no haste.
    let bears = e.state.next_object_id;
    e.state.next_object_id += 1;
    let pid = e.state.players[0].id;
    e.state.objects.insert(
        bears,
        tricerules_core::state::GameObject {
            id: bears,
            owner: pid,
            base_controller: pid,
            controller: pid,
            card_id: "grizzly_bears".to_string(),
            copiable_values: None,
            copy_revision: 0,
            zone: tricerules_core::Zone::Battlefield,
            tapped: false,
            summoning_sick: true,
            power: Some(2),
            toughness: Some(2),
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
            counter_timestamps: std::collections::BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            face_down: false,
        },
    );
    e.state.players[0].battlefield.push(bears);

    assert!(
        e.apply_command(0, &declare_attackers(vec![bears])).is_err(),
        "summoning-sick creature without haste must not be allowed to attack"
    );
}

/// CR 702.2b / CR 704.5h: A deathtouch attacker destroys a blocker even when
/// the attacker's power is less than the blocker's toughness.
/// Pharika's Chosen (1/1 Deathtouch) attacks → Walking Corpse (2/2) blocks.
/// Chosen deals 1 damage: insufficient to kill normally (toughness 2), but
/// lethal via deathtouch. Walking Corpse dies; Chosen dies to the 2 damage back.
#[test]
fn deathtouch_attacker_kills_blocker_with_higher_toughness() {
    let mut e = GameEngine::new(9040, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    // Pharika's Chosen: 1/1 deathtouch — inject at actual power/toughness.
    let chosen = inject_creature_with_stats(&mut e, 0, "pharikas_chosen", 1, 1);
    // Walking Corpse: 2/2 — would survive 1 damage without deathtouch.
    let corpse = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);

    e.apply_command(0, &declare_attackers(vec![chosen]))
        .expect("declare deathtouch attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: chosen,
            blocker_id: corpse,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let _ = e
        .apply_command(1, &pass())
        .expect("defender pass -> combat damage");

    // Both should be gone: Chosen takes 2 damage ≥ toughness 1; Corpse takes 1
    // deathtouch damage (lethal regardless of toughness).
    assert!(
        !e.state
            .objects
            .get(&chosen)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "Pharika's Chosen should have died to 2 back-damage"
    );
    assert!(
        !e.state
            .objects
            .get(&corpse)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "CR 702.2b: Walking Corpse must die to deathtouch even with 2 toughness"
    );
}

/// CR 702.2b: A deathtouch blocker destroys an attacker with higher toughness.
/// Walking Corpse (2/2) attacks → Pharika's Chosen (1/1 Deathtouch) blocks.
/// Chosen deals 1 deathtouch damage to the Corpse (lethal). Corpse deals 2
/// damage to Chosen (also lethal at 1 toughness). Both die.
#[test]
fn deathtouch_blocker_kills_attacker_with_higher_toughness() {
    let mut e = GameEngine::new(9041, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let corpse = inject_creature_with_stats(&mut e, 0, "walking_corpse", 2, 2);
    let chosen = inject_creature_with_stats(&mut e, 1, "pharikas_chosen", 1, 1);

    e.apply_command(0, &declare_attackers(vec![corpse]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: corpse,
            blocker_id: chosen,
        }]),
    )
    .expect("declare deathtouch blocker");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let _ = e
        .apply_command(1, &pass())
        .expect("defender pass -> combat damage");

    assert!(
        !e.state
            .objects
            .get(&chosen)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "Pharika's Chosen should die to 2 damage (toughness 1)"
    );
    assert!(
        !e.state
            .objects
            .get(&corpse)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "CR 702.2b: Walking Corpse must die to deathtouch blocker's 1 damage"
    );
}

/// CR 702.2b (non-deathtouch control): a non-deathtouch 1-power attacker deals
/// 1 damage to a 2-toughness blocker — the blocker does NOT die.
/// Without deathtouch, 1 damage is not enough to kill a 2/2.
#[test]
fn non_deathtouch_one_power_does_not_kill_two_toughness_blocker() {
    let mut e = GameEngine::new(9042, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    // Raging Goblin: 1/1, Haste — no deathtouch.
    let goblin = inject_creature_with_stats(&mut e, 0, "raging_goblin", 1, 1);
    let corpse = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);

    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: goblin,
            blocker_id: corpse,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let _ = e
        .apply_command(1, &pass())
        .expect("defender pass -> combat damage");

    // Goblin (1/1) dies to 2 back-damage. Corpse (2/2) takes only 1 non-deathtouch
    // damage — survives with 1 marked damage remaining.
    assert!(
        !e.state
            .objects
            .get(&goblin)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "Raging Goblin should die to 2 back-damage"
    );
    assert!(
        e.state
            .objects
            .get(&corpse)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "Walking Corpse should survive 1 non-deathtouch damage (toughness 2)"
    );
    assert_eq!(
        e.state.objects[&corpse].damage, 1,
        "Corpse should have 1 marked damage from the non-deathtouch hit"
    );
}

// ── Menace Keyword Tests ──────────────────────────────────────────────────────
//
// Tests for CR 702.111: a creature with menace can't be blocked except by two
// or more creatures. A single blocker is illegal; zero blockers (unblocked) is
// always fine; two or more blockers is legal.

/// CR 702.111 illegal path: attempting to block a menace creature with exactly
/// one blocker must be rejected with "Illegal blocks."
/// Goblin Trailblazer (2/1 Menace) is attacked; P1 tries to block with a single
/// Grizzly Bears — the engine must reject the declaration, leaving game state in
/// DeclareBlockers so the defender can correct their blocks.
#[test]
fn menace_single_blocker_is_illegal() {
    let mut e = GameEngine::new(9050, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let trailblazer = inject_creature_on_battlefield(&mut e, 0, "goblin_trailblazer");
    // Give P1 two blockers so the engine won't auto-skip — the defender will
    // manually submit an illegal single-blocker declaration.
    let bears1 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let _bears2 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![trailblazer]))
        .expect("declare menace attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    // One blocker on a menace attacker — must be rejected.
    let err = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: trailblazer,
                blocker_id: bears1,
            }]),
        )
        .expect_err("single blocker on menace must be rejected");
    assert_eq!(
        err.to_string(),
        "illegal command: Illegal blocks.",
        "error must say 'Illegal blocks.', got: {err}"
    );
    // Game must NOT have advanced — still in DeclareBlockers, blockers_declared still false.
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "game must stay in DeclareBlockers after illegal menace block"
    );
    assert!(
        !e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared must remain false after illegal menace block"
    );
}

/// CR 702.111 happy path: a menace creature may be blocked by two or more creatures.
/// Goblin Trailblazer blocked by two Grizzly Bears — must succeed.
#[test]
fn menace_two_blockers_is_legal() {
    let mut e = GameEngine::new(9051, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let trailblazer = inject_creature_on_battlefield(&mut e, 0, "goblin_trailblazer");
    let bears1 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let bears2 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![trailblazer]))
        .expect("declare menace attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    // Two blockers on a menace attacker — must be accepted.
    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair {
                attacker_id: trailblazer,
                blocker_id: bears1,
            },
            BlockPair {
                attacker_id: trailblazer,
                blocker_id: bears2,
            },
        ]),
    )
    .expect("two blockers on a menace creature must be legal");
    assert!(
        e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared must be true after legal two-blocker menace block"
    );
}

/// CR 702.111 unblocked case: a menace creature that is not blocked at all is
/// perfectly legal — menace only restricts how it *can* be blocked, not whether
/// the defending player is forced to block it.
/// Two defender creatures are needed here so that `defending_player_has_eligible_blockers`
/// returns true (both can legally co-block the menace attacker), which keeps the engine
/// in the manual declare-blockers step where we can submit an empty declaration.
#[test]
fn menace_unblocked_is_legal() {
    let mut e = GameEngine::new(9052, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let trailblazer = inject_creature_on_battlefield(&mut e, 0, "goblin_trailblazer");
    // Two defender creatures: together they could legally co-block the menace attacker, so the
    // engine keeps the manual step open. The defender then voluntarily declares no blocks.
    let _bears1 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let _bears2 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![trailblazer]))
        .expect("declare menace attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    // Empty blocker declaration — menace creature goes unblocked, which is always fine.
    e.apply_command(1, &declare_blockers(vec![]))
        .expect("empty blockers (menace unblocked) must be legal");
    assert!(
        e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared must be true after legal empty block"
    );
}

/// CR 702.111 auto-skip: when every attacker has menace and the defender has only one
/// creature, no legal non-empty blocking assignment exists (a single creature can't
/// satisfy the 2-blocker minimum alone). The engine must auto-declare empty blockers
/// and skip the manual declare-blockers step, exactly as it does for flying evasion.
#[test]
fn menace_single_creature_auto_skips_blockers() {
    let mut e = GameEngine::new(9053, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let trailblazer = inject_creature_on_battlefield(&mut e, 0, "goblin_trailblazer");
    // Only one defender creature — cannot satisfy menace's 2-blocker requirement alone.
    let _bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![trailblazer]))
        .expect("declare menace attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");

    // P1 passes — engine detects no eligible blockers (menace prevents single-blocker block)
    // and must auto-declare empty blockers, just like flying when no reach/flyers exist.
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass → auto-skip blockers");

    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "should enter DeclareBlockers after menace auto-skip"
    );
    let bd = blockers_declared_in(&b);
    assert_eq!(
        bd.len(),
        1,
        "exactly one BlockersDeclared event from auto-skip"
    );
    assert!(
        bd[0].block_pairs.is_empty(),
        "auto-skipped block must be empty (menace creature goes unblocked)"
    );
    assert!(
        e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared must be set after menace auto-skip"
    );
}

#[test]
fn trample_single_blocker_damage_assignment_needed() {
    // Verify that a trample attacker with a single blocker sets damage_assignment_needed
    // (unlike a non-trample single-blocker which proceeds automatically).
    setup_trample_single_blocker_assign_phase();
}

#[test]
fn trample_single_blocker_lethal_plus_excess_to_player() {
    // Colossal Dreadmaw (6/6 Trample) vs Grizzly Bears (2/2).
    // Legal assignment: 2 to blocker (lethal), 4 to player.
    // Blocker dies (2 damage = toughness). Player takes 4 trample damage.
    let (mut e, attacker, blocker) = setup_trample_single_blocker_assign_phase();
    let p1_life_before = e.state.players[1].life;

    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 2)], 4),
        )
        .expect("assign 2 to blocker + 4 to player");

    let dead: Vec<u32> = permanents_moved_in(&b)
        .iter()
        .map(|p| p.object_id)
        .collect();

    // Blocker (2/2) receives 2 lethal → dies.
    assert!(
        dead.contains(&blocker),
        "blocker dies from lethal damage: {dead:?}"
    );
    // Attacker (6/6) receives 2 damage from blocker but survives (toughness 6).
    let att_obj = e.state.objects.get(&attacker);
    // Attacker may still be alive (6 toughness vs 2 damage); just confirm it's not in dead list.
    assert!(
        !dead.contains(&attacker),
        "dreadmaw survives (toughness 6 vs blocker power 2): {dead:?}"
    );

    // Defending player takes 4 trample damage.
    let life_evs = life_changes_in(&b);
    assert!(
        life_evs
            .iter()
            .any(|lc| lc.player_id == 1 && lc.delta == -4),
        "player must take 4 trample damage: {life_evs:?}"
    );
    assert_eq!(
        e.state.players[1].life,
        p1_life_before - 4,
        "player life after trample"
    );
    // Attacker stat: blocker dealt 2 power back
    if let Some(att_o) = att_obj {
        assert_eq!(
            att_o.damage, 2,
            "dreadmaw has 2 marked damage from the blocker"
        );
    }
    assert!(e.state.combat.is_none(), "combat cleared after resolution");
}

#[test]
fn trample_rejects_less_than_lethal_to_blocker() {
    // CR 702.19b: must assign >= lethal damage to each blocker before trample to player.
    // Colossal Dreadmaw (6/6) vs Grizzly Bears (2/2): assigning only 1 to blocker (< lethal 2) is illegal.
    let (mut e, attacker, blocker) = setup_trample_single_blocker_assign_phase();
    let err = e.apply_command(
        0,
        &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 1)], 5),
    );
    assert!(
        err.is_err(),
        "assigning less than lethal to blocker must be rejected"
    );
    // State remains in assign phase for retry.
    assert!(e.state.combat.as_ref().unwrap().assign_combat_damage_phase);
    assert!(e.state.combat.as_ref().unwrap().damage_assignment_needed);
}

#[test]
fn trample_rejects_sum_mismatch() {
    // Colossal Dreadmaw (6/6) vs Grizzly Bears (2/2).
    // 2 to blocker + 3 to player = 5 ≠ 6 (power): must be rejected.
    let (mut e, attacker, blocker) = setup_trample_single_blocker_assign_phase();
    assert!(
        e.apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 2)], 3),
        )
        .is_err(),
        "blocker + player damage not equal to attacker power must be rejected"
    );
}

#[test]
fn trample_rejects_player_damage_without_trample() {
    // Non-trample multi-blocked attacker (Grizzly Bears 2/2) must not accept defending_player_damage > 0.
    let (mut e, attacker, a, b) = setup_two_blockers_assign_phase(5010);
    assert!(
        e.apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(a, 1), (b, 0)], 1),
        )
        .is_err(),
        "player damage on non-trample attacker must be rejected"
    );
}

#[test]
fn trample_all_damage_to_blocker_zero_to_player() {
    // Colossal Dreadmaw (6/6 Trample) vs Grizzly Bears (2/2).
    // It is legal to assign all 6 damage to the single blocker (0 to player), provided
    // lethal is still met (6 >= 2). Blocker dies, player takes 0.
    let (mut e, attacker, blocker) = setup_trample_single_blocker_assign_phase();
    let p1_life_before = e.state.players[1].life;

    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 6)], 0),
        )
        .expect("assign all 6 to blocker, 0 to player");

    let dead: Vec<u32> = permanents_moved_in(&b)
        .iter()
        .map(|p| p.object_id)
        .collect();
    assert!(dead.contains(&blocker), "blocker dies: {dead:?}");
    assert_eq!(
        e.state.players[1].life, p1_life_before,
        "player takes 0 trample damage when all assigned to blocker"
    );
    assert!(
        life_changes_in(&b)
            .iter()
            .all(|lc| lc.delta >= 0 || lc.player_id != 1),
        "no negative life event for defending player"
    );
}

#[test]
fn trample_multi_blocked_excess_to_player() {
    // Colossal Dreadmaw (6/6 Trample) vs two Grizzly Bears (2/2 each).
    // Legal: assign 2 to first blocker (lethal), 2 to second (lethal), 2 to player.
    // Both blockers die; player takes 2.
    let decks = Some(vec![
        std::iter::repeat_n("colossal_dreadmaw".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(5020, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "colossal_dreadmaw");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "colossal_dreadmaw");
    let b1 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b2 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

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
    e.apply_command(1, &pass())
        .expect("defender pass → assign phase");

    let p1_life_before = e.state.players[1].life;
    let batch = e
        .apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(b1, 2), (b2, 2)], 2),
        )
        .expect("assign 2+2+2 trample");

    let dead: Vec<u32> = permanents_moved_in(&batch)
        .iter()
        .map(|p| p.object_id)
        .collect();
    assert!(dead.contains(&b1), "b1 dies: {dead:?}");
    assert!(dead.contains(&b2), "b2 dies: {dead:?}");
    assert!(
        !dead.contains(&attacker),
        "dreadmaw survives (6 toughness vs 2+2 blocker power): {dead:?}"
    );

    let life_evs = life_changes_in(&batch);
    assert!(
        life_evs
            .iter()
            .any(|lc| lc.player_id == 1 && lc.delta == -2),
        "defending player takes 2 trample damage: {life_evs:?}"
    );
    assert_eq!(e.state.players[1].life, p1_life_before - 2);
    assert!(e.state.combat.is_none(), "combat cleared");
}

#[test]
fn trample_multi_blocked_rejects_less_than_lethal_to_first_blocker() {
    // Colossal Dreadmaw (6/6 Trample) vs two Grizzly Bears (2/2 each).
    // Assign 1 to first blocker (< lethal 2): must be rejected.
    let decks = Some(vec![
        std::iter::repeat_n("colossal_dreadmaw".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(5021, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "colossal_dreadmaw");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "colossal_dreadmaw");
    let b1 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b2 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");
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
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    // 1 to first blocker < lethal (2): illegal.
    assert!(
        e.apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(b1, 1), (b2, 2)], 3),
        )
        .is_err(),
        "must reject when first blocker receives less than lethal"
    );
}

// CR 510.4 + 702.7 + 702.4: first strike / double strike — these tests verify the engine
// splits combat damage into a first-strike substep when applicable, and that participation
// follows the CR 510.4 rule (FirstStrike/DoubleStrike in first step; remainder + DoubleStrike
// in regular step). The pass-priority button label change is verified separately in the C++
// prompt-widget code path (the engine emits `first_strike_step_pending` via zone view).

/// CR 702.7 (first strike) + CR 510.4: a first-strike attacker kills a vanilla blocker in
/// the first-strike step (CR 510.2 SBAs run between steps) and takes no return damage in the
/// regular step. Goblin Striker (1/1 FS+Haste) attacks; defender blocks with Walking Corpse
/// (2/2). Walking Corpse has only 2 toughness — but the attacker only has power 1, so the
/// corpse survives if there's no first strike. With first strike: Goblin Striker deals 1 in
/// first-strike step (corpse marked but not lethal — toughness 2), then in the regular step
/// the corpse deals 2 back. The corpse survives at 1 toughness, goblin dies. This documents
/// the *order* and shows the first-strike step ran (corpse remained alive without taking
/// return damage from goblin a second time).
#[test]
fn first_strike_attacker_against_vanilla_blocker_survives_or_dies_per_pt() {
    let mut e = GameEngine::new(11_001, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let goblin = inject_creature_with_stats(&mut e, 0, "goblin_striker", 1, 1);
    let corpse = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);

    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: goblin,
            blocker_id: corpse,
        }]),
    )
    .expect("declare blocker");

    // Both players pass priority in declare blockers → engine enters first-strike substep.
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    e.apply_command(1, &pass()).expect("def pass dec blk");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage,
        "engine must enter FirstStrikeDamage substep when an attacker has First Strike"
    );

    // Goblin is alive (corpse hasn't dealt damage yet); corpse has 1 marked damage.
    assert!(
        e.state
            .objects
            .get(&goblin)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "goblin must be alive entering the regular damage step"
    );
    assert_eq!(
        e.state.objects.get(&corpse).map(|o| o.damage),
        Some(1),
        "corpse must have 1 marked damage from first-strike"
    );

    // Pass priority in first-strike step → engine runs regular combat damage step.
    e.apply_command(0, &pass()).expect("ap pass fs damage");
    e.apply_command(1, &pass()).expect("def pass fs damage");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::CombatDamage,
        "engine must reach CombatDamage after the regular damage step resolves"
    );
    // Corpse deals 2 to goblin in regular step → goblin dies. Corpse keeps its 1 damage.
    assert!(
        !e.state
            .objects
            .get(&goblin)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "goblin must die to corpse's 2 return damage in the regular step"
    );
    assert!(
        e.state
            .objects
            .get(&corpse)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "corpse must survive with 1 marked damage (toughness 2)"
    );
}

/// CR 702.4 (double strike): a double-strike attacker deals damage in both steps. Fencing Ace
/// (1/1 double strike) attacks → Grizzly Bears (2/2 vanilla) blocks. First-strike step: Ace
/// deals 1, Bears marked. Bears die not yet (toughness 2). Regular step: both deal damage —
/// Ace deals another 1 (Bears now at 2 → dies), Bears deal 2 (Ace dies). Both die.
#[test]
fn double_strike_attacker_against_vanilla_blocker_both_die() {
    let mut e = GameEngine::new(11_002, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let ace = inject_creature_with_stats(&mut e, 0, "fencing_ace", 1, 1);
    let bears = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);

    e.apply_command(0, &declare_attackers(vec![ace]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: ace,
            blocker_id: bears,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    e.apply_command(1, &pass()).expect("def pass dec blk");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage,
        "double strike triggers the first-strike substep"
    );
    // After first strike: bears at 1 damage, ace alive (bears haven't swung yet).
    assert_eq!(e.state.objects.get(&bears).map(|o| o.damage), Some(1));
    assert!(e
        .state
        .objects
        .get(&ace)
        .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield));

    e.apply_command(0, &pass()).expect("ap pass fs damage");
    e.apply_command(1, &pass()).expect("def pass fs damage");
    // Regular step: ace deals another 1 (bears die at 2 damage = toughness), bears deal 2 (ace dies).
    assert!(
        !e.state
            .objects
            .get(&ace)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "ace must die to bears' return damage"
    );
    assert!(
        !e.state
            .objects
            .get(&bears)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "bears must die to ace's two damage instances"
    );
}

/// CR 510.4: when no attacker or blocker has FirstStrike/DoubleStrike, the engine skips the
/// first-strike substep — DeclareBlockers transitions directly to CombatDamage, just like
/// vanilla combat before this change.
#[test]
fn vanilla_combat_skips_first_strike_step() {
    let mut e = GameEngine::new(11_003, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let corpse = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);
    e.apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: bears,
            blocker_id: corpse,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    e.apply_command(1, &pass()).expect("def pass dec blk");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::CombatDamage,
        "vanilla combat must skip FirstStrikeDamage and go straight to CombatDamage"
    );
    // Both 2/2s trade as before.
    assert!(!e
        .state
        .objects
        .get(&bears)
        .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield));
    assert!(!e
        .state
        .objects
        .get(&corpse)
        .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield));
}

/// CR 510.4: when a first-strike attacker is unblocked, the defending player takes damage in
/// the first-strike step. Verified via the LifeChanged event and player life total.
#[test]
fn first_strike_unblocked_deals_damage_in_first_strike_step() {
    let mut e = GameEngine::new(11_004, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let goblin = inject_creature_with_stats(&mut e, 0, "goblin_striker", 1, 1);
    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    // No blockers: engine auto-declares empty blockers then awaits passes.
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    e.apply_command(1, &pass()).expect("def pass dec blk");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage,
        "first-strike substep must run for an unblocked FS attacker"
    );
    let life_after_fs = e.state.players[1].life;
    assert_eq!(
        life_after_fs, 19,
        "defender should take 1 damage in first-strike step"
    );
    e.apply_command(0, &pass()).expect("ap pass fs damage");
    e.apply_command(1, &pass()).expect("def pass fs damage");
    assert_eq!(
        e.state.players[1].life, 19,
        "regular step deals no extra damage for first strike"
    );
}

/// CR 702.4 double strike unblocked: deals damage in BOTH steps. Fencing Ace unblocked hits
/// the defender for 1 in first-strike step, then 1 more in the regular step → 2 total.
#[test]
fn double_strike_unblocked_deals_damage_in_both_steps() {
    let mut e = GameEngine::new(11_005, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let ace = inject_creature_with_stats(&mut e, 0, "fencing_ace", 1, 1);
    e.apply_command(0, &declare_attackers(vec![ace]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    e.apply_command(1, &pass()).expect("def pass dec blk");
    assert_eq!(e.state.players[1].life, 19, "first-strike step deals 1");
    e.apply_command(0, &pass()).expect("ap pass fs damage");
    e.apply_command(1, &pass()).expect("def pass fs damage");
    assert_eq!(
        e.state.players[1].life, 18,
        "double strike deals damage again in the regular step (total 2)"
    );
}

// ---------------------------------------------------------------------------
// CR 702.12 — Indestructible
// ---------------------------------------------------------------------------

/// CR 702.12b: A "destroy" spell has no effect on an indestructible permanent.
/// Darksteel Myr survives Murder (used here because Murder has no targeting restrictions
/// that will need future enforcement; Go for the Throat can't target artifacts).
#[test]
fn indestructible_survives_destroy_spell() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "murder".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec![
            "mountain".into(),
            "darksteel_myr".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(7001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let myr = put_creature_on_battlefield(&mut e, 1, "darksteel_myr");

    // Give P0 three black mana for Murder (1BB): seed two swamps and play a third.
    for _ in 0..2 {
        let idx = hand_index_for_card(&e, 0, "swamp");
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        e.state.objects.get_mut(&oid).unwrap().zone = tricerules_core::Zone::Battlefield;
    }
    let swamp_idx = hand_index_for_card(&e, 0, "swamp");
    e.apply_command(0, &play_land(swamp_idx))
        .expect("play swamp");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );

    let murder_idx = hand_index_for_card(&e, 0, "murder");
    e.apply_command(
        0,
        &cast_spell(
            murder_idx,
            vec![TargetRef {
                object_id: myr,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast murder");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    // Myr must still be on the battlefield.
    assert_eq!(
        e.state.objects.get(&myr).expect("myr object").zone,
        tricerules_core::Zone::Battlefield,
        "CR 702.12b: indestructible Myr must not be destroyed by Go for the Throat"
    );
    assert!(
        !e.state.players[1].graveyard.contains(&myr),
        "Myr must not be in graveyard"
    );
}

/// CR 702.12b: An indestructible creature is not destroyed by lethal combat damage.
/// Darksteel Myr (0/1 indestructible) blocks a 5/5; it takes lethal damage but stays.
#[test]
fn indestructible_survives_lethal_combat_damage() {
    let mut e = GameEngine::new(7002, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let attacker = inject_creature_with_stats(&mut e, 0, "colossal_dreadmaw", 6, 6);
    let myr = inject_creature_with_stats(&mut e, 1, "darksteel_myr", 0, 1);

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("p0 pass after attackers");
    e.apply_command(1, &pass())
        .expect("p1 pass after attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: myr,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass()).expect("p0 pass after blockers");
    e.apply_command(1, &pass())
        .expect("p1 pass after blockers -> combat damage resolves");

    assert_eq!(
        e.state.objects.get(&myr).expect("myr object").zone,
        tricerules_core::Zone::Battlefield,
        "CR 702.12b: indestructible Myr must survive lethal combat damage"
    );
}

/// CR 704.5f + CR 702.12b: indestructible does NOT protect against toughness dropping to 0.
/// A -0/-1 pump on a 0/1 Darksteel Myr brings toughness to 0; SBA kills it.
#[test]
fn indestructible_dies_when_toughness_reaches_zero() {
    let mut e = GameEngine::new(7003, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Darksteel Myr (0/1) directly onto P1's battlefield.
    let myr = inject_creature_with_stats(&mut e, 1, "darksteel_myr", 0, 1);

    // Manually set toughness to 0 to simulate a -0/-1 effect, then trigger SBAs via
    // a pass-priority sequence (SBAs run at the start of each priority check).
    e.state.objects.get_mut(&myr).unwrap().toughness = Some(0);

    // Pass priority — SBAs fire on the next priority check.
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(
        e.state.objects.get(&myr).expect("myr object").zone,
        tricerules_core::Zone::Graveyard,
        "CR 704.5f: indestructible Myr with toughness 0 must still die"
    );
}

#[test]
fn defender_creature_cannot_be_declared_as_attacker() {
    let decks = Some(vec![
        deck_with("mountain", &["wall_of_stone", "grizzly_bears"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(7001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let wall = relocate_to_battlefield(&mut e, 0, "wall_of_stone", false);
    let bears = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);

    // Main1 -> BeginCombat -> DeclareAttackers (an eligible non-defender attacker exists).
    e.apply_command(0, &primitive_yield())
        .expect("main1 -> begin combat");
    e.apply_command(0, &primitive_yield())
        .expect("begin combat -> declare attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    // CR 702.3b: a creature with defender can't attack.
    let err = e.apply_command(0, &declare_attackers(vec![wall]));
    assert!(
        err.is_err(),
        "defender creature must be rejected as an attacker"
    );
    // The rejected declaration mutates nothing: the bears can still be declared.
    e.apply_command(0, &declare_attackers(vec![bears]))
        .expect("non-defender attacks");
    assert_eq!(
        e.state.combat.as_ref().expect("combat").attacking,
        vec![bears]
    );
}

#[test]
fn lone_defender_provides_no_eligible_attackers() {
    let decks = Some(vec![
        deck_with("mountain", &["wall_of_stone"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(7002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "wall_of_stone", false);

    e.apply_command(0, &primitive_yield())
        .expect("main1 -> begin combat");
    e.apply_command(0, &primitive_yield())
        .expect("begin combat advance");
    // With only a defender out, combat never enters the declare-attackers step.
    assert_ne!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers,
        "a lone defender is not an eligible attacker"
    );
}

#[test]
fn flash_creature_castable_at_instant_speed_unlike_flashless() {
    // 7-card opening hand holds both creatures; we cast during P0's own upkeep, which is
    // instant timing (CR 503) but NOT sorcery speed.
    let decks = Some(vec![
        deck_with("forest", &["ambush_viper", "grizzly_bears"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(7100, &[0, 1], 20, decks, true).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    relocate_to_hand(&mut e, 0, "ambush_viper");
    relocate_to_hand(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 4,
            ..Default::default()
        },
    );

    // A flashless creature can't be cast at instant speed (CR 601.3a).
    let bears_idx = hand_index_for_card(&e, 0, "grizzly_bears");
    let err = e.apply_command(0, &cast_spell(bears_idx, vec![]));
    assert!(
        err.is_err(),
        "flashless creature must be sorcery-speed only"
    );

    // The flash creature casts in the same window (CR 702.8b).
    let viper_idx = hand_index_for_card(&e, 0, "ambush_viper");
    e.apply_command(0, &cast_spell(viper_idx, vec![]))
        .expect("flash creature casts at instant speed");
    assert_eq!(
        e.state.stack.last().expect("viper on stack").card_id,
        "ambush_viper"
    );
}

// ── Keyword-Granting Continuous Effects (Layer 6) ────────────────────────────
//
// Tests for CR 613 layer 6: keywords granted by static abilities (lords) and
// one-shot sorceries. Uses `effective_has_keyword` — the rules-visible check
// that considers both card-def keywords and Layer6AddKeyword continuous effects.

/// Goblin Chieftain grants Haste to other Goblins you control (static AnthemKeyword).
/// A freshly-injected Goblin Trailblazer (summoning-sick, no innate Haste) must be
/// able to attack once the Chieftain is on the battlefield.
#[test]
fn goblin_chieftain_grants_haste_to_other_goblins() {
    let decks = Some(vec![
        deck_with("mountain", &["goblin_chieftain"]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(6100, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    // Guarantee the card is in hand regardless of shuffle order.
    take_card_from_library_to_hand(&mut e, 0, "goblin_chieftain");

    // Inject a goblin trailblazer for P0 that is summoning-sick (no innate Haste).
    let trailblazer = inject_creature_on_battlefield(&mut e, 0, "goblin_trailblazer");
    if let Some(obj) = e.state.objects.get_mut(&trailblazer) {
        obj.summoning_sick = true;
    }

    // Without Goblin Chieftain the trailblazer cannot attack.
    assert!(
        !e.effective_has_keyword(trailblazer, tricerules_cards::Keyword::Haste),
        "goblin trailblazer has no haste before chieftain enters"
    );

    // Cast Goblin Chieftain ({1}{R}{R}).
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "goblin_chieftain");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast goblin chieftain");
    resolve_entire_stack_two_player(&mut e);

    // After Chieftain resolves, its static AnthemKeyword(Haste) fires for other Goblins.
    let chieftain = battlefield_object_for_card(&e, 0, "goblin_chieftain");
    assert!(
        e.effective_has_keyword(trailblazer, tricerules_cards::Keyword::Haste),
        "trailblazer gains haste from goblin chieftain's layer-6 continuous effect"
    );
    // Chieftain does NOT grant haste to itself (exclude_self = true).
    assert!(
        e.effective_has_keyword(chieftain, tricerules_cards::Keyword::Haste),
        "chieftain has haste from its own card definition (not self-grant)"
    );
}

/// Goblin Chieftain's Haste grant lets a summoning-sick Goblin attack legally.
#[test]
fn goblin_chieftain_haste_grant_allows_sick_goblin_to_attack() {
    let decks = Some(vec![
        deck_with("mountain", &["goblin_chieftain"]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(6101, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    take_card_from_library_to_hand(&mut e, 0, "goblin_chieftain");

    // Cast Goblin Chieftain first.
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "goblin_chieftain");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast goblin chieftain");
    resolve_entire_stack_two_player(&mut e);

    // Inject a summoning-sick Goblin Trailblazer for P0.
    let trailblazer = inject_creature_on_battlefield(&mut e, 0, "goblin_trailblazer");
    if let Some(obj) = e.state.objects.get_mut(&trailblazer) {
        obj.summoning_sick = true;
    }

    // Advance to DeclareAttackers; the trailblazer should be eligible due to granted haste.
    e.apply_command(0, &primitive_yield()).expect("main1 yield");
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    // Goblin Trailblazer (summoning-sick) should be able to attack — Chieftain grants Haste.
    e.apply_command(0, &declare_attackers(vec![trailblazer]))
        .expect("summoning-sick goblin attacks due to haste grant from chieftain");
}

/// Goblin Chieftain does NOT grant Haste to non-Goblin creatures.
#[test]
fn goblin_chieftain_does_not_grant_haste_to_non_goblins() {
    let decks = Some(vec![
        deck_with("mountain", &["goblin_chieftain"]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(6102, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "goblin_chieftain");

    // Inject a non-goblin creature (Grizzly Bears).
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    if let Some(obj) = e.state.objects.get_mut(&bear) {
        obj.summoning_sick = true;
    }

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "goblin_chieftain");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast goblin chieftain");
    resolve_entire_stack_two_player(&mut e);

    // Bear is not a Goblin — chieftain's haste grant doesn't apply.
    assert!(
        !e.effective_has_keyword(bear, tricerules_cards::Keyword::Haste),
        "non-goblin bear must not gain haste from goblin chieftain"
    );
}

/// Overrun grants Trample to all your creatures until end of turn (GrantKeywordsAll).
#[test]
fn overrun_grants_trample_until_end_of_turn() {
    let decks = Some(vec![
        deck_with("forest", &["overrun"]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(6103, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    take_card_from_library_to_hand(&mut e, 0, "overrun");

    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    assert!(
        !e.effective_has_keyword(creature, tricerules_cards::Keyword::Trample),
        "creature has no trample before overrun"
    );

    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 5,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "overrun");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast overrun");
    resolve_entire_stack_two_player(&mut e);

    // Your creature gains Trample from GrantKeywordsAll.
    assert!(
        e.effective_has_keyword(creature, tricerules_cards::Keyword::Trample),
        "your creature gains trample from overrun"
    );
    // Opponent's creature is unaffected (YouControl filter).
    assert!(
        !e.effective_has_keyword(opponent_creature, tricerules_cards::Keyword::Trample),
        "opponent's creature does not gain trample from overrun"
    );
}

/// Captain of the Watch grants Vigilance to other Soldiers you control.
/// Soldiers it creates should not tap when attacking.
#[test]
fn captain_of_the_watch_grants_vigilance_to_soldiers() {
    let decks = Some(vec![
        deck_with("plains", &["captain_of_the_watch"]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(6105, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject a Soldier token placeholder: use the generated 1/1 soldier type.
    // We'll use a savannah_lions (not a Soldier) vs soldier_w_1_1 injection.
    // The Captain's ETB trigger creates three soldier tokens, so use those.

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 6,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "captain_of_the_watch");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast captain of the watch");
    // Captain enters; its ETB trigger (create 3 soldier tokens) is on the stack.
    // Resolve the stack entirely (ETB trigger + spell resolution).
    resolve_entire_stack_two_player(&mut e);

    let captain = battlefield_object_for_card(&e, 0, "captain_of_the_watch");

    // The captain's AnthemKeyword grants Vigilance to other Soldiers you control.
    // Inject a soldier token manually and verify the grant applies.
    let soldier = inject_creature_on_battlefield(&mut e, 0, "soldier_w_1_1");
    assert!(
        e.effective_has_keyword(soldier, tricerules_cards::Keyword::Vigilance),
        "soldier token gains vigilance from captain of the watch"
    );
    // Captain itself has Vigilance from card definition (not self-grant since exclude_self=true,
    // but the captain IS in the Soldier subtype, so the effect would apply if not excluded).
    // Verify that the captain still has vigilance (from its own card def).
    assert!(
        e.effective_has_keyword(captain, tricerules_cards::Keyword::Vigilance),
        "captain retains vigilance from its own keyword list"
    );
}
