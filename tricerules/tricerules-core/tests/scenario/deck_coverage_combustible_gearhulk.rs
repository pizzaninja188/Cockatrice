//! Actual-card coverage for Combustible Gearhulk's targeted draw-or-mill trigger.
//!
//! Wizards' Kaladesh Release Notes cover X=0 outside the stack, draw-three with a short library,
//! and the uninterrupted mill-then-damage sequence. CR 115.1d/603.3 govern target announcement;
//! 608.2b governs target legality; 202.3/202.3d govern mana value outside the stack; 701.17 and
//! 120.2 govern the mill result and damage source.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    dev_command::Dev, ruled_command::Cmd, DevCommand, DevMoveCard, DevZone,
    ResolutionChoiceDecision, SubmitResolutionChoice,
};

const GEARHULK: &str = "combustible_gearhulk";

fn engine(seed: u64) -> GameEngine {
    let mut engine =
        GameEngine::new(seed, &[0, 1, 2], 20, None, true).expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_trigger_target(player: i32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: target_player(player),
            ..Default::default()
        })),
    }
}

fn resolution_decision(decision: ResolutionChoiceDecision, branch_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: decision as i32,
            selected_branch_index: branch_index,
            ..Default::default()
        })),
    }
}

fn cast_gearhulk_and_choose_target(engine: &mut GameEngine, target: i32) -> u32 {
    let gearhulk = inject_card_into_hand(engine, 0, GEARHULK);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 4,
            r: 2,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(engine, 0, GEARHULK);
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Combustible Gearhulk");

    for _ in 0..8 {
        if !engine.state.pending_triggers.is_empty() {
            break;
        }
        pass_priority_round(engine);
    }
    assert_eq!(engine.state.pending_triggers.len(), 1);

    let before_invalid_target = format!("{:?}", engine.state);
    assert!(
        engine.apply_command(0, &choose_trigger_target(0)).is_err(),
        "the trigger cannot target its controller"
    );
    assert_eq!(format!("{:?}", engine.state), before_invalid_target);
    engine
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose one target opponent");
    gearhulk
}

fn pass_until_resolution_choice(engine: &mut GameEngine) {
    for _ in 0..12 {
        if engine.state.pending_resolution.is_some() {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("Gearhulk's target must receive its resolution choice");
}

fn set_library_top(engine: &mut GameEngine, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, 0, card_id))
        .collect::<Vec<_>>();
    engine.state.players[0]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[0].library.push_front(*object_id);
    }
    object_ids
}

fn move_gearhulk_to_graveyard(engine: &mut GameEngine) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: "Combustible Gearhulk".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("Gearhulk leaves the battlefield after its trigger is on the stack");
}

#[test]
fn targeted_opponent_can_decline_and_take_the_sum_of_exact_milled_card_mana_values() {
    let mut engine = engine(202_610_701);
    let milled = set_library_top(
        &mut engine,
        &["fireball", "fire_ice", "derelict_attic_widows_walk"],
    );
    let gearhulk = cast_gearhulk_and_choose_target(&mut engine, 2);
    move_gearhulk_to_graveyard(&mut engine);
    assert_eq!(engine.state.objects[&gearhulk].zone, Zone::Graveyard);
    pass_until_resolution_choice(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        2,
        "only the targeted opponent chooses"
    );

    let before_wrong_actor = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(
            1,
            &resolution_decision(ResolutionChoiceDecision::Decline, 0)
        )
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before_wrong_actor);

    engine
        .apply_command(
            2,
            &resolution_decision(ResolutionChoiceDecision::Decline, 0),
        )
        .expect("target opponent declines the draw");
    assert!(milled
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Graveyard));
    assert!(milled
        .iter()
        .all(|object_id| engine.state.players[0].graveyard.contains(object_id)));
    assert_eq!(
        engine.state.players[2].life, 8,
        "1 + 4 + 7 mana value damage"
    );
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.objects[&gearhulk].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn targeted_opponent_may_choose_the_draw_branch_with_fewer_than_three_cards() {
    let mut engine = engine(202_610_702);
    let previous_library = std::mem::take(&mut engine.state.players[0].library);
    for object_id in previous_library {
        engine.state.players[0].graveyard.push(object_id);
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    }
    inject_library_card(&mut engine, 0, "island");
    assert_eq!(engine.state.players[0].library.len(), 1);
    cast_gearhulk_and_choose_target(&mut engine, 1);
    pass_until_resolution_choice(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );

    engine
        .apply_command(
            1,
            &resolution_decision(ResolutionChoiceDecision::SelectBranch, 0),
        )
        .expect("target opponent chooses to have the controller draw three");
    assert!(engine.state.players[0].has_lost);
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.players[2].life, 20);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn trigger_does_not_resolve_if_its_target_opponent_leaves_before_resolution() {
    let mut engine = engine(202_610_703);
    let library = set_library_top(&mut engine, &["island", "mountain", "forest"]);
    cast_gearhulk_and_choose_target(&mut engine, 1);
    engine
        .apply_command(1, &concede())
        .expect("target opponent concedes before the trigger resolves");
    assert!(engine.state.players[1].has_lost);

    for _ in 0..8 {
        if engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty() {
            break;
        }
        pass_priority_round(&mut engine);
    }
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(library
        .iter()
        .all(|object_id| engine.state.players[0].library.contains(object_id)));
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[2].life, 20);
}
