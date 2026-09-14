//! Issue #271 — exact generated recipes reuse the established target, cost, linked-exile,
//! private Surveil, and creature-as-damage-source contracts. Oracle/rulings and the current
//! Comprehensive Rules were checked 2026-09-13. CR 115, 118.7, 119-121, 400.7, 601.2c/f,
//! 608.2b-c/h, 610.3, and 701.25 govern these scenarios.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChooseTriggerTarget, RuledCommand, TargetRef,
    TargetRefKind,
};

fn two_player_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn three_player_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    while engine.state.turn_step != TurnStep::Main1 {
        let player = engine.state.priority_player_id();
        engine.apply_command(player, &pass()).expect("advance turn");
    }
    engine
}

fn fund(engine: &mut GameEngine) {
    give_mana(
        engine,
        0,
        ManaGift {
            w: 8,
            u: 8,
            b: 8,
            g: 8,
            c: 12,
            ..Default::default()
        },
    );
}

fn permanent_target(object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

fn choose_trigger_target(object_id: Option<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: object_id.map(target_object).unwrap_or_default(),
        })),
    }
}

fn cast_named(engine: &mut GameEngine, card_id: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
}

fn leave_battlefield(engine: &mut GameEngine, player: usize, object_id: u32, zone: Zone) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    match zone {
        Zone::Hand => engine.state.players[player].hand.push(object_id),
        Zone::Graveyard => engine.state.players[player].graveyard.push(object_id),
        _ => panic!("unsupported test destination"),
    }
    engine.state.objects.get_mut(&object_id).unwrap().zone = zone;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn resolve_three_player_stack(engine: &mut GameEngine) -> Vec<RuledEventBatch> {
    let mut batches = Vec::new();
    while !engine.state.stack.is_empty() {
        let player = engine.state.priority_player_id();
        batches.push(
            engine
                .apply_command(player, &pass())
                .expect("priority pass"),
        );
    }
    batches
}

#[test]
fn tapped_target_reduction_applies_once_and_only_for_the_announced_tapped_creature() {
    let mut engine = two_player_engine(271_001);
    let tapped = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let untapped = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.objects.get_mut(&tapped).unwrap().tapped = true;
    inject_card_into_hand(&mut engine, 0, "grounded_for_life");
    inject_card_into_hand(&mut engine, 0, "quicksand_whirlpool");

    let grounded = hand_index_for_card(&engine, 0, "grounded_for_life");
    let published = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((grounded as u32) << 8)];
    assert_eq!(published.targeted_cost_reduction_applications.len(), 1);
    assert_eq!(
        published.targeted_cost_reduction_applications[0].generic_mana,
        3
    );
    assert_eq!(
        published.targeted_cost_reduction_applications[0]
            .qualifying_targets
            .iter()
            .map(|target| target.object_id)
            .collect::<Vec<_>>(),
        [tapped]
    );

    engine.state.players[0].mana_pool.white = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_command(0, &cast_spell(grounded, target_object(tapped)))
        .expect("Grounded for Life costs {1}{W} against the tapped target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&tapped].zone, Zone::Graveyard);

    engine.state.players[0].mana_pool.white = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let whirlpool = hand_index_for_card(&engine, 0, "quicksand_whirlpool");
    let command_before = engine.state.command_index;
    assert!(engine
        .apply_command(0, &cast_spell(whirlpool, target_object(untapped)))
        .is_err());
    assert_eq!(engine.state.command_index, command_before);
}

#[test]
fn optional_linked_exile_handles_decline_illegal_target_and_departed_source() {
    let mut declined = two_player_engine(271_002);
    fund(&mut declined);
    declined.state.players[0].life = 15;
    cast_named(&mut declined, "liminal_hold", vec![]);
    pass_both_players(&mut declined);
    declined
        .apply_command(0, &choose_trigger_target(None))
        .expect("decline the optional target");
    resolve_entire_stack_two_player(&mut declined);
    assert_eq!(declined.state.players[0].life, 17);

    let mut illegal = two_player_engine(271_003);
    fund(&mut illegal);
    illegal.state.players[0].life = 15;
    let target = inject_creature_on_battlefield(&mut illegal, 1, "grizzly_bears");
    cast_named(&mut illegal, "prayer_of_binding", vec![]);
    pass_both_players(&mut illegal);
    illegal
        .apply_command(0, &choose_trigger_target(Some(target)))
        .expect("choose the optional target");
    leave_battlefield(&mut illegal, 1, target, Zone::Hand);
    resolve_entire_stack_two_player(&mut illegal);
    assert_eq!(illegal.state.players[0].life, 15);
    assert_eq!(illegal.state.objects[&target].zone, Zone::Hand);

    let mut departed = two_player_engine(271_004);
    fund(&mut departed);
    departed.state.players[0].life = 15;
    let target = inject_creature_on_battlefield(&mut departed, 1, "grizzly_bears");
    cast_named(&mut departed, "liminal_hold", vec![]);
    pass_both_players(&mut departed);
    departed
        .apply_command(0, &choose_trigger_target(Some(target)))
        .expect("choose a legal target");
    let source = battlefield_object_for_card(&departed, 0, "liminal_hold");
    leave_battlefield(&mut departed, 0, source, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut departed);
    assert_eq!(departed.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(departed.state.players[0].life, 17);
}

#[test]
fn surveil_choice_finishes_before_draw_two_and_life_loss() {
    let mut engine = two_player_engine(271_005);
    fund(&mut engine);
    engine.state.players[0].life = 20;
    let top = inject_library_card(&mut engine, 0, "forest");
    let second = inject_library_card(&mut engine, 0, "island");
    let third = inject_library_card(&mut engine, 0, "swamp");
    engine.state.players[0]
        .library
        .retain(|candidate| ![top, second, third].contains(candidate));
    for object_id in [third, second, top] {
        engine.state.players[0].library.push_front(object_id);
    }
    let hand_before = engine.state.players[0].hand.len();
    cast_named(&mut engine, "risky_research", vec![]);
    let resolving = engine.apply_command(0, &pass()).expect("caster pass");
    assert!(find_resolution_choice(&resolving).is_none());
    let resolving = engine.apply_command(1, &pass()).expect("opponent pass");
    let choice = find_resolution_choice(&resolving).expect("private Surveil choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, [top, second]);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);

    engine
        .apply_command(0, &submit_resolution_choice(vec![top]))
        .expect("finish Surveil and resume the effect tail");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&top].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&third].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].life, 18);
}

#[test]
fn illegal_bounce_target_prevents_the_whole_spell_and_surveil() {
    let mut engine = two_player_engine(271_006);
    fund(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let top = inject_library_card(&mut engine, 0, "forest");
    engine.state.players[0]
        .library
        .retain(|candidate| *candidate != top);
    engine.state.players[0].library.push_front(top);
    let library_before = engine.state.players[0].library.clone();
    cast_named(
        &mut engine,
        "unauthorized_exit",
        vec![permanent_target(target, 0)],
    );
    leave_battlefield(&mut engine, 1, target, Zone::Hand);
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].library, library_before);
    assert_eq!(engine.state.objects[&top].zone, Zone::Library);
}

#[test]
fn power_damage_revalidates_both_roles_and_uses_current_power_and_source_identity_in_multiplayer() {
    let mut engine = three_player_engine(271_007);
    fund(&mut engine);
    let source = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    let p1_target = inject_creature_with_stats(&mut engine, 1, "hill_giant", 6, 6);
    let p2_target = inject_creature_with_stats(&mut engine, 2, "hill_giant", 6, 6);
    inject_card_into_hand(&mut engine, 0, "quarrel");
    let slot = hand_index_for_card(&engine, 0, "quarrel");
    let legal = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    assert_eq!(legal.groups[0].valid_permanent_ids, [source]);
    assert!(legal.groups[1].valid_permanent_ids.contains(&p1_target));
    assert!(legal.groups[1].valid_permanent_ids.contains(&p2_target));
    engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![permanent_target(source, 0), permanent_target(p2_target, 1)],
            ),
        )
        .expect("cast Quarrel across the opponent set");
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let batches = resolve_three_player_stack(&mut engine);
    assert_eq!(engine.state.objects[&p2_target].damage, 3);
    assert!(batches.iter().flat_map(|batch| &batch.events).any(|event| {
        matches!(&event.ev, Some(Ev::Log(log)) if log.text.starts_with("Grizzly Bears deals 3 damage"))
    }));

    let mut stale = two_player_engine(271_008);
    fund(&mut stale);
    let source = inject_creature_on_battlefield(&mut stale, 0, "grizzly_bears");
    let recipient = inject_creature_with_stats(&mut stale, 1, "hill_giant", 6, 6);
    cast_named(
        &mut stale,
        "rocky_rebuke",
        vec![permanent_target(source, 0), permanent_target(recipient, 1)],
    );
    leave_battlefield(&mut stale, 1, recipient, Zone::Hand);
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&recipient].damage, 0);
    assert_eq!(stale.state.objects[&source].zone, Zone::Battlefield);

    let mut stale_source = two_player_engine(271_009);
    fund(&mut stale_source);
    let source = inject_creature_on_battlefield(&mut stale_source, 0, "grizzly_bears");
    let recipient = inject_creature_with_stats(&mut stale_source, 1, "hill_giant", 6, 6);
    cast_named(
        &mut stale_source,
        "rocky_rebuke",
        vec![permanent_target(source, 0), permanent_target(recipient, 1)],
    );
    leave_battlefield(&mut stale_source, 0, source, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut stale_source);
    assert_eq!(stale_source.state.objects[&recipient].damage, 0);
}
