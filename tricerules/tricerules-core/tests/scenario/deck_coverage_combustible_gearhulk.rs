//! Actual-card coverage for Combustible Gearhulk's targeted draw-or-mill trigger.
//!
//! Wizards' Kaladesh Release Notes cover X=0 outside the stack, draw-three with a short library,
//! and the uninterrupted mill-then-damage sequence. CR 115.1d/603.3 govern target announcement;
//! 608.2b governs target legality; 202.3/202.3d govern mana value outside the stack; 701.17 and
//! 120.2 govern the mill result and damage source.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    dev_command::Dev, ruled_command::Cmd, DevCommand, DevMoveCard, DevZone,
    ResolutionChoiceDecision, SubmitResolutionChoice,
};

const GEARHULK: &str = "combustible_gearhulk";

fn engine(seed: u64) -> GameEngine {
    engine_with_players(seed, &[0, 1, 2])
}
fn engine_with_players(seed: u64, player_ids: &[i32]) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        player_ids,
        20,
        None,
        true,
    )
    .expect("new player-set Gearhulk game");
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
            resolution_prompt_id: 0,
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
        .apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
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
        engine
            .apply_scenario_command(0, &choose_trigger_target(0))
            .is_err(),
        "the trigger cannot target its controller"
    );
    assert_eq!(format!("{:?}", engine.state), before_invalid_target);
    engine
        .apply_scenario_command(0, &choose_trigger_target(target))
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

fn resolution_decision_with_prompt(
    resolution_prompt_id: u64,
    decision: ResolutionChoiceDecision,
    branch_index: u32,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id,
            decision: decision as i32,
            selected_branch_index: branch_index,
            ..Default::default()
        })),
    }
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
        .apply_scenario_command(
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
        .apply_scenario_command(
            1,
            &resolution_decision(ResolutionChoiceDecision::Decline, 0)
        )
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before_wrong_actor);

    engine
        .apply_scenario_command(
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
        .apply_scenario_command(
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
fn narset_checks_gearhulks_controller_draw_instead_of_its_unrestricted_chooser() {
    let mut engine = engine(202_610_704);
    inject_card_into_hand(&mut engine, 1, "narset,_parter_of_veils");
    move_ready_to_battlefield(&mut engine, 1, "narset,_parter_of_veils");
    let milled = set_library_top(&mut engine, &["opt", "arcane_denial", "island"]);
    let controller_hand_before = engine.state.players[0].hand.len();
    let chooser_life_before = engine.state.players[1].life;

    cast_gearhulk_and_choose_target(&mut engine, 1);
    for _ in 0..12 {
        if engine.state.stack.is_empty() {
            break;
        }
        assert!(
            engine.state.pending_resolution.is_none(),
            "Narset makes the controller's draw-three branch unavailable, so its unrestricted chooser gets no prompt"
        );
        pass_priority_round(&mut engine);
    }

    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand.len(), controller_hand_before);
    assert!(milled
        .iter()
        .all(|object_id| engine.state.players[0].graveyard.contains(object_id)));
    assert_eq!(
        engine.state.players[1].life,
        chooser_life_before - 3,
        "the eligible target chooses the otherwise mill-and-damage branch"
    );
}

#[test]
fn trigger_does_not_resolve_if_its_target_opponent_leaves_before_resolution() {
    let mut engine = engine(202_610_703);
    let library = set_library_top(&mut engine, &["island", "mountain", "forest"]);
    cast_gearhulk_and_choose_target(&mut engine, 1);
    engine
        .apply_scenario_command(1, &concede())
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

#[test]
fn departing_target_delegates_the_original_gearhulk_choice_and_replays_exact_commands() {
    use prost::Message;

    type RecordedCommand = (
        i32,
        Vec<u8>,
        tricerules_proto::ruled::v1::RuledEventBatch,
        serde_json::Value,
    );

    fn prepared(seed: u64) -> GameEngine {
        let mut engine = engine_with_players(seed, &[0, 1, 2, 3]);
        set_library_top(
            &mut engine,
            &["fireball", "fire_ice", "derelict_attic_widows_walk"],
        );
        inject_card_into_hand(&mut engine, 0, GEARHULK);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 4,
                r: 2,
                ..Default::default()
            },
        );
        engine.enable_dev_commands();
        engine
    }

    fn accept(
        engine: &mut GameEngine,
        accepted: &mut Vec<RecordedCommand>,
        actor: i32,
        command: RuledCommand,
    ) -> tricerules_proto::ruled::v1::RuledEventBatch {
        let batch = engine
            .apply_command(actor, &command)
            .expect("record only accepted commands");
        accepted.push((
            actor,
            command.encode_to_vec(),
            batch.clone(),
            engine.diagnostic_snapshot().expect("diagnostic snapshot"),
        ));
        batch
    }

    fn pass_to_pending_trigger(engine: &mut GameEngine, accepted: &mut Vec<RecordedCommand>) {
        for _ in 0..16 {
            if !engine.state.pending_triggers.is_empty() {
                return;
            }
            let live = engine
                .state
                .players
                .iter()
                .filter(|player| !player.has_lost)
                .count();
            let needed = live
                .checked_sub(engine.state.passes_since_stack_change as usize)
                .expect("consistent priority pass count");
            for _ in 0..needed {
                let actor = engine.state.priority_player_id();
                accept(engine, accepted, actor, pass());
                if !engine.state.pending_triggers.is_empty() {
                    return;
                }
            }
        }
        panic!("Gearhulk ETB trigger must be pending");
    }

    fn pass_to_resolution_offer(
        engine: &mut GameEngine,
        accepted: &mut Vec<RecordedCommand>,
    ) -> tricerules_proto::ruled::v1::ResolutionChoiceRequired {
        for _ in 0..24 {
            let live = engine
                .state
                .players
                .iter()
                .filter(|player| !player.has_lost)
                .count();
            let needed = live
                .checked_sub(engine.state.passes_since_stack_change as usize)
                .expect("consistent priority pass count");
            for _ in 0..needed {
                let actor = engine.state.priority_player_id();
                let batch = accept(engine, accepted, actor, pass());
                if engine.state.pending_resolution.is_some() {
                    return find_resolution_choice(&batch)
                        .expect("the resolving Gearhulk publishes its ProgramChoose offer");
                }
            }
        }
        panic!("Gearhulk must offer its ProgramChoose choice");
    }

    fn target_choice(target: i32) -> RuledCommand {
        RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                targets: target_player(target),
                ..Default::default()
            })),
        }
    }

    fn leave_source() -> RuledCommand {
        RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: 0,
                dev: Some(Dev::MoveCard(DevMoveCard {
                    card_name: "Combustible Gearhulk".into(),
                    zone: DevZone::Graveyard as i32,
                    ready: false,
                })),
            })),
        }
    }

    fn delegated_decline_stream(engine: &mut GameEngine) -> Vec<RecordedCommand> {
        let mut accepted = Vec::new();
        let hand_index = hand_index_for_card(engine, 0, GEARHULK);
        accept(engine, &mut accepted, 0, cast_spell(hand_index, Vec::new()));
        pass_to_pending_trigger(engine, &mut accepted);
        accept(engine, &mut accepted, 0, target_choice(3));
        let gearhulk = engine
            .state
            .objects
            .values()
            .find(|object| object.card_id == GEARHULK && object.owner == 0)
            .expect("injected Gearhulk object")
            .id;
        accept(engine, &mut accepted, 0, leave_source());
        assert_eq!(engine.state.objects[&gearhulk].zone, Zone::Graveyard);

        let target_offer = pass_to_resolution_offer(engine, &mut accepted);
        assert_eq!(target_offer.deciding_player_id, 3);
        assert_ne!(target_offer.resolution_prompt_id, 0);
        let original_prompt_id = target_offer.resolution_prompt_id;
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.deciding_player, 3);
        assert_eq!(pending.resolution_prompt_id.get(), original_prompt_id);

        let before_wrong_actor = engine.diagnostic_snapshot().unwrap();
        let wrong_actor_command = resolution_decision_with_prompt(
            original_prompt_id,
            ResolutionChoiceDecision::Decline,
            0,
        );
        assert!(engine.apply_command(1, &wrong_actor_command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before_wrong_actor);

        let departure = accept(engine, &mut accepted, 3, concede());
        let delegate_offer = find_resolution_choice(&departure)
            .expect("controller receives the delegate-selection offer");
        assert_eq!(delegate_offer.deciding_player_id, 0);
        assert_ne!(delegate_offer.resolution_prompt_id, 0);
        assert!(delegate_offer
            .resolution_branches
            .iter()
            .any(|branch| branch.label == "P1 makes the choice"));
        assert!(delegate_offer
            .resolution_branches
            .iter()
            .any(|branch| branch.label == "P2 makes the choice"));
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            0
        );

        let after_departure = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_command(3, &wrong_actor_command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), after_departure);

        let delegate_selected = accept(
            engine,
            &mut accepted,
            0,
            resolution_decision_with_prompt(
                delegate_offer.resolution_prompt_id,
                ResolutionChoiceDecision::SelectBranch,
                0,
            ),
        );
        let delegate_choice = find_resolution_choice(&delegate_selected)
            .expect("selected delegate receives the original choice");
        assert_eq!(delegate_choice.deciding_player_id, 1);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            1
        );

        let before_stale = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(
                0,
                &resolution_decision_with_prompt(
                    delegate_offer.resolution_prompt_id,
                    ResolutionChoiceDecision::SelectBranch,
                    0,
                ),
            )
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before_stale);
        let before_wrong_delegate_actor = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(
                2,
                &resolution_decision_with_prompt(
                    delegate_choice.resolution_prompt_id,
                    ResolutionChoiceDecision::Decline,
                    0,
                ),
            )
            .is_err());
        assert_eq!(
            engine.diagnostic_snapshot().unwrap(),
            before_wrong_delegate_actor
        );

        accept(
            engine,
            &mut accepted,
            1,
            resolution_decision_with_prompt(
                delegate_choice.resolution_prompt_id,
                ResolutionChoiceDecision::Decline,
                0,
            ),
        );
        let after_decline = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(
                1,
                &resolution_decision_with_prompt(
                    delegate_choice.resolution_prompt_id,
                    ResolutionChoiceDecision::Decline,
                    0,
                ),
            )
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), after_decline);
        accepted
    }

    let mut original = prepared(202_610_705);
    let mut replay = prepared(202_610_705);
    let expected_milled = original.state.players[0]
        .library
        .iter()
        .take(3)
        .copied()
        .collect::<Vec<_>>();
    let accepted = delegated_decline_stream(&mut original);

    for (actor, encoded, expected_batch, expected_state) in accepted {
        let command = RuledCommand::decode(encoded.as_slice()).expect("decode accepted command");
        assert_eq!(
            replay
                .apply_command(actor, &command)
                .expect("replay accepted command"),
            expected_batch
        );
        assert_eq!(replay.diagnostic_snapshot().unwrap(), expected_state);
    }

    for player in &original.state.players {
        if player.id != 3 {
            assert_eq!(player.life, 20, "no live player is damaged or retargeted");
        }
    }
    assert!(original.state.players[3].has_lost);
    assert!(original.state.pending_resolution.is_none());
    assert!(original.state.stack.is_empty());
    assert!(expected_milled
        .iter()
        .all(|object_id| original.state.objects[object_id].zone == Zone::Graveyard));
}

#[test]
fn gearhulk_mill_cohort_waits_for_ordered_prevention_then_deals_ten_once() {
    use tricerules_core::state::{
        ActiveDamagePrevention, DamagePreventionAmount, DamagePreventionScope,
        ReplacementSourcePresentation,
    };

    fn add_one_per_event_prevention(engine: &mut GameEngine, target: i32, label: &str) {
        let effect_id = engine.state.next_damage_prevention_effect_id;
        engine
            .state
            .damage_prevention_effects
            .push(ActiveDamagePrevention {
                id: effect_id,
                static_origin: None,
                source_id: None,
                source_label: label.to_string(),
                source_presentation: ReplacementSourcePresentation::default(),
                scope: DamagePreventionScope::Recipient(target as u32),
                amount: DamagePreventionAmount::FixedPerEvent(1),
                duration: tricerules_card_model::primitives::EffectDuration::UntilEndOfTurn,
                additional_effect: None,
            });
        engine.state.next_damage_prevention_effect_id = effect_id
            .checked_add(1)
            .expect("prevention effect ID increments");
    }

    let mut engine = engine(202_610_706);
    let milled = set_library_top(
        &mut engine,
        &["fireball", "fire_ice", "derelict_attic_widows_walk"],
    );
    let library_before_mill = engine.state.players[0].library.len();
    let gearhulk = cast_gearhulk_and_choose_target(&mut engine, 2);
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&gearhulk)
        .copied()
        .expect("the battlefield Gearhulk has a captured generation");
    let trigger_stack_id = engine
        .state
        .stack
        .iter()
        .find(|item| {
            item.card_id == GEARHULK
                && item.is_triggered
                && item.source_permanent_id == Some(gearhulk)
        })
        .map(|item| item.id)
        .expect("the triggered ability has its own stack object ID");
    move_gearhulk_to_graveyard(&mut engine);
    add_one_per_event_prevention(&mut engine, 2, "first fixed prevention");
    add_one_per_event_prevention(&mut engine, 2, "second fixed prevention");
    pass_until_resolution_choice(&mut engine);

    let choose_offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("targeted ProgramChoose offer")
        .resolution_prompt_id
        .get();
    assert_ne!(choose_offer, 0);
    let mill_and_damage = engine
        .apply_command(
            2,
            &resolution_decision_with_prompt(choose_offer, ResolutionChoiceDecision::Decline, 0),
        )
        .expect("target opponent declines into the exact Mill cohort");

    assert!(milled
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Graveyard));
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before_mill - 3
    );
    assert_eq!(engine.state.objects[&gearhulk].zone, Zone::Graveyard);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("two fixed preventions require a CR 616 ordering choice");
    assert_eq!(pending.deciding_player, 2);
    assert_eq!(
        pending.presentation.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::ReplacementEffect
    );
    assert_ne!(pending.resolution_prompt_id.get(), 0);
    assert_eq!(pending.presentation.candidates.len(), 2);
    let choice = find_resolution_choice(&mill_and_damage)
        .expect("the pending damage event publishes its replacement offer");
    assert!(choice
        .candidate_names
        .iter()
        .all(|name| { name.contains("Combustible Gearhulk") && name.contains("12 damage") }));
    let library_after_mill = engine.state.players[0].library.len();
    let pending_snapshot = engine.diagnostic_snapshot().unwrap();
    let tricerules_core::state::ResolutionContinuation::DamageReplacement { stack, .. } =
        &pending.continuation
    else {
        panic!("the ordered prevention offer parks Gearhulk's stack resolution");
    };
    assert_eq!(stack.item.id, trigger_stack_id);
    assert_eq!(stack.item.card_id, GEARHULK);
    assert_eq!(stack.item.source_permanent_id, Some(gearhulk));
    assert_eq!(stack.item.source_owner, Some(0));
    assert_eq!(stack.item.source_zone_change, source_generation);
    assert_eq!(stack.item.targets[0].object_id, 2);

    let continuation = &pending_snapshot["state"]["pending_resolution"]["continuation"];
    assert_eq!(continuation["variant"], "DamageReplacement");
    let parked_stack = &continuation["value"]["stack"];
    assert!(parked_stack["previous_result"]["cards"]
        .as_array()
        .expect("the previous primitive result is retained")
        .is_empty());
    let parked_program = &parked_stack["program"];
    let owner = &parked_program["owner"];
    assert_eq!(owner["variant"], "Ability");
    assert_eq!(owner["value"]["card_id"], GEARHULK);
    assert_eq!(owner["value"]["face_id"], GEARHULK);
    assert_eq!(owner["value"]["ability_path"].as_array().unwrap().len(), 1);
    assert_eq!(owner["value"]["ability_path"][0], "triggered_01");
    assert!(parked_program["pending_owners"]
        .as_array()
        .unwrap()
        .is_empty());

    let frames = parked_program["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 2);
    assert!(frames[0]["path"].as_array().unwrap().is_empty());
    assert_eq!(frames[0]["next_instruction"], 1);
    assert_eq!(frames[1]["next_instruction"], 2);
    assert_eq!(frames[1]["path"].as_array().unwrap().len(), 1);
    assert_eq!(frames[1]["path"][0]["instruction_index"], 0);
    assert_eq!(frames[1]["path"][0]["arm"], "ChooseOtherwise");

    assert_eq!(parked_program["in_flight"], serde_json::json!([]));
    assert!(parked_program["in_flight_produced_object_provenance"].is_null());
    let slots = parked_program["slots"].as_array().unwrap();
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0]["variant"], "CardResultCohort");
    let card_entries = slots[0]["value"]["cards"].as_array().unwrap();
    let parked_mill_ids = card_entries
        .iter()
        .map(|entry| {
            entry["object_id"]
                .as_u64()
                .map(|object_id| object_id as u32)
                .or_else(|| {
                    entry["object_id"]
                        .as_str()
                        .and_then(|object_id| object_id.parse().ok())
                })
                .expect("parked Mill cohort retains numeric engine object IDs")
        })
        .collect::<Vec<_>>();
    assert_eq!(parked_mill_ids, milled);
    assert!(card_entries
        .iter()
        .all(|entry| entry["action"] == "Mill" && entry["affected_player"] == 0));

    let prevention_prompt_id = pending.resolution_prompt_id.get();
    let selected_prevention = pending.presentation.candidates[0];
    let applied = engine
        .apply_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: prevention_prompt_id,
                    chosen_object_ids: vec![selected_prevention],
                    ..Default::default()
                })),
            },
        )
        .expect("one ordered prevention sequence resumes Gearhulk's parked damage");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[2].life, 10,
        "12 damage minus two prevention"
    );
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.players[0].library.len(), library_after_mill);
    assert!(milled
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Graveyard));
    let life_changes = applied
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::LifeChanged(change)) => Some(change),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(life_changes.len(), 1, "the combined damage applies once");
    assert_eq!(life_changes[0].player_id, 2);
    assert_eq!(life_changes[0].delta, -10);
    assert!(applied.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text.contains("prevents 1 damage from Combustible Gearhulk")
    )));
}
