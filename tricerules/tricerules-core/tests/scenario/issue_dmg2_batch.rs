//! Reviewed damage-and-riders scenarios: Lightning Helix, Winter's Intervention, Deadly Riposte,
//! Cosmium Blast, Vibrant Outburst, Fear of Lost Teeth and Bile-Vial Boggart.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 115 (targets),
//! 120 (damage), 119.3 (life gain), 508/509 (attacking/blocking), 701.26 (tap), 122.1 (counters),
//! and 608.2b (an illegal target at resolution fizzles the whole spell).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::TargetRefKind;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn combat_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

/// Cast `card` from P0's hand with `targets` and resolve the whole stack.
fn cast_and_resolve(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

/// Kill `source` with Murder so the committed battlefield-to-graveyard move emits the dies event.
fn kill_with_murder(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    let slot = hand_index_for_card(e, 0, "murder");
    semantic::accepted(e, 0, &cast_spell(slot, target_object(source)));
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("murder resolves");
}

fn choose_trigger_target(object_id: u32, kind: TargetRefKind) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: kind as i32,
                ..Default::default()
            }],
        })),
    }
}

#[test]
fn issue_dmg2_lightning_helix() {
    let mut e = engine(724_001);
    let p0_before = e.state.players[0].life;
    let p1_before = e.state.players[1].life;
    cast_and_resolve(&mut e, "lightning_helix", target_player_damage(1, 3));
    assert_eq!(e.state.players[1].life, p1_before - 3);
    assert_eq!(e.state.players[0].life, p0_before + 3);

    // If the only target is illegal at resolution, the spell fizzles: no damage, no life.
    let mut fizzle = engine(724_011);
    let target = inject_creature_on_battlefield(&mut fizzle, 1, "grizzly_bears");
    let p0_before = fizzle.state.players[0].life;
    let p1_before = fizzle.state.players[1].life;
    inject_card_into_hand(&mut fizzle, 0, "lightning_helix");
    let slot = hand_index_for_card(&fizzle, 0, "lightning_helix");
    semantic::accepted(&mut fizzle, 0, &cast_spell(slot, target_object(target)));
    fizzle.state.players[1]
        .battlefield
        .retain(|id| *id != target);
    fizzle.state.players[1].graveyard.push(target);
    fizzle.state.objects.get_mut(&target).expect("object").zone = Zone::Graveyard;
    *fizzle
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(
        fizzle.state.players[0].life, p0_before,
        "no life on a fizzle"
    );
    assert_eq!(
        fizzle.state.players[1].life, p1_before,
        "no damage on a fizzle"
    );
}

#[test]
fn issue_dmg2_winters_intervention() {
    let mut e = engine(724_002);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let life = e.state.players[0].life;
    cast_and_resolve(&mut e, "winters_intervention", target_object(bear));
    assert_eq!(
        e.state.objects[&bear].zone,
        Zone::Graveyard,
        "two damage kills a 2/2"
    );
    assert_eq!(e.state.players[0].life, life + 2);

    // A noncreature permanent is not a legal target.
    let mut bad = engine(724_012);
    let boots = inject_permanent_on_battlefield(&mut bad, 1, "swiftfoot_boots");
    inject_card_into_hand(&mut bad, 0, "winters_intervention");
    let slot = hand_index_for_card(&bad, 0, "winters_intervention");
    assert!(
        bad.apply_command(0, &cast_spell(slot, target_object(boots)))
            .is_err(),
        "an artifact is not a creature"
    );
}

#[test]
fn issue_dmg2_deadly_riposte() {
    let mut e = engine(724_003);
    let tapped = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    e.state.objects.get_mut(&tapped).expect("object").tapped = true;
    let life = e.state.players[0].life;
    cast_and_resolve(&mut e, "deadly_riposte", target_object(tapped));
    assert_eq!(e.state.objects[&tapped].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, life + 2);

    // An untapped creature is not a legal target.
    let mut untapped = engine(724_013);
    let bear = inject_creature_on_battlefield(&mut untapped, 1, "grizzly_bears");
    inject_card_into_hand(&mut untapped, 0, "deadly_riposte");
    let slot = hand_index_for_card(&untapped, 0, "deadly_riposte");
    assert!(
        untapped
            .apply_command(0, &cast_spell(slot, target_object(bear)))
            .is_err(),
        "an untapped creature is not a legal target"
    );
}

#[test]
fn issue_dmg2_cosmium_blast() {
    let mut e = combat_engine(724_004);
    let attacker = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    grant_pool(&mut e, 0);
    cast_and_resolve(&mut e, "cosmium_blast", target_object(attacker));
    assert_eq!(
        e.state.objects[&attacker].zone,
        Zone::Graveyard,
        "four damage kills the attacking creature"
    );

    // A creature that is neither attacking nor blocking is not a legal target.
    let mut idle = engine(724_014);
    let bear = inject_creature_on_battlefield(&mut idle, 1, "grizzly_bears");
    inject_card_into_hand(&mut idle, 0, "cosmium_blast");
    let slot = hand_index_for_card(&idle, 0, "cosmium_blast");
    assert!(
        idle.apply_command(0, &cast_spell(slot, target_object(bear)))
            .is_err(),
        "an idle creature is not attacking or blocking"
    );
}

#[test]
fn issue_dmg2_vibrant_outburst() {
    let mut e = engine(724_005);
    let to_tap = inject_creature_on_battlefield(&mut e, 1, "serra_angel");
    let p1_before = e.state.players[1].life;
    let p1_id = e.state.players[1].id as u32;
    inject_card_into_hand(&mut e, 0, "vibrant_outburst");
    let slot = hand_index_for_card(&e, 0, "vibrant_outburst");
    semantic::accepted(
        &mut e,
        0,
        &cast_spell(
            slot,
            vec![
                TargetRef {
                    object_id: p1_id,
                    group_index: 0,
                    kind: TargetRefKind::Player as i32,
                    ..Default::default()
                },
                TargetRef {
                    object_id: to_tap,
                    group_index: 1,
                    kind: TargetRefKind::Permanent as i32,
                    ..Default::default()
                },
            ],
        ),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life,
        p1_before - 3,
        "three damage to the player"
    );
    assert!(
        e.state.objects[&to_tap].tapped,
        "the second target is tapped"
    );

    // The tap target is optional: casting with only the damage target is legal, and a bystander
    // creature must not be tapped.
    let mut optional = engine(724_015);
    let bystander = inject_creature_on_battlefield(&mut optional, 1, "serra_angel");
    let p1_before = optional.state.players[1].life;
    let p1_id = optional.state.players[1].id as u32;
    inject_card_into_hand(&mut optional, 0, "vibrant_outburst");
    let slot = hand_index_for_card(&optional, 0, "vibrant_outburst");
    semantic::accepted(
        &mut optional,
        0,
        &cast_spell(
            slot,
            vec![TargetRef {
                object_id: p1_id,
                group_index: 0,
                kind: TargetRefKind::Player as i32,
                ..Default::default()
            }],
        ),
    );
    resolve_entire_stack_two_player(&mut optional);
    assert_eq!(optional.state.players[1].life, p1_before - 3);
    assert!(
        !optional.state.objects[&bystander].tapped,
        "an untargeted creature is not tapped"
    );
}

#[test]
fn issue_dmg2_fear_of_lost_teeth() {
    let mut e = engine(724_006);
    let fear = inject_creature_with_stats(&mut e, 0, "fear_of_lost_teeth", 1, 1);
    let p0_before = e.state.players[0].life;
    let p1_before = e.state.players[1].life;
    kill_with_murder(&mut e, fear);
    assert_eq!(e.state.objects[&fear].zone, Zone::Graveyard);
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "the dies trigger waits for its target"
    );
    e.apply_command(
        0,
        &choose_trigger_target(e.state.players[1].id as u32, TargetRefKind::Player),
    )
    .expect("choose the opponent as the damage target");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life,
        p1_before - 1,
        "one damage to the chosen target"
    );
    assert_eq!(
        e.state.players[0].life,
        p0_before + 1,
        "its controller gains one life"
    );
}

#[test]
fn issue_dmg2_bile_vial_boggart() {
    // Choosing a target places a -1/-1 counter.
    let mut e = engine(724_007);
    let boggart = inject_creature_with_stats(&mut e, 0, "bile-vial_boggart", 1, 1);
    let target = inject_creature_with_stats(&mut e, 1, "serra_angel", 4, 4);
    kill_with_murder(&mut e, boggart);
    assert_eq!(e.state.pending_triggers.len(), 1);
    e.apply_command(0, &choose_trigger_target(target, TargetRefKind::Permanent))
        .expect("choose the target creature");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(target),
        Some(3),
        "a -1/-1 counter shrinks the target"
    );
    assert_eq!(e.effective_toughness(target), Some(3));

    // Declining the optional target places no counter and does not stall.
    let mut decline = engine(724_017);
    let boggart = inject_creature_with_stats(&mut decline, 0, "bile-vial_boggart", 1, 1);
    let bystander = inject_creature_with_stats(&mut decline, 1, "serra_angel", 4, 4);
    kill_with_murder(&mut decline, boggart);
    assert_eq!(decline.state.pending_triggers.len(), 1);
    let no_target_cmd = RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: Vec::new(),
        })),
    };
    decline
        .apply_command(0, &no_target_cmd)
        .expect("decline the optional target");
    resolve_entire_stack_two_player(&mut decline);
    assert_eq!(
        decline.effective_power(bystander),
        Some(4),
        "declining leaves the creature unchanged"
    );
}
