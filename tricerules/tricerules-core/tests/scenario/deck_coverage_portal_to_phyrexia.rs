//! Actual Portal to Phyrexia: APNAP sacrifice choices and targeted upkeep reanimation.
use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

fn setup(players: &[i32]) -> GameEngine {
    let decks = (0..players.len())
        .map(|seat| {
            if seat == 0 {
                deck_with("mountain", &["portal_to_phyrexia"])
            } else {
                deck_with("mountain", &[])
            }
        })
        .collect();
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        710_241,
        players,
        20,
        Some(decks),
        true,
    )
    .expect("complete Portal to Phyrexia registration");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "portal_to_phyrexia");
    engine
}

fn cast_portal(engine: &mut GameEngine) {
    give_mana(
        engine,
        engine.state.players[0].id,
        ManaGift {
            c: 9,
            ..Default::default()
        },
    );
    let player = engine.state.players[0].id;
    let slot = hand_index_for_card(engine, 0, "portal_to_phyrexia");
    engine
        .apply_command(player, &cast_spell(slot, vec![]))
        .expect("cast Portal to Phyrexia");
}

#[test]
fn portal_etb_collects_apnap_choices_before_one_simultaneous_sacrifice() {
    let players = [4, 9, 27, 51];
    let mut engine = setup(&players);
    let creatures: Vec<_> = (1..players.len())
        .map(|seat| {
            (0..4)
                .map(|_| inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears"))
                .collect::<Vec<_>>()
        })
        .collect();

    cast_portal(&mut engine);
    pass_priority_round(&mut engine); // the permanent spell resolves and the ETB trigger stacks
    pass_priority_round(&mut engine); // the ETB trigger reaches its first opponent choice
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        9
    );

    for (index, player) in [9, 27, 51].into_iter().enumerate() {
        let picks = creatures[index][..3].to_vec();
        let batch = engine
            .apply_command(player, &submit_resolution_choice(picks.clone()))
            .unwrap();
        if index < 2 {
            assert!(creatures
                .iter()
                .flatten()
                .all(|oid| { engine.state.objects[oid].zone == Zone::Battlefield }));
            let next_prompt = batch
                .events
                .iter()
                .find_map(|event| match event.ev.as_ref()? {
                    Ev::ResolutionChoiceRequired(choice) => Some(choice),
                    _ => None,
                })
                .expect("next APNAP chooser receives a public prompt");
            assert_eq!(next_prompt.deciding_player_id, [27, 51][index]);
            assert_eq!(
                next_prompt.accepted_permanent_selections.len(),
                (index + 1) * 3
            );
            for chosen in creatures
                .iter()
                .take(index + 1)
                .flat_map(|group| group.iter().take(3))
            {
                assert!(next_prompt
                    .accepted_permanent_selections
                    .iter()
                    .any(|selection| {
                        selection.object_id == *chosen && selection.zone_change_generation == 0
                    }));
            }
        }
    }

    for chosen in creatures.iter().flat_map(|group| group.iter().take(3)) {
        assert_eq!(engine.state.objects[chosen].zone, Zone::Graveyard);
    }
    for unchosen in creatures.iter().map(|group| group[3]) {
        assert_eq!(engine.state.objects[&unchosen].zone, Zone::Battlefield);
    }
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        ResolutionContinuation::MassSacrificeGraveyardOrder { .. }
    ));
}

#[test]
fn portal_auto_selects_all_available_creatures_when_an_opponent_has_fewer_than_three() {
    let players = [4, 9, 27, 51];
    let mut engine = setup(&players);
    let first = (0..2)
        .map(|_| inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears"))
        .collect::<Vec<_>>();
    let first_noncreatures = [
        inject_permanent_on_battlefield(&mut engine, 1, "sol_ring"),
        inject_permanent_on_battlefield(&mut engine, 1, "mountain"),
    ];
    let second = (0..3)
        .map(|_| inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears"))
        .collect::<Vec<_>>();
    let third = (0..4)
        .map(|_| inject_creature_on_battlefield(&mut engine, 3, "grizzly_bears"))
        .collect::<Vec<_>>();
    cast_portal(&mut engine);
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);

    let prompt = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(prompt.deciding_player, 51);
    let accepted = engine
        .state
        .pending_resolution
        .as_ref()
        .and_then(|pending| match &pending.continuation {
            ResolutionContinuation::PlayerSetSacrifice { sacrifice, .. } => {
                Some(sacrifice.accepted.clone())
            }
            _ => None,
        })
        .expect("Portal's ordered sacrifice continuation");
    assert_eq!(accepted.len(), 5);
    assert!(first.iter().chain(&second).all(|oid| accepted
        .iter()
        .any(|selection| { selection.object_id == *oid })));
    assert!(first_noncreatures
        .iter()
        .all(|oid| !accepted.iter().any(|selection| selection.object_id == *oid)));

    engine
        .apply_command(51, &submit_resolution_choice(third[..3].to_vec()))
        .unwrap();
    for oid in first.iter().chain(&second).chain(&third[..3]) {
        assert_eq!(engine.state.objects[oid].zone, Zone::Graveyard);
    }
    assert_eq!(engine.state.objects[&third[3]].zone, Zone::Battlefield);
    assert!(first_noncreatures
        .iter()
        .all(|oid| engine.state.objects[oid].zone == Zone::Battlefield));
}

#[test]
fn portal_current_chooser_concession_skips_only_that_players_choice() {
    let players = [4, 9, 27, 51];
    let mut engine = setup(&players);
    let creatures: Vec<_> = (1..players.len())
        .map(|seat| {
            (0..4)
                .map(|_| inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears"))
                .collect::<Vec<_>>()
        })
        .collect();
    cast_portal(&mut engine);
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    engine
        .apply_command(9, &submit_resolution_choice(creatures[0][..3].to_vec()))
        .unwrap();

    let conceded = engine.apply_command(27, &concede()).unwrap();
    assert!(engine.state.players[2].has_lost);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        51
    );
    let next_prompt = conceded
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            Ev::ResolutionChoiceRequired(choice) => Some(choice),
            _ => None,
        })
        .expect("the next live opponent receives a prompt");
    assert_eq!(next_prompt.deciding_player_id, 51);
    assert_eq!(next_prompt.accepted_permanent_selections.len(), 3);
    assert!(creatures[0]
        .iter()
        .chain(&creatures[2])
        .all(|oid| engine.state.objects[oid].zone == Zone::Battlefield));
    assert!(creatures[1].iter().all(|oid| engine
        .state
        .objects
        .get(oid)
        .is_none_or(|object| object.zone != Zone::Battlefield)));

    engine
        .apply_command(51, &submit_resolution_choice(creatures[2][..3].to_vec()))
        .unwrap();
    for oid in creatures[0][..3].iter().chain(&creatures[2][..3]) {
        assert_eq!(engine.state.objects[oid].zone, Zone::Graveyard);
    }
    assert_eq!(
        engine.state.objects[&creatures[0][3]].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&creatures[2][3]].zone,
        Zone::Battlefield
    );
}

#[test]
fn portal_effect_continues_after_its_controller_concedes_mid_choice() {
    let players = [4, 9, 27, 51];
    let mut engine = setup(&players);
    let creatures: Vec<_> = (1..players.len())
        .map(|seat| {
            (0..4)
                .map(|_| inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears"))
                .collect::<Vec<_>>()
        })
        .collect();
    cast_portal(&mut engine);
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    engine
        .apply_command(9, &submit_resolution_choice(creatures[0][..3].to_vec()))
        .unwrap();

    let after_departure = engine.apply_command(4, &concede()).unwrap();
    assert!(engine.state.players[0].has_lost);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        27
    );
    let next_prompt = after_departure
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            Ev::ResolutionChoiceRequired(choice) => Some(choice),
            _ => None,
        })
        .expect("resolution remains active under its next live chooser");
    assert_eq!(next_prompt.deciding_player_id, 27);
    assert_eq!(next_prompt.accepted_permanent_selections.len(), 3);
    for (index, player) in [(1, 27), (2, 51)] {
        engine
            .apply_command(
                player,
                &submit_resolution_choice(creatures[index][..3].to_vec()),
            )
            .unwrap();
    }

    for group in &creatures {
        for chosen in group.iter().take(3) {
            assert_eq!(engine.state.objects[chosen].zone, Zone::Graveyard);
        }
    }
    assert!(engine.state.pending_resolution.is_some());
}

#[test]
fn portal_sacrifice_choices_resume_after_concession_triggers_an_aura_return_choice() {
    use tricerules_core::state::{
        ActiveEventObserver, EventObserverMatcher, EventObserverPayload, TriggerObjectRef,
    };

    let players = [4, 9, 27, 51];
    let mut engine = setup(&players);
    let creatures: Vec<_> = (1..players.len())
        .map(|seat| {
            (0..4)
                .map(|_| inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears"))
                .collect::<Vec<_>>()
        })
        .collect();
    let light = inject_permanent_on_battlefield(&mut engine, 2, "banishing_light");
    let aura = inject_card_into_hand(&mut engine, 1, "pacifism");
    engine.state.players[1]
        .hand
        .retain(|object_id| *object_id != aura);
    engine.state.players[1].exile.push(aura);
    engine.state.objects.get_mut(&aura).unwrap().zone = Zone::Exile;
    engine
        .state
        .active_event_observers
        .push(ActiveEventObserver {
            watched: Some(TriggerObjectRef {
                object_id: light,
                zone_change_generation: 0,
                controller_at_event: 27,
            }),
            matcher: EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
            payload: EventObserverPayload::ReturnExiledObject {
                exiled: TriggerObjectRef {
                    object_id: aura,
                    zone_change_generation: 0,
                    controller_at_event: 9,
                },
            },
        });

    cast_portal(&mut engine);
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    engine
        .apply_command(9, &submit_resolution_choice(creatures[0][..3].to_vec()))
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        27
    );

    let departure = engine.apply_command(27, &concede()).unwrap();
    let aura_prompt = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(aura_prompt.deciding_player, 9);
    assert_eq!(
        aura_prompt.presentation.choice_kind,
        rv1::ChoiceKind::AuraPermanent
    );
    assert!(departure.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == rv1::ChoiceKind::AuraPermanent as i32
    )));

    engine
        .apply_command(9, &submit_resolution_choice(vec![creatures[2][0]]))
        .unwrap();
    let resumed = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(resumed.deciding_player, 51);
    assert_eq!(
        resumed.presentation.choice_kind,
        rv1::ChoiceKind::PermanentObjects
    );
    let accepted = match &resumed.continuation {
        ResolutionContinuation::PlayerSetSacrifice { sacrifice, .. } => &sacrifice.accepted,
        _ => panic!("Portal's sacrifice continuation must survive the Aura choice"),
    };
    assert_eq!(accepted.len(), 3);
    assert!(creatures[0][..3].iter().all(|object_id| accepted
        .iter()
        .any(|selection| selection.object_id == *object_id)));
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(
            creatures[2][0]
        ))
    );
    assert!(creatures[0][..3]
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Battlefield));
}

#[test]
fn portal_sacrifice_choices_resume_after_concession_return_batch_timestamp_order() {
    use tricerules_core::state::{
        ActiveEventObserver, EventObserverMatcher, EventObserverPayload, TriggerObjectRef,
    };

    let players = [4, 9, 27, 51];
    let mut engine = setup(&players);
    let creatures: Vec<_> = (1..players.len())
        .map(|seat| {
            (0..4)
                .map(|_| inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears"))
                .collect::<Vec<_>>()
        })
        .collect();
    let lights: Vec<_> = (0..2)
        .map(|_| inject_permanent_on_battlefield(&mut engine, 2, "banishing_light"))
        .collect();
    let returned: Vec<_> = ["grizzly_bears", "mind_stone"]
        .into_iter()
        .map(|card_id| inject_card_into_hand(&mut engine, 1, card_id))
        .collect();
    for (light, exiled) in lights.into_iter().zip(returned.iter().copied()) {
        engine.state.players[1]
            .hand
            .retain(|object_id| *object_id != exiled);
        engine.state.players[1].exile.push(exiled);
        engine.state.objects.get_mut(&exiled).unwrap().zone = Zone::Exile;
        engine
            .state
            .active_event_observers
            .push(ActiveEventObserver {
                watched: Some(TriggerObjectRef {
                    object_id: light,
                    zone_change_generation: 0,
                    controller_at_event: 27,
                }),
                matcher: EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
                payload: EventObserverPayload::ReturnExiledObject {
                    exiled: TriggerObjectRef {
                        object_id: exiled,
                        zone_change_generation: 0,
                        controller_at_event: 9,
                    },
                },
            });
    }

    cast_portal(&mut engine);
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    engine
        .apply_command(9, &submit_resolution_choice(creatures[0][..3].to_vec()))
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        27
    );

    engine.apply_command(27, &concede()).unwrap();
    let order_prompt = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        order_prompt.presentation.choice_kind,
        rv1::ChoiceKind::SimultaneousEntryOrder
    );
    assert_eq!(order_prompt.deciding_player, 9);
    assert_eq!(order_prompt.presentation.candidates.len(), 2);
    engine
        .apply_command(
            9,
            &submit_resolution_choice(order_prompt.presentation.candidates.clone()),
        )
        .unwrap();

    let resumed = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(resumed.deciding_player, 51);
    let accepted = match &resumed.continuation {
        ResolutionContinuation::PlayerSetSacrifice { sacrifice, .. } => &sacrifice.accepted,
        _ => panic!("Portal's sacrifice continuation must survive timestamp ordering"),
    };
    assert_eq!(accepted.len(), 3);
    assert!(creatures[0][..3].iter().all(|object_id| accepted
        .iter()
        .any(|selection| selection.object_id == *object_id)));

    engine
        .apply_command(51, &submit_resolution_choice(creatures[2][..3].to_vec()))
        .unwrap();
    for chosen in creatures[0][..3].iter().chain(&creatures[2][..3]) {
        assert_eq!(engine.state.objects[chosen].zone, Zone::Graveyard);
    }
    for unchosen in [creatures[0][3], creatures[2][3]] {
        assert_eq!(engine.state.objects[&unchosen].zone, Zone::Battlefield);
    }
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| &pending.continuation),
        Some(ResolutionContinuation::MassSacrificeGraveyardOrder { .. })
    ));
}

#[test]
fn portal_upkeep_reanimates_a_target_from_an_opponents_graveyard_as_phyrexian() {
    let mut engine = setup(&[4, 9]);
    let target = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    cast_portal(&mut engine);
    pass_priority_round(&mut engine); // resolve Portal
    pass_priority_round(&mut engine); // resolve its empty ETB sacrifice instruction
    assert!(engine.state.stack.is_empty());

    end_active_turn(&mut engine, 4);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 9);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert_eq!(engine.state.pending_triggers.front().unwrap().controller, 4);

    let generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or_default();
    engine
        .apply_command(
            4,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(rv1::ChooseTriggerTarget {
                    targets: vec![TargetRef {
                        object_id: target,
                        kind: rv1::TargetRefKind::Graveyard as i32,
                        expected_zone_change_generation: Some(generation),
                        ..Default::default()
                    }],
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].owner, 9);
    assert_eq!(engine.state.objects[&target].controller, 4);
    assert_eq!(engine.state.zone_change_generation[&target], generation + 1);
    let characteristics = engine.characteristics(target).unwrap();
    assert!(characteristics.is_creature());
    assert!(characteristics.has_type("Phyrexian"));
}
