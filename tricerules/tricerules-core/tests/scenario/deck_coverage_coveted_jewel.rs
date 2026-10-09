use crate::helpers::*;
use tricerules_core::{GameEngine, TurnStep};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, BlockPair, DevCommand, DevMoveCard, DevZone, RuledCommand,
};

#[test]
fn coveted_jewel_auto_empty_blocker_event_draws_transfers_control_and_untaps() {
    let decks = Some(vec![vec!["forest".into(); 30], vec!["island".into(); 30]]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        98492,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    advance_to_declare_attackers(&mut engine);
    let jewel = inject_permanent_on_battlefield(&mut engine, 1, "coveted_jewel");
    engine.state.objects.get_mut(&jewel).unwrap().tapped = true;
    engine.initial_response_batch();
    let attacker = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "grizzly_bears")
        .expect("injected attacker");
    let hand_before = engine.state.players[0].hand.len();

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker at player 1");
    assert!(engine.state.stack.is_empty(), "Jewel waits for blockers");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_command(1, &pass())
        .expect("defending player passes attackers step");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the auto-empty blocker declaration must collect Coveted Jewel's trigger"
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 3);
    assert_eq!(engine.state.objects[&jewel].controller, 0);
    assert!(!engine.state.objects[&jewel].tapped);
}

#[test]
fn coveted_jewel_explicit_mixed_blockers_trigger_once_for_the_unblocked_group() {
    let decks = Some(vec![vec!["forest".into(); 30], vec!["island".into(); 30]]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        98493,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    advance_to_declare_attackers(&mut engine);
    let jewel = inject_permanent_on_battlefield(&mut engine, 1, "coveted_jewel");
    engine.state.objects.get_mut(&jewel).unwrap().tapped = true;
    let attacker_a = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "grizzly_bears")
        .expect("injected attacker");
    let attacker_b = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let hand_before = engine.state.players[0].hand.len();

    engine
        .apply_command(0, &declare_attackers(vec![attacker_a, attacker_b]))
        .expect("declare both attackers at player 1");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_command(1, &pass())
        .expect("defending player passes attackers step");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);

    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker_a,
                blocker_id: blocker,
            }]),
        )
        .expect("block one attacker and leave one unblocked");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one mixed attack group creates one trigger, not one trigger per attacker"
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 3);
    assert_eq!(engine.state.objects[&jewel].controller, 0);
    assert!(!engine.state.objects[&jewel].tapped);
}

#[test]
fn coveted_jewel_enters_with_three_cards_and_makes_three_mana_of_one_color() {
    let decks = Some(vec![vec!["forest".into(); 30], vec!["island".into(); 30]]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        98494,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let jewel = inject_card_into_hand(&mut engine, 0, "coveted_jewel");
    let hand_before_cast = engine.state.players[0].hand.len();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 6,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "coveted_jewel"), vec![]),
        )
        .expect("cast Coveted Jewel");

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_cast - 1 + 3,
        "the entry trigger draws three cards"
    );
    assert_eq!(
        engine.state.objects[&jewel].zone,
        tricerules_core::Zone::Battlefield
    );
    let mut activation = activate_ability_for(&engine, jewel, 0, vec![]);
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::ActivateAbility(command)) =
        activation.cmd.as_mut()
    else {
        unreachable!()
    };
    command.mana_option_index = 3;
    engine
        .apply_command(0, &activation)
        .expect("choose Coveted Jewel's red three-mana option");
    assert_eq!(engine.state.players[0].mana_pool.red, 3);
    assert!(engine.state.objects[&jewel].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "mana abilities do not use the stack"
    );
}

#[test]
fn old_coveted_jewel_trigger_does_not_change_returned_source_incarnation() {
    let decks = Some(vec![vec!["forest".into(); 30], vec!["island".into(); 30]]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        98495,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    advance_to_declare_attackers(&mut engine);
    let jewel = inject_permanent_on_battlefield(&mut engine, 1, "coveted_jewel");
    engine.state.objects.get_mut(&jewel).unwrap().tapped = true;
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&jewel)
        .copied()
        .unwrap_or(0);
    engine.initial_response_batch();
    let attacker = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "grizzly_bears")
        .expect("injected attacker");
    let attacker_hand_before = engine.state.players[0].hand.len();

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker at player 1");
    engine
        .apply_command(0, &pass())
        .expect("pass attackers step");
    engine
        .apply_command(1, &pass())
        .expect("defending player passes attackers step");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
    assert_eq!(engine.state.stack.len(), 1, "old Jewel trigger is pending");

    engine.enable_dev_commands();
    engine
        .apply_command(
            engine.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Grizzly Bears".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("the attacking creature leaves before the old trigger resolves");
    for zone in [DevZone::Graveyard, DevZone::Battlefield] {
        engine
            .apply_command(
                engine.state.priority_player_id(),
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 1,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: "Coveted Jewel".into(),
                            zone: zone as i32,
                            ready: true,
                        })),
                    })),
                },
            )
            .expect("move the source out and return it while the old trigger is pending");
    }
    assert_ne!(
        engine.state.zone_change_generation[&jewel], original_generation,
        "the returned source has a new incarnation"
    );
    engine.state.objects.get_mut(&jewel).unwrap().tapped = true;
    assert_eq!(
        engine.state.stack.len(),
        2,
        "the return also triggers its ETB"
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        attacker_hand_before + 3,
        "the old attack trigger still draws for the attacking player"
    );
    assert_eq!(
        engine.state.objects[&jewel].controller, 1,
        "the old trigger cannot transfer control of a new source incarnation"
    );
    assert!(
        engine.state.objects[&jewel].tapped,
        "the old trigger cannot untap a new source incarnation"
    );
}

#[test]
fn coveted_jewel_keeps_the_triggering_attacker_after_its_creature_leaves() {
    let decks = Some(vec![vec!["forest".into(); 30], vec!["island".into(); 30]]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        98497,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    advance_to_declare_attackers(&mut engine);
    let jewel = inject_permanent_on_battlefield(&mut engine, 1, "coveted_jewel");
    engine.state.objects.get_mut(&jewel).unwrap().tapped = true;
    engine.initial_response_batch();
    let attacker = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "grizzly_bears")
        .expect("injected attacker");
    let hand_before = engine.state.players[0].hand.len();

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker at player 1");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_command(1, &pass())
        .expect("defending player passes attackers step");
    assert_eq!(engine.state.stack.len(), 1, "Jewel trigger is on the stack");

    engine.enable_dev_commands();
    engine
        .apply_command(
            engine.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Grizzly Bears".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("the attacking creature leaves after the Jewel trigger is stacked");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].hand.len(), hand_before + 3);
    assert_eq!(engine.state.objects[&jewel].controller, 0);
    assert!(
        !engine.state.objects[&jewel].tapped,
        "the event-time attacker receives the control effect and its following untap"
    );
}

#[test]
fn coveted_jewel_multiplayer_attack_group_waits_for_last_defender_and_uses_effective_controller() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        98496,
        &[0, 1, 2],
        20,
        None,
        true,
    )
    .expect("new three-player engine");
    advance_to_main1_from_game_start(&mut engine);
    let jewel = inject_permanent_on_battlefield(&mut engine, 1, "coveted_jewel");
    engine.state.objects.get_mut(&jewel).unwrap().tapped = true;
    engine
        .state
        .continuous_effects
        .push(tricerules_core::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: tricerules_core::AffectedScope::Single(jewel),
            kind: tricerules_cards::ContinuousEffectKind::Layer2Control {
                controller: tricerules_cards::ControllerReference::Fixed(2),
            },
            condition: None,
            duration: tricerules_cards::EffectDuration::Indefinite,
            timestamp: engine.state.command_index,
        });
    assert_eq!(
        engine.characteristics(jewel).unwrap().controller,
        2,
        "Layer 2 gives the Jewel to player 2"
    );

    let attacker_at_player_one = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let attacker_at_player_two = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let _player_one_blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let _player_two_blocker = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");

    engine
        .apply_command(0, &primitive_yield())
        .expect("begin combat");
    for _ in 0..3 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("pass combat start");
    }
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);

    let legal_assignments = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .clone();
    let assignment_for = |attacker, defender| {
        legal_assignments
            .iter()
            .find(|assignment| {
                assignment.attacker_object_id == attacker
                    && assignment.defending_player_id == defender
            })
            .cloned()
            .unwrap_or_else(|| panic!("missing legal attack assignment {attacker} -> {defender}"))
    };
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(
                    tricerules_proto::ruled::v1::DeclareAttackers {
                        assignments: vec![
                            assignment_for(attacker_at_player_one, 1),
                            assignment_for(attacker_at_player_two, 2),
                        ],
                    },
                )),
            },
        )
        .expect("attack separate defenders in one combat group");
    assert!(
        engine.state.stack.is_empty(),
        "the trigger waits for blockers"
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
    assert_eq!(
        engine.state.priority_player_id(),
        1,
        "player 1 declares first"
    );

    let first_defender = engine
        .apply_command(1, &declare_blockers(vec![]))
        .expect("player 1 declares no blockers");
    assert!(
        engine.state.stack.is_empty(),
        "the event waits until player 2, the last defender, declares"
    );
    assert!(first_defender.events.iter().all(|event| !matches!(
        event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::BlockersDeclared(_))
    )));
    assert_eq!(engine.state.priority_player_id(), 2);

    let last_defender = engine
        .apply_command(2, &declare_blockers(vec![]))
        .expect("player 2 makes the final blocker declaration");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one attack group creates exactly one Jewel trigger"
    );
    assert!(last_defender.events.iter().any(|event| matches!(
        event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::BlockersDeclared(_))
    )));
    let trigger = engine.state.stack.last().unwrap();
    assert_eq!(trigger.trigger_context.attacking_player, Some(0));
    assert_eq!(trigger.trigger_context.defending_player, Some(2));
}
