use crate::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

#[test]
fn omen_normal_and_alternative_faces_are_published_for_one_hand_object() {
    let decks = Some(vec![
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(100_001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let oid = relocate_to_hand(&mut e, 0, "dirgur_island_dragon_skimming_strike");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == oid)
        .expect("Omen in hand") as u32;

    let batch = e.initial_response_batch();
    let legal = batch.legal_by_player.get(&0).expect("P0 legal actions");
    let faces: Vec<_> = legal
        .hand_actions
        .iter()
        .filter(|action| action.hand_index == slot)
        .collect();
    assert_eq!(faces.len(), 2);
    assert_eq!(faces[0].face_index, 0);
    assert_eq!(faces[0].card_name, "Dirgur Island Dragon");
    assert_eq!(faces[0].cost, "{5}{U}");
    assert!(!faces[0].needs_target);
    assert_eq!(faces[1].face_index, 1);
    assert_eq!(faces[1].card_name, "Skimming Strike");
    assert_eq!(faces[1].cost, "{1}{U}");
    assert!(faces[1].needs_target);
    let group = legal
        .valid_targets_by_hand_slot
        .get(&((slot << 8) | 1))
        .expect("target schema")
        .groups
        .first()
        .expect("optional target group");
    assert_eq!(group.min, 0);
    assert_eq!(group.max, 1);
}

#[test]
fn omen_with_no_chosen_target_resolves_then_shuffles_into_owners_library() {
    let decks = Some(vec![
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(100_002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let oid = relocate_to_hand(&mut e, 0, "dirgur_island_dragon_skimming_strike");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == oid)
        .expect("Omen in hand");
    let hand_before = e.state.players[0].hand.len();
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );

    let cast = e
        .apply_command(0, &cast_spell_face(slot, vec![], 1))
        .expect("cast Skimming Strike without a target");
    let pushed = cast
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(value)) => Some(value),
            _ => None,
        })
        .expect("stack push");
    assert_eq!(pushed.description, "Skimming Strike");
    assert_eq!(pushed.ability_annotation, "Skimming Strike");

    e.apply_command(0, &pass()).expect("caster pass");
    let resolved = e.apply_command(1, &pass()).expect("opponent pass");
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.objects[&oid].zone, Zone::Library);
    assert!(e.state.players[0].library.contains(&oid));
    assert!(!e.state.players[0].graveyard.contains(&oid));
    assert_eq!(e.state.players[0].hand.len(), hand_before);
    assert_eq!(e.state.players[0].library.len(), library_before);
    let stack_resolved = resolved.events.iter().find_map(|event| match &event.ev {
        Some(Ev::StackResolved(value)) if value.object_id == oid => Some(value),
        _ => None,
    });
    assert_eq!(
        stack_resolved.map(|value| value.destination),
        Some(rv1::StackResolveDestination::Library as i32)
    );
}

#[test]
fn omen_shuffle_is_replay_identical_for_the_same_seed_and_commands() {
    fn resolve_omen(seed: u64) -> Vec<String> {
        let decks = Some(vec![
            deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
            vec!["forest".into(); 20],
        ]);
        let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
        advance_to_main1_from_game_start(&mut e);
        let oid = relocate_to_hand(&mut e, 0, "dirgur_island_dragon_skimming_strike");
        let slot = e.state.players[0]
            .hand
            .iter()
            .position(|candidate| *candidate == oid)
            .expect("Omen in hand");
        give_mana(
            &mut e,
            0,
            ManaGift {
                u: 2,
                ..Default::default()
            },
        );

        e.apply_command(0, &cast_spell_face(slot, vec![], 1))
            .expect("cast Skimming Strike without a target");
        e.apply_command(0, &pass()).expect("caster pass");
        e.apply_command(1, &pass()).expect("opponent pass");

        e.state.players[0]
            .library
            .iter()
            .map(|object_id| e.state.objects[object_id].card_id.clone())
            .collect()
    }

    assert_eq!(resolve_omen(100_008), resolve_omen(100_008));
}

#[test]
fn omen_normal_face_resolves_as_the_permanent_face() {
    let decks = Some(vec![
        deck_with("forest", &["sagu_wildling_roost_seek"]),
        vec!["island".into(); 20],
    ]);
    let mut e = GameEngine::new(100_003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let oid = relocate_to_hand(&mut e, 0, "sagu_wildling_roost_seek");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == oid)
        .expect("Omen in hand");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 5,
            ..Default::default()
        },
    );

    e.apply_command(0, &cast_spell_face(slot, vec![], 0))
        .expect("cast normal face");
    e.apply_command(0, &pass()).expect("caster pass");
    e.apply_command(1, &pass()).expect("opponent pass");
    assert_eq!(e.state.objects[&oid].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&oid].face_up_index, 0);
}

#[test]
fn chosen_target_becoming_illegal_fizzles_omen_to_graveyard_without_drawing() {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                "dirgur_island_dragon_skimming_strike",
                "lightning_bolt",
                "grizzly_bears",
            ],
        ),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(100_004, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let omen = relocate_to_hand(&mut e, 0, "dirgur_island_dragon_skimming_strike");
    relocate_to_hand(&mut e, 0, "lightning_bolt");
    let target = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            r: 1,
            ..Default::default()
        },
    );
    let hand_before = e.state.players[0].hand.len();
    let omen_slot = e.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == omen)
        .expect("Omen in hand");
    e.apply_command(0, &cast_spell_face(omen_slot, target_object(target), 1))
        .expect("cast targeted Skimming Strike");
    let bolt_slot = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_slot, target_object(target)))
        .expect("cast Bolt above the Omen");

    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.objects[&omen].zone, Zone::Graveyard);
    assert!(e.state.players[0].graveyard.contains(&omen));
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 2,
        "a fizzled Skimming Strike must not draw"
    );
}

#[test]
fn countered_omen_uses_the_ordinary_graveyard_path() {
    let decks = Some(vec![
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
        deck_with("island", &["counterspell"]),
    ]);
    let mut e = GameEngine::new(100_005, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let omen = relocate_to_hand(&mut e, 0, "dirgur_island_dragon_skimming_strike");
    relocate_to_hand(&mut e, 1, "counterspell");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let omen_slot = e.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == omen)
        .expect("Omen in hand");
    e.apply_command(0, &cast_spell_face(omen_slot, vec![], 1))
        .expect("cast Omen");
    e.apply_command(0, &pass()).expect("pass to opponent");
    let counter_slot = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(1, &cast_spell(counter_slot, target_object(omen)))
        .expect("counter the Omen");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.objects[&omen].zone, Zone::Graveyard);
    assert!(e.state.players[0].graveyard.contains(&omen));
    assert!(!e.state.players[0].library.contains(&omen));
}

#[test]
fn parked_library_search_finishes_before_the_physical_omen_is_shuffled() {
    let decks = Some(vec![
        deck_with("forest", &["sagu_wildling_roost_seek"]),
        vec!["island".into(); 20],
    ]);
    let mut e = GameEngine::new(100_006, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let omen = relocate_to_hand(&mut e, 0, "sagu_wildling_roost_seek");
    let forest = inject_library_card(&mut e, 0, "forest");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == omen)
        .expect("Omen in hand");
    e.apply_command(0, &cast_spell_face(slot, vec![], 1))
        .expect("cast Roost Seek");
    e.apply_command(0, &pass()).expect("caster passes");
    let choice_batch = e.apply_command(1, &pass()).expect("begin resolving Omen");
    let choice = find_resolution_choice(&choice_batch).expect("basic-land search choice");
    assert!(choice.candidate_object_ids.contains(&forest));
    assert_eq!(e.state.objects[&omen].zone, Zone::Stack);
    assert!(e.state.pending_resolution.is_some());

    let completion = e
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose the basic land");

    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.objects[&forest].zone, Zone::Hand);
    assert_eq!(e.state.objects[&omen].zone, Zone::Library);
    assert!(completion.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::StackResolved(value))
                if value.object_id == omen
                    && value.destination == rv1::StackResolveDestination::Library as i32
        )
    }));
}

#[test]
fn successfully_resolving_omen_copy_shuffles_without_creating_a_library_object() {
    let decks = Some(vec![
        deck_with("plains", &["riling_dawnbreaker_signaling_roar"]),
        vec!["island".into(); 20],
    ]);
    let mut e = GameEngine::new(100_007, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let omen = relocate_to_hand(&mut e, 0, "riling_dawnbreaker_signaling_roar");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == omen)
        .expect("Omen in hand");
    e.apply_command(0, &cast_spell_face(slot, vec![], 1))
        .expect("cast Signaling Roar");

    let physical = e
        .state
        .stack
        .last()
        .expect("physical Omen on stack")
        .clone();
    let copy_id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let mut copy = physical.clone();
    copy.id = copy_id;
    copy.is_copy = true;
    e.state.stack.push(copy);
    let library_len = e.state.players[0].library.len();

    e.apply_command(0, &pass()).expect("caster passes");
    let copy_resolution = e.apply_command(1, &pass()).expect("copy resolves");

    assert!(!e.state.objects.contains_key(&copy_id));
    assert!(!e.state.players[0].library.contains(&copy_id));
    assert_eq!(e.state.players[0].library.len(), library_len);
    assert_eq!(e.state.objects[&omen].zone, Zone::Stack);
    assert_eq!(e.state.stack.len(), 1);
    let resolved_copy = copy_resolution
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackResolved(value)) if value.object_id == copy_id => Some(value),
            _ => None,
        });
    assert_eq!(
        resolved_copy.map(|value| value.destination),
        Some(rv1::StackResolveDestination::Library as i32)
    );
    assert_eq!(resolved_copy.and_then(|value| value.owner_player_id), None);

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&omen].zone, Zone::Library);
}
