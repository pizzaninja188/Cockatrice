//! Actual-card coverage for Molten Echoes' linked creature-type choice and token copy.

use super::helpers::*;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ChoiceKind, DevCommand, DevMoveCard, DevZone, ResolutionChoiceDecision,
};

fn choose_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn pass_until_resolution_choice(engine: &mut GameEngine) -> RuledEventBatch {
    for _ in 0..8 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("the current priority holder can pass");
        if find_resolution_choice(&batch).is_some() {
            return batch;
        }
    }
    panic!("a resolution choice should be offered before eight priority passes");
}

fn move_card_to_zone(
    engine: &mut GameEngine,
    player_id: i32,
    card_name: &str,
    zone: DevZone,
) -> RuledEventBatch {
    engine.enable_dev_commands();
    engine
        .apply_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move a card through the logged dev command")
}

fn add_clone_as_enters_copy_values(engine: &mut GameEngine, object_id: u32) {
    let registry = tricerules_cards::registry::global();
    let mut face = registry
        .get("grizzly_bears")
        .expect("Grizzly Bears definition")
        .primary_face()
        .clone();
    face.static_abilities = registry
        .get("clone")
        .expect("Clone definition")
        .primary_face()
        .static_abilities
        .clone();
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("the live Grizzly Bears")
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        source_card_id: "grizzly_bears".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
}

#[test]
fn molten_echoes_reissues_entry_choice_after_unrelated_player_concedes() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_902,
        &[4, 9, 15],
        20,
        None,
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);

    let molten_echoes = inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast Molten Echoes");
    pass_priority_round(&mut engine);

    let pending_before = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Molten Echoes is waiting for its controller's creature type");
    assert_eq!(pending_before.deciding_player, 4);
    assert_eq!(pending_before.presentation.source_object_id, molten_echoes);
    let (key_before, entering_generation_before) = match &pending_before.continuation {
        tricerules_core::state::ResolutionContinuation::EntryChooseCreatureType {
            key,
            entering_generation,
            ..
        } => (key.clone(), *entering_generation),
        other => panic!("expected creature-type entry continuation, got {other:?}"),
    };

    let concession = engine
        .apply_command(9, &concede())
        .expect("an unrelated opponent can concede while the choice is pending");
    let restored_choice = find_resolution_choice(&concession)
        .expect("the concession batch reprojects the surviving mandatory choice");
    assert_eq!(restored_choice.deciding_player_id, 4);
    assert_eq!(restored_choice.source_object_id, molten_echoes);
    assert_eq!(restored_choice.resolution_branches.len(), 324);

    let pending_after = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the original mandatory choice remains pending");
    assert_eq!(pending_after.deciding_player, 4);
    assert_eq!(pending_after.presentation.source_object_id, molten_echoes);
    match &pending_after.continuation {
        tricerules_core::state::ResolutionContinuation::EntryChooseCreatureType {
            key,
            entering_generation,
            ..
        } => {
            assert_eq!(key, &key_before, "the choice occurrence key is unchanged");
            assert_eq!(
                *entering_generation, entering_generation_before,
                "the entrant's zone-change generation is unchanged"
            );
        }
        other => panic!("expected creature-type entry continuation, got {other:?}"),
    }
}

#[test]
fn molten_echoes_entry_choice_cleans_up_when_its_chooser_concedes() {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_908,
        &[4, 9, 15],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);

    let molten_echoes = inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let molten_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(molten_index, vec![]))
        .expect("cast Molten Echoes");
    let choice_batch = pass_until_resolution_choice(&mut engine);
    let choice = find_resolution_choice(&choice_batch).expect("the chooser receives the prompt");
    assert_eq!(choice.deciding_player_id, 4);
    assert_eq!(choice.source_object_id, molten_echoes);

    let concession = engine
        .apply_command(4, &concede())
        .expect("the pending chooser can concede");
    assert!(find_resolution_choice(&concession).is_none());
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.players[0].battlefield.contains(&molten_echoes));
    assert!(engine.state.players[1..]
        .iter()
        .any(|player| !player.has_lost));
}

#[test]
fn owner_departure_cancels_a_molten_entry_choice_and_rejects_its_stale_response() {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_909,
        &[4, 9, 15],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);

    let molten_echoes = inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let molten_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(molten_index, vec![]))
        .expect("cast Molten Echoes");
    let type_choice = pass_until_resolution_choice(&mut engine);
    let bear = find_resolution_choice(&type_choice)
        .unwrap()
        .resolution_branches
        .into_iter()
        .find(|branch| branch.label == "Bear")
        .expect("Bear is a canonical creature type");
    engine
        .apply_command(4, &choose_branch(bear.branch_index))
        .expect("choose Bear");

    let clever = inject_graveyard_card(&mut engine, 1, "clever_impersonator");
    let reanimate = inject_card_into_hand(&mut engine, 0, "reanimate");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let reanimate_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == reanimate)
        .expect("Reanimate is in hand");
    engine
        .apply_command(4, &cast_spell(reanimate_index, target_object(clever)))
        .expect("cast Reanimate on the opponent-owned Clever Impersonator");
    let copy_source_choice = pass_until_resolution_choice(&mut engine);
    let pending_copy_source = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the reanimated Clever Impersonator chooses a copy source");
    assert_eq!(
        pending_copy_source.deciding_player, 4,
        "an as-enters choice belongs to the would-be controller, not the owner"
    );
    assert_eq!(
        pending_copy_source.presentation.choice_kind,
        ChoiceKind::CopySource
    );
    assert!(pending_copy_source
        .presentation
        .candidates
        .contains(&molten_echoes));
    let copied_echoes_entry = engine
        .apply_command(4, &submit_resolution_choice(vec![molten_echoes]))
        .expect("copy the controller's Molten Echoes");
    let type_choice = find_resolution_choice(&copied_echoes_entry)
        .expect("the copied Molten Echoes ability asks the surviving controller for a type");
    assert_eq!(type_choice.source_object_id, clever);
    assert_eq!(type_choice.deciding_player_id, 4);
    assert_eq!(type_choice.choice_kind(), ChoiceKind::ResolutionBranch);

    let owner_concession = engine
        .apply_command(9, &concede())
        .expect("the owner can leave while the controller's type choice is pending");
    assert!(find_resolution_choice(&owner_concession).is_none());
    assert!(engine.state.players[1].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.players[0].battlefield.contains(&clever));
    assert!(engine.state.players[0].battlefield.contains(&molten_echoes));
    assert_eq!(engine.state.players[0].life, 16);
    assert!(engine.state.stack.is_empty());
    assert!(
        engine.apply_command(4, &choose_branch(0)).is_err(),
        "the departed entrant's old type response is stale"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.objects.contains_key(&reanimate));
    assert!(copy_source_choice.events.iter().any(|event| {
        matches!(
            event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )
    }));
}

#[test]
fn reanimated_copy_choice_log_names_the_would_be_controller() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_910,
        &[4, 9],
        20,
        None,
        true,
    )
    .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);

    let molten_echoes = inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let molten_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(molten_index, vec![]))
        .expect("cast Molten Echoes");
    let type_choice = pass_until_resolution_choice(&mut engine);
    let bear = find_resolution_choice(&type_choice)
        .unwrap()
        .resolution_branches
        .into_iter()
        .find(|branch| branch.label == "Bear")
        .expect("Bear is a canonical creature type");
    engine
        .apply_command(4, &choose_branch(bear.branch_index))
        .expect("choose Bear");

    let clever = inject_graveyard_card(&mut engine, 1, "clever_impersonator");
    let reanimate = inject_card_into_hand(&mut engine, 0, "reanimate");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let reanimate_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == reanimate)
        .expect("Reanimate is in hand");
    engine
        .apply_command(4, &cast_spell(reanimate_index, target_object(clever)))
        .expect("reanimate the opponent-owned Clever Impersonator");
    let _copy_source_choice = pass_until_resolution_choice(&mut engine);
    let copied_echoes_entry = engine
        .apply_command(4, &submit_resolution_choice(vec![molten_echoes]))
        .expect("copy the controller's Molten Echoes");
    let type_choice = find_resolution_choice(&copied_echoes_entry)
        .expect("the copied as-enters ability asks for a creature type");
    assert_eq!(type_choice.deciding_player_id, 4);
    let bear = type_choice
        .resolution_branches
        .iter()
        .find(|branch| branch.label == "Bear")
        .expect("Bear remains a legal type");
    let chosen = engine
        .apply_command(4, &choose_branch(bear.branch_index))
        .expect("the would-be controller chooses the copied ability's type");
    let choice_logs = chosen
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log)) => Some(log.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        choice_logs.contains(&"P4 chooses Bear for this permanent."),
        "choice log should name the entering permanent's controller; logs: {choice_logs:?}"
    );
    assert!(engine.state.players[0].battlefield.contains(&clever));
}

#[test]
fn copied_molten_echoes_gets_an_independent_creature_type_choice() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_903,
        &[4, 9],
        20,
        None,
        true,
    )
    .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);

    let molten_echoes = inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let molten_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(molten_index, vec![]))
        .expect("cast Molten Echoes");
    let original_choice = pass_until_resolution_choice(&mut engine);
    let original_type_choice = find_resolution_choice(&original_choice).unwrap();
    let bear = original_type_choice
        .resolution_branches
        .iter()
        .find(|branch| branch.label == "Bear")
        .expect("Bear is a canonical creature type");
    engine
        .apply_command(4, &choose_branch(bear.branch_index))
        .expect("the original Molten Echoes chooses Bear");

    let mirror = inject_card_into_hand(&mut engine, 0, "mirrormade");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let mirror_index = hand_index_for_card(&engine, 0, "mirrormade");
    engine
        .apply_command(4, &cast_spell(mirror_index, vec![]))
        .expect("cast Mirrormade");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Mirrormade must choose its copy source")
            .presentation
            .choice_kind,
        ChoiceKind::CopySource
    );
    let copy_source = engine
        .apply_command(4, &submit_resolution_choice(vec![molten_echoes]))
        .expect("choose the original Molten Echoes as the copy source");
    let copied_type_choice = find_resolution_choice(&copy_source)
        .expect("the copied as-enters ability asks for its own type");
    assert_eq!(copied_type_choice.source_object_id, mirror);
    assert_eq!(copied_type_choice.deciding_player_id, 4);
    assert_eq!(copied_type_choice.resolution_branches.len(), 324);
    let elf = copied_type_choice
        .resolution_branches
        .iter()
        .find(|branch| branch.label == "Elf")
        .expect("Elf is a canonical creature type");
    engine
        .apply_command(4, &choose_branch(elf.branch_index))
        .expect("the copied Molten Echoes chooses Elf independently");
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 0, molten_echoes)
            .iter()
            .any(|label| label == "Chosen creature type: Bear")
    );
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, mirror)
        .iter()
        .any(|label| label == "Chosen creature type: Elf"));

    let bear_creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let bear_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == bear_creature)
        .expect("the physical Bear is in hand");
    engine
        .apply_command(4, &cast_spell(bear_index, vec![]))
        .expect("cast Grizzly Bears");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0]
            .trigger_context
            .observed_object
            .expect("the original source captured the matching entry")
            .object_id,
        bear_creature
    );
    pass_priority_round(&mut engine);

    let elf_creature = inject_card_into_hand(&mut engine, 0, "elvish_mystic");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let elf_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == elf_creature)
        .expect("the Elf is in hand");
    engine
        .apply_command(4, &cast_spell(elf_index, vec![]))
        .expect("cast Elvish Mystic");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0]
            .trigger_context
            .observed_object
            .expect("only the Elf-designated copy observes this entry")
            .object_id,
        elf_creature
    );
}

#[test]
fn molten_echoes_trigger_keeps_event_choice_and_entrant_lki_across_source_reentry() {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_904,
        &[4, 9],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);

    let molten_echoes = inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let molten_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(molten_index, vec![]))
        .expect("cast Molten Echoes");
    let type_choice = pass_until_resolution_choice(&mut engine);
    let bear = find_resolution_choice(&type_choice)
        .unwrap()
        .resolution_branches
        .into_iter()
        .find(|branch| branch.label == "Bear")
        .expect("Bear is a canonical creature type");
    engine
        .apply_command(4, &choose_branch(bear.branch_index))
        .expect("choose Bear for the original source incarnation");
    let original_source_generation = engine.state.zone_change_generation[&molten_echoes];

    let entrant = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let entrant_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == entrant)
        .expect("the physical Grizzly Bears is in hand");
    engine
        .apply_command(4, &cast_spell(entrant_index, vec![]))
        .expect("cast Grizzly Bears");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the Bear trigger is on the stack"
    );
    assert_eq!(
        engine.state.stack[0]
            .trigger_context
            .observed_object
            .expect("the trigger retains its exact entrant")
            .object_id,
        entrant
    );

    move_card_to_zone(&mut engine, 4, "Molten Echoes", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&molten_echoes].zone, Zone::Graveyard);
    let reentry_choice = move_card_to_zone(&mut engine, 4, "Molten Echoes", DevZone::Battlefield);
    let reentered_source_choice = find_resolution_choice(&reentry_choice)
        .expect("the re-entering source asks for a fresh type");
    assert_eq!(reentered_source_choice.source_object_id, molten_echoes);
    let elf = reentered_source_choice
        .resolution_branches
        .iter()
        .find(|branch| branch.label == "Elf")
        .expect("Elf is a canonical creature type");
    engine
        .apply_command(4, &choose_branch(elf.branch_index))
        .expect("choose Elf for the new source incarnation");
    assert!(engine.state.zone_change_generation[&molten_echoes] > original_source_generation);
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 0, molten_echoes)
            .iter()
            .any(|label| label == "Chosen creature type: Elf")
    );

    move_card_to_zone(&mut engine, 4, "Grizzly Bears", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Graveyard);
    pass_priority_round(&mut engine);

    let tokens = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| engine.state.objects[object_id].token_origin.is_some())
        .collect::<Vec<_>>();
    assert_eq!(
        tokens.len(),
        1,
        "the old Bear trigger resolves from its snapshot"
    );
    assert_eq!(
        engine.state.objects[&tokens[0]]
            .token_origin
            .as_ref()
            .expect("the result is a copy token")
            .source_card_id,
        "grizzly_bears",
        "the trigger uses the entrant's last-known copiable values"
    );
    assert_eq!(engine.state.objects[&molten_echoes].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn molten_echoes_observes_each_matching_simultaneous_entry() {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_905,
        &[4, 9],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);

    let molten_echoes = inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let molten_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(molten_index, vec![]))
        .expect("cast Molten Echoes");
    let type_choice = pass_until_resolution_choice(&mut engine);
    let myr = find_resolution_choice(&type_choice)
        .unwrap()
        .resolution_branches
        .into_iter()
        .find(|branch| branch.label == "Myr")
        .expect("Myr is a canonical creature type");
    engine
        .apply_command(4, &choose_branch(myr.branch_index))
        .expect("choose Myr");

    let entrants = [
        inject_graveyard_card(&mut engine, 0, "darksteel_myr"),
        inject_graveyard_card(&mut engine, 0, "iron_myr"),
    ];
    let scrap_mastery = inject_card_into_hand(&mut engine, 0, "scrap_mastery");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 3,
            ..Default::default()
        },
    );
    let spell_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == scrap_mastery)
        .expect("Scrap Mastery is in hand");
    engine
        .apply_command(4, &cast_spell(spell_index, vec![]))
        .expect("cast Scrap Mastery");
    pass_priority_round(&mut engine);
    answer_simultaneous_entry_order_in_engine_order(&mut engine);
    answer_trigger_order_in_engine_order(&mut engine);

    for entrant in entrants {
        assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    }
    assert_eq!(engine.state.stack.len(), 2);
    let observed: std::collections::HashSet<_> = engine
        .state
        .stack
        .iter()
        .filter_map(|item| {
            item.trigger_context
                .observed_object
                .map(|object| object.object_id)
        })
        .collect();
    assert_eq!(observed, entrants.into_iter().collect());

    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    let copied_identities: std::collections::HashSet<_> = engine.state.players[0]
        .battlefield
        .iter()
        .filter_map(|object_id| {
            engine.state.objects[object_id]
                .token_origin
                .as_ref()
                .map(|values| values.source_card_id.clone())
        })
        .collect();
    assert_eq!(copied_identities.len(), 2);
    assert!(copied_identities.contains("darksteel_myr"));
    assert!(copied_identities.contains("iron_myr"));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.objects.contains_key(&molten_echoes));
}

#[test]
fn molten_echoes_copy_token_runs_its_own_as_enters_copy_choice() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_907,
        &[4, 9],
        20,
        None,
        true,
    )
    .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);

    let molten_echoes = inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let molten_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(molten_index, vec![]))
        .expect("cast Molten Echoes");
    let type_choice = pass_until_resolution_choice(&mut engine);
    let bear = find_resolution_choice(&type_choice)
        .unwrap()
        .resolution_branches
        .into_iter()
        .find(|branch| branch.label == "Bear")
        .expect("Bear is a canonical creature type");
    engine
        .apply_command(4, &choose_branch(bear.branch_index))
        .expect("choose Bear");

    let sorcerer = inject_permanent_on_battlefield(&mut engine, 0, "prodigal_sorcerer");
    let entrant = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let entrant_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == entrant)
        .expect("the physical Grizzly Bears is in hand");
    engine
        .apply_command(4, &cast_spell(entrant_index, vec![]))
        .expect("cast Grizzly Bears");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Molten Echoes' trigger is staged"
    );
    let entrant_generation = engine.state.zone_change_generation[&entrant];

    // This focused engine fixture models a copy-layer change to a still-live event object. Its
    // object and zone generation stay fixed while the newer copiable face supplies Clone's own
    // as-enters choice to the token created during resolution.
    add_clone_as_enters_copy_values(&mut engine, entrant);
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the copy token's Clone ability asks for its own entry choice");
    assert_eq!(pending.deciding_player, 4);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert!(pending.presentation.candidates.contains(&sorcerer));
    assert_eq!(
        engine.state.zone_change_generation[&entrant],
        entrant_generation
    );
    engine
        .apply_command(4, &submit_resolution_choice(vec![sorcerer]))
        .expect("choose Prodigal Sorcerer for the copy token's own as-enters ability");

    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| {
            engine.state.objects[object_id]
                .token_origin
                .as_ref()
                .is_some_and(|values| values.source_card_id == "grizzly_bears")
        })
        .expect("the copy token enters after its own as-enters choice");
    assert_eq!(engine.state.objects[&token].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&token]
            .token_origin
            .as_ref()
            .expect("the token retains its intrinsic identity")
            .source_card_id,
        "grizzly_bears"
    );
    assert_eq!(
        engine.state.objects[&token]
            .copiable_values
            .as_ref()
            .map(|values| values.source_card_id.as_str()),
        Some("prodigal_sorcerer"),
        "the token's own as-enters choice installs the selected copy values"
    );
    let characteristics = engine.characteristics(token).unwrap();
    assert!(characteristics.has_name("Prodigal Sorcerer"));
    assert!(characteristics.has_type("Wizard"));
    assert_eq!(characteristics.power, Some(1));
    assert!(engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));
    assert!(engine.state.active_event_observers.iter().any(|observer| {
        observer.watched.is_some_and(|watched| {
            watched.object_id == token
                && watched.zone_change_generation == engine.state.zone_change_generation[&token]
        })
    }));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.objects.contains_key(&molten_echoes));
}

#[test]
fn molten_echoes_filters_noncreatures_and_other_types_but_accepts_changeling() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_906,
        &[4, 9],
        20,
        None,
        true,
    )
    .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);

    let molten_echoes = inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let molten_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(molten_index, vec![]))
        .expect("cast Molten Echoes");
    let type_choice = pass_until_resolution_choice(&mut engine);
    let bear = find_resolution_choice(&type_choice)
        .unwrap()
        .resolution_branches
        .into_iter()
        .find(|branch| branch.label == "Bear")
        .expect("Bear is a canonical creature type");
    engine
        .apply_command(4, &choose_branch(bear.branch_index))
        .expect("choose Bear");

    let sol_ring = inject_card_into_hand(&mut engine, 0, "sol_ring");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let sol_ring_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == sol_ring)
        .expect("Sol Ring is in hand");
    engine
        .apply_command(4, &cast_spell(sol_ring_index, vec![]))
        .expect("cast a noncreature artifact");
    pass_priority_round(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "a noncreature does not match"
    );

    let elf = inject_card_into_hand(&mut engine, 0, "elvish_mystic");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let elf_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == elf)
        .expect("Elvish Mystic is in hand");
    engine
        .apply_command(4, &cast_spell(elf_index, vec![]))
        .expect("cast an Elf that does not have the chosen subtype");
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty(), "an Elf does not match Bear");

    let changeling = inject_card_into_hand(&mut engine, 0, "sizzling_changeling");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let changeling_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == changeling)
        .expect("Sizzling Changeling is in hand");
    engine
        .apply_command(4, &cast_spell(changeling_index, vec![]))
        .expect("cast a creature with Changeling");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0]
            .trigger_context
            .observed_object
            .expect("Changeling matches every creature type")
            .object_id,
        changeling
    );
    assert!(engine.state.objects.contains_key(&molten_echoes));
}

#[test]
fn molten_echoes_chooses_a_creature_type_and_copies_a_matching_entry() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_901,
        &[4, 9],
        20,
        None,
        true,
    )
    .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);

    let molten_echoes = inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast Molten Echoes");
    engine.apply_command(4, &pass()).unwrap();
    let entry_choice = engine.apply_command(9, &pass()).unwrap();
    let choice = find_resolution_choice(&entry_choice).expect("choose a type as Molten enters");

    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(choice.resolution_branches.len(), 324);
    let bear = choice
        .resolution_branches
        .iter()
        .find(|option| option.label == "Bear")
        .expect("Bear is a legal creature type");
    assert!(
        engine.apply_command(4, &choose_branch(324)).is_err(),
        "an out-of-range creature-type index is rejected"
    );
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_command(4, &choose_branch(bear.branch_index))
        .expect("choose Bear");
    assert_eq!(engine.state.objects[&molten_echoes].zone, Zone::Battlefield);
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 0, molten_echoes)
            .iter()
            .any(|label| label == "Chosen creature type: Bear")
    );

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
    let bears_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == bears)
        .expect("the injected physical Bear is in hand");
    engine
        .apply_command(4, &cast_spell(bears_index, vec![]))
        .expect("cast Grizzly Bears");
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&bears].zone, Zone::Battlefield);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0]
            .trigger_context
            .observed_object
            .expect("the matching entrant is captured")
            .object_id,
        bears
    );

    let before = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    pass_priority_round(&mut engine);
    let tokens = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| {
            !before.contains(object_id) && engine.state.objects[object_id].token_origin.is_some()
        })
        .collect::<Vec<_>>();
    assert_eq!(tokens.len(), 1);
    let token = &engine.state.objects[&tokens[0]];
    let copied_identity = token
        .token_origin
        .as_ref()
        .expect("copy token retains its intrinsic copied identity");
    assert_eq!(copied_identity.source_card_id, "grizzly_bears");
    assert_eq!(copied_identity.face.name, "Grizzly Bears");
    let token_characteristics = engine
        .characteristics(tokens[0])
        .expect("the token has rules-visible characteristics");
    assert!(token_characteristics.has_name("Grizzly Bears"));
    assert!(token_characteristics.has_type("Creature"));
    assert!(token_characteristics.has_type("Bear"));
    assert_eq!(token_characteristics.power, Some(2));
    assert_eq!(token_characteristics.toughness, Some(2));
    assert!(engine.effective_has_keyword(tokens[0], tricerules_cards::Keyword::Haste));
    assert!(
        engine.state.stack.is_empty(),
        "the hasty token does not retrigger the nontoken observer"
    );
    assert!(engine.state.active_event_observers.iter().any(|observer| {
        observer.watched.is_some_and(|watched| {
            watched.object_id == tokens[0]
                && watched.zone_change_generation == engine.state.zone_change_generation[&tokens[0]]
        })
    }));

    engine
        .apply_command(4, &primitive_yield())
        .expect("main one to combat");
    engine
        .apply_command(4, &primitive_yield())
        .expect("begin combat");
    if engine.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
        engine
            .apply_command(4, &primitive_yield())
            .expect("skip attackers");
    }
    engine
        .apply_command(4, &primitive_yield())
        .expect("end combat to main two");
    engine
        .apply_command(4, &primitive_yield())
        .expect("main two to end step");
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
    assert_eq!(engine.state.stack.len(), 1);
    pass_priority_round(&mut engine);
    assert!(
        !engine.state.objects.contains_key(&tokens[0]),
        "the exact token ceases to exist after its next-end-step exile"
    );
}

#[test]
fn molten_echoes_doubling_season_gives_each_token_haste_and_one_cohort_exile() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        2_026_101_015,
        &[4, 9],
        20,
        None,
        true,
    )
    .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");

    inject_card_into_hand(&mut engine, 0, "molten_echoes");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let molten_index = hand_index_for_card(&engine, 0, "molten_echoes");
    engine
        .apply_command(4, &cast_spell(molten_index, vec![]))
        .expect("cast Molten Echoes");
    engine
        .apply_command(4, &pass())
        .expect("active player passes");
    let entry_choice = engine
        .apply_command(9, &pass())
        .expect("Molten Echoes enters and asks for a creature type");
    let choice = find_resolution_choice(&entry_choice).expect("choose a creature type");
    let bear_type = choice
        .resolution_branches
        .iter()
        .find(|option| option.label == "Bear")
        .expect("Bear is a supported creature type");
    engine
        .apply_command(4, &choose_branch(bear_type.branch_index))
        .expect("choose Bear");

    let _bear = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let bear_index = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(4, &cast_spell(bear_index, vec![]))
        .expect("cast Grizzly Bears");
    pass_priority_round(&mut engine);

    let before = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    pass_priority_round(&mut engine);
    answer_simultaneous_entry_order_in_engine_order(&mut engine);
    let tokens = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object| {
            !before.contains(object)
                && engine.state.objects[object].card_id == "grizzly_bears"
                && engine.state.objects[object].is_token()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        tokens.len(),
        2,
        "Doubling Season replaces one token with two"
    );
    for token in &tokens {
        assert!(engine.effective_has_keyword(*token, tricerules_cards::Keyword::Haste));
    }
    assert_eq!(engine.state.observed_object_cohorts.len(), 1);
    assert_eq!(
        engine
            .state
            .active_event_observers
            .iter()
            .filter(|observer| {
                matches!(
                    observer.matcher,
                    tricerules_core::state::EventObserverMatcher::AtBeginningOfNextEndStep
                )
            })
            .count(),
        1,
        "one delayed trigger represents the complete token batch"
    );

    engine
        .apply_command(4, &primitive_yield())
        .expect("main one to combat");
    engine
        .apply_command(4, &primitive_yield())
        .expect("begin combat");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_command(4, &primitive_yield())
            .expect("skip attackers");
    }
    engine
        .apply_command(4, &primitive_yield())
        .expect("end combat to main two");
    engine
        .apply_command(4, &primitive_yield())
        .expect("main two to end step");
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    for token in tokens {
        assert!(!engine.state.objects.contains_key(&token));
    }
    assert!(engine.state.observed_object_cohorts.is_empty());
}
