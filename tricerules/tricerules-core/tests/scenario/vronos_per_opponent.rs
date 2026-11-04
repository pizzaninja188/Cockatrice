//! Prepared target-contract fixtures; whole-card admission is separate.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use std::sync::OnceLock;
use tricerules_cards::{primitives::CounterKind, CardRegistry};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

const SOURCE: &str = "opponent_return_fixture";
const FIXTURE: &str = r#"(id: "opponent_return_fixture", name: "Opponent Return Fixture", face_id: "opponent_return_fixture", types: ["Planeswalker"], loyalty: 5,
    activated_abilities: [(ability_id: "minus", presentation: Fallback, costs: [Loyalty(-2)],
        targeting: Some((groups: [(min: 0, max: 1, prompt: "Choose up to one nonland permanent", effect_indices: [0], for_each_opponent: true)])),
        effect: [ReturnToOwnersHand(subject: Chosen((kind: AnyPermanent, controller: Opponent, excluded_permanent_types: [Land])))])])"#;

fn game(players: &[i32]) -> (GameEngine, u32) {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(&[
        FIXTURE,
        r#"(id: "plain_relic", name: "Plain Relic", face_id: "plain_relic", types: ["Artifact"])"#,
        include_str!("../../../tricerules-cards/data/forest.ron"),
        include_str!("../../../tricerules-cards/data/rings_of_brighthearth.ron"),
    ], &[]).unwrap()
    });
    let mut engine = GameEngine::new(
        registry,
        731_372,
        players,
        20,
        Some(vec![vec!["forest".into(); 40]; players.len()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = permanent(&mut engine, 0, SOURCE);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    (engine, source)
}

fn permanent(engine: &mut GameEngine, player: usize, id: &str) -> u32 {
    let object = inject_permanent_on_battlefield(engine, player, id);
    engine
        .state
        .zone_change_generation
        .entry(object)
        .or_insert(0);
    object
}

fn groups(engine: &mut GameEngine, source: u32) -> Vec<rv1::LegalTargetGroup> {
    engine.initial_response_batch().legal_by_player[&17].valid_targets_by_ability
        [&(u64::from(source) << 32)]
        .groups
        .clone()
}

fn target(engine: &GameEngine, object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        kind: TargetRefKind::Permanent as i32,
        expected_zone_change_generation: Some(engine.state.zone_change_generation[&object_id]),
        ..Default::default()
    }
}

fn announce(engine: &mut GameEngine, source: u32, targets: Vec<TargetRef>) {
    let command = activate_ability_for(engine, source, 0, targets);
    engine.apply_scenario_command(17, &command).unwrap();
}

fn change_controller(engine: &mut GameEngine, object_id: u32, player: usize) {
    for state in &mut engine.state.players {
        state.battlefield.retain(|id| *id != object_id);
    }
    let controller = engine.state.players[player].id;
    engine.state.players[player].battlefield.push(object_id);
    let object = engine.state.objects.get_mut(&object_id).unwrap();
    object.controller = controller;
    object.base_controller = controller;
}

#[test]
fn vronos_per_opponent_publishes_one_optional_disjoint_group_per_opponent() {
    let (mut engine, source) = game(&[17, 29, 41]);
    let own = permanent(&mut engine, 0, "plain_relic");
    let b = permanent(&mut engine, 1, "plain_relic");
    let c = permanent(&mut engine, 2, "plain_relic");
    let land = permanent(&mut engine, 1, "forest");
    let offered = groups(&mut engine, source);
    assert_eq!(
        offered.len(),
        2,
        "one optional target slot for EACH opponent"
    );
    for (index, target) in [b, c].into_iter().enumerate() {
        assert_eq!(
            (
                offered[index].group_index,
                offered[index].min,
                offered[index].max
            ),
            (index as u32, 0, 1)
        );
        assert_eq!(offered[index].valid_permanent_ids, [target]);
        assert!(!offered[index].valid_permanent_ids.contains(&own));
        assert!(!offered[index].valid_permanent_ids.contains(&land));
    }
}

#[test]
fn vronos_per_opponent_zero_subset_all_and_source_death_return_to_each_owner() {
    for seats in [vec![17, 29], vec![17, 29, 41], vec![17, 29, 41, 53]] {
        for chosen in 0..seats.len() {
            let (mut engine, source) = game(&seats);
            let objects = (1..seats.len())
                .map(|index| permanent(&mut engine, index, "plain_relic"))
                .collect::<Vec<_>>();
            let targets = objects
                .iter()
                .take(chosen)
                .enumerate()
                .map(|(index, id)| target(&engine, *id, index as u32))
                .collect();
            engine
                .state
                .objects
                .get_mut(&source)
                .unwrap()
                .set_counter(CounterKind::Loyalty, 2);
            announce(&mut engine, source, targets);
            assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
            pass_priority_round(&mut engine);
            assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
            for (index, id) in objects.iter().enumerate() {
                let expected = if index < chosen {
                    Zone::Hand
                } else {
                    Zone::Battlefield
                };
                assert_eq!(engine.state.objects[id].zone, expected);
                if index < chosen {
                    assert!(engine.state.players[index + 1].hand.contains(id));
                }
            }
        }
    }
}

#[test]
fn vronos_per_opponent_forged_announcements_preserve_loyalty_and_state() {
    let (mut engine, source) = game(&[17, 29, 41]);
    let own = permanent(&mut engine, 0, "plain_relic");
    let b1 = permanent(&mut engine, 1, "plain_relic");
    let b2 = permanent(&mut engine, 1, "plain_relic");
    let c = permanent(&mut engine, 2, "plain_relic");
    let land = permanent(&mut engine, 1, "forest");
    let mut stale = target(&engine, b1, 0);
    stale.expected_zone_change_generation = Some(99);
    let mut wrong_kind = target(&engine, b1, 0);
    wrong_kind.kind = TargetRefKind::Player as i32;
    let mut untyped = target(&engine, b1, 0);
    untyped.kind = TargetRefKind::Unspecified as i32;
    let mut missing_generation = target(&engine, b1, 0);
    missing_generation.expected_zone_change_generation = None;
    for targets in [
        vec![target(&engine, own, 0)],
        vec![target(&engine, land, 0)],
        vec![target(&engine, b1, 1)],
        vec![target(&engine, c, 0)],
        vec![target(&engine, c, 2)],
        vec![target(&engine, b1, 0), target(&engine, b2, 0)],
        vec![target(&engine, b1, 0), target(&engine, b2, 1)],
        vec![stale],
        vec![wrong_kind],
        vec![untyped],
        vec![missing_generation],
    ] {
        let command = activate_ability_for(&engine, source, 0, targets);
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_scenario_command(17, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    let command = activate_ability_for(&engine, source, 0, vec![target(&engine, b1, 0)]);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_scenario_command(29, &command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5
    );
}

#[test]
fn vronos_per_opponent_controller_and_generation_changes_leave_other_targets_resolving() {
    for invalidation in 0..3 {
        let (mut engine, source) = game(&[17, 29, 41]);
        let b = permanent(&mut engine, 1, "plain_relic");
        let c = permanent(&mut engine, 2, "plain_relic");
        let targets = vec![target(&engine, b, 0), target(&engine, c, 1)];
        announce(&mut engine, source, targets);
        assert_eq!(
            engine.state.stack[0].targets[0].required_controller,
            Some(29)
        );
        match invalidation {
            0 => change_controller(&mut engine, b, 2),
            1 => {
                *engine.state.zone_change_generation.get_mut(&b).unwrap() += 1;
            }
            _ => {
                engine.state.objects.get_mut(&b).unwrap().card_id = "forest".into();
            }
        }
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.objects[&b].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&c].zone, Zone::Hand);
        assert!(engine.state.players[2].hand.contains(&c));
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Loyalty),
            3
        );
    }
}

#[test]
fn vronos_per_opponent_uses_current_controller_for_targeting_and_original_owner_for_return() {
    let (mut engine, source) = game(&[17, 29, 41]);
    let object = permanent(&mut engine, 1, "plain_relic");
    change_controller(&mut engine, object, 2);
    let offered = groups(&mut engine, source);
    assert!(!offered[0].valid_permanent_ids.contains(&object));
    assert!(offered[1].valid_permanent_ids.contains(&object));
    let targets = vec![target(&engine, object, 1)];
    announce(&mut engine, source, targets);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&object].zone, Zone::Hand);
    assert!(engine.state.players[1].hand.contains(&object));
    assert!(!engine.state.players[2].hand.contains(&object));
}

#[test]
fn vronos_per_opponent_departure_keeps_later_group_indices_and_bindings() {
    let (mut engine, source) = game(&[17, 29, 41, 53]);
    let c = permanent(&mut engine, 2, "plain_relic");
    let d = permanent(&mut engine, 3, "plain_relic");
    let targets = vec![target(&engine, c, 1), target(&engine, d, 2)];
    announce(&mut engine, source, targets);
    engine.apply_scenario_command(29, &concede()).unwrap();
    assert!(engine.state.players[1].has_lost);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&c].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&d].zone, Zone::Hand);
}

#[test]
fn vronos_per_opponent_collects_two_commander_choices_before_one_cohort_commit() {
    let (mut engine, source) = game(&[17, 29, 41]);
    let b = permanent(&mut engine, 1, "plain_relic");
    let c = permanent(&mut engine, 2, "plain_relic");
    engine.state.players[1]
        .declared_commander_object_ids
        .push(b);
    engine.state.players[2]
        .declared_commander_object_ids
        .push(c);
    let targets = vec![target(&engine, b, 0), target(&engine, c, 1)];
    announce(&mut engine, source, targets);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        29
    );
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_scenario_command(
            17,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch)
        )
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine
        .apply_scenario_command(
            29,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        41
    );
    for id in [b, c] {
        assert_eq!(engine.state.objects[&id].zone, Zone::Battlefield);
    }
    let mut hand = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    if let Some(Cmd::SubmitResolutionChoice(choice)) = hand.cmd.as_mut() {
        choice.selected_branch_index = 1;
    }
    engine.apply_scenario_command(41, &hand).unwrap();
    assert_eq!(engine.state.objects[&b].zone, Zone::Command);
    assert_eq!(engine.state.objects[&c].zone, Zone::Hand);
    assert!(engine.state.players[1].command_zone.contains(&b));
    assert!(engine.state.players[2].hand.contains(&c));
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
}

fn pay_for_rings(engine: &mut GameEngine) {
    pass_priority_round(engine);
    engine
        .apply_scenario_command(
            17,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .unwrap();
    submit_mana_resolution_decision(engine, 17, ResolutionChoiceDecision::PayMana).unwrap();
}

#[test]
fn vronos_per_opponent_rings_preserves_skipped_slots_and_rejects_cross_role_changes() {
    let (mut engine, source) = game(&[17, 29, 41]);
    let b = permanent(&mut engine, 1, "plain_relic");
    let c = permanent(&mut engine, 2, "plain_relic");
    let fresh_c = permanent(&mut engine, 2, "plain_relic");
    permanent(&mut engine, 0, "rings_of_brighthearth");
    give_mana(
        &mut engine,
        17,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let targets = vec![target(&engine, c, 1)];
    announce(&mut engine, source, targets);
    pay_for_rings(&mut engine);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![b]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![fresh_c]))
        .unwrap();
    let copy = engine.state.stack.last().unwrap();
    assert!(copy.is_copy);
    assert_eq!(copy.targets.len(), 1);
    assert_eq!(
        (
            copy.targets[0].group_index,
            copy.targets[0].required_controller
        ),
        (1, Some(41))
    );
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    for id in [c, fresh_c] {
        assert_eq!(engine.state.objects[&id].zone, Zone::Hand);
    }
    assert_eq!(engine.state.objects[&b].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        3
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn vronos_per_opponent_rings_can_keep_illegal_b_but_c_cannot_retarget_to_b() {
    let (mut engine, source) = game(&[17, 29, 41, 53]);
    let b = permanent(&mut engine, 1, "plain_relic");
    let fresh_b = permanent(&mut engine, 1, "plain_relic");
    let c = permanent(&mut engine, 2, "plain_relic");
    let fresh_c = permanent(&mut engine, 2, "plain_relic");
    permanent(&mut engine, 0, "rings_of_brighthearth");
    give_mana(
        &mut engine,
        17,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let targets = vec![target(&engine, b, 0), target(&engine, c, 1)];
    announce(&mut engine, source, targets);
    change_controller(&mut engine, b, 3);
    pay_for_rings(&mut engine);
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![]))
        .unwrap();
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![fresh_b]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![fresh_c]))
        .unwrap();
    let copy = engine.state.stack.last().unwrap();
    assert!(copy.is_copy && copy.targets.len() == 2);
    assert_eq!(copy.targets[0].required_controller, Some(29));
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    for id in [b, fresh_b] {
        assert_eq!(engine.state.objects[&id].zone, Zone::Battlefield);
    }
    for id in [c, fresh_c] {
        assert_eq!(engine.state.objects[&id].zone, Zone::Hand);
    }
}

#[test]
fn vronos_per_opponent_all_illegal_fizzles_but_zero_targets_resolves() {
    for choose_target in [false, true] {
        let (mut engine, source) = game(&[17, 29, 41]);
        let b = permanent(&mut engine, 1, "plain_relic");
        let targets = if choose_target {
            vec![target(&engine, b, 0)]
        } else {
            vec![]
        };
        announce(&mut engine, source, targets);
        change_controller(&mut engine, b, 2);
        let mut fizzled = false;
        for _ in 0..3 {
            let actor = engine.state.priority_player_id();
            let batch = engine.apply_scenario_command(actor, &pass()).unwrap();
            fizzled |= batch.events.iter().any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("fizzles (no legal targets)")));
        }
        assert_eq!(fizzled, choose_target);
        assert_eq!(engine.state.objects[&b].zone, Zone::Battlefield);
        assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
    }
}

#[test]
fn vronos_per_opponent_phase_out_is_not_an_offered_or_legal_target() {
    let (mut engine, source) = game(&[17, 29, 41]);
    let b = permanent(&mut engine, 1, "plain_relic");
    engine.state.objects.get_mut(&b).unwrap().phase_status =
        Some(tricerules_core::state::PhasedOutStatus {
            phase_out_under: 29,
            phase_in_due_after_departed_turn: false,
            phase_in_with: None,
            direct_attachment_snapshot: None,
        });
    assert!(!groups(&mut engine, source)[0]
        .valid_permanent_ids
        .contains(&b));
    let command = activate_ability_for(&engine, source, 0, vec![target(&engine, b, 0)]);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_scenario_command(17, &command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn vronos_per_opponent_lost_player_slot_is_empty_and_rings_zero_stays_zero() {
    let (mut engine, source) = game(&[17, 29, 41]);
    let c = permanent(&mut engine, 2, "plain_relic");
    engine.apply_scenario_command(29, &concede()).unwrap();
    let offered = groups(&mut engine, source);
    assert_eq!(offered.len(), 2);
    assert!(offered[0].valid_permanent_ids.is_empty());
    assert_eq!(offered[1].group_index, 1);
    assert_eq!(offered[1].valid_permanent_ids, [c]);
    permanent(&mut engine, 0, "rings_of_brighthearth");
    give_mana(
        &mut engine,
        17,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    announce(&mut engine, source, vec![]);
    pay_for_rings(&mut engine);
    let copy = engine.state.stack.last().unwrap();
    assert!(copy.is_copy && copy.targets.is_empty());
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&c].zone, Zone::Battlefield);
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
}

#[test]
fn vronos_per_opponent_rings_replaces_b_with_b_and_c_with_c() {
    let (mut engine, source) = game(&[17, 29, 41]);
    let b = permanent(&mut engine, 1, "plain_relic");
    let fresh_b = permanent(&mut engine, 1, "plain_relic");
    let c = permanent(&mut engine, 2, "plain_relic");
    let fresh_c = permanent(&mut engine, 2, "plain_relic");
    permanent(&mut engine, 0, "rings_of_brighthearth");
    give_mana(
        &mut engine,
        17,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let targets = vec![target(&engine, b, 0), target(&engine, c, 1)];
    announce(&mut engine, source, targets);
    pay_for_rings(&mut engine);
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![fresh_b]))
        .unwrap();
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![fresh_c]))
        .unwrap();
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    for id in [b, fresh_b, c, fresh_c] {
        assert_eq!(engine.state.objects[&id].zone, Zone::Hand);
    }
}

#[test]
fn vronos_per_opponent_copy_cannot_duplicate_a_kept_illegal_target_in_another_slot() {
    let (mut engine, source) = game(&[17, 29, 41]);
    let b = permanent(&mut engine, 1, "plain_relic");
    let fresh_b = permanent(&mut engine, 1, "plain_relic");
    let c = permanent(&mut engine, 2, "plain_relic");
    let fresh_c = permanent(&mut engine, 2, "plain_relic");
    permanent(&mut engine, 0, "rings_of_brighthearth");
    give_mana(
        &mut engine,
        17,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let targets = vec![target(&engine, b, 0), target(&engine, c, 1)];
    announce(&mut engine, source, targets);
    change_controller(&mut engine, b, 2);
    pay_for_rings(&mut engine);
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![]))
        .unwrap();
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![b]))
        .unwrap();
    assert!(
        engine.state.pending_resolution.is_some(),
        "CR115.3 final duplicate must retry choosing targets"
    );
    assert!(!engine.state.stack.iter().any(|item| item.is_copy));
    for id in [b, fresh_b, c, fresh_c] {
        assert_eq!(engine.state.objects[&id].zone, Zone::Battlefield);
    }
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        3
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![fresh_b]))
        .unwrap();
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![fresh_c]))
        .unwrap();
    assert!(engine.state.stack.last().unwrap().is_copy);
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&fresh_b].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&fresh_c].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&c].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&b].zone, Zone::Battlefield);
}

#[test]
fn vronos_per_opponent_copy_distinguishes_old_and_new_incarnations_of_one_physical_card() {
    let (mut engine, source) = game(&[17, 29, 41]);
    let b = permanent(&mut engine, 1, "plain_relic");
    let c = permanent(&mut engine, 2, "plain_relic");
    permanent(&mut engine, 0, "rings_of_brighthearth");
    give_mana(
        &mut engine,
        17,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let targets = vec![target(&engine, b, 0), target(&engine, c, 1)];
    announce(&mut engine, source, targets);
    // Prepared new battlefield incarnation under C: same physical ID, different rules object.
    *engine.state.zone_change_generation.get_mut(&b).unwrap() += 1;
    change_controller(&mut engine, b, 2);
    pay_for_rings(&mut engine);
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![]))
        .unwrap();
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![b]))
        .unwrap();
    let copy = engine.state.stack.last().unwrap();
    assert!(copy.is_copy);
    assert_eq!(copy.targets[0].object_id, copy.targets[1].object_id);
    assert_ne!(
        copy.targets[0].zone_change_generation,
        copy.targets[1].zone_change_generation
    );
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&b].zone, Zone::Hand);
    assert!(engine.state.players[1].hand.contains(&b));
    assert_eq!(engine.state.objects[&c].zone, Zone::Hand);
}
