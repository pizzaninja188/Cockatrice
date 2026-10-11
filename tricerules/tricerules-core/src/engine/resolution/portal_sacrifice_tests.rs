//! Focused Portal test fixtures for the ordered, simultaneous sacrifice choice.
use super::*;

const PORTAL: &str = include_str!("../../../../tricerules-cards/data/portal_to_phyrexia.ron");

fn fresh() -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
                PORTAL,
            ],
            &[],
        )
        .unwrap(),
    ));
    let deck = EngineDeck {
        mainboard: vec!["mountain".into(); 20],
        ..Default::default()
    };
    let mut engine = GameEngine::new_with_registry(
        670_121,
        &[4, 9, 27, 51],
        20,
        Some(vec![deck; 4]),
        true,
        registry,
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine
}

fn place(engine: &mut GameEngine, seat: usize, card: &str, zone: Zone) -> ObjectId {
    let oid = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let player = engine.state.players[seat].id;
    let object = new_object_from_card(
        oid,
        player,
        card,
        zone,
        engine.registry.get(card).unwrap().primary_face(),
    );
    engine.state.objects.insert(oid, object);
    engine.state.zone_change_generation.insert(oid, 0);
    match zone {
        Zone::Hand => engine.state.players[seat].hand.push(oid),
        Zone::Battlefield => {
            engine.state.players[seat].battlefield.push(oid);
            engine.emit_static_abilities_on_enter(oid);
        }
        Zone::Graveyard => engine.state.players[seat].graveyard.push(oid),
        _ => unreachable!(),
    }
    oid
}

fn answer(ids: Vec<ObjectId>) -> RuledCommand {
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                chosen_object_ids: ids,
                decision: rv1::ResolutionChoiceDecision::Unspecified as i32,
                ..Default::default()
            },
        )),
    }
}

fn resolve_portal_to_first_choice(engine: &mut GameEngine) -> Vec<rv1::RuledEvent> {
    let portal = place(engine, 0, "portal_to_phyrexia", Zone::Hand);
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == portal)
        .unwrap();
    engine.state.players[0].mana_pool.colorless = 9;
    engine
        .apply_command(
            4,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        location: Some(rv1::cast_source::Location::HandIndex(slot as u32)),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    let mut events = Vec::new();
    for _ in 0..12 {
        if engine.state.pending_resolution.is_some() {
            return events;
        }
        let batch = engine
            .apply_command(
                engine.state.priority_player_id(),
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(Default::default())),
                },
            )
            .unwrap();
        events.extend(batch.events);
    }
    panic!("Portal did not reach its ETB sacrifice choice");
}

fn target_portal_upkeep_trigger(engine: &mut GameEngine, target: ObjectId) {
    let active_player = engine.state.active_player_id();
    engine.state.turn_step = TurnStep::Upkeep;
    let mut events = Vec::new();
    engine.fire_triggers(
        &[GameEvent::PhaseBegan {
            phase: rv1::PhaseId::Upkeep,
            active_player,
        }],
        &mut events,
    );
    engine.flush_staged_triggers(&mut events);
    assert_eq!(
        engine.state.pending_triggers.front().unwrap().controller,
        active_player
    );
    let generation = engine.state.zone_change_generation[&target];
    engine
        .apply_command(
            active_player,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ChooseTriggerTarget(
                    rv1::ChooseTriggerTarget {
                        targets: vec![rv1::TargetRef {
                            object_id: target,
                            kind: rv1::TargetRefKind::Graveyard as i32,
                            expected_zone_change_generation: Some(generation),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
}

#[test]
fn portal_upkeep_returns_an_opponents_creature_as_phyrexian_until_it_changes_zones() {
    let mut engine = fresh();
    let _portal = place(&mut engine, 0, "portal_to_phyrexia", Zone::Battlefield);
    let target = place(&mut engine, 1, "grizzly_bears", Zone::Graveyard);

    target_portal_upkeep_trigger(&mut engine, target);
    let pass = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::PassPriority(Default::default())),
    };
    for _ in 0..12 {
        if engine.state.objects[&target].zone == Zone::Battlefield {
            break;
        }
        let player = engine.state.priority_player_id();
        engine.apply_command(player, &pass).unwrap();
    }

    let object = &engine.state.objects[&target];
    assert_eq!(object.zone, Zone::Battlefield);
    assert_eq!(object.owner, 9);
    assert_eq!(object.controller, 4);
    assert_eq!(engine.state.zone_change_generation[&target], 1);
    assert!(engine.characteristics(target).unwrap().is_creature());
    assert!(engine
        .characteristics(target)
        .unwrap()
        .has_type("Phyrexian"));

    super::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        target,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    assert!(!engine
        .characteristics(target)
        .unwrap()
        .has_type("Phyrexian"));
}

#[test]
fn portal_collects_exact_apnap_choices_before_one_simultaneous_sacrifice() {
    let mut engine = fresh();
    let opponent_one = (0..4)
        .map(|_| place(&mut engine, 1, "grizzly_bears", Zone::Battlefield))
        .collect::<Vec<_>>();
    let opponent_two = (0..4)
        .map(|_| place(&mut engine, 2, "grizzly_bears", Zone::Battlefield))
        .collect::<Vec<_>>();
    let opponent_three = (0..4)
        .map(|_| place(&mut engine, 3, "grizzly_bears", Zone::Battlefield))
        .collect::<Vec<_>>();
    resolve_portal_to_first_choice(&mut engine);

    let first = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(first.deciding_player, 9);
    assert_eq!(first.presentation.candidates.len(), 4);
    let first_picks = opponent_one[..3].to_vec();
    let accepted = engine
        .apply_command(9, &answer(first_picks.clone()))
        .unwrap();
    let second = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(second.deciding_player, 27);
    assert!(matches!(
        second.continuation,
        ResolutionContinuation::PlayerSetSacrifice { .. }
    ));
    assert!(opponent_one
        .iter()
        .chain(&opponent_two)
        .chain(&opponent_three)
        .all(|oid| { engine.state.objects[oid].zone == Zone::Battlefield }));
    let prompt = accepted
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::ResolutionChoiceRequired(choice) => Some(choice),
            _ => None,
        })
        .expect("next APNAP prompt is public");
    assert_eq!(prompt.accepted_permanent_selections.len(), 3);
    for pick in &first_picks {
        assert!(prompt
            .accepted_permanent_selections
            .iter()
            .any(|selection| {
                selection.selecting_player_id == 9
                    && selection.object_id == *pick
                    && selection.zone_change_generation == 0
            }));
    }

    let second_picks = opponent_two[..3].to_vec();
    let third_prompt = engine
        .apply_command(27, &answer(second_picks.clone()))
        .unwrap();
    let third = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(third.deciding_player, 51);
    let prompt = third_prompt
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::ResolutionChoiceRequired(choice) => Some(choice),
            _ => None,
        })
        .expect("third APNAP prompt is public");
    assert_eq!(prompt.accepted_permanent_selections.len(), 6);
    assert!(first_picks.iter().chain(&second_picks).all(|pick| {
        prompt
            .accepted_permanent_selections
            .iter()
            .any(|selection| selection.object_id == *pick && selection.zone_change_generation == 0)
    }));

    let third_picks = opponent_three[..3].to_vec();
    engine
        .apply_command(51, &answer(third_picks.clone()))
        .unwrap();
    for pick in first_picks.iter().chain(&second_picks).chain(&third_picks) {
        assert_eq!(engine.state.objects[pick].zone, Zone::Graveyard);
    }
    assert_eq!(
        engine.state.objects[&opponent_one[3]].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&opponent_two[3]].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&opponent_three[3]].zone,
        Zone::Battlefield
    );
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
fn portal_rejects_stale_exact_object_choice_without_consuming_the_prompt() {
    let mut engine = fresh();
    let candidates = (0..4)
        .map(|_| place(&mut engine, 1, "grizzly_bears", Zone::Battlefield))
        .collect::<Vec<_>>();
    place(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
    place(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
    place(&mut engine, 3, "grizzly_bears", Zone::Battlefield);
    place(&mut engine, 3, "grizzly_bears", Zone::Battlefield);
    resolve_portal_to_first_choice(&mut engine);
    engine.state.zone_change_generation.insert(candidates[0], 1);
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(9, &answer(candidates[..3].to_vec()))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        9
    );
}
