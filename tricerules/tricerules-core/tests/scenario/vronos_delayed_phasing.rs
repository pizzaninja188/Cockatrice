//! Prepared capability fixtures only. Vronos itself remains unregistered.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use std::sync::OnceLock;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::CardRegistry;
use tricerules_core::{state::EventObserverPayload, TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

const SOURCE: &str = "delayed_phase_fixture";
const FIXTURE: &str = r#"(id: "delayed_phase_fixture", name: "Delayed Phase Fixture", face_id: "delayed_phase_fixture", types: ["Planeswalker"], loyalty: 5,
    activated_abilities: [(ability_id: "plus", presentation: Fallback, costs: [Loyalty(1)],
        targeting: Some((groups: [(min: 0, max: 2, prompt: "Choose up to two other planeswalkers you control", effect_indices: [0])])),
        effect: [CreateDelayedTrigger(subject: Some(Chosen((kind: AnyPermanent, controller: You, permanent_types: [Planeswalker], excluded_objects: [Source]))),
            ability: (ability_id: "delayed", presentation: Fallback, trigger: AtBeginningOfNextEndStep, effect: [PhaseOutObservedObjects]))])])"#;

fn game() -> (GameEngine, u32) {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| CardRegistry::from_chunks_and_tokens(&[
        FIXTURE,
        r#"(id: "other_walker", name: "Other Walker", face_id: "other_walker", types: ["Planeswalker"], loyalty: 5)"#,
        r#"(id: "second_walker", name: "Second Walker", face_id: "second_walker", types: ["Planeswalker"], loyalty: 5)"#,
        r#"(id: "walker_aura", name: "Walker Aura", face_id: "walker_aura", types: ["Enchantment", "Aura"], spell_effect: [AuraAttach(target: (kind: AnyPermanent))])"#,
        include_str!("../../../tricerules-cards/data/forest.ron"),
        include_str!("../../../tricerules-cards/data/oath_of_teferi.ron"),
        include_str!("../../../tricerules-cards/data/sol_ring.ron"),
        include_str!("../../../tricerules-cards/data/rings_of_brighthearth.ron"),
    ], &[]).unwrap());
    let mut engine = GameEngine::new(
        registry,
        731_370,
        &[17, 29, 41],
        20,
        Some(vec![vec!["forest".into(); 40]; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = permanent(&mut engine, 0, SOURCE);
    (engine, source)
}

fn permanent(engine: &mut GameEngine, player: usize, id: &str) -> u32 {
    let object = inject_permanent_on_battlefield(engine, player, id);
    engine
        .state
        .zone_change_generation
        .entry(object)
        .or_insert(0);
    if [SOURCE, "other_walker", "second_walker"].contains(&id) {
        engine
            .state
            .objects
            .get_mut(&object)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 5);
    }
    object
}

fn activate(engine: &mut GameEngine, source: u32, targets: Vec<rv1::TargetRef>) {
    let command = activate_ability_for(engine, source, 0, targets);
    engine.apply_scenario_command(17, &command).unwrap();
    pass_priority_round(engine);
}

fn end_step(engine: &mut GameEngine) {
    for _ in 0..10 {
        if engine.state.turn_step == TurnStep::EndStep {
            return;
        }
        let active = engine.state.active_player_id();
        engine
            .apply_scenario_command(active, &primitive_yield())
            .unwrap();
    }
    panic!("fixture did not reach end step");
}

fn move_card(engine: &mut GameEngine, name: &str, zone: rv1::DevZone) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(rv1::DevCommand {
                    target_player_id: 17,
                    dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap();
}

#[test]
fn vronos_zero_targets_schedules_one_empty_next_end_step_occurrence() {
    let (mut engine, source) = game();
    activate(&mut engine, source, vec![]);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    let EventObserverPayload::StageDelayedTrigger(payload) =
        &engine.state.active_event_observers[0].payload
    else {
        panic!("delayed trigger");
    };
    assert_eq!(payload.delayed_phasing_cohort, Some([None, None]));
    assert!(engine.state.active_event_observers[0].watched.is_none());
    end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn vronos_delayed_cohort_ignores_later_type_controller_and_source_changes() {
    let (mut engine, source) = game();
    let first = permanent(&mut engine, 0, "other_walker");
    let second = permanent(&mut engine, 0, "second_walker");
    let generations = [
        engine.state.zone_change_generation[&first],
        engine.state.zone_change_generation[&second],
    ];
    let mut targets = target_object(first);
    targets.extend(target_object(second));
    activate(&mut engine, source, targets);
    // Prepared continuous-control and type changes, retaining both exact incarnations.
    engine.state.players[0]
        .battlefield
        .retain(|id| *id != first);
    engine.state.players[1].battlefield.push(first);
    engine.state.objects.get_mut(&first).unwrap().controller = 29;
    engine
        .state
        .objects
        .get_mut(&first)
        .unwrap()
        .base_controller = 29;
    engine.state.objects.get_mut(&first).unwrap().card_id = "forest".into();
    move_card(
        &mut engine,
        "Delayed Phase Fixture",
        rv1::DevZone::Graveyard,
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    end_step(&mut engine);
    pass_priority_round(&mut engine);
    for (index, id) in [first, second].into_iter().enumerate() {
        assert!(engine.state.objects[&id].phase_status.is_some());
        assert_eq!(engine.state.zone_change_generation[&id], generations[index]);
    }
    assert_eq!(
        engine.state.objects[&first]
            .phase_status
            .as_ref()
            .unwrap()
            .phase_out_under,
        29
    );
    assert_eq!(
        engine.state.objects[&second]
            .phase_status
            .as_ref()
            .unwrap()
            .phase_out_under,
        17
    );
}

#[test]
fn vronos_delayed_cohort_does_not_phase_a_blinked_incarnation() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "other_walker");
    let generation = engine.state.zone_change_generation[&target];
    activate(&mut engine, source, target_object(target));
    move_card(&mut engine, "Other Walker", rv1::DevZone::Graveyard);
    move_card(&mut engine, "Other Walker", rv1::DevZone::Battlefield);
    assert!(engine.state.zone_change_generation[&target] > generation);
    end_step(&mut engine);
    pass_priority_round(&mut engine);
    assert!(engine.state.objects[&target].phase_status.is_none());
}

#[test]
fn vronos_partial_illegal_targets_capture_only_the_remaining_incarnation() {
    let (mut engine, source) = game();
    let first = permanent(&mut engine, 0, "other_walker");
    let second = permanent(&mut engine, 0, "second_walker");
    let mut targets = target_object(first);
    targets.extend(target_object(second));
    let command = activate_ability_for(&engine, source, 0, targets);
    engine.apply_scenario_command(17, &command).unwrap();
    move_card(&mut engine, "Other Walker", rv1::DevZone::Graveyard);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    end_step(&mut engine);
    pass_priority_round(&mut engine);
    assert!(engine.state.objects[&second].phase_status.is_some());
    assert!(engine.state.objects[&first].phase_status.is_none());
}

#[test]
fn vronos_all_chosen_targets_illegal_does_not_create_an_empty_occurrence() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "other_walker");
    let command = activate_ability_for(&engine, source, 0, target_object(target));
    engine.apply_scenario_command(17, &command).unwrap();
    move_card(&mut engine, "Other Walker", rv1::DevZone::Graveyard);
    pass_priority_round(&mut engine);
    assert!(engine.state.active_event_observers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn vronos_invalid_targets_reject_atomically_before_loyalty_payment() {
    let (mut engine, source) = game();
    let first = permanent(&mut engine, 0, "other_walker");
    let second = permanent(&mut engine, 0, "second_walker");
    let third = permanent(&mut engine, 0, "other_walker");
    let opponent = permanent(&mut engine, 1, "other_walker");
    let land = permanent(&mut engine, 0, "forest");
    let mut duplicate = target_object(first);
    duplicate.extend(target_object(first));
    let mut three = target_object(first);
    three.extend(target_object(second));
    three.extend(target_object(third));
    for targets in [
        target_object(source),
        target_object(opponent),
        target_object(land),
        duplicate,
        three,
    ] {
        let command = activate_ability_for(&engine, source, 0, targets);
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_scenario_command(17, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
}

#[test]
fn vronos_two_oath_activations_retain_independent_identical_cohorts() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "other_walker");
    permanent(&mut engine, 0, "oath_of_teferi");
    activate(&mut engine, source, target_object(target));
    activate(&mut engine, source, target_object(target));
    assert_eq!(engine.state.active_event_observers.len(), 2);
    let payloads = engine
        .state
        .active_event_observers
        .iter()
        .map(|observer| {
            let EventObserverPayload::StageDelayedTrigger(payload) = &observer.payload else {
                panic!("delayed trigger");
            };
            payload.delayed_phasing_cohort.unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(payloads[0], payloads[1]);
    end_step(&mut engine);
    assert!(engine.state.active_event_observers.is_empty());
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 2);
    pass_priority_round(&mut engine);
    assert!(engine.state.objects[&target].phase_status.is_some());
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn vronos_source_departure_before_resolution_does_not_cancel_scheduling() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "other_walker");
    let command = activate_ability_for(&engine, source, 0, target_object(target));
    engine.apply_scenario_command(17, &command).unwrap();
    move_card(
        &mut engine,
        "Delayed Phase Fixture",
        rv1::DevZone::Graveyard,
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    end_step(&mut engine);
    pass_priority_round(&mut engine);
    assert!(engine.state.objects[&target].phase_status.is_some());
}

#[test]
fn vronos_rings_copy_creates_its_own_empty_or_selected_cohort_without_another_cost() {
    for select_target in [false, true] {
        let (mut engine, source) = game();
        let target = permanent(&mut engine, 0, "other_walker");
        permanent(&mut engine, 0, "rings_of_brighthearth");
        give_mana(
            &mut engine,
            17,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let targets = if select_target {
            target_object(target)
        } else {
            vec![]
        };
        let command = activate_ability_for(&engine, source, 0, targets);
        engine.apply_scenario_command(17, &command).unwrap();
        pass_priority_round(&mut engine);
        engine
            .apply_scenario_command(
                17,
                &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
            )
            .unwrap();
        submit_mana_resolution_decision(&mut engine, 17, ResolutionChoiceDecision::PayMana)
            .unwrap();
        if select_target {
            engine
                .apply_scenario_command(17, &submit_resolution_choice(vec![]))
                .unwrap();
        }
        assert!(engine.state.stack.last().unwrap().is_copy);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Loyalty),
            6
        );
        pass_priority_round(&mut engine);
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.active_event_observers.len(), 2);
        let cohorts = engine
            .state
            .active_event_observers
            .iter()
            .map(|observer| {
                let EventObserverPayload::StageDelayedTrigger(payload) = &observer.payload else {
                    panic!("delayed trigger");
                };
                payload.delayed_phasing_cohort.unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(cohorts[0], cohorts[1]);
        assert_eq!(
            cohorts[0].iter().flatten().count(),
            usize::from(select_target)
        );
    }
}

#[test]
fn vronos_phasing_preserves_attached_incarnations_and_returns_at_untap() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "other_walker");
    let aura = permanent(&mut engine, 0, "walker_aura");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::state::AttachmentRecipient::Object(target));
    activate(&mut engine, source, target_object(target));
    let generations = [
        engine.state.zone_change_generation[&target],
        engine.state.zone_change_generation[&aura],
    ];
    end_step(&mut engine);
    pass_priority_round(&mut engine);
    for id in [target, aura] {
        assert!(engine.state.objects[&id].phase_status.is_some());
    }
    let parent = engine.state.objects[&aura]
        .phase_status
        .as_ref()
        .unwrap()
        .phase_in_with
        .as_ref()
        .unwrap();
    assert_eq!(parent.object_id, target);
    assert_eq!(parent.zone_change_generation, generations[0]);
    // Advance actual turn boundaries; the phasing controller is 17, not the next player 29.
    for _ in 0..60 {
        if engine.state.active_player_id() == 17 && engine.state.turn_step == TurnStep::Upkeep {
            break;
        }
        resolve_cleanup_discards_if_any(&mut engine);
        if engine.state.active_player_id() == 17 && engine.state.turn_step == TurnStep::Upkeep {
            break;
        }
        let actor = engine.state.active_player_id();
        engine
            .apply_scenario_command(actor, &primitive_yield())
            .unwrap();
        if engine.state.active_player_id() != 17 {
            assert!(engine.state.objects[&target].phase_status.is_some());
        }
    }
    assert_eq!(engine.state.active_player_id(), 17);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    for (index, id) in [target, aura].into_iter().enumerate() {
        assert!(engine.state.objects[&id].phase_status.is_none());
        assert_eq!(engine.state.zone_change_generation[&id], generations[index]);
    }
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::state::AttachmentRecipient::Object(target))
    );
}
