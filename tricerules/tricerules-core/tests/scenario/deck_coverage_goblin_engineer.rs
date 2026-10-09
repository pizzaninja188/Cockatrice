//! Exact Goblin Engineer: optional artifact search and targeted artifact return.
use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

fn game(seed: u64) -> GameEngine {
    let deck = deck_with("mountain", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck; 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_one(engine: &mut GameEngine) -> RuledEventBatch {
    let count = engine.state.players.len() - engine.state.passes_since_stack_change as usize;
    let mut batch = RuledEventBatch::default();
    for _ in 0..count {
        batch = engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    batch
}

fn rejected_unchanged(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = format!("{:?}", engine.state);
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}

fn shuffle_count(batch: &RuledEventBatch) -> usize {
    batch.events.iter().filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")).count()
}

fn cast_engineer(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, "goblin_engineer");
    give_mana(
        engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "goblin_engineer");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    resolve_one(engine);
    source
}

fn return_command(engine: &GameEngine, source: u32, sacrifice: u32, target: u32) -> RuledCommand {
    let targets = vec![TargetRef {
        kind: TargetRefKind::Graveyard as i32,
        object_id: target,
        ..Default::default()
    }];
    let mut command = activate_ability_for(engine, source, 0, targets);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.cost_selections = vec![permanent_cost_selection(2, sacrifice)];
    command
}

#[test]
fn engineer_actual_etb_selects_exact_duplicate_into_graveyard() {
    let mut engine = game(2026100502);
    let source = cast_engineer(&mut engine);
    let first = inject_library_card(&mut engine, 0, "sol_ring");
    let selected = inject_library_card(&mut engine, 0, "sol_ring");
    let high_value = inject_library_card(&mut engine, 0, "pentavus");
    let invalid = inject_library_card(&mut engine, 0, "grizzly_bears");
    let foreign = inject_library_card(&mut engine, 1, "sol_ring");
    let generation = engine
        .state
        .zone_change_generation
        .get(&selected)
        .copied()
        .unwrap_or(0);
    let branch = resolve_one(&mut engine);
    assert_eq!(
        find_resolution_choice(&branch).unwrap().choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    rejected_unchanged(
        &mut engine,
        1,
        &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
    );
    let batch = engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .unwrap();
    let choice = find_resolution_choice(&batch).unwrap();
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(
        choice.candidate_object_ids.contains(&first)
            && choice.candidate_object_ids.contains(&selected)
    );
    assert!(
        choice.candidate_object_ids.contains(&high_value),
        "ETB has no mana-value bound"
    );
    assert!(
        !choice.candidate_object_ids.contains(&invalid)
            && !choice.candidate_object_ids.contains(&foreign)
    );
    for (actor, selection) in [
        (1, vec![selected]),
        (0, vec![invalid]),
        (0, vec![foreign]),
        (0, vec![first, selected]),
    ] {
        rejected_unchanged(&mut engine, actor, &submit_resolution_choice(selection));
    }
    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![selected]))
        .unwrap();
    assert_eq!(shuffle_count(&completion), 1);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&selected].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&selected],
        generation + 1
    );
    assert!(engine.state.players[0].graveyard.contains(&selected));
    assert!(engine.state.players[0].library.contains(&first));
    assert!(completion.events.iter().any(|event| matches!(&event.ev, Some(Ev::PermanentMoved(moved)) if moved.object_id == selected && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32)));
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn engineer_declining_search_differs_from_accepting_and_finding_zero() {
    for accept in [false, true] {
        let mut engine = game(2026100503);
        cast_engineer(&mut engine);
        let hit = inject_library_card(&mut engine, 0, "sol_ring");
        resolve_one(&mut engine);
        let original = engine.state.players[0].library.clone();
        let completion = if accept {
            engine
                .apply_command(
                    0,
                    &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
                )
                .unwrap();
            engine
                .apply_command(0, &submit_resolution_choice(vec![]))
                .unwrap()
        } else {
            engine
                .apply_command(
                    0,
                    &submit_resolution_decision(ResolutionChoiceDecision::Decline),
                )
                .unwrap()
        };
        assert_eq!(shuffle_count(&completion), usize::from(accept));
        if !accept {
            assert_eq!(engine.state.players[0].library, original);
        }
        assert_eq!(engine.state.objects[&hit].zone, Zone::Library);
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn engineer_return_pays_costs_after_valid_target_and_enters_untapped() {
    for target_card in ["chromatic_lantern", "astral_cornucopia"] {
        let mut engine = game(2026100504);
        let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_engineer");
        let sacrifice = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let target = inject_graveyard_card(&mut engine, 0, target_card);
        let nonartifact = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
        let too_large = inject_graveyard_card(&mut engine, 0, "solemn_simulacrum");
        let foreign = inject_graveyard_card(&mut engine, 1, "sol_ring");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        for invalid in [nonartifact, too_large, foreign, sacrifice] {
            let command = return_command(&engine, source, sacrifice, invalid);
            rejected_unchanged(&mut engine, 0, &command);
        }
        let command = return_command(&engine, source, sacrifice, target);
        rejected_unchanged(&mut engine, 1, &command);
        engine.apply_command(0, &command).unwrap();
        assert!(engine.state.objects[&source].tapped);
        assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Graveyard);
        assert_eq!(engine.state.players[0].mana_pool.red, 0);
        assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
        resolve_one(&mut engine);
        assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
        assert!(!engine.state.objects[&target].tapped);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn engineer_unpayable_and_stale_activation_reject_without_mutation() {
    let mut engine = game(2026100505);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_engineer");
    let sacrifice = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let target = inject_graveyard_card(&mut engine, 0, "chromatic_lantern");
    let foreign = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let nonartifact = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let command = return_command(&engine, source, sacrifice, target);
    rejected_unchanged(&mut engine, 0, &command); // No red mana.
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    for invalid_cost in [foreign, nonartifact] {
        let invalid = return_command(&engine, source, invalid_cost, target);
        rejected_unchanged(&mut engine, 0, &invalid);
    }
    engine.state.objects.get_mut(&source).unwrap().tapped = true;
    rejected_unchanged(&mut engine, 0, &command);
    engine.state.objects.get_mut(&source).unwrap().tapped = false;
    let mut stale = command.clone();
    let Some(Cmd::ActivateAbility(activation)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation += 1;
    rejected_unchanged(&mut engine, 0, &stale);
    let mut duplicate_cost = command.clone();
    let Some(Cmd::ActivateAbility(activation)) = duplicate_cost.cmd.as_mut() else {
        unreachable!()
    };
    activation
        .cost_selections
        .push(activation.cost_selections[0].clone());
    rejected_unchanged(&mut engine, 0, &duplicate_cost);
}

#[test]
fn engineer_fresh_cast_cannot_tap_and_stale_return_target_keeps_paid_costs() {
    let mut engine = game(2026100506);
    let source = cast_engineer(&mut engine);
    resolve_one(&mut engine);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .unwrap();
    let sacrifice = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let target = inject_graveyard_card(&mut engine, 0, "chromatic_lantern");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let command = return_command(&engine, source, sacrifice, target);
    rejected_unchanged(&mut engine, 0, &command);

    // The helper installs an established permanent to isolate post-payment target identity.
    let established = inject_permanent_on_battlefield(&mut engine, 0, "goblin_engineer");
    let command = return_command(&engine, established, sacrifice, target);
    engine.apply_command(0, &command).unwrap();
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 2;
    resolve_one(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Graveyard);
    assert!(engine.state.objects[&established].tapped);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn engineer_sacrifices_controlled_foreign_artifact_or_artifact_self() {
    for sacrifice_self in [false, true] {
        let mut engine = game(2026100507);
        let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_engineer");
        let target = inject_graveyard_card(&mut engine, 0, "chromatic_lantern");
        let sacrifice = if sacrifice_self {
            let torque = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_torque");
            apply_ability(&mut engine, 0, torque, 1, target_object(source)).unwrap();
            resolve_one(&mut engine);
            assert!(engine.characteristics(source).unwrap().is_artifact());
            source
        } else {
            let artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
            engine.state.players[1]
                .battlefield
                .retain(|oid| *oid != artifact);
            engine.state.players[0].battlefield.push(artifact);
            let object = engine.state.objects.get_mut(&artifact).unwrap();
            object.controller = 0;
            object.base_controller = 0;
            artifact
        };
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let command = return_command(&engine, source, sacrifice, target);
        engine.apply_command(0, &command).unwrap();
        assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Graveyard);
        let owner = usize::from(!sacrifice_self);
        assert!(engine.state.players[owner].graveyard.contains(&sacrifice));
        assert_eq!(engine.state.stack[0].controller, 0);
        resolve_one(&mut engine);
        assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
        assert!(!engine.state.objects[&target].tapped);
        assert_eq!(engine.state.objects[&target].controller, 0);
    }
}

#[test]
fn engineer_actual_paid_cast_has_exact_characteristics_and_optional_etb() {
    assert!(tricerules_cards::registry::global()
        .get("goblin_engineer")
        .is_some());
    let mut engine = game(2026100501);
    let source = inject_card_into_hand(&mut engine, 0, "goblin_engineer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "goblin_engineer");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    let characteristics = engine.characteristics(source).unwrap();
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(1), Some(2))
    );
    assert!(characteristics.is_creature() && !characteristics.is_artifact());
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "actual ETB trigger is on the stack"
    );
}
