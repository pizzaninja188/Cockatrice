//! Actual-card coverage for Mirrorworks' per-entry optional payment and event-object copy.

use super::helpers::*;
use tricerules_core::state::CopiableValues;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    dev_command::Dev, ruled_command::Cmd, ChoiceKind, DevCommand, DevMoveCard, DevZone,
    ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn hand_index_for_object(engine: &GameEngine, player: usize, object_id: u32) -> usize {
    engine.state.players[player]
        .hand
        .iter()
        .position(|candidate| *candidate == object_id)
        .expect("the exact inserted card remains in hand")
}

fn enter_sol_ring_and_put_mirrorworks_trigger_on_stack(engine: &mut GameEngine) -> u32 {
    let controller = engine.state.players[0].id;
    let sol_ring = inject_card_into_hand(engine, 0, "sol_ring");
    give_mana(
        engine,
        controller,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(engine, 0, sol_ring);
    engine
        .apply_command(controller, &cast_spell(hand_index, vec![]))
        .expect("cast Sol Ring");
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&sol_ring].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one Mirrorworks trigger is on the stack"
    );
    let observed = engine
        .state
        .stack
        .last()
        .unwrap()
        .trigger_context
        .observed_object;
    assert_eq!(
        observed.map(|reference| reference.object_id),
        Some(sol_ring)
    );
    sol_ring
}

fn move_card(engine: &mut GameEngine, player: i32, card_name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            player,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the artifact during the pending Mirrorworks trigger");
}

fn pass_until_resolution_choice(engine: &mut GameEngine) {
    for _ in 0..8 {
        if engine.state.pending_resolution.is_some() {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("Mirrorworks must offer its optional copy branch");
}

#[test]
fn mirrorworks_pays_per_trigger_and_copies_the_exact_entering_artifact_lki() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_701,
        &[4, 9, 27],
        20,
        None,
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "mirrorworks");

    // The permanent-type filter rejects a creature even when its controller is the same.
    let bears = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(&engine, 0, bears);
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast a nonartifact creature");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&bears].zone,
        Zone::Battlefield,
        "step={:?}, priority={}, passes={}, stack={}, pending_spell_cast={:?}, pending_resolution={:?}",
        engine.state.turn_step,
        engine.state.priority_player_id(),
        engine.state.passes_since_stack_change,
        engine.state.stack.len(),
        engine.state.pending_spell_cast,
        engine.state.pending_resolution
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());

    let sol_ring = enter_sol_ring_and_put_mirrorworks_trigger_on_stack(&mut engine);
    let observed = engine
        .state
        .stack
        .last()
        .unwrap()
        .trigger_context
        .observed_object
        .unwrap();
    let generation = observed.zone_change_generation;

    // Model an already-applied copy effect, then let the observed artifact leave. Mirrorworks
    // must use this exact generation's last-known copiable values when its trigger resolves.
    let mirrorworks = tricerules_cards::registry::global()
        .get("mirrorworks")
        .expect("Mirrorworks is a registered copy-value fixture");
    engine
        .state
        .objects
        .get_mut(&sol_ring)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        source_card_id: "mirrorworks".into(),
        source_face_index: 0,
        face: mirrorworks.primary_face().clone(),
        room_faces: None,
        display_name: "Mirrorworks".into(),
    });
    move_card(&mut engine, 4, "Sol Ring", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&sol_ring].zone, Zone::Graveyard);
    move_card(&mut engine, 4, "Sol Ring", DevZone::Battlefield);
    assert_eq!(engine.state.objects[&sol_ring].zone, Zone::Battlefield);
    let returned_generation = engine.state.zone_change_generation[&sol_ring];
    assert_ne!(generation, returned_generation);
    assert_eq!(engine.state.stack.len(), 2);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| { item.trigger_context.observed_object == Some(observed) }));
    assert!(engine.state.stack.iter().any(|item| {
        item.trigger_context.observed_object.is_some_and(|current| {
            current.object_id == sol_ring && current.zone_change_generation == returned_generation
        })
    }));
    assert_ne!(
        generation, 0,
        "the entrant has a tracked battlefield incarnation"
    );

    let before_battlefield = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    pass_until_resolution_choice(&mut engine);
    let choice = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        choice.deciding_player, 4,
        "the Mirrorworks controller decides"
    );
    assert_eq!(
        choice.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    let before_wrong_player = format!("{:?}", engine.state);
    assert!(engine.apply_command(9, &select_branch(0)).is_err());
    assert_eq!(format!("{:?}", engine.state), before_wrong_player);
    engine
        .apply_command(
            4,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the second entry's separate trigger");
    assert_eq!(engine.state.stack.len(), 1);

    pass_until_resolution_choice(&mut engine);
    let choice = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        choice.deciding_player, 4,
        "the first trigger still belongs to Mirrorworks"
    );
    give_mana(
        &mut engine,
        4,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(4, &select_branch(0))
        .expect("choose the optional {2} copy branch");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::ManaPayment,
        "payment uses the existing staged mana selection"
    );
    let before_wrong_payer = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(
            9,
            &submit_resolution_decision(ResolutionChoiceDecision::PayMana),
        )
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before_wrong_payer);
    submit_mana_resolution_decision(&mut engine, 4, ResolutionChoiceDecision::PayMana)
        .expect("pay {2} during the trigger's resolution");

    let copied_tokens = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| {
            !before_battlefield.contains(object_id)
                && engine.state.objects[object_id].token_origin.is_some()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        copied_tokens.len(),
        1,
        "one payment creates exactly one token"
    );
    let copied_token = &engine.state.objects[&copied_tokens[0]];
    assert_eq!(copied_token.owner, 4);
    assert_eq!(copied_token.controller, 4);
    assert_eq!(
        copied_token.token_origin.as_ref().unwrap().source_card_id,
        "mirrorworks",
        "copy effects are included in the entrant's LKI values"
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(
        engine.state.stack.is_empty(),
        "the artifact token is excluded by token:false"
    );

    // The returned incarnation generated its own trigger, which was declined before the old
    // generation's LKI trigger resolved. The copy token did not recursively trigger Mirrorworks.
    let all_copies = engine.state.players[0]
        .battlefield
        .iter()
        .filter(|object_id| engine.state.objects[object_id].token_origin.is_some())
        .count();
    assert_eq!(all_copies, 1, "only the paid entry trigger creates a token");
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}
