//! Issue #375 focused scenarios for the four retained graveyard-return identities.
//!
//! These drive the generated definitions through the authoritative command path. CR 404.2 keeps
//! descend/threshold counts on public printed graveyard data; CR 603.4 re-checks an intervening-if
//! at trigger staging and again on resolution; CR 115.1 keeps the printed target contracts,
//! including Council of Echoes' source exclusion and the up-to-one bounds; CR 400.7 gives a card
//! moved between zones new-object identity; and CR 508.4 / 702.181 keep mobilize tokens tapped
//! and attacking while a dynamic graveyard count sets their number.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::TargetRefKind;

fn engine_with(seed: u64, own: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", own), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_graveyard_trigger_target(engine: &mut GameEngine, object_id: u32) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![TargetRef {
                        object_id,
                        damage_amount: 0,
                        group_index: 0,
                        kind: TargetRefKind::Graveyard as i32,
                    }],
                })),
            },
        )
        .expect("choose the published graveyard target");
}

fn choose_permanent_trigger_target(engine: &mut GameEngine, object_id: u32) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(object_id),
                })),
            },
        )
        .expect("choose the published permanent target");
}

fn choose_no_trigger_targets(engine: &mut GameEngine) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![],
                })),
            },
        )
        .expect("choose no targets for the up-to-one trigger");
}

fn pending_graveyard_targets(engine: &GameEngine, batch: &RuledEventBatch) -> Vec<u32> {
    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("a graveyard target is pending");
    let key = (pending.source_permanent_id as u64) << 32 | pending.ability_index as u64;
    batch
        .legal_by_player
        .get(&0)
        .expect("controller legal actions")
        .valid_targets_by_ability
        .get(&key)
        .expect("published graveyard target group")
        .groups[0]
        .valid_graveyard_ids
        .clone()
}

fn move_graveyard_object_to_exile(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|id| *id != object_id);
    engine.state.players[player].exile.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("graveyard object")
        .zone = Zone::Exile;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

#[test]
fn issue_375_coati_descend_4_needs_four_permanent_cards_and_targets_only_permanents() {
    // Three permanent cards plus an instant card is below the printed gate, so the ETB trigger
    // never gets created.
    let mut below = engine_with(375_001, &["coati_scavenger"]);
    for _ in 0..3 {
        inject_graveyard_card(&mut below, 0, "grizzly_bears");
    }
    inject_graveyard_card(&mut below, 0, "lightning_bolt");
    move_ready_to_battlefield(&mut below, 0, "coati_scavenger");
    assert!(
        below.state.pending_triggers.is_empty(),
        "three permanent cards is below the descend-4 gate"
    );

    // The fourth permanent card enables the trigger; the instant card is not a legal target.
    let mut engine = engine_with(375_002, &["coati_scavenger"]);
    let bears: Vec<u32> = (0..4)
        .map(|_| inject_graveyard_card(&mut engine, 0, "grizzly_bears"))
        .collect();
    let bolt = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    let _coati = move_ready_to_battlefield(&mut engine, 0, "coati_scavenger");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let batch = engine.initial_response_batch();
    let candidates = pending_graveyard_targets(&engine, &batch);
    assert!(
        !candidates.contains(&bolt),
        "an instant card is not a permanent card"
    );
    for bear in &bears {
        assert!(
            candidates.contains(bear),
            "creature cards are permanent cards"
        );
    }

    // The illegal choice is rejected without consuming the pending trigger.
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![TargetRef {
                        object_id: bolt,
                        damage_amount: 0,
                        group_index: 0,
                        kind: TargetRefKind::Graveyard as i32,
                    }],
                })),
            },
        )
        .expect_err("an instant card is an illegal target");
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "the rejected choice must leave the trigger pending"
    );

    choose_graveyard_trigger_target(&mut engine, bears[0]);
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.state.players[0].hand.contains(&bears[0]),
        "the chosen permanent card returns to hand"
    );
    assert!(!engine.state.players[0].graveyard.contains(&bears[0]));
}

#[test]
fn issue_375_coati_intervening_if_rechecks_graveyard_state_on_resolution() {
    let mut engine = engine_with(375_003, &["coati_scavenger"]);
    let bears: Vec<u32> = (0..4)
        .map(|_| inject_graveyard_card(&mut engine, 0, "grizzly_bears"))
        .collect();
    move_ready_to_battlefield(&mut engine, 0, "coati_scavenger");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    choose_graveyard_trigger_target(&mut engine, bears[0]);
    assert_eq!(engine.state.stack.len(), 1, "the trigger is on the stack");

    // The gate is re-evaluated on resolution; emptying one permanent card below the bound fizzles
    // the return even though the target was legal when announced.
    move_graveyard_object_to_exile(&mut engine, 0, bears[1]);
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.state.players[0].graveyard.contains(&bears[0]),
        "the chosen card stays in the graveyard after the intervene re-check fails"
    );
    assert!(!engine.state.players[0].hand.contains(&bears[0]));
}

#[test]
fn issue_375_council_descend_4_bounce_can_choose_no_target_and_excludes_its_source() {
    // The up-to-one target can be declined by choosing no targets, leaving every permanent in
    // place.
    let mut decline = engine_with(375_010, &["council_of_echoes"]);
    for _ in 0..4 {
        inject_graveyard_card(&mut decline, 0, "grizzly_bears");
    }
    inject_permanent_on_battlefield(&mut decline, 0, "forest");
    let bear = inject_creature_on_battlefield(&mut decline, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut decline, 0, "council_of_echoes");
    assert_eq!(decline.state.pending_triggers.len(), 1);
    choose_no_trigger_targets(&mut decline);
    resolve_entire_stack_two_player(&mut decline);
    assert!(decline.state.players[0].battlefield.contains(&bear));

    // The source is excluded, lands are not legal, and a legal nonland permanent bounces.
    let mut engine = engine_with(375_011, &["council_of_echoes"]);
    for _ in 0..4 {
        inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    }
    inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let council = move_ready_to_battlefield(&mut engine, 0, "council_of_echoes");
    assert_eq!(engine.state.pending_triggers.len(), 1);

    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(council),
                })),
            },
        )
        .expect_err("the printed clause excludes this creature");
    assert_eq!(engine.state.pending_triggers.len(), 1);

    choose_permanent_trigger_target(&mut engine, bear);
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.state.players[0].hand.contains(&bear),
        "the chosen nonland permanent returns to hand"
    );
}

#[test]
fn issue_375_tidecaller_threshold_7_is_inclusive_and_allows_the_source() {
    // Six cards is one below the printed threshold, so the ETB trigger never gets created.
    let mut below = engine_with(375_020, &["tidecaller_mentor"]);
    for _ in 0..6 {
        inject_graveyard_card(&mut below, 0, "forest");
    }
    move_ready_to_battlefield(&mut below, 0, "tidecaller_mentor");
    assert!(
        below.state.pending_triggers.is_empty(),
        "six cards is below the threshold-7 gate"
    );

    // Seven cards enables the trigger; the printed clause has no source exclusion, so the Mentor
    // itself is a legal nonland permanent target and can be returned to hand.
    let mut engine = engine_with(375_021, &["tidecaller_mentor"]);
    for _ in 0..7 {
        inject_graveyard_card(&mut engine, 0, "forest");
    }
    let mentor = move_ready_to_battlefield(&mut engine, 0, "tidecaller_mentor");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    choose_permanent_trigger_target(&mut engine, mentor);
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.players[0].hand.contains(&mentor));
}

#[test]
fn issue_375_avenger_mobilize_counts_only_creature_cards_and_enters_attacking() {
    // Two creature cards in the controller's graveyard scale the mobilize count; the land card
    // and the opponent's creature card are ignored.
    let decks = Some(vec![
        deck_with("forest", &["avenger_of_the_fallen"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(375_030, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut engine);
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    let avenger = relocate_to_battlefield(&mut engine, 0, "avenger_of_the_fallen", false);
    let assignment = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|assignment| {
            assignment.attacker_object_id == avenger
                && assignment
                    .defender
                    .as_ref()
                    .is_some_and(|defender| defender.kind == TargetRefKind::Player as i32)
        })
        .cloned()
        .expect("Avenger may attack the opponent");
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .expect("declare Avenger");
    resolve_entire_stack_two_player(&mut engine);

    let tokens = battlefield_token_oids(&engine, 0, "warrior_r_1_1");
    assert_eq!(
        tokens.len(),
        2,
        "one Warrior per creature card; the land and the opposing card do not count"
    );
    for token in &tokens {
        assert!(engine.state.objects[token].tapped, "mobilize enters tapped");
        assert!(
            engine
                .state
                .combat
                .as_ref()
                .expect("combat state")
                .attacking
                .contains(token),
            "mobilize enters attacking"
        );
    }

    // An empty creature-card graveyard produces no token cohort at all.
    let decks = Some(vec![
        deck_with("forest", &["avenger_of_the_fallen"]),
        deck_with("forest", &[]),
    ]);
    let mut empty = GameEngine::new(375_031, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut empty);
    let avenger = relocate_to_battlefield(&mut empty, 0, "avenger_of_the_fallen", false);
    let assignment = empty.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|assignment| {
            assignment.attacker_object_id == avenger
                && assignment
                    .defender
                    .as_ref()
                    .is_some_and(|defender| defender.kind == TargetRefKind::Player as i32)
        })
        .cloned()
        .expect("Avenger may attack the opponent");
    empty
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .expect("declare Avenger");
    resolve_entire_stack_two_player(&mut empty);
    assert!(battlefield_token_oids(&empty, 0, "warrior_r_1_1").is_empty());
}
