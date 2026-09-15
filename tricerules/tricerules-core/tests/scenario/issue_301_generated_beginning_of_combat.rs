//! Issue #301 — the reviewed beginning-of-combat graveyard exile cohort.
//!
//! Target selection and revalidation follow CR 115.1, 115.6, and 608.2b; beginning-of-combat
//! timing follows CR 506.1 and 603.2b; graveyards are public zones under CR 404.2, while each
//! zone change creates a new object under CR 400.7.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind,
};

fn issue_301_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn graveyard_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                kind: TargetRefKind::Graveyard as i32,
                ..Default::default()
            }],
        })),
    }
}

fn permanent_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

fn graveyard_targets(object_ids: &[u32]) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: object_ids
                .iter()
                .copied()
                .map(|object_id| TargetRef {
                    object_id,
                    kind: TargetRefKind::Graveyard as i32,
                    ..Default::default()
                })
                .collect(),
        })),
    }
}

fn choose_zero_trigger_targets() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            // The ability itself is mandatory; only its target group is optional. An empty
            // target list is therefore the CR 115.6 "up to one" choice, not a trigger decline.
            decline: false,
            selected_modes: Vec::new(),
            targets: Vec::new(),
        })),
    }
}

fn move_graveyard_to_exile(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].exile.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Exile;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn return_exile_to_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .exile
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn move_battlefield_to_hand(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].hand.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn return_hand_to_battlefield(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .hand
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].battlefield.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Battlefield;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

#[test]
fn issue_301_triggers_only_on_controller_beginning_of_combat_and_allows_zero_or_one_target() {
    let mut engine = issue_301_engine(301_001);
    inject_creature_on_battlefield(&mut engine, 0, "ascendant_dustspeaker");
    let own = inject_graveyard_card(&mut engine, 0, "forest");
    let opponent = inject_graveyard_card(&mut engine, 1, "island");
    let battlefield = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    let batch = engine
        .apply_command(0, &primitive_yield())
        .expect("enter controller beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::BeginCombat);
    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("targeted trigger pending");
    let key = (pending.source_permanent_id as u64) << 32 | pending.ability_index as u64;
    let group = &batch.legal_by_player[&0].valid_targets_by_ability[&key].groups[0];
    assert_eq!(group.min, 0);
    assert_eq!(group.max, 1);
    assert_eq!(group.valid_graveyard_ids, [own, opponent]);

    assert!(engine
        .apply_command(0, &graveyard_targets(&[own, opponent]))
        .is_err());
    assert!(engine
        .apply_command(0, &graveyard_target(battlefield))
        .is_err());
    engine
        .apply_command(0, &choose_zero_trigger_targets())
        .expect("zero targets is legal");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opponent].zone, Zone::Graveyard);

    let mut one = issue_301_engine(301_002);
    inject_creature_on_battlefield(&mut one, 0, "startled_relic_sloth");
    let target = inject_graveyard_card(&mut one, 1, "island");
    one.apply_command(0, &primitive_yield())
        .expect("enter beginning of combat");
    one.apply_command(0, &graveyard_target(target))
        .expect("choose one target from either graveyard");
    resolve_entire_stack_two_player(&mut one);
    assert_eq!(one.state.objects[&target].zone, Zone::Exile);

    let mut opponent_turn = issue_301_engine(301_003);
    inject_creature_on_battlefield(&mut opponent_turn, 0, "ascendant_dustspeaker");
    opponent_turn.state.active_player_idx = 1;
    opponent_turn.state.priority_idx = 1;
    opponent_turn
        .apply_command(1, &primitive_yield())
        .expect("enter opponent beginning of combat");
    assert!(opponent_turn.state.pending_triggers.is_empty());
    assert!(opponent_turn.state.stack.is_empty());

    let mut late_source = issue_301_engine(301_004);
    late_source
        .apply_command(0, &primitive_yield())
        .expect("enter beginning of combat without source");
    inject_creature_on_battlefield(&mut late_source, 0, "startled_relic_sloth");
    assert!(late_source.state.pending_triggers.is_empty());
    assert!(late_source.state.stack.is_empty());

    let mut other_step = issue_301_engine(301_005);
    inject_creature_on_battlefield(&mut other_step, 0, "ascendant_dustspeaker");
    other_step.state.turn_step = TurnStep::Upkeep;
    other_step.state.active_player_idx = 0;
    other_step.state.priority_idx = 0;
    other_step
        .apply_command(0, &primitive_yield())
        .expect("advance a non-combat step");
    assert_eq!(other_step.state.turn_step, TurnStep::Draw);
    assert!(other_step.state.pending_triggers.is_empty());
    assert!(other_step.state.stack.is_empty());
}

#[test]
fn issue_301_revalidates_zone_and_generation_at_resolution() {
    let mut removed = issue_301_engine(301_010);
    inject_creature_on_battlefield(&mut removed, 0, "ascendant_dustspeaker");
    let target = inject_graveyard_card(&mut removed, 1, "island");
    removed
        .apply_command(0, &primitive_yield())
        .expect("enter beginning of combat");
    removed
        .apply_command(0, &graveyard_target(target))
        .expect("choose graveyard target");
    move_graveyard_to_exile(&mut removed, 1, target);
    resolve_entire_stack_two_player(&mut removed);
    assert_eq!(removed.state.objects[&target].zone, Zone::Exile);

    let mut returned = issue_301_engine(301_011);
    inject_creature_on_battlefield(&mut returned, 0, "startled_relic_sloth");
    let target = inject_graveyard_card(&mut returned, 1, "island");
    returned
        .apply_command(0, &primitive_yield())
        .expect("enter beginning of combat");
    returned
        .apply_command(0, &graveyard_target(target))
        .expect("choose graveyard target");
    move_graveyard_to_exile(&mut returned, 1, target);
    return_exile_to_graveyard(&mut returned, 1, target);
    resolve_entire_stack_two_player(&mut returned);
    assert_eq!(returned.state.objects[&target].zone, Zone::Graveyard);
    assert!(returned.state.players[1].graveyard.contains(&target));
}

#[test]
fn issue_301_rejects_stale_graveyard_target_at_selection_without_wire_generation() {
    let mut engine = issue_301_engine(301_012);
    inject_creature_on_battlefield(&mut engine, 0, "ascendant_dustspeaker");
    let target = inject_graveyard_card(&mut engine, 1, "island");
    engine
        .apply_command(0, &primitive_yield())
        .expect("enter beginning of combat");
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    move_graveyard_to_exile(&mut engine, 1, target);
    let new_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    assert!(
        new_generation > old_generation,
        "zone move must create a new generation"
    );
    assert!(
        engine.apply_command(0, &graveyard_target(target)).is_err(),
        "a target that left the graveyard must be rejected before stack placement"
    );
    assert_eq!(engine.state.pending_triggers.len(), 1);
    // TargetRef carries only object_id/kind, not a generation. This proves the applicable wire
    // boundary (current-zone legality plus generation change); same-ID wrong-generation input
    // cannot be expressed by this command and is covered at resolution below.
    return_exile_to_graveyard(&mut engine, 1, target);
    engine
        .apply_command(0, &graveyard_target(target))
        .expect("the new graveyard object may be targeted");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}

#[test]
fn issue_301_stages_and_orders_simultaneous_cohort_triggers_before_targeting() {
    let mut engine = issue_301_engine(301_013);
    let dustspeaker = inject_creature_on_battlefield(&mut engine, 0, "ascendant_dustspeaker");
    let sloth = inject_creature_on_battlefield(&mut engine, 0, "startled_relic_sloth");
    let own_target = inject_graveyard_card(&mut engine, 0, "forest");
    let opponent_target = inject_graveyard_card(&mut engine, 1, "island");

    let batch = engine
        .apply_command(0, &primitive_yield())
        .expect("stage both beginning-of-combat triggers");
    let ordering = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("same-controller simultaneous triggers require ordering");
    assert_eq!(ordering.deciding_player, 0);
    assert_eq!(ordering.candidates.len(), 2);
    let offered_sources = ordering
        .candidates
        .iter()
        .map(|candidate| candidate.source_permanent_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(offered_sources, [dustspeaker, sloth].into_iter().collect());
    assert!(batch.legal_by_player[&0]
        .valid_targets_by_ability
        .is_empty());
    assert!(engine.state.stack.is_empty());

    let first = ordering.candidates[0].object_id;
    engine
        .apply_command(0, &submit_trigger_order(first))
        .expect("choose the first trigger to place");
    let first_pending = engine
        .state
        .pending_triggers
        .front()
        .expect("the chosen trigger asks for targets before the next is placed");
    assert_eq!(first_pending.object_id, first);
    assert_eq!(first_pending.controller, 0);
    engine
        .apply_command(0, &graveyard_target(own_target))
        .expect("choose the first trigger target");
    let second_pending = engine
        .state
        .pending_triggers
        .front()
        .expect("the remaining trigger is placed after the first target choice");
    assert_ne!(second_pending.object_id, first);
    engine
        .apply_command(0, &graveyard_target(opponent_target))
        .expect("choose the second trigger target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&own_target].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&opponent_target].zone, Zone::Exile);
}

#[test]
fn issue_301_keeps_event_time_controller_after_source_control_changes() {
    let mut engine = issue_301_engine(301_014);
    let source = inject_creature_on_battlefield(&mut engine, 0, "ascendant_dustspeaker");
    let target = inject_graveyard_card(&mut engine, 1, "island");
    let batch = engine
        .apply_command(0, &primitive_yield())
        .expect("capture the controller's trigger");
    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("trigger remains pending for target selection");
    let key = (pending.source_permanent_id as u64) << 32 | pending.ability_index as u64;
    assert_eq!(pending.controller, 0);
    assert!(batch.legal_by_player[&0]
        .valid_targets_by_ability
        .contains_key(&key));
    let captured_source_generation = pending.source_zone_change;

    engine.state.players[0]
        .battlefield
        .retain(|candidate| *candidate != source);
    engine.state.players[1].battlefield.push(source);
    let source_object = engine.state.objects.get_mut(&source).unwrap();
    source_object.base_controller = 1;
    source_object.controller = 1;
    move_battlefield_to_hand(&mut engine, 1, source);
    return_hand_to_battlefield(&mut engine, 1, source);
    let recreated_source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    assert!(
        recreated_source_generation > captured_source_generation,
        "leaving and re-entering the source must create a new generation"
    );
    assert_eq!(
        engine.state.pending_triggers.front().unwrap().controller,
        0,
        "the staged trigger keeps its event-time controller"
    );
    assert!(!engine.initial_response_batch().legal_by_player[&1]
        .valid_targets_by_ability
        .contains_key(&key));
    engine
        .apply_command(0, &graveyard_target(target))
        .expect("the event-time controller still answers the trigger");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}

#[test]
fn issue_301_preserves_dustspeaker_etb_target_restriction_and_counter() {
    let mut engine = issue_301_engine(301_015);
    let friendly = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "ascendant_dustspeaker");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "ascendant_dustspeaker");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Ascendant Dustspeaker");
    pass_both_players(&mut engine);
    assert!(engine
        .apply_command(0, &permanent_target(opposing))
        .is_err());
    let dustspeaker = battlefield_object_for_card(&engine, 0, "ascendant_dustspeaker");
    assert!(engine
        .apply_command(0, &permanent_target(dustspeaker))
        .is_err());
    engine
        .apply_command(0, &permanent_target(friendly))
        .expect("target another creature controlled by Dustspeaker's controller");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&friendly].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.objects[&opposing].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn issue_301_uses_event_time_controller_and_player_set_generic_graveyards() {
    // M2 currently has a two-player fixture. Put the source on the opponent's battlefield and
    // change its controller to prove the trigger uses event-time control without inventing an
    // unsupported third seat; AnyPlayer is still exercised with the controller's opponent graveyard.
    let mut engine = issue_301_engine(301_020);
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    let source = inject_creature_on_battlefield(&mut engine, 0, "ascendant_dustspeaker");
    engine.state.players[0]
        .battlefield
        .retain(|candidate| *candidate != source);
    engine.state.players[1].battlefield.push(source);
    let source_object = engine.state.objects.get_mut(&source).unwrap();
    source_object.base_controller = 1;
    source_object.controller = 1;
    let target = inject_graveyard_card(&mut engine, 0, "island");

    let batch = engine
        .apply_command(1, &primitive_yield())
        .expect("controller enters beginning of combat");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let pending = engine.state.pending_triggers.front().unwrap();
    assert_eq!(pending.controller, 1);
    let key = (pending.source_permanent_id as u64) << 32 | pending.ability_index as u64;
    assert!(batch.legal_by_player.contains_key(&1));
    assert!(
        !batch.legal_by_player[&1].valid_targets_by_ability[&key].groups[0]
            .valid_graveyard_ids
            .is_empty()
    );
    assert!(!batch.legal_by_player[&0]
        .valid_targets_by_ability
        .contains_key(&key));
    engine
        .apply_command(1, &graveyard_target(target))
        .expect("controller may target the other player's graveyard");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}
