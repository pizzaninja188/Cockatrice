//! Myriad's necessary library-to-battlefield batch contract, using an actual shipped search.
use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{CastCostGroupSelection, ChoiceKind};

fn myriad_search(
    seed: u64,
) -> (
    GameEngine,
    u32,
    tricerules_proto::ruled::v1::ResolutionChoiceRequired,
) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["myriad_landscape"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "myriad_landscape", false);
    inject_library_card(&mut engine, 0, "island");
    inject_library_card(&mut engine, 0, "wastes");
    let nonbasic = inject_library_card(&mut engine, 0, "taiga");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability(source, 1, vec![]))
        .expect("pay two, tap and sacrifice actual Myriad");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    engine.apply_command(0, &pass()).unwrap();
    let batch = engine.apply_command(1, &pass()).unwrap();
    let choice = find_resolution_choice(&batch).unwrap().clone();
    assert_eq!((choice.min, choice.max), (0, 2));
    assert!(
        !choice.candidate_object_ids.contains(&nonbasic),
        "Taiga's land subtypes do not make it Basic"
    );
    (engine, source, choice)
}

#[test]
fn actual_myriad_rejects_incompatible_pair_atomically_and_publishes_relations() {
    let (mut engine, _, choice) = myriad_search(202_610_032);
    let forest = choice
        .candidate_object_ids
        .iter()
        .copied()
        .find(|id| engine.state.objects[id].card_id == "forest")
        .unwrap();
    let island = choice
        .candidate_object_ids
        .iter()
        .copied()
        .find(|id| engine.state.objects[id].card_id == "island")
        .unwrap();
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(
        engine
            .apply_command(0, &submit_resolution_choice(vec![forest, island]))
            .is_err(),
        "two basics without a shared land type must be rejected"
    );
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    assert!(
        !choice.selection_alternatives.is_empty(),
        "private picker receives engine authored positive-cardinality alternatives"
    );
    assert!(choice
        .selection_alternatives
        .iter()
        .any(|alternative| alternative.count == 1
            && alternative.candidate_indices.len() == choice.candidate_object_ids.len()));
}

#[test]
fn actual_myriad_zero_singleton_wastes_and_same_name_pair_complete() {
    for count in 0..=2 {
        let (mut engine, _, choice) = myriad_search(202_610_040 + count);
        let selected = if count == 1 {
            choice
                .candidate_object_ids
                .iter()
                .copied()
                .filter(|id| engine.state.objects[id].card_id == "wastes")
                .take(1)
                .collect::<Vec<_>>()
        } else {
            choice
                .candidate_object_ids
                .iter()
                .copied()
                .filter(|id| engine.state.objects[id].card_id == "forest")
                .take(count as usize)
                .collect::<Vec<_>>()
        };
        assert_eq!(selected.len(), count as usize);
        engine
            .apply_command(0, &submit_resolution_choice(selected.clone()))
            .expect("legal zero/single/pair");
        answer_simultaneous_entry_order_in_engine_order(&mut engine);
        assert!(selected
            .iter()
            .all(|id| engine.state.objects[id].zone == Zone::Battlefield
                && engine.state.objects[id].tapped));
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn actual_myriad_every_invalid_answer_preserves_the_pending_search() {
    for case in 0..11 {
        let (mut engine, _, choice) = myriad_search(202_610_050 + case);
        let forests = choice
            .candidate_object_ids
            .iter()
            .copied()
            .filter(|id| engine.state.objects[id].card_id == "forest")
            .take(3)
            .collect::<Vec<_>>();
        let wastes = choice
            .candidate_object_ids
            .iter()
            .copied()
            .find(|id| engine.state.objects[id].card_id == "wastes")
            .unwrap();
        let foreign = inject_library_card(&mut engine, 1, "forest");
        let nonbasic = inject_library_card(&mut engine, 0, "taiga");
        let second_wastes = inject_library_card(&mut engine, 0, "wastes");
        let mut actor = 0;
        let selected = match case {
            0 => {
                actor = 1;
                vec![forests[0]]
            }
            1 => vec![forests[0], forests[0]],
            2 => forests.clone(),
            3 => vec![foreign],
            4 => vec![nonbasic],
            5 => vec![999_999],
            6 => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(forests[0])
                    .or_default() += 1;
                vec![forests[0]]
            }
            7 => {
                engine.state.objects.get_mut(&forests[0]).unwrap().card_id = "taiga".into();
                vec![forests[0]]
            }
            8 => {
                engine.state.players[0]
                    .library
                    .retain(|id| *id != forests[0]);
                engine.state.players[0].hand.push(forests[0]);
                engine.state.objects.get_mut(&forests[0]).unwrap().zone = Zone::Hand;
                vec![forests[0]]
            }
            9 => {
                // Change another original candidate to Wastes without a zone change: both
                // are still basic lands, but two Wastes share no land subtype.
                engine.state.objects.get_mut(&forests[0]).unwrap().card_id = "wastes".into();
                vec![wastes, forests[0]]
            }
            10 => {
                engine.state.objects.get_mut(&forests[1]).unwrap().card_id = "island".into();
                vec![forests[0], forests[1]]
            }
            _ => unreachable!(),
        };
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(
            engine
                .apply_command(actor, &submit_resolution_choice(selected))
                .is_err(),
            "illegal case {case}"
        );
        assert_eq!(
            engine.diagnostic_snapshot().unwrap(),
            before,
            "atomic case {case}"
        );
        assert_eq!(engine.state.objects[&second_wastes].zone, Zone::Library);
        engine
            .apply_command(0, &submit_resolution_choice(vec![]))
            .expect("pending search remains usable");
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn actual_myriad_survives_source_owner_departure_at_search_and_timestamp_choice() {
    for stage in [0, 2] {
        let deck = deck_with("forest", &[]);
        let mut engine = GameEngine::new(
            202_610_070 + stage,
            &[0, 1, 2],
            20,
            Some(vec![deck.clone(), deck.clone(), deck]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let source = inject_creature_under_foreign_control(&mut engine, 1, 0, "myriad_landscape");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        engine
            .apply_command(0, &activate_ability(source, 1, vec![]))
            .unwrap();
        pass_priority_round(&mut engine);
        let candidates = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates
            .clone();
        let selected = candidates.into_iter().take(2).collect::<Vec<_>>();
        if stage > 0 {
            engine
                .apply_command(0, &submit_resolution_choice(selected.clone()))
                .unwrap();
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .presentation
                    .choice_kind,
                ChoiceKind::SimultaneousEntryOrder
            );
        }
        engine.apply_command(1, &concede()).unwrap();
        assert!(!engine.state.objects.contains_key(&source));
        assert!(engine.state.pending_resolution.is_some(), "surviving controller's ability at stage {stage} is independent of departed source owner");
        if stage == 0 {
            engine
                .apply_command(0, &submit_resolution_choice(selected.clone()))
                .unwrap();
        }
        for _ in 0..6 {
            let Some(pending) = engine.state.pending_resolution.as_ref() else {
                break;
            };
            match pending.presentation.choice_kind {
                ChoiceKind::SimultaneousEntryOrder => {
                    answer_simultaneous_entry_order_in_engine_order(&mut engine);
                }
                other => panic!("unexpected pending choice {other:?}"),
            }
        }
        assert!(engine.state.pending_resolution.is_none());
        assert!(selected
            .iter()
            .all(|id| engine.state.objects[id].zone == Zone::Battlefield
                && engine.state.objects[id].tapped));
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn actual_myriad_enters_tapped_makes_colorless_and_requires_each_source_cost() {
    let mut engine = GameEngine::new(
        202_610_080,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["myriad_landscape"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "myriad_landscape");
    let slot = hand_index_for_card(&engine, 0, "myriad_landscape");
    let source = engine.state.players[0].hand[slot];
    engine.apply_command(0, &play_land(slot)).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(
        engine.state.objects[&source].tapped,
        "actual land play applies the enters-tapped line"
    );
    for index in [0, 1] {
        let before = engine.diagnostic_snapshot().unwrap();
        let command = activate_ability_for(&engine, source, index, vec![]);
        assert!(
            engine.apply_command(0, &command).is_err(),
            "tapped source cannot pay tap"
        );
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    engine.state.objects.get_mut(&source).unwrap().tapped = false;
    for actor in [0, 1] {
        let before = engine.diagnostic_snapshot().unwrap();
        let command = activate_ability_for(&engine, source, 1, vec![]);
        assert!(
            engine.apply_command(actor, &command).is_err(),
            "no two mana or foreign actor"
        );
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    engine
        .state
        .continuous_effects
        .push(tricerules_core::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: tricerules_core::AffectedScope::Single(source),
            kind: tricerules_cards::ContinuousEffectKind::Layer4AddTypes(
                tricerules_cards::TypeLineAddition {
                    card_types: vec![tricerules_cards::PermanentTypeFilter::Creature],
                    ..Default::default()
                },
            ),
            condition: None,
            duration: tricerules_cards::EffectDuration::Indefinite,
            timestamp: 1,
        });
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = true;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    for index in [0, 1] {
        let before = engine.diagnostic_snapshot().unwrap();
        let command = activate_ability_for(&engine, source, index, vec![]);
        assert!(
            engine.apply_command(0, &command).is_err(),
            "animated sick land cannot pay tap"
        );
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    engine.state.continuous_effects.clear();
    let before = engine.state.players[0].mana_pool.colorless;
    apply_ability(&mut engine, 0, source, 0, vec![]).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, before + 1);
    assert!(engine.state.objects[&source].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "colorless mana resolves without the stack"
    );
}

#[test]
fn actual_myriad_paid_search_choices_replay_deterministically_with_physical_generations() {
    let (mut first, source, choice) = myriad_search(202_610_090);
    let (mut replay, replay_source, replay_choice) = myriad_search(202_610_090);
    assert_eq!(source, replay_source);
    assert_eq!(choice, replay_choice);
    assert_eq!(
        first.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    let selected = choice
        .candidate_object_ids
        .iter()
        .copied()
        .filter(|id| first.state.objects[id].card_id == "forest")
        .take(2)
        .collect::<Vec<_>>();
    let generations = selected
        .iter()
        .map(|id| {
            first
                .state
                .zone_change_generation
                .get(id)
                .copied()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let commands = [
        submit_resolution_choice(selected.clone()),
        submit_resolution_choice(selected.iter().rev().copied().collect()),
    ];
    for command in commands {
        let original = first.apply_command(0, &command).unwrap();
        let repeated = replay.apply_command(0, &command).unwrap();
        assert_eq!(original, repeated);
        assert_eq!(
            first.diagnostic_snapshot().unwrap(),
            replay.diagnostic_snapshot().unwrap()
        );
    }
    for (id, generation) in selected.iter().zip(generations) {
        assert_eq!(
            first.state.zone_change_generation.get(id).copied(),
            Some(generation + 1)
        );
        assert_eq!(first.state.objects[id].zone, Zone::Battlefield);
        assert_eq!(first.state.objects[id].owner, 0);
        assert_eq!(first.state.objects[id].controller, 0);
    }
    assert_eq!(first.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(first.state.players[0].mana_pool.colorless, 0);
    assert!(first.state.pending_resolution.is_none());
}

#[test]
fn actual_myriad_timestamp_choice_rejects_a_stale_cohort_before_any_entry() {
    let (mut engine, _, choice) = myriad_search(202_610_091);
    let selected = choice
        .candidate_object_ids
        .iter()
        .copied()
        .filter(|id| engine.state.objects[id].card_id == "forest")
        .take(2)
        .collect::<Vec<_>>();
    engine
        .apply_command(0, &submit_resolution_choice(selected.clone()))
        .unwrap();
    *engine
        .state
        .zone_change_generation
        .entry(selected[1])
        .or_default() += 1;
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(0, &submit_resolution_choice(selected.clone()))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    assert!(selected
        .iter()
        .all(|id| engine.state.objects[id].zone == Zone::Library));
}

#[test]
fn grow_searched_cohort_cancels_when_searcher_leaves_during_entry_replacement() {
    let deck = deck_with(
        "forest",
        &["grow_from_the_ashes", "orb_of_dreams", "orb_of_dreams"],
    );
    let mut engine = GameEngine::new(
        202_610_092,
        &[0, 1, 2],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_battlefield(&mut engine, 0, "orb_of_dreams", false);
    relocate_to_battlefield(&mut engine, 0, "orb_of_dreams", false);
    ensure_card_in_hand(&mut engine, 0, "grow_from_the_ashes");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                hand_index_for_card(&engine, 0, "grow_from_the_ashes"),
                vec![],
                vec![CastCostGroupSelection {
                    group_index: 0,
                    option_index: 0,
                    ..Default::default()
                }],
            ),
        )
        .unwrap();
    pass_priority_round(&mut engine);
    let selected = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .iter()
        .copied()
        .take(2)
        .collect::<Vec<_>>();
    engine
        .apply_command(0, &submit_resolution_choice(selected))
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::ReplacementEffect
    );
    let departed = engine.apply_command(0, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
    assert!(engine.state.stack.is_empty());
    assert!(!permanents_moved_in(&departed)
        .iter()
        .any(|movement| movement.destination
            == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield as i32));
    assert!(!departed.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    ));
}

#[test]
fn actual_myriad_searching_player_departure_cancels_search_and_entry_order_without_moves() {
    for selected_already in [false, true] {
        let deck = deck_with("forest", &["myriad_landscape"]);
        let mut engine = GameEngine::new(
            202_610_081 + u64::from(selected_already),
            &[0, 1, 2],
            20,
            Some(vec![deck.clone(), deck.clone(), deck]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let source = relocate_to_battlefield(&mut engine, 0, "myriad_landscape", false);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        engine
            .apply_command(0, &activate_ability(source, 1, vec![]))
            .unwrap();
        pass_priority_round(&mut engine);
        if selected_already {
            let selected = engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates
                .iter()
                .copied()
                .take(2)
                .collect();
            engine
                .apply_command(0, &submit_resolution_choice(selected))
                .unwrap();
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .presentation
                    .choice_kind,
                ChoiceKind::SimultaneousEntryOrder
            );
        }
        let departed = engine.apply_command(0, &concede()).unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert!(
            engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null()
        );
        assert!(engine.state.stack.is_empty());
        assert!(!departed.events.iter().any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")));
        assert!(!permanents_moved_in(&departed)
            .iter()
            .any(|movement| movement.destination
                == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield as i32));
    }
}

#[test]
fn grow_from_the_ashes_kicked_search_enters_one_simultaneous_cohort() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &["grow_from_the_ashes", "orb_of_dreams", "orb_of_dreams"],
        ),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(202_610_031, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_battlefield(&mut engine, 0, "orb_of_dreams", false);
    relocate_to_battlefield(&mut engine, 0, "orb_of_dreams", false);
    ensure_card_in_hand(&mut engine, 0, "grow_from_the_ashes");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    let cast = cast_spell_with_cast_cost_groups(
        hand_index_for_card(&engine, 0, "grow_from_the_ashes"),
        vec![],
        vec![CastCostGroupSelection {
            group_index: 0,
            option_index: 0,
            ..Default::default()
        }],
    );
    engine.apply_command(0, &cast).expect("actual kicked Grow");
    engine.apply_command(0, &pass()).unwrap();
    let search = engine.apply_command(1, &pass()).unwrap();
    let choice = find_resolution_choice(&search).expect("actual two-basic search");
    assert_eq!((choice.min, choice.max), (0, 2));
    let selected = choice
        .candidate_object_ids
        .iter()
        .copied()
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 2);
    let replacement = engine
        .apply_command(0, &submit_resolution_choice(selected.clone()))
        .unwrap();
    let choice = find_resolution_choice(&replacement).expect("first entry's Orb replacement order");
    assert_eq!(choice.replacement_options.len(), 2);
    assert!(selected
        .iter()
        .all(|id| engine.state.objects[id].zone == Zone::Library));
    let later = engine
        .apply_command(
            0,
            &submit_resolution_choice(vec![choice.candidate_object_ids[0]]),
        )
        .unwrap();
    let later_choice = find_resolution_choice(&later).expect("later entry's replacement order");
    assert_eq!(later_choice.replacement_options.len(), 2);
    assert!(
        selected
            .iter()
            .all(|id| engine.state.objects[id].zone == Zone::Library),
        "preflight every selected entry before committing any member of one search action"
    );
    let ordering = engine
        .apply_command(
            0,
            &submit_resolution_choice(vec![later_choice.candidate_object_ids[0]]),
        )
        .unwrap();
    let order = find_resolution_choice(&ordering).expect("whole cohort timestamp choice");
    assert_eq!(order.choice_kind(), ChoiceKind::SimultaneousEntryOrder);
    assert!(selected
        .iter()
        .all(|id| engine.state.objects[id].zone == Zone::Library));
    assert!(!ordering.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    ));
    let committed = engine
        .apply_command(0, &submit_resolution_choice(vec![selected[1], selected[0]]))
        .unwrap();
    assert!(selected
        .iter()
        .all(|id| engine.state.objects[id].zone == Zone::Battlefield
            && engine.state.objects[id].tapped));
    assert_eq!(
        permanents_moved_in(&committed)
            .iter()
            .filter(|moved| moved.destination
                == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield as i32)
            .count(),
        2
    );
    assert_eq!(committed.events.iter().filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")).count(), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}
