//! Actual-card coverage for Monument to Endurance's per-turn modal trigger choices.
//!
//! Exact Scryfall Oracle text and ruling were checked 2026-10-08. CR 603.2c, 603.3b/c, 700.2b,
//! 514.2, and 400.7 govern one trigger per discarded card, ordering and mode announcement,
//! per-turn history, and a new source after a zone change.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_event::Ev, ChooseTriggerTarget, SelectedSpellMode, TargetRef, TargetRefKind,
};

const MONUMENT: &str = "monument_to_endurance";

fn prompt_in_batch(
    batch: &tricerules_proto::ruled::v1::RuledEventBatch,
) -> Option<tricerules_proto::ruled::v1::TriggerNeedsTarget> {
    batch.events.iter().find_map(|event| match &event.ev {
        Some(Ev::TriggerNeedsTarget(prompt)) => Some(prompt.clone()),
        _ => None,
    })
}

fn prompt_for_next_trigger(
    engine: &mut GameEngine,
    batch: Option<tricerules_proto::ruled::v1::RuledEventBatch>,
    source_permanent_id: Option<u32>,
) -> tricerules_proto::ruled::v1::TriggerNeedsTarget {
    if let Some(prompt) = batch.as_ref().and_then(prompt_in_batch) {
        if let Some(source) = source_permanent_id {
            assert_eq!(prompt.source_permanent_id, source);
        }
        return prompt;
    }
    loop {
        let Some(pending) = engine.state.pending_trigger_order.as_ref() else {
            panic!(
                "remaining Monument triggers await order or a choice; pending={}, staged={}, blocking={:?}, stack={}",
                engine.state.pending_triggers.len(),
                engine.state.staged_trigger_groups.len(),
                engine.state.blocking_choice(),
                engine.state.stack.len(),
            );
        };
        let player = pending.deciding_player;
        let candidate = pending
            .candidates
            .iter()
            .find(|candidate| {
                source_permanent_id.is_none_or(|source| candidate.source_permanent_id == source)
            })
            .expect("the requested Monument source has an unplaced trigger");
        let trigger_object_id = candidate.object_id;
        let batch = engine
            .apply_command(player, &submit_trigger_order(trigger_object_id))
            .expect("choose the next Monument trigger");
        if let Some(prompt) = prompt_in_batch(&batch) {
            if let Some(source) = source_permanent_id {
                assert_eq!(prompt.source_permanent_id, source);
            }
            return prompt;
        }
    }
}

fn choose_mode(mode_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            selected_modes: vec![SelectedSpellMode {
                mode_index,
                targets: Vec::new(),
            }],
            ..Default::default()
        })),
    }
}

fn clear_hand_to_library(engine: &mut GameEngine, player: usize) {
    let hand = std::mem::take(&mut engine.state.players[player].hand);
    for object_id in hand {
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Library;
        engine.state.players[player].library.push_back(object_id);
    }
}

fn pending_trigger_ids(engine: &GameEngine) -> Vec<u32> {
    engine
        .state
        .pending_triggers
        .iter()
        .map(|trigger| trigger.object_id)
        .collect()
}

fn stack_ids(engine: &GameEngine) -> Vec<u32> {
    engine.state.stack.iter().map(|item| item.id).collect()
}

fn cast_windfall_with_one_discard(
    engine: &mut GameEngine,
    player: usize,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    for hand_player in 0..engine.state.players.len() {
        clear_hand_to_library(engine, hand_player);
    }
    let player_id = engine.state.players[player].id;
    inject_card_into_hand(engine, player, "windfall");
    inject_card_into_hand(engine, player, "forest");
    give_mana(
        engine,
        player_id,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let spell_slot = hand_index_for_card(engine, player, "windfall");
    engine
        .apply_command(player_id, &cast_spell(spell_slot, Vec::new()))
        .expect("cast Windfall with one card left to discard");
    let first = engine.state.priority_player_id();
    engine
        .apply_command(first, &pass())
        .expect("first player passes");
    let second = engine.state.priority_player_id();
    engine
        .apply_command(second, &pass())
        .expect("Windfall resolves and causes one discard")
}

fn move_card(
    engine: &mut GameEngine,
    owner: i32,
    name: &str,
    zone: tricerules_proto::ruled::v1::DevZone,
) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            owner,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(tricerules_proto::ruled::v1::DevCommand {
                    target_player_id: owner,
                    dev: Some(tricerules_proto::ruled::v1::dev_command::Dev::MoveCard(
                        tricerules_proto::ruled::v1::DevMoveCard {
                            card_name: name.into(),
                            zone: zone as i32,
                            ready: false,
                        },
                    )),
                })),
            },
        )
        .expect("move the card through the engine dev command");
}

fn move_monument(engine: &mut GameEngine, owner: i32, zone: tricerules_proto::ruled::v1::DevZone) {
    move_card(engine, owner, "Monument to Endurance", zone);
}

fn choose_stack_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                object_id,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    }
}

#[test]
fn monument_each_discard_trigger_excludes_modes_chosen_by_earlier_trigger() {
    let mut engine = GameEngine::new(20_261_080, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, MONUMENT);
    move_ready_to_battlefield(&mut engine, 0, MONUMENT);
    let looting = inject_card_into_hand(&mut engine, 0, "faithless_looting");
    let first_discard = inject_card_into_hand(&mut engine, 0, "forest");
    let second_discard = inject_card_into_hand(&mut engine, 0, "island");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "faithless_looting");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Faithless Looting");
    pass_priority_round(&mut engine);
    let discarded = engine
        .apply_command(
            0,
            &submit_resolution_choice(vec![first_discard, second_discard]),
        )
        .expect("discard two cards to Faithless Looting");

    let first = prompt_for_next_trigger(&mut engine, Some(discarded), None);
    assert_eq!(
        (first.min_modes, first.max_modes, first.modes.len()),
        (1, 1, 3)
    );
    assert!(first.modes.iter().all(|mode| mode.selectable));
    let pending_before = pending_trigger_ids(&engine);
    let history_before = engine.state.trigger_modes_used_this_turn.clone();
    let stack_before = stack_ids(&engine);
    let command_index_before = engine.state.command_index;
    engine
        .apply_command(1, &choose_mode(0))
        .expect_err("an opponent cannot choose Monument's mode");
    assert_eq!(pending_trigger_ids(&engine), pending_before);
    assert_eq!(engine.state.trigger_modes_used_this_turn, history_before);
    assert_eq!(stack_ids(&engine), stack_before);
    assert_eq!(engine.state.command_index, command_index_before);
    let first_choice = engine
        .apply_command(0, &choose_mode(0))
        .expect("choose the draw mode for the first discard trigger");
    assert!(!engine
        .diagnostic_snapshot()
        .expect("mode history remains available to diagnostics")["state"]
        ["trigger_modes_used_this_turn"]
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .expect("composite keys serialize as structured entries")
        .is_empty());

    let second = prompt_for_next_trigger(&mut engine, Some(first_choice), None);
    assert!(
        !second.modes[0].selectable,
        "the draw mode selected by the first trigger must be unavailable to this one"
    );
    assert!(second.modes[1].selectable);
    assert!(second.modes[2].selectable);
    let pending_before = pending_trigger_ids(&engine);
    let history_before = engine.state.trigger_modes_used_this_turn.clone();
    let stack_before = stack_ids(&engine);
    let command_index_before = engine.state.command_index;
    engine
        .apply_command(0, &choose_mode(0))
        .expect_err("an earlier mode cannot be selected again this turn");
    assert_eq!(pending_trigger_ids(&engine), pending_before);
    assert_eq!(engine.state.trigger_modes_used_this_turn, history_before);
    assert_eq!(stack_ids(&engine), stack_before);
    assert_eq!(engine.state.command_index, command_index_before);
    engine
        .apply_command(0, &choose_mode(1))
        .expect("a different mode remains legal");
    assert_eq!(
        engine.state.objects[&looting].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn monument_instances_keep_independent_mode_histories() {
    let mut engine = GameEngine::new(20_261_082, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let first_source = inject_permanent_on_battlefield(&mut engine, 0, MONUMENT);
    let second_source = inject_permanent_on_battlefield(&mut engine, 0, MONUMENT);
    assert_ne!(first_source, second_source);

    let looting = inject_card_into_hand(&mut engine, 0, "faithless_looting");
    let first_discard = inject_card_into_hand(&mut engine, 0, "forest");
    let second_discard = inject_card_into_hand(&mut engine, 0, "island");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "faithless_looting");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Faithless Looting");
    pass_priority_round(&mut engine);
    let discarded = engine
        .apply_command(
            0,
            &submit_resolution_choice(vec![first_discard, second_discard]),
        )
        .expect("discard two cards");

    let first = prompt_for_next_trigger(&mut engine, Some(discarded), None);
    assert!(
        first.source_permanent_id == first_source || first.source_permanent_id == second_source
    );
    engine
        .apply_command(0, &choose_mode(0))
        .expect("select the draw mode for one Monument");

    let other_source = if first.source_permanent_id == first_source {
        second_source
    } else {
        first_source
    };
    let second = prompt_for_next_trigger(&mut engine, None, Some(other_source));
    assert_ne!(
        second.source_permanent_id, first.source_permanent_id,
        "the second Monument receives an independent trigger identity"
    );
    assert!(second.modes[0].selectable);
    assert!(second.modes[1].selectable);
    assert!(second.modes[2].selectable);
    engine
        .apply_command(0, &choose_mode(0))
        .expect("the other Monument may also choose draw this turn");
    assert_eq!(engine.state.objects[&looting].zone, Zone::Graveyard);
}

#[test]
fn monument_mode_history_starts_fresh_after_source_changes_zone_generation() {
    let mut engine = GameEngine::new(20_261_083, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, MONUMENT);
    let source = move_ready_to_battlefield(&mut engine, 0, MONUMENT);
    let initial_generation = engine.state.zone_change_generation[&source];

    let first_batch = cast_windfall_with_one_discard(&mut engine, 0);
    prompt_for_next_trigger(&mut engine, Some(first_batch), Some(source));
    engine
        .apply_command(0, &choose_mode(0))
        .expect("choose draw for this source generation");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.trigger_modes_used_this_turn.len(), 1);

    move_monument(&mut engine, 0, tricerules_proto::ruled::v1::DevZone::Exile);
    move_monument(
        &mut engine,
        0,
        tricerules_proto::ruled::v1::DevZone::Battlefield,
    );
    let returned_generation = engine.state.zone_change_generation[&source];
    assert!(returned_generation > initial_generation);

    let second_batch = cast_windfall_with_one_discard(&mut engine, 0);
    let returned = prompt_for_next_trigger(&mut engine, Some(second_batch), Some(source));
    assert!(returned.modes.iter().all(|mode| mode.selectable));
    engine
        .apply_command(0, &choose_mode(0))
        .expect("returned Monument has no memory of the prior object");
}

#[test]
fn monument_mode_history_resets_at_the_next_turn_boundary() {
    let mut engine = GameEngine::new(20_261_084, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, MONUMENT);
    let source = move_ready_to_battlefield(&mut engine, 0, MONUMENT);

    let first_batch = cast_windfall_with_one_discard(&mut engine, 0);
    prompt_for_next_trigger(&mut engine, Some(first_batch), Some(source));
    engine
        .apply_command(0, &choose_mode(0))
        .expect("choose draw during the first turn");
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.trigger_modes_used_this_turn.is_empty());
    for player in 0..engine.state.players.len() {
        clear_hand_to_library(&mut engine, player);
    }

    let first_turn = engine.state.turn_instance;
    end_active_turn(&mut engine, 0);
    pass_priority_round(&mut engine);
    assert!(
        engine.state.turn_instance > first_turn,
        "turn did not advance: step={:?}, active={}, priority={}, turn_instance={}",
        engine.state.turn_step,
        engine.state.active_player_id(),
        engine.state.priority_player_id(),
        engine.state.turn_instance,
    );
    assert!(engine.state.trigger_modes_used_this_turn.is_empty());
    for _ in 0..12 {
        if engine.state.turn_step == tricerules_core::TurnStep::Main1
            && engine.state.active_player_id() == 1
        {
            break;
        }
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);

    for player in 0..engine.state.players.len() {
        clear_hand_to_library(&mut engine, player);
    }
    end_active_turn(&mut engine, 1);
    pass_priority_round(&mut engine);
    for _ in 0..12 {
        if engine.state.turn_step == tricerules_core::TurnStep::Main1
            && engine.state.active_player_id() == 0
        {
            break;
        }
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);

    let next_turn_batch = cast_windfall_with_one_discard(&mut engine, 0);
    let next_turn = prompt_for_next_trigger(&mut engine, Some(next_turn_batch), Some(source));
    assert!(next_turn.modes.iter().all(|mode| mode.selectable));
    engine
        .apply_command(0, &choose_mode(0))
        .expect("the controller may choose the same mode during the next turn");
}

#[test]
fn monument_mode_remains_used_after_its_trigger_is_countered() {
    let mut engine = GameEngine::new(20_261_085, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, MONUMENT);
    let source = move_ready_to_battlefield(&mut engine, 0, MONUMENT);

    let first_batch = cast_windfall_with_one_discard(&mut engine, 0);
    prompt_for_next_trigger(&mut engine, Some(first_batch), Some(source));
    inject_card_into_hand(&mut engine, 1, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &choose_mode(0))
        .expect("choose draw as the trigger is put on the stack");
    let selected_trigger = engine
        .state
        .stack
        .iter()
        .rev()
        .find(|item| item.is_triggered && item.source_permanent_id == Some(source))
        .expect("Monument trigger on the stack")
        .id;
    if engine.state.priority_player_id() == 0 {
        engine
            .apply_command(0, &pass())
            .expect("pass to the opponent");
    }
    assert_eq!(engine.state.priority_player_id(), 1);
    let tidebinder_slot = hand_index_for_card(&engine, 1, "tishanas_tidebinder");
    let tidebinder = engine.state.players[1].hand[tidebinder_slot];
    engine
        .apply_command(1, &cast_spell(tidebinder_slot, Vec::new()))
        .expect("cast Tishana's Tidebinder with flash");
    pass_priority_round(&mut engine);
    assert!(engine.state.players[1].battlefield.contains(&tidebinder));
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine.initial_response_batch();
    engine
        .apply_command(1, &choose_stack_target(selected_trigger))
        .expect("Tidebinder targets the Monument triggered ability");
    pass_priority_round(&mut engine);
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == selected_trigger));
    assert_eq!(
        engine
            .state
            .trigger_modes_used_this_turn
            .values()
            .map(|modes| modes.len())
            .sum::<usize>(),
        1,
        "countering the trigger does not refund the mode"
    );

    resolve_entire_stack_two_player(&mut engine);
    move_card(
        &mut engine,
        1,
        "Tishana's Tidebinder",
        tricerules_proto::ruled::v1::DevZone::Exile,
    );
    let second_batch = cast_windfall_with_one_discard(&mut engine, 0);
    let second = prompt_for_next_trigger(&mut engine, Some(second_batch), Some(source));
    assert!(!second.modes[0].selectable);
    assert!(second.modes[1].selectable);
    assert!(second.modes[2].selectable);
}

fn monument_mode_history_snapshot_after_three_choices() -> serde_json::Value {
    let mut engine = GameEngine::new(20_261_088, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, MONUMENT);
    let source = move_ready_to_battlefield(&mut engine, 0, MONUMENT);

    for mode_index in 0..3 {
        let batch = cast_windfall_with_one_discard(&mut engine, 0);
        prompt_for_next_trigger(&mut engine, Some(batch), Some(source));
        engine
            .apply_command(0, &choose_mode(mode_index))
            .expect("choose a different unchosen mode");
        if mode_index < 2 {
            resolve_entire_stack_two_player(&mut engine);
        }
    }

    engine
        .diagnostic_snapshot()
        .expect("diagnostic snapshot after choosing all three modes")["state"]
        ["trigger_modes_used_this_turn"]
        .clone()
}

#[test]
fn monument_mode_history_diagnostic_is_replay_deterministic() {
    let expected = serde_json::json!(["create_treasure", "draw_a_card", "opponents_lose_life"]);
    let first = monument_mode_history_snapshot_after_three_choices();
    let first_entries = first["entries"]
        .as_array()
        .expect("composite history key serializes as entries");
    assert_eq!(first_entries.len(), 1);
    assert_eq!(first_entries[0]["value"], expected);

    for _ in 0..15 {
        let replay = monument_mode_history_snapshot_after_three_choices();
        assert_eq!(
            replay, first,
            "the same seeded command sequence must produce identical replay diagnostics"
        );
    }
}

#[test]
fn monument_exhausts_three_modes_without_a_fourth_stack_item_and_resolves_each_mode() {
    let players = [10, 20, 30];
    let mut engine = GameEngine::new(20_261_081, &players, 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    for player in 0..players.len() {
        clear_hand_to_library(&mut engine, player);
    }
    inject_card_into_hand(&mut engine, 0, MONUMENT);
    move_ready_to_battlefield(&mut engine, 0, MONUMENT);
    let windfall = inject_card_into_hand(&mut engine, 0, "windfall");
    for card_id in ["forest", "island", "mountain", "grizzly_bears"] {
        inject_card_into_hand(&mut engine, 0, card_id);
    }
    give_mana(
        &mut engine,
        players[0],
        ManaGift {
            c: 2,
            u: 1,
            ..Default::default()
        },
    );
    let windfall_slot = hand_index_for_card(&engine, 0, "windfall");
    engine
        .apply_command(players[0], &cast_spell(windfall_slot, Vec::new()))
        .expect("cast Windfall");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&windfall].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .pending_trigger_order
            .as_ref()
            .expect("four discard triggers order together")
            .candidates
            .len(),
        4
    );

    let library_after_windfall = engine.state.players[0].library.len();
    let mut previous_batch = None;
    for mode_index in 0..3 {
        let prompt = prompt_for_next_trigger(&mut engine, previous_batch.take(), None);
        assert_eq!(
            (prompt.min_modes, prompt.max_modes, prompt.modes.len()),
            (1, 1, 3)
        );
        assert!(prompt.modes[mode_index as usize].selectable);
        previous_batch = Some(
            engine
                .apply_command(players[0], &choose_mode(mode_index))
                .expect("choose one previously unused mode"),
        );
    }

    assert!(
        prompt_in_batch(previous_batch.as_ref().unwrap()).is_none(),
        "the fourth trigger has no unused mode and must not ask for a choice"
    );
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.staged_trigger_groups.is_empty());
    assert_eq!(
        engine.state.stack.len(),
        3,
        "only three chosen modes use the stack"
    );
    assert_eq!(
        engine
            .state
            .trigger_modes_used_this_turn
            .values()
            .map(|modes| modes.len())
            .sum::<usize>(),
        3
    );

    while !engine.state.stack.is_empty() {
        pass_priority_round(&mut engine);
    }
    assert_eq!(
        engine.state.players[0].library.len(),
        library_after_windfall - 1
    );
    assert_eq!(engine.state.players[1].life, 17);
    assert_eq!(engine.state.players[2].life, 17);
    assert_eq!(
        engine
            .state
            .objects
            .values()
            .filter(|object| object.card_id == "treasure" && object.zone == Zone::Battlefield)
            .count(),
        1
    );
}
