//! Issue #92: look at a bounded library window, optionally choose a matching card for hand, and
//! put the rest on the bottom in either a deterministic random order or a player-chosen order.
use crate::helpers::ScenarioCommandExt;

use crate::helpers::*;
use tricerules_core::{EngineDeck, Zone};
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision};

fn green_deck_with(card: &str) -> Option<Vec<Vec<String>>> {
    Some(vec![
        deck_with("forest", &[card]),
        vec!["island".into(); 20],
    ])
}

/// Put `card_ids` on top, first entry first, without changing zones or generations.
fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect();
    e.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        e.state.players[player].library.push_front(oid);
    }
    oids
}

fn green_mana() -> ManaGift {
    ManaGift {
        g: 1,
        ..Default::default()
    }
}

fn declared_commander_on_battlefield_engine(
    seed: u64,
    spell_id: &str,
    commander_id: &str,
) -> (GameEngine, u32) {
    use tricerules_proto::ruled::v1::{
        dev_command, ruled_command::Cmd, DevCommand, DevMoveCard, DevZone,
    };

    let mut p0_mainboard = vec!["island".into(); 12];
    p0_mainboard.push(spell_id.into());
    let p0_deck = EngineDeck {
        mainboard: p0_mainboard,
        commanders: Vec::new(),
    };
    let p1_deck = EngineDeck {
        mainboard: vec!["island".into(); 40],
        commanders: vec![commander_id.into()],
    };
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![p0_deck, p1_deck]),
        true,
    )
    .expect("declared commander engine starts");
    advance_to_main1_from_game_start(&mut engine);
    let commander = engine.state.players[1].command_zone[0];
    let commander_name = tricerules_cards::registry::global()
        .get(commander_id)
        .expect("commander definition")
        .name
        .clone();
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 1,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: commander_name,
                        zone: DevZone::Battlefield as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the exact declared commander to the battlefield");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Battlefield);
    (engine, commander)
}

fn three_player_return_to_sewers_owner_choice(seed: u64) -> (GameEngine, u32, u32) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("forest", &["return_to_the_sewers"]),
            vec!["island".into(); 20],
            vec!["grizzly_bears".into(); 20],
        ]),
        true,
    )
    .expect("three-player owner-library fixture starts");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "return_to_the_sewers");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let spell_slot = hand_index_for_card(&engine, 0, "return_to_the_sewers");
    engine
        .apply_scenario_command(0, &cast_spell(spell_slot, target_object(target)))
        .expect("P0 casts Return to the Sewers at P2's creature");
    let spell = engine.state.stack.last().expect("spell is on the stack").id;
    (engine, spell, target)
}

fn three_player_commander_return_to_sewers_owner_choice(seed: u64) -> (GameEngine, u32) {
    use tricerules_proto::ruled::v1::{
        dev_command, ruled_command::Cmd, DevCommand, DevMoveCard, DevZone,
    };

    let mut p0_mainboard = vec!["island".into(); 12];
    p0_mainboard.push("return_to_the_sewers".into());
    let decks = vec![
        EngineDeck {
            mainboard: p0_mainboard,
            commanders: Vec::new(),
        },
        EngineDeck {
            mainboard: vec!["island".into(); 40],
            commanders: Vec::new(),
        },
        EngineDeck {
            mainboard: vec!["island".into(); 40],
            commanders: vec!["kami_of_the_crescent_moon".into()],
        },
    ];
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(decks),
        true,
    )
    .expect("three-player declared-commander engine starts");
    advance_to_main1_from_game_start(&mut engine);
    let commander = engine.state.players[2].command_zone[0];
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 2,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Kami of the Crescent Moon".into(),
                        zone: DevZone::Battlefield as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the exact declared commander to the battlefield");
    ensure_in_hand(&mut engine, 0, "return_to_the_sewers");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let spell_slot = hand_index_for_card(&engine, 0, "return_to_the_sewers");
    engine
        .apply_scenario_command(0, &cast_spell(spell_slot, target_object(commander)))
        .expect("P0 casts Return to the Sewers at P2's commander");
    engine.state.stack.last_mut().unwrap().controller = 1;
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    (engine, commander)
}

fn select_owner_library_branch(branch_index: u32) -> RuledCommand {
    use tricerules_proto::ruled::v1::{ruled_command::Cmd, SubmitResolutionChoice};

    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch_index,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_228_sleight_requires_one_private_card() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        22801,
        &[0, 1],
        20,
        green_deck_with("sleight_of_hand"),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "sleight_of_hand");
    let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain"]);
    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "sleight_of_hand",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let choice = find_resolution_choice(&batch).unwrap();
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(e
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .is_err());
    let batch = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .unwrap();
    assert!(!batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(
            _
        ))
    )));
    assert!(e.state.pending_resolution.is_none());
    assert!(e.state.players[0].hand.contains(&top[0]));
    assert_eq!(e.state.players[0].library.back(), Some(&top[1]));
    assert_eq!(e.state.players[0].library.front(), Some(&top[2]));
}

fn issue_228_engine(card: &str) -> GameEngine {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        22802,
        &[0, 1],
        20,
        green_deck_with(card),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, card);
    e
}

fn issue_228_resolve(e: &mut GameEngine, card: &str) -> RuledEventBatch {
    cast_instant_and_resolve(
        e,
        0,
        card,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    )
}

fn assert_no_public_library_names(batch: &RuledEventBatch) {
    use tricerules_proto::ruled::v1::ruled_event::Ev;
    for event in &batch.events {
        match &event.ev {
            Some(Ev::CardsRevealed(_)) => panic!("selection is not a reveal"),
            Some(Ev::Log(log)) if log.visible_to_player_id.is_none() => {
                for name in ["Forest", "Island", "Mountain"] {
                    assert!(!log.text.contains(name), "public name leak: {}", log.text);
                }
            }
            _ => {}
        }
    }
}

/// Synthetic-only fixture for proving a parked program resumes after its private library choice.
/// The life-gain tail is not part of either Oracle owner definition.
fn look_choose_tail_fixture(card_id: &str, card_name: &str, count: u32) -> String {
    format!(
        r#"(
          id: "{card_id}", name: "{card_name}", face_id: "{card_id}",
          mana_cost: "{{0}}", types: ["Instant"],
          spell_program: Some((body: (steps: [
            (id: "private_look", bind: [], input: None,
             kind: Perform(LookChooseToHand(
                count: {count}, filter: None, min: 1, max: 1,
                reveal: false, bottom_order: Chosen,
             ))),
            (id: "tail_life", bind: [], input: None,
             kind: Perform(GainLife(amount: 7))),
          ], yields: []))),
        )"#
    )
}

fn look_choose_tail_engine(seed: u64, card_id: &str) -> GameEngine {
    let three = look_choose_tail_fixture(
        "look_choose_tail_three_fixture",
        "Look Choose Tail Three Fixture",
        3,
    );
    let two = look_choose_tail_fixture(
        "look_choose_tail_two_fixture",
        "Look Choose Tail Two Fixture",
        2,
    );
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                three.as_str(),
                two.as_str(),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/mountain.ron"),
            ],
            &[],
        )
        .expect("the private library-look tail fixtures compile"),
    ));
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &[card_id]),
            vec!["island".into(); 20],
        ]),
        true,
    )
    .expect("the private library-look tail fixture engine starts");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, card_id);
    engine
}

#[test]
fn owner_library_choice_publicly_logs_the_selected_bottom_placement() {
    use tricerules_proto::ruled::v1::{ruled_event::Ev, ResolutionChoiceDecision, TargetRef};

    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        22834,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["misleading_motes"]),
            vec!["grizzly_bears".into(); 20],
        ]),
        true,
    )
    .expect("engine starts");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "misleading_motes");

    let batch = cast_targeted_instant_and_resolve(
        &mut engine,
        0,
        "misleading_motes",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        vec![TargetRef {
            expected_zone_change_generation: None,
            object_id: target,
            damage_amount: 0,
            group_index: 0,
            kind: 0,
        }],
    );
    let choice = find_resolution_choice(&batch).expect("the target owner chooses placement");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.resolution_branches[1].label, "Bottom");

    let mut answer = submit_resolution_choice(vec![]);
    let Some(Cmd::SubmitResolutionChoice(command)) = answer.cmd.as_mut() else {
        unreachable!()
    };
    command.decision = ResolutionChoiceDecision::SelectBranch as i32;
    command.selected_branch_index = 1;
    let result = engine
        .apply_scenario_command(1, &answer)
        .expect("target owner selects bottom");

    assert!(
        result.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log))
                if log.visible_to_player_id.is_none()
                    && log.hidden_from_player_id.is_none()
                    && log.text.contains("on the bottom of its owner's library")
        )),
        "both players need the selected public placement outcome"
    );
    assert_eq!(engine.state.players[1].library.back(), Some(&target));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn declared_commander_destination_precedes_fixed_library_placement() {
    use tricerules_proto::ruled::v1::{permanent_moved::Destination, ruled_event::Ev};

    let (mut engine, commander) =
        declared_commander_on_battlefield_engine(22835, "griptide", "grizzly_bears");
    // Keep the physical commander under the caster's control so the replacement still has to
    // ask its owner, P1.
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != commander);
    engine.state.players[0].battlefield.push(commander);
    let object = engine.state.objects.get_mut(&commander).unwrap();
    object.controller = 0;
    object.base_controller = 0;

    let library_before = engine.state.players[1].library.clone();
    let batch = cast_targeted_instant_and_resolve(
        &mut engine,
        0,
        "griptide",
        ManaGift {
            u: 4,
            ..Default::default()
        },
        target_object(commander),
    );
    let choice = find_resolution_choice(&batch).expect("commander destination choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(choice.resolution_branches[0].label, "Command zone");
    assert_eq!(choice.resolution_branches[1].label, "Library");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[1].library, library_before);
    let moved = engine
        .apply_scenario_command(1, &select_owner_library_branch(1))
        .expect("owner keeps the commander in the library");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
    assert_eq!(engine.state.players[1].library.front(), Some(&commander));
    assert!(!engine.state.players[1].command_zone.contains(&commander));
    assert!(moved.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(movement))
            if movement.object_id == commander && movement.destination == Destination::Library as i32
    )));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn declared_commander_can_choose_command_zone_and_publicly_logs_destination() {
    use tricerules_proto::ruled::v1::{permanent_moved::Destination, ruled_event::Ev};

    let (mut engine, commander) =
        declared_commander_on_battlefield_engine(22836, "griptide", "grizzly_bears");
    let batch = cast_targeted_instant_and_resolve(
        &mut engine,
        0,
        "griptide",
        ManaGift {
            u: 4,
            ..Default::default()
        },
        target_object(commander),
    );
    assert!(find_resolution_choice(&batch).is_some());
    let command = engine
        .apply_scenario_command(1, &select_owner_library_branch(0))
        .expect("owner chooses the command zone");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
    assert!(engine.state.players[1].command_zone.contains(&commander));
    assert!(!engine.state.players[1].library.contains(&commander));
    assert!(command.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(movement))
            if movement.object_id == commander && movement.destination == Destination::Command as i32
    )));
    assert!(command.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log))
            if log.visible_to_player_id.is_none()
                && log.hidden_from_player_id.is_none()
                && log.text.contains("puts Grizzly Bears into the command zone instead")
    )));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn commander_library_choice_precedes_owner_position_choice_and_program_tail() {
    use tricerules_proto::ruled::v1::{ruled_event::Ev, TokenCreated};

    let (mut engine, commander) =
        declared_commander_on_battlefield_engine(22837, "return_to_the_sewers", "grizzly_bears");
    let destination = cast_targeted_instant_and_resolve(
        &mut engine,
        0,
        "return_to_the_sewers",
        ManaGift {
            u: 4,
            ..Default::default()
        },
        target_object(commander),
    );
    assert_eq!(
        find_resolution_choice(&destination)
            .expect("commander destination comes first")
            .deciding_player_id,
        1
    );
    let library_before = engine.state.players[1].library.clone();
    assert!(engine
        .apply_scenario_command(0, &select_owner_library_branch(1))
        .is_err());
    assert_eq!(engine.state.players[1].library, library_before);
    assert_eq!(engine.state.objects[&commander].zone, Zone::Battlefield);
    assert!(engine.state.pending_resolution.is_some());

    let position = engine
        .apply_scenario_command(1, &select_owner_library_branch(1))
        .expect("owner chooses the library destination");
    let choice = find_resolution_choice(&position).expect("owner placement follows destination");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.resolution_branches[0].label, "Top");
    assert_eq!(choice.resolution_branches[1].label, "Bottom");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Battlefield);

    let tail = engine
        .apply_scenario_command(1, &select_owner_library_branch(1))
        .expect("owner chooses bottom");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
    assert_eq!(engine.state.players[1].library.back(), Some(&commander));
    assert!(tail.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::TokenCreated(TokenCreated { card_id, .. })) if card_id == "mutagen"
    )));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn commander_position_choice_survives_source_controller_departure_between_stages() {
    use tricerules_proto::ruled::v1::ruled_event::Ev;

    let (mut engine, commander) = three_player_commander_return_to_sewers_owner_choice(22844);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("commander destination choice is first");
    assert_eq!(pending.deciding_player, 2);
    assert!(matches!(
        &pending.continuation,
        tricerules_core::state::ResolutionContinuation::OwnerLibraryPlacement {
            commander_destination_choice: true,
            ..
        }
    ));

    engine
        .apply_scenario_command(0, &concede())
        .expect("spell owner leaves while the surviving controller's commander choice is pending");
    assert!(engine.state.players[0].has_lost);
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        tricerules_core::state::ResolutionContinuation::OwnerLibraryPlacement {
            stack_source_departed: true,
            ..
        }
    ));

    let position = engine
        .apply_scenario_command(2, &select_owner_library_branch(1))
        .expect("commander owner chooses Library");
    assert_eq!(
        find_resolution_choice(&position)
            .expect("position choice follows Library")
            .deciding_player_id,
        2
    );
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        tricerules_core::state::ResolutionContinuation::OwnerLibraryPlacement {
            stack_source_departed: true,
            ..
        }
    ));

    let completed = engine
        .apply_scenario_command(2, &select_owner_library_branch(1))
        .expect("source departure must remain accepted through the position offer");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
    assert_eq!(engine.state.players[2].library.back(), Some(&commander));
    assert_eq!(
        completed
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "Return to the Sewers resolves."
            ))
            .count(),
        1,
        "the departed controller's Parked Program tail completes exactly once"
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn commander_shuffle_runs_once_after_either_destination_choice() {
    use tricerules_proto::ruled::v1::ruled_event::Ev;

    fn run(seed: u64, destination: u32) -> (Vec<u32>, Vec<u32>, Zone) {
        let (mut engine, commander) =
            declared_commander_on_battlefield_engine(seed, "deglamer", "sol_ring");
        let before: Vec<_> = engine.state.players[1].library.iter().copied().collect();
        assert!(
            before.len() >= 20,
            "the fixture gives Shuffle a real library"
        );
        let offer = cast_targeted_instant_and_resolve(
            &mut engine,
            0,
            "deglamer",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            target_object(commander),
        );
        assert!(find_resolution_choice(&offer).is_some());
        assert_eq!(
            engine.state.players[1]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            before,
            "the prompt itself must not force a shuffle"
        );
        assert!(!offer.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P1 shuffles their library."
        )));
        assert!(
            engine
                .apply_scenario_command(0, &select_owner_library_branch(destination))
                .is_err(),
            "only the commander owner may answer"
        );
        assert_eq!(
            engine.state.players[1]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            before,
            "an illegal answer cannot shuffle or move the target"
        );
        assert_eq!(engine.state.objects[&commander].zone, Zone::Battlefield);
        assert!(engine.state.pending_resolution.is_some());

        let completed = engine
            .apply_scenario_command(1, &select_owner_library_branch(destination))
            .expect("owner chooses the replacement destination");
        let shuffle_count = completed
            .events
            .iter()
            .filter(|event| {
                matches!(
                    &event.ev,
                    Some(Ev::Log(log)) if log.text == "P1 shuffles their library."
                )
            })
            .count();
        assert_eq!(
            shuffle_count, 1,
            "CR 701.24c still requires exactly one Shuffle"
        );
        assert!(
            engine.state.pending_resolution.is_none(),
            "no late choice is offered"
        );
        let after: Vec<_> = engine.state.players[1].library.iter().copied().collect();
        (before, after, engine.state.objects[&commander].zone)
    }

    let (command_before, command_after, command_zone) = run(22838, 0);
    assert_eq!(command_zone, Zone::Command);
    assert_ne!(
        command_after, command_before,
        "the Command replacement still shuffles"
    );
    let (command_before_again, command_after_again, _) = run(22838, 0);
    assert_eq!(command_before_again, command_before);
    assert_eq!(
        command_after_again, command_after,
        "the shuffle order is replay-stable"
    );

    let (library_before, library_after, library_zone) = run(22839, 1);
    assert_eq!(library_zone, Zone::Library);
    assert_ne!(library_after, library_before);
    let (library_before_again, library_after_again, _) = run(22839, 1);
    assert_eq!(library_before_again, library_before);
    assert_eq!(
        library_after_again, library_after,
        "the library branch is replay-stable"
    );
}

#[test]
fn owner_library_choice_survives_spell_owner_and_controller_departure() {
    use tricerules_proto::ruled::v1::ruled_event::Ev;

    for (seed, departed, controller) in [(22841, 0, 1), (22842, 1, 1)] {
        let (mut engine, spell, target) = three_player_return_to_sewers_owner_choice(seed);
        engine.state.stack.last_mut().unwrap().controller = controller;
        pass_priority_round(&mut engine);
        assert!(engine.state.pending_resolution.is_some());
        assert_eq!(engine.state.objects[&target].owner, 2);
        assert!(matches!(
            &engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .continuation,
            tricerules_core::state::ResolutionContinuation::OwnerLibraryPlacement { .. }
        ));

        engine
            .apply_scenario_command(departed, &concede())
            .expect("a participant can leave after the owner-placement operation begins");
        assert!(engine.state.players[departed as usize].has_lost);
        assert!(
            engine.state.pending_resolution.is_some(),
            "departure of P{departed} must preserve the exact live target-owner choice"
        );
        assert!(engine.state.stack.iter().all(|item| item.id != spell));

        let completed = engine
            .apply_scenario_command(2, &select_owner_library_branch(1))
            .expect("the surviving target owner can finish the begun operation");
        assert_eq!(engine.state.objects[&target].zone, Zone::Library);
        assert_eq!(engine.state.players[2].library.back(), Some(&target));
        let token_count = completed
            .events
            .iter()
            .filter(|event| {
                matches!(
                    &event.ev,
                    Some(Ev::TokenCreated(token)) if token.card_id == "mutagen"
                )
            })
            .count();
        assert_eq!(
            token_count,
            usize::from(departed == 0),
            "a surviving controller creates the token; a departed controller has no live recipient"
        );
        assert_eq!(
            completed
                .events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(Ev::Log(log)) if log.text == "Return to the Sewers resolves."
                ))
                .count(),
            1,
            "the Parked Program reaches its end exactly once after source or controller departure"
        );
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn departed_target_owner_retires_owner_library_choice_without_moving_target() {
    use tricerules_proto::ruled::v1::ruled_event::Ev;

    let (mut engine, _spell, target) = three_player_return_to_sewers_owner_choice(22843);
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());

    let departure = engine
        .apply_scenario_command(2, &concede())
        .expect("target owner can leave while the placement prompt is pending");
    assert!(engine.state.players[2].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.players[2].library.contains(&target));
    assert!(!engine.state.objects.contains_key(&target));
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::TokenCreated(token)) if token.card_id == "mutagen"
            ))
            .count(),
        1,
        "the later Program instruction finishes once after the stale operation is retired"
    );
}

#[test]
fn stale_commander_destination_offer_cannot_move_or_shuffle() {
    use tricerules_proto::ruled::v1::ruled_event::Ev;

    let (mut engine, commander) =
        declared_commander_on_battlefield_engine(22840, "deglamer", "sol_ring");
    let library_before = engine.state.players[1].library.clone();
    let offer = cast_targeted_instant_and_resolve(
        &mut engine,
        0,
        "deglamer",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        target_object(commander),
    );
    assert!(find_resolution_choice(&offer).is_some());

    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != commander);
    engine.state.players[1].graveyard.push(commander);
    engine.state.objects.get_mut(&commander).unwrap().zone = Zone::Graveyard;
    let new_generation = engine.state.zone_change_generation[&commander] + 1;
    engine
        .state
        .zone_change_generation
        .insert(commander, new_generation);

    assert!(
        engine
            .apply_scenario_command(1, &select_owner_library_branch(0))
            .is_err(),
        "the old battlefield generation is stale"
    );
    assert_eq!(engine.state.objects[&commander].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].library, library_before);
    assert!(!engine.state.players[1].command_zone.contains(&commander));
    assert!(engine.state.pending_resolution.is_some());
    assert!(!offer.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "P1 shuffles their library."
    )));
}

#[test]
fn issue_92_program_library_look_resumes_tail_after_chosen_bottom_order() {
    let card = "look_choose_tail_three_fixture";
    let mut engine = look_choose_tail_engine(22831, card);
    let top = seat_on_top(&mut engine, 0, &["forest", "island", "mountain"]);
    let life_before = engine.state.players[0].life;
    let batch = cast_instant_and_resolve(&mut engine, 0, card, ManaGift::default());
    let pick = find_resolution_choice(&batch).expect("the private selection prompt is offered");
    assert_eq!((pick.min, pick.max), (1, 1));
    assert_eq!(pick.candidate_object_ids, top);
    assert_no_public_library_names(&batch);
    assert_eq!(engine.state.players[0].life, life_before);

    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("the selected card moves to hand");
    assert_no_public_library_names(&batch);
    let order = find_resolution_choice(&batch).expect("two remaining cards need chosen order");
    assert!(order.ordered);
    assert_eq!((order.min, order.max), (2, 2));
    assert_eq!(order.candidate_object_ids, vec![top[1], top[2]]);
    assert_eq!(engine.state.players[0].life, life_before);

    let bottom_order = vec![top[2], top[1]];
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(bottom_order.clone()))
        .expect("the chosen remainder order completes the library look");
    assert_no_public_library_names(&batch);
    assert!(find_resolution_choice(&batch).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.players[0].hand.contains(&top[0]));
    let library: Vec<_> = engine.state.players[0].library.iter().copied().collect();
    assert_eq!(&library[library.len() - 2..], bottom_order.as_slice());
    assert_eq!(engine.state.players[0].life, life_before + 7);
}

#[test]
fn issue_92_program_library_look_resumes_tail_with_one_card_to_bottom() {
    let card = "look_choose_tail_two_fixture";
    let mut engine = look_choose_tail_engine(22832, card);
    let top = seat_on_top(&mut engine, 0, &["forest", "island"]);
    let life_before = engine.state.players[0].life;
    let batch = cast_instant_and_resolve(&mut engine, 0, card, ManaGift::default());
    let pick = find_resolution_choice(&batch).expect("the private selection prompt is offered");
    assert_eq!((pick.min, pick.max), (1, 1));
    assert_eq!(pick.candidate_object_ids, top);
    assert_no_public_library_names(&batch);
    assert_eq!(engine.state.players[0].life, life_before);

    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("the selected card moves to hand and the lone remainder is bottomed");
    assert_no_public_library_names(&batch);
    assert!(find_resolution_choice(&batch).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.players[0].hand.contains(&top[0]));
    assert_eq!(engine.state.players[0].library.back(), Some(&top[1]));
    assert_eq!(engine.state.players[0].life, life_before + 7);
}

#[test]
fn issue_92_program_library_look_with_empty_library_continues_tail_once() {
    let card = "look_choose_tail_three_fixture";
    let mut engine = look_choose_tail_engine(22833, card);
    engine.state.players[0].library.clear();
    let life_before = engine.state.players[0].life;

    let batch = cast_instant_and_resolve(&mut engine, 0, card, ManaGift::default());

    assert!(find_resolution_choice(&batch).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.players[0].library.is_empty());
    assert!(!engine.state.players[0].has_lost);
    assert_eq!(engine.state.players[0].life, life_before + 7);
}

#[test]
fn issue_228_flow_state_checks_each_graveyard_type_and_orders_the_remaining_cards() {
    for (instant, sorcery) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut e = issue_228_engine("flow_state");
        if instant {
            inject_graveyard_card(&mut e, 0, "lightning_bolt");
        }
        if sorcery {
            inject_graveyard_card(&mut e, 0, "divination");
        }
        // The opponent's types never satisfy "your graveyard".
        inject_graveyard_card(&mut e, 1, "lightning_bolt");
        inject_graveyard_card(&mut e, 1, "divination");
        let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain", "forest"]);
        let batch = issue_228_resolve(&mut e, "flow_state");
        let choice = find_resolution_choice(&batch).unwrap();
        let count = if instant && sorcery { 2 } else { 1 };
        assert_eq!((choice.min, choice.max), (count, count));
        assert_eq!(choice.candidate_object_ids, top[..3]);
        assert_eq!(choice.candidate_selectable, [true; 3]);
        assert_no_public_library_names(&batch);
        let selected = top[..count as usize].to_vec();
        let batch = e
            .apply_scenario_command(0, &submit_resolution_choice(selected.clone()))
            .unwrap();
        assert_no_public_library_names(&batch);
        let remainder: Vec<_> = top[count as usize..3].iter().rev().copied().collect();
        if remainder.len() > 1 {
            let order = find_resolution_choice(&batch).unwrap();
            assert!(order.ordered);
            assert_eq!((order.min, order.max), (2, 2));
            let batch = e
                .apply_scenario_command(0, &submit_resolution_choice(remainder.clone()))
                .unwrap();
            assert_no_public_library_names(&batch);
        }
        assert!(e.state.pending_resolution.is_none());
        assert!(e.state.stack.is_empty());
        for oid in selected {
            assert!(e.state.players[0].hand.contains(&oid));
        }
        let library: Vec<_> = e.state.players[0].library.iter().copied().collect();
        assert_eq!(library[0], top[3]);
        assert_eq!(&library[library.len() - remainder.len()..], remainder);
    }
}

#[test]
fn issue_228_short_libraries_do_as_much_as_possible_without_drawing() {
    for card in ["sleight_of_hand", "flow_state"] {
        for size in 0..=3 {
            let mut e = issue_228_engine(card);
            inject_graveyard_card(&mut e, 0, "lightning_bolt");
            inject_graveyard_card(&mut e, 0, "divination");
            let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain"][..size]);
            // Remove surplus library objects consistently from this focused fixture.
            let surplus: Vec<_> = e.state.players[0]
                .library
                .iter()
                .copied()
                .filter(|oid| !top.contains(oid))
                .collect();
            for oid in surplus {
                e.state.objects.remove(&oid);
            }
            e.state.players[0].library.retain(|oid| top.contains(oid));
            let drawn_before = e.state.turn_history.current.player(0).cards_drawn;
            let batch = issue_228_resolve(&mut e, card);
            assert_no_public_library_names(&batch);
            let desired = if card == "flow_state" { 2 } else { 1 };
            if size == 0 {
                assert!(find_resolution_choice(&batch).is_none());
            } else {
                let choice = find_resolution_choice(&batch).unwrap();
                let required = desired.min(size) as u32;
                assert_eq!((choice.min, choice.max), (required, required));
                let batch = e
                    .apply_scenario_command(
                        0,
                        &submit_resolution_choice(top[..required as usize].to_vec()),
                    )
                    .unwrap();
                assert_no_public_library_names(&batch);
            }
            assert!(e.state.pending_resolution.is_none());
            assert!(e.state.stack.is_empty());
            assert_eq!(
                e.state.turn_history.current.player(0).cards_drawn,
                drawn_before
            );
            assert!(
                e.state.players.iter().all(|player| !player.has_lost),
                "empty library selection does not cause a loss"
            );
        }
    }
}

#[test]
fn issue_228_flow_state_condition_is_evaluated_at_resolution_and_then_locked() {
    let mut e = issue_228_engine("flow_state");
    let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain"]);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&e, 0, "flow_state");
    e.apply_scenario_command(0, &cast_spell(index, vec![]))
        .unwrap();
    inject_graveyard_card(&mut e, 0, "lightning_bolt");
    inject_graveyard_card(&mut e, 0, "divination");
    e.apply_scenario_command(0, &pass()).unwrap();
    let batch = e.apply_scenario_command(1, &pass()).unwrap();
    assert_eq!(find_resolution_choice(&batch).unwrap().min, 2);
    // Simulate changing information after the resolution instruction was already applied.
    e.state.players[0].graveyard.clear();
    e.apply_scenario_command(0, &submit_resolution_choice(top[..2].to_vec()))
        .unwrap();
    assert!(e.state.pending_resolution.is_none());
    assert!(top[..2]
        .iter()
        .all(|oid| e.state.players[0].hand.contains(oid)));
}

#[test]
fn issue_228_illegal_and_stale_choices_are_atomic_in_both_stages() {
    let mut e = issue_228_engine("flow_state");
    let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain"]);
    issue_228_resolve(&mut e, "flow_state");
    let before = serde_json::to_string(&e.state).unwrap();
    assert!(matches!(
        e.apply_scenario_command(1, &submit_resolution_choice(vec![top[0]])),
        Err(tricerules_core::EngineError::Illegal(_))
    ));
    assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
    let mut decline = submit_resolution_choice(vec![]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = &mut decline.cmd else {
        unreachable!()
    };
    answer.decision = tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline as i32;
    let before = serde_json::to_string(&e.state).unwrap();
    assert!(e.apply_scenario_command(0, &decline).is_err());
    assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
    for chosen in [vec![], vec![top[0], top[1]], vec![u32::MAX]] {
        let before = serde_json::to_string(&e.state).unwrap();
        assert!(e
            .apply_scenario_command(0, &submit_resolution_choice(chosen))
            .is_err());
        assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
    }
    // Even an unselected card cannot change generation before the cohort is committed.
    *e.state.zone_change_generation.entry(top[2]).or_default() += 1;
    let before = serde_json::to_string(&e.state).unwrap();
    assert!(e
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .is_err());
    assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
    *e.state.zone_change_generation.get_mut(&top[2]).unwrap() -= 1;
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .unwrap();
    let before = serde_json::to_string(&e.state).unwrap();
    assert!(matches!(
        e.apply_scenario_command(1, &submit_resolution_choice(vec![top[1], top[2]])),
        Err(tricerules_core::EngineError::Illegal(_))
    ));
    assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
    for chosen in [vec![top[1]], vec![top[1], top[1]], vec![top[0], top[2]]] {
        let before = serde_json::to_string(&e.state).unwrap();
        assert!(e
            .apply_scenario_command(0, &submit_resolution_choice(chosen))
            .is_err());
        assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
    }
    *e.state.zone_change_generation.entry(top[2]).or_default() += 1;
    let before = serde_json::to_string(&e.state).unwrap();
    assert!(e
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[1], top[2]]))
        .is_err());
    assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
}

#[test]
fn issue_228_two_card_selection_rejects_duplicates_and_replays_deterministically() {
    fn play() -> serde_json::Value {
        let mut e = issue_228_engine("flow_state");
        inject_graveyard_card(&mut e, 0, "lightning_bolt");
        inject_graveyard_card(&mut e, 0, "divination");
        let top = seat_on_top(&mut e, 0, &["forest", "forest", "forest"]);
        issue_228_resolve(&mut e, "flow_state");
        let before = serde_json::to_value(&e.state).unwrap();
        assert!(e
            .apply_scenario_command(0, &submit_resolution_choice(vec![top[0], top[0]]))
            .is_err());
        assert_eq!(serde_json::to_value(&e.state).unwrap(), before);
        e.apply_scenario_command(0, &submit_resolution_choice(vec![top[2], top[0]]))
            .unwrap();
        assert!(e.state.players[0].hand.contains(&top[2]));
        assert!(e.state.players[0].hand.contains(&top[0]));
        assert_eq!(e.state.players[0].library.back(), Some(&top[1]));
        serde_json::to_value(&e.state).unwrap()
    }
    assert_eq!(play(), play());
}

#[test]
fn commune_uses_images_for_all_looked_cards_then_orders_the_remainder() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        9201,
        &[0, 1],
        20,
        green_deck_with("commune_with_nature"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "commune_with_nature");
    let top = seat_on_top(
        &mut e,
        0,
        &[
            "grizzly_bears",
            "forest",
            "hill_giant",
            "island",
            "mountain",
            "storm_crow",
        ],
    );

    let batch = cast_instant_and_resolve(&mut e, 0, "commune_with_nature", green_mana());
    let choose = find_resolution_choice(&batch).expect("look choice");
    assert!(
        choose.public_reveal.is_none(),
        "looking does not reveal the candidate set"
    );
    assert_eq!(choose.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choose.candidate_object_ids, top[..5]);
    assert_eq!(
        choose.candidate_selectable,
        [true, false, true, false, false],
        "all five images are visible but only creature images are clickable"
    );
    assert_eq!((choose.min, choose.max), (0, 1));
    assert!(!choose.ordered, "the first step chooses at most one card");
    assert!(matches!(
        &e.state
            .pending_resolution
            .as_ref()
            .expect("library-look continuation")
            .continuation,
        ResolutionContinuation::LibraryLook {
            stage: PendingLibraryLookStage::ChooseToHand { .. },
            ..
        }
    ));

    let order_batch = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("choose Grizzly Bears");
    let reveal = order_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal)) => {
                Some(reveal)
            }
            _ => None,
        })
        .expect("the chosen card is publicly revealed before moving to hand");
    assert_eq!(reveal.cards.len(), 1);
    assert_eq!(reveal.cards[0].object_id, top[0]);
    assert_eq!(reveal.cards[0].card_name, "Grizzly Bears");
    assert!(!reveal.reveal_id.is_empty());
    assert_eq!(e.state.objects[&top[0]].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&top[0]));

    let order = find_resolution_choice(&order_batch).expect("bottom ordering choice");
    assert!(
        order.public_reveal.is_none(),
        "unselected cards remain private"
    );
    assert_eq!(order.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(order.candidate_object_ids, top[1..5]);
    assert_eq!(order.candidate_selectable, [true; 4]);
    assert_eq!((order.min, order.max), (4, 4));
    assert!(order.ordered);
    assert!(matches!(
        &e.state
            .pending_resolution
            .as_ref()
            .expect("bottom-order continuation")
            .continuation,
        ResolutionContinuation::LibraryLook {
            stage: PendingLibraryLookStage::OrderBottom,
            ..
        }
    ));

    let submitted = vec![top[4], top[3], top[2], top[1]];
    e.apply_scenario_command(0, &submit_resolution_choice(submitted.clone()))
        .expect("submit bottom order");
    assert!(e.state.pending_resolution.is_none());
    let library: Vec<u32> = e.state.players[0].library.iter().copied().collect();
    assert_eq!(&library[library.len() - submitted.len()..], submitted);
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(top[5]),
        "the first unlooked card becomes the top card"
    );
}

#[test]
fn brightwood_rejects_noncreatures_and_random_order_is_replay_deterministic() {
    fn play() -> Vec<u32> {
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            9202,
            &[0, 1],
            20,
            green_deck_with("brightwood_tracker"),
            true,
        )
        .expect("new");
        advance_to_main1_from_game_start(&mut e);
        let tracker = relocate_to_battlefield(&mut e, 0, "brightwood_tracker", false);
        let top = seat_on_top(
            &mut e,
            0,
            &[
                "forest",
                "grizzly_bears",
                "island",
                "hill_giant",
                "storm_crow",
            ],
        );
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 5,
                g: 1,
                ..Default::default()
            },
        );

        e.apply_scenario_command(0, &activate_ability(tracker, 0, vec![]))
            .expect("activate Tracker");
        assert!(e.state.objects[&tracker].tapped, "the tap cost is paid");
        e.apply_scenario_command(0, &pass())
            .expect("controller pass");
        let batch = e.apply_scenario_command(1, &pass()).expect("opponent pass");
        let choose = find_resolution_choice(&batch).expect("look choice");
        assert_eq!(choose.candidate_object_ids, top[..4]);
        assert_eq!(choose.candidate_selectable, [false, true, false, true]);

        let before: Vec<u32> = e.state.players[0].library.iter().copied().collect();
        assert!(
            e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
                .is_err(),
            "a noncreature image is not a legal submission"
        );
        assert!(e.state.pending_resolution.is_some());
        assert_eq!(
            e.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            before,
            "rejection preserves the pending choice and library"
        );

        e.apply_scenario_command(0, &submit_resolution_choice(vec![top[1]]))
            .expect("choose creature");
        assert!(e.state.pending_resolution.is_none());
        assert!(e.state.players[0].hand.contains(&top[1]));
        let library: Vec<u32> = e.state.players[0].library.iter().copied().collect();
        assert_eq!(library.first().copied(), Some(top[4]));
        assert_eq!(
            library[library.len() - 3..]
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            [top[0], top[2], top[3]].into_iter().collect(),
            "only the unchosen looked-at cohort is randomized onto the bottom"
        );
        library
    }

    assert_eq!(
        play(),
        play(),
        "same seed and command log reproduce the random order"
    );
}
