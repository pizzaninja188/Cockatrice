//! Actual unchanged Vronos definition in prepared scenarios; normal reachability is separate.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};

const ID: &str = "vronos,_masked_inquisitor";

fn game() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        731_373,
        &[17, 29, 41],
        20,
        Some(vec![vec!["island".into(); 40]; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn permanent(engine: &mut GameEngine, player: usize, id: &str) -> u32 {
    let object = inject_permanent_on_battlefield(engine, player, id);
    engine
        .state
        .zone_change_generation
        .entry(object)
        .or_insert(0);
    if let Some(loyalty) = tricerules_cards::registry::global()
        .get(id)
        .unwrap()
        .primary_face()
        .loyalty
    {
        engine
            .state
            .objects
            .get_mut(&object)
            .unwrap()
            .set_counter(CounterKind::Loyalty, loyalty);
    }
    object
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

fn activate(engine: &mut GameEngine, source: u32, index: u32, targets: Vec<TargetRef>) {
    let command = activate_ability_for(engine, source, index, targets);
    engine.apply_scenario_command(17, &command).unwrap();
}

#[test]
fn actual_vronos_cast_pays_three_generic_two_blue_and_enters_with_five_loyalty() {
    let mut engine = game();
    inject_card_into_hand(&mut engine, 0, ID);
    give_mana(
        &mut engine,
        17,
        ManaGift {
            c: 3,
            u: 2,
            ..Default::default()
        },
    );
    let command = cast_spell(hand_index_for_card(&engine, 0, ID), vec![]);
    engine.apply_scenario_command(17, &command).unwrap();
    pass_priority_round(&mut engine);
    let source = *engine.state.players[0]
        .battlefield
        .iter()
        .find(|id| engine.state.objects[id].card_id == ID)
        .unwrap();
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let view = engine.initial_response_batch();
    assert_eq!(
        view.legal_by_player[&17].valid_targets_by_ability[&((u64::from(source) << 32) | 1)]
            .groups
            .len(),
        2
    );
}

#[test]
fn actual_vronos_plus_zero_one_two_retains_exact_planeswalkers_until_next_end_step_and_untap() {
    for count in 0..=2 {
        let mut engine = game();
        let source = permanent(&mut engine, 0, ID);
        let jace = permanent(&mut engine, 0, "jace_beleren");
        let ajani = permanent(&mut engine, 0, "ajani_steadfast");
        let objects = [jace, ajani];
        let generations = objects.map(|id| engine.state.zone_change_generation[&id]);
        let targets = objects
            .iter()
            .take(count)
            .map(|id| target(&engine, *id, 0))
            .collect();
        activate(&mut engine, source, 0, targets);
        pass_priority_round(&mut engine);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Loyalty),
            6
        );
        assert_eq!(engine.state.active_event_observers.len(), 1);
        assert!(objects
            .iter()
            .all(|id| engine.state.objects[id].phase_status.is_none()));
        for _ in 0..10 {
            if engine.state.turn_step == TurnStep::EndStep {
                break;
            }
            engine
                .apply_scenario_command(17, &primitive_yield())
                .unwrap();
        }
        assert_eq!(engine.state.turn_step, TurnStep::EndStep);
        assert!(!engine.state.stack.is_empty());
        pass_priority_round(&mut engine);
        engine
            .apply_scenario_command(17, &primitive_yield())
            .unwrap();
        resolve_cleanup_discards_if_any(&mut engine);
        assert_eq!(engine.state.active_player_id(), 29);
        for (index, id) in objects.iter().enumerate() {
            assert_eq!(
                engine.state.objects[id].phase_status.is_some(),
                index < count
            );
            assert_eq!(engine.state.zone_change_generation[id], generations[index]);
        }
        advance_to_main1_from_game_start(&mut engine);
        end_active_turn(&mut engine, 29);
        advance_to_main1_from_game_start(&mut engine);
        end_active_turn(&mut engine, 41);
        assert_eq!(engine.state.active_player_id(), 17);
        assert!(objects
            .iter()
            .all(|id| engine.state.objects[id].phase_status.is_none()));
    }
}

#[test]
fn actual_vronos_plus_rejects_self_opponents_nonwalkers_duplicates_and_three() {
    let mut engine = game();
    let source = permanent(&mut engine, 0, ID);
    let jace = permanent(&mut engine, 0, "jace_beleren");
    let ajani = permanent(&mut engine, 0, "ajani_steadfast");
    let teferi = permanent(&mut engine, 0, "teferi,_master_of_time");
    let opponent = permanent(&mut engine, 1, "jace_beleren");
    let relic = permanent(&mut engine, 0, "ornithopter");
    for ids in [
        vec![source],
        vec![opponent],
        vec![relic],
        vec![jace, jace],
        vec![jace, ajani, teferi],
    ] {
        let targets = ids.into_iter().map(|id| target(&engine, id, 0)).collect();
        let command = activate_ability_for(&engine, source, 0, targets);
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_scenario_command(17, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
}

#[test]
fn actual_vronos_minus_two_selects_each_opponent_once_and_returns_to_owners() {
    for count in 0..=2 {
        let mut engine = game();
        let source = permanent(&mut engine, 0, ID);
        let b = permanent(&mut engine, 1, "sol_ring");
        let c = permanent(&mut engine, 2, "grizzly_bears");
        let targets = [b, c]
            .into_iter()
            .take(count)
            .enumerate()
            .map(|(index, id)| target(&engine, id, index as u32))
            .collect();
        activate(&mut engine, source, 1, targets);
        pass_priority_round(&mut engine);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Loyalty),
            3
        );
        for (index, id) in [b, c].into_iter().enumerate() {
            assert_eq!(
                engine.state.objects[&id].zone,
                if index < count {
                    Zone::Hand
                } else {
                    Zone::Battlefield
                }
            );
            if index < count {
                assert!(engine.state.players[index + 1].hand.contains(&id));
            }
        }
    }
}

#[test]
fn actual_vronos_ultimate_pays_source_away_and_persistently_animates_one_shared_artifact() {
    let mut engine = game();
    let source = permanent(&mut engine, 0, ID);
    let artifact = permanent(&mut engine, 0, "ornithopter");
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 7);
    engine
        .state
        .objects
        .get_mut(&artifact)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    let targets = vec![target(&engine, artifact, 0)];
    activate(&mut engine, source, 2, targets);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    pass_priority_round(&mut engine);
    let current = engine.characteristics(artifact).unwrap();
    assert!(current.is_artifact() && current.is_creature() && current.has_type("Construct"));
    assert!(!current.has_type("Thopter"));
    assert_eq!((current.power, current.toughness), (Some(11), Some(11)));
    for keyword in [Keyword::Flying, Keyword::Vigilance, Keyword::Indestructible] {
        assert!(current.has_keyword(keyword));
    }
    assert!(!current.has_keyword(Keyword::Haste));
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, artifact)
        .contains(&"Can't be blocked".into()));
    end_active_turn(&mut engine, 17);
    assert_ne!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(engine.characteristics(artifact).unwrap().power, Some(11));
}
