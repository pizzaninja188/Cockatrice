//! Actual paid Deploy the Gatewatch and its private simultaneous entry contract.
use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

const MIXED: [&str; 7] = [
    "forest",
    "jace_beleren",
    "divination",
    "chandra,_novice_pyromancer",
    "island",
    "forest",
    "divination",
];

#[test]
fn actual_deploy_random_bottom_varies_with_seed_without_reordering_untouched_suffix() {
    let mut orders = std::collections::BTreeSet::new();
    for seed in 202_610_096..202_610_100 {
        let (mut engine, looked, suffix) = paid_look(seed, &MIXED);
        engine
            .apply_command(0, &submit_resolution_choice(vec![looked[1], looked[3]]))
            .unwrap();
        finish_entries(&mut engine);
        let library = engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(&library[..suffix.len()], &suffix);
        orders.insert(
            library[suffix.len()..]
                .iter()
                .map(|oid| {
                    looked
                        .iter()
                        .position(|candidate| candidate == oid)
                        .unwrap()
                })
                .collect::<Vec<_>>(),
        );
    }
    assert!(
        orders.len() > 1,
        "random bottom cannot be an unchanged or fixed original order"
    );
}

#[test]
fn actual_deploy_private_images_have_engine_eligibility_and_constant_waiting_wording() {
    let mut prompts = Vec::new();
    let mut public_logs = Vec::new();
    for planeswalkers in 0..3 {
        let mut engine = paid_stack(202_610_092, &[0, 1]);
        let mut looked = Vec::new();
        for index in 0..7 {
            looked.push(inject_library_card(
                &mut engine,
                0,
                if index < planeswalkers {
                    "jace_beleren"
                } else {
                    "forest"
                },
            ));
        }
        engine.state.players[0]
            .library
            .retain(|oid| !looked.contains(oid));
        for &oid in looked.iter().rev() {
            engine.state.players[0].library.push_front(oid);
        }
        engine.apply_command(0, &pass()).unwrap();
        let batch = engine.apply_command(1, &pass()).unwrap();
        let choice = find_resolution_choice(&batch).unwrap();
        assert_eq!(choice.candidate_object_ids, looked);
        assert_eq!(
            choice.candidate_selectable,
            (0..7)
                .map(|index| index < planeswalkers)
                .collect::<Vec<_>>()
        );
        assert_eq!((choice.min, choice.max), (0, planeswalkers.min(2)));
        assert!(!choice.unique_names && !choice.ordered && choice.public_reveal.is_none());
        assert_eq!(choice.candidate_names.len(), 7);
        prompts.push(choice.prompt_text);
        public_logs.push(
            batch
                .events
                .iter()
                .filter_map(|event| match &event.ev {
                    Some(Ev::Log(log)) if log.visible_to_player_id.is_none() => {
                        Some(log.text.clone())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
        );
    }
    assert!(prompts.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(public_logs.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn actual_deploy_requires_two_white_mana_and_rejects_unaffordable_cast_atomically() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_093,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["deploy_the_gatewatch"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "deploy_the_gatewatch");
    let index = hand_index_for_card(&engine, 0, "deploy_the_gatewatch");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 6,
            ..Default::default()
        },
    );
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(0, &cast_spell(index, vec![])).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(index, vec![])).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
}

#[test]
fn actual_deploy_is_not_a_library_search_and_does_not_fire_search_observers() {
    let (mut engine, looked, _) = paid_look(202_610_094, &MIXED);
    inject_permanent_on_battlefield(&mut engine, 1, "wan_shi_tong,_librarian");
    engine
        .apply_command(0, &submit_resolution_choice(vec![looked[1]]))
        .unwrap();
    finish_entries(&mut engine);
    assert_eq!(engine.state.objects[&looked[1]].zone, Zone::Battlefield);
    assert!(
        engine.state.stack.is_empty(),
        "no search observer trigger from a private look"
    );
}

#[test]
fn actual_deploy_twincast_copy_looks_at_its_controller_library_and_original_looks_afresh() {
    let mut engine = paid_stack(202_610_095, &[0, 1]);
    let original = engine.state.stack.last().unwrap().id;
    let original_top = inject_library_card(&mut engine, 0, "jace_beleren");
    engine.state.players[0]
        .library
        .retain(|oid| *oid != original_top);
    engine.state.players[0].library.push_front(original_top);
    let copy_top = inject_library_card(&mut engine, 1, "chandra,_novice_pyromancer");
    engine.state.players[1]
        .library
        .retain(|oid| *oid != copy_top);
    engine.state.players[1].library.push_front(copy_top);
    let original_library = engine.state.players[0].library.clone();
    engine.apply_command(0, &pass()).unwrap();
    inject_card_into_hand(&mut engine, 1, "twincast");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "twincast");
    engine
        .apply_command(1, &cast_spell(slot, target_object(original)))
        .unwrap();
    pass_priority_round(&mut engine);
    let copy = engine.state.stack.last().unwrap();
    assert!(copy.is_copy && copy.controller == 1 && copy.id != original);
    pass_priority_round(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 1);
    assert!(pending.presentation.candidates.contains(&copy_top));
    assert!(!pending.presentation.candidates.contains(&original_top));
    engine
        .apply_command(1, &submit_resolution_choice(vec![copy_top]))
        .unwrap();
    finish_entries(&mut engine);
    assert_eq!(engine.state.players[0].library, original_library);
    assert_eq!(engine.state.objects[&copy_top].zone, Zone::Battlefield);
    let fresh = inject_library_card(&mut engine, 0, "chandra,_novice_pyromancer");
    engine.state.players[0].library.retain(|oid| *oid != fresh);
    engine.state.players[0].library.push_front(fresh);
    pass_priority_round(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 0);
    assert!(pending.presentation.candidates.contains(&fresh));
    engine
        .apply_command(0, &submit_resolution_choice(vec![fresh]))
        .unwrap();
    finish_entries(&mut engine);
    assert_eq!(engine.state.objects[&fresh].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&original_top].zone, Zone::Library);
    assert_eq!(engine.state.objects[&copy_top].controller, 1);
    assert_eq!(engine.state.objects[&fresh].controller, 0);
    assert!(engine.state.stack.is_empty());
}

fn finish_entries(engine: &mut GameEngine) -> Vec<tricerules_proto::ruled::v1::RuledEventBatch> {
    let mut batches = Vec::new();
    for _ in 0..10 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            return batches;
        };
        let ids = match pending.presentation.choice_kind {
            ChoiceKind::ReplacementEffect => vec![pending.presentation.candidates[0]],
            ChoiceKind::SimultaneousEntryOrder => pending.presentation.candidates.clone(),
            other => panic!("unexpected entry choice {other:?}"),
        };
        batches.push(
            engine
                .apply_command(pending.deciding_player, &submit_resolution_choice(ids))
                .unwrap(),
        );
    }
    panic!("entry resolution exceeded explicit bound");
}

#[test]
fn actual_deploy_zero_one_and_no_match_keep_suffix_and_bottom_the_full_rest() {
    for (count, cards) in [
        (0, MIXED),
        (1, MIXED),
        (
            0,
            [
                "forest",
                "island",
                "divination",
                "forest",
                "island",
                "forest",
                "divination",
            ],
        ),
    ] {
        let (mut engine, looked, suffix) = paid_look(202_610_082, &cards);
        let selected = if count == 1 { vec![looked[1]] } else { vec![] };
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .max,
            if cards == MIXED { 2 } else { 0 }
        );
        let generations = engine.state.zone_change_generation.clone();
        engine
            .apply_command(0, &submit_resolution_choice(selected.clone()))
            .unwrap();
        finish_entries(&mut engine);
        let library = engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(&library[..suffix.len()], &suffix);
        let mut rest = library[suffix.len()..].to_vec();
        rest.sort_unstable();
        let mut expected = looked
            .iter()
            .filter(|oid| !selected.contains(oid))
            .copied()
            .collect::<Vec<_>>();
        expected.sort_unstable();
        assert_eq!(rest, expected);
        for oid in &expected {
            assert_eq!(engine.state.objects[oid].zone, Zone::Library);
            assert_eq!(
                engine
                    .state
                    .zone_change_generation
                    .get(oid)
                    .copied()
                    .unwrap_or(0),
                generations.get(oid).copied().unwrap_or(0)
            );
        }
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn actual_deploy_short_and_empty_library_complete_without_inventing_cards() {
    for count in [0, 1, 3] {
        let mut engine = paid_stack(202_610_083, &[0, 1]);
        let removed = engine.state.players[0]
            .library
            .drain(..)
            .collect::<Vec<_>>();
        for oid in removed {
            engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Exile;
            engine.state.players[0].exile.push(oid);
        }
        let looked = (0..count)
            .map(|_| inject_library_card(&mut engine, 0, "forest"))
            .collect::<Vec<_>>();
        engine.apply_command(0, &pass()).unwrap();
        let batch = engine.apply_command(1, &pass()).unwrap();
        if count == 0 {
            assert!(find_resolution_choice(&batch).is_none());
            assert!(engine.state.pending_resolution.is_none());
        } else {
            let choice = find_resolution_choice(&batch).unwrap();
            assert_eq!(choice.candidate_object_ids, looked);
            assert_eq!(choice.candidate_selectable, vec![false; count]);
            assert_eq!((choice.min, choice.max), (0, 0));
            engine
                .apply_command(0, &submit_resolution_choice(vec![]))
                .unwrap();
        }
        assert_eq!(engine.state.players[0].library.len(), count);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn actual_deploy_wrong_actor_three_duplicates_nonplaneswalker_and_unrelated_are_atomic() {
    let cards = [
        "jace_beleren",
        "chandra,_novice_pyromancer",
        "jace_beleren",
        "forest",
        "forest",
        "forest",
        "forest",
    ];
    let (mut engine, looked, _) = paid_look(202_610_084, &cards);
    for (actor, ids) in [
        (1, vec![looked[0]]),
        (0, looked[..3].to_vec()),
        (0, vec![looked[0], looked[0]]),
        (0, vec![looked[3]]),
        (0, vec![999_999]),
    ] {
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(actor, &submit_resolution_choice(ids))
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    engine
        .apply_command(0, &submit_resolution_choice(vec![looked[0]]))
        .unwrap();
    finish_entries(&mut engine);
    assert_eq!(engine.state.objects[&looked[0]].zone, Zone::Battlefield);
}

#[test]
fn actual_deploy_stale_looked_top_owner_zone_generation_or_selected_type_is_atomic() {
    for kind in 0..5 {
        let (mut engine, looked, _) = paid_look(202_610_085, &MIXED);
        match kind {
            0 => engine.state.players[0].library.swap(0, 1),
            1 => engine.state.objects.get_mut(&looked[0]).unwrap().owner = 1,
            2 => engine.state.objects.get_mut(&looked[0]).unwrap().zone = Zone::Graveyard,
            3 => {
                engine.state.zone_change_generation.insert(looked[0], 1);
            }
            _ => engine.state.objects.get_mut(&looked[1]).unwrap().card_id = "forest".into(),
        }
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(0, &submit_resolution_choice(vec![looked[1]]))
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        assert_eq!(engine.state.objects[&looked[3]].zone, Zone::Library);
    }
}

#[test]
fn actual_deploy_stale_unselected_remainder_during_timestamp_is_rejected_before_any_entry() {
    for kind in 0..4 {
        let (mut engine, looked, _) = paid_look(202_610_086, &MIXED);
        engine
            .apply_command(0, &submit_resolution_choice(vec![looked[1], looked[3]]))
            .unwrap();
        match kind {
            0 => engine.state.players[0].library.swap(0, 2),
            1 => engine.state.objects.get_mut(&looked[0]).unwrap().owner = 1,
            2 => engine.state.objects.get_mut(&looked[0]).unwrap().zone = Zone::Exile,
            _ => {
                engine.state.zone_change_generation.insert(looked[0], 1);
            }
        }
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(0, &submit_resolution_choice(vec![looked[1], looked[3]]))
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        for oid in [looked[1], looked[3]] {
            assert_eq!(engine.state.objects[&oid].zone, Zone::Library);
        }
    }
}

#[test]
fn actual_deploy_replacement_then_timestamp_keeps_whole_library_until_simultaneous_commit() {
    let (mut engine, looked, _) = paid_look(202_610_087, &MIXED);
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    let original = engine.state.players[0].library.clone();
    engine
        .apply_command(0, &submit_resolution_choice(vec![looked[1], looked[3]]))
        .unwrap();
    let mut replacement_count = 0;
    let mut timestamp_count = 0;
    let mut bottom_count = 0;
    for _ in 0..10 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            break;
        };
        assert_eq!(engine.state.players[0].library, original);
        assert!(looked
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Library));
        let ids = if pending.presentation.choice_kind == ChoiceKind::ReplacementEffect {
            replacement_count += 1;
            vec![pending.presentation.candidates[0]]
        } else {
            assert_eq!(
                pending.presentation.choice_kind,
                ChoiceKind::SimultaneousEntryOrder
            );
            timestamp_count += 1;
            pending.presentation.candidates.clone()
        };
        let batch = engine
            .apply_command(0, &submit_resolution_choice(ids))
            .unwrap();
        bottom_count += batch.events.iter().filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("in a random order"))).count();
    }
    assert!(replacement_count >= 2);
    assert_eq!(timestamp_count, 1);
    assert_eq!(bottom_count, 1);
    assert!(engine.state.pending_resolution.is_none());
    for oid in [looked[1], looked[3]] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
        assert!(engine.state.objects[&oid].tapped);
    }
    assert_eq!(
        engine.state.objects[&looked[1]].counter_count(tricerules_cards::CounterKind::Loyalty),
        3
    );
    assert_eq!(
        engine.state.objects[&looked[3]].counter_count(tricerules_cards::CounterKind::Loyalty),
        5
    );
}

#[test]
fn actual_deploy_replacement_rejects_stale_remainder_and_concession_still_succeeds() {
    let (mut engine, looked, _) = paid_look_for(202_610_088, &[0, 4, 9], &MIXED);
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    engine
        .apply_command(0, &submit_resolution_choice(vec![looked[1], looked[3]]))
        .unwrap();
    let application = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    engine.state.zone_change_generation.insert(looked[0], 1);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![application]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    let batch = engine
        .apply_command(9, &concede())
        .expect("concession bypasses stale-choice checks and safely abandons");
    assert!(engine.state.pending_resolution.is_none());
    assert!(looked
        .iter()
        .all(|oid| engine.state.objects[oid].zone == Zone::Library));
    assert!(!batch.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("in a random order"))
    ));
}

#[test]
fn actual_deploy_three_player_caster_and_unrelated_departure_preserve_correct_work() {
    for stage in 0..3 {
        for caster in [true, false] {
            let (mut engine, looked, _) = paid_look_for(202_610_089, &[0, 4, 9], &MIXED);
            if stage == 1 {
                inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
                inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
            }
            if stage != 0 {
                engine
                    .apply_command(0, &submit_resolution_choice(vec![looked[1], looked[3]]))
                    .unwrap();
            }
            engine
                .apply_command(if caster { 0 } else { 9 }, &concede())
                .unwrap();
            if caster {
                assert!(engine.state.pending_resolution.is_none());
                assert!(engine.diagnostic_snapshot().unwrap()["state"]
                    ["pending_replacement_event"]
                    .is_null());
                assert!(looked
                    .iter()
                    .all(|oid| !engine.state.objects.contains_key(oid)));
            } else {
                if stage == 0 {
                    engine
                        .apply_command(0, &submit_resolution_choice(vec![looked[1], looked[3]]))
                        .unwrap();
                }
                finish_entries(&mut engine);
                assert_eq!(engine.state.objects[&looked[1]].zone, Zone::Battlefield);
                assert_eq!(engine.state.objects[&looked[3]].zone, Zone::Battlefield);
                assert!(engine.state.stack.is_empty());
            }
        }
    }
}

#[test]
fn actual_deploy_duplicate_names_are_distinct_entries_and_legend_choice_follows_bottoming() {
    let (mut engine, looked, suffix) = paid_look(
        202_610_090,
        &[
            "jace_beleren",
            "jace_beleren",
            "forest",
            "forest",
            "forest",
            "forest",
            "forest",
        ],
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![looked[0], looked[1]]))
        .unwrap();
    let done = engine
        .apply_command(0, &submit_resolution_choice(vec![looked[0], looked[1]]))
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::LegendKeep
    );
    assert_eq!(
        &engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>()[..suffix.len()],
        &suffix
    );
    assert!(done.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("in a random order"))
    ));
    assert!(
        engine.state.stack.is_empty(),
        "spell completed before SBA decision"
    );
    assert_eq!(engine.state.objects[&looked[0]].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&looked[1]].zone, Zone::Battlefield);
    engine
        .apply_command(0, &submit_resolution_choice(vec![looked[1]]))
        .unwrap();
    assert_eq!(engine.state.objects[&looked[0]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&looked[1]].zone, Zone::Battlefield);
}

#[test]
fn actual_deploy_logged_choices_replay_identically_without_random_order_logs() {
    let (mut first, looked, _) = paid_look(202_610_091, &MIXED);
    let (mut second, same, _) = paid_look(202_610_091, &MIXED);
    assert_eq!(looked, same);
    let before = first.diagnostic_snapshot().unwrap();
    assert!(first
        .apply_command(1, &submit_resolution_choice(vec![]))
        .is_err());
    assert_eq!(first.diagnostic_snapshot().unwrap(), before);
    for selected in [vec![looked[1], looked[3]], vec![looked[3], looked[1]]] {
        let a = first
            .apply_command(0, &submit_resolution_choice(selected.clone()))
            .unwrap();
        let b = second
            .apply_command(0, &submit_resolution_choice(selected))
            .unwrap();
        assert_eq!(a, b);
        for batch in [&a, &b] {
            assert!(!batch.events.iter().any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("randomly bottoms") || (log.visible_to_player_id.is_some() && (log.text.contains("Divination") || log.text.contains("Forest"))))));
        }
    }
    assert_eq!(
        first.diagnostic_snapshot().unwrap(),
        second.diagnostic_snapshot().unwrap()
    );
}

fn paid_look(seed: u64, cards: &[&str]) -> (GameEngine, Vec<u32>, Vec<u32>) {
    paid_look_for(seed, &[0, 1], cards)
}

fn paid_stack(seed: u64, players: &[i32]) -> GameEngine {
    assert!(
        tricerules_cards::registry::global()
            .get("deploy_the_gatewatch")
            .is_some(),
        "actual Deploy the Gatewatch must be registered"
    );
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        players,
        20,
        Some(
            players
                .iter()
                .map(|_| deck_with("forest", &["deploy_the_gatewatch"]))
                .collect(),
        ),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "deploy_the_gatewatch");
    let index = hand_index_for_card(&engine, 0, "deploy_the_gatewatch");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            w: 2,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(index, vec![])).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    engine
}

fn paid_look_for(seed: u64, players: &[i32], cards: &[&str]) -> (GameEngine, Vec<u32>, Vec<u32>) {
    let mut engine = paid_stack(seed, players);
    let suffix = engine.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let looked = cards
        .iter()
        .map(|card| inject_library_card(&mut engine, 0, card))
        .collect::<Vec<_>>();
    engine.state.players[0].library = looked.iter().chain(&suffix).copied().collect();
    for _ in 1..players.len() {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    let batch = engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .unwrap();
    let choice = find_resolution_choice(&batch).expect("actual spell parks private look");
    assert_eq!(choice.choice_kind, ChoiceKind::LibraryLook as i32);
    assert_eq!(
        choice.candidate_object_ids,
        looked
            .iter()
            .chain(&suffix)
            .take(7)
            .copied()
            .collect::<Vec<_>>()
    );
    assert_eq!(choice.min, 0);
    assert!(!choice.unique_names);
    assert!(choice.public_reveal.is_none());
    (engine, looked, suffix)
}

#[test]
fn actual_deploy_paid_cast_selects_two_planeswalkers_then_random_bottoms_only_the_rest() {
    let (mut engine, looked, suffix) = paid_look(
        202_610_081,
        &[
            "forest",
            "jace_beleren",
            "divination",
            "chandra,_novice_pyromancer",
            "island",
            "forest",
            "divination",
        ],
    );
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![looked[1], looked[3]]))
        .unwrap();
    let order =
        find_resolution_choice(&batch).expect("simultaneous entry order remains a logged choice");
    assert_eq!(order.choice_kind, ChoiceKind::SimultaneousEntryOrder as i32);
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        looked.iter().chain(&suffix).copied().collect::<Vec<_>>()
    );
    assert!(looked
        .iter()
        .all(|oid| engine.state.objects[oid].zone == Zone::Library));
    let done = engine
        .apply_command(0, &submit_resolution_choice(vec![looked[3], looked[1]]))
        .unwrap();
    for oid in [looked[1], looked[3]] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
        assert!(!engine.state.objects[&oid].tapped);
        assert_eq!(engine.state.zone_change_generation[&oid], 1);
    }
    assert_eq!(
        engine.state.objects[&looked[1]].counter_count(tricerules_cards::CounterKind::Loyalty),
        3
    );
    assert_eq!(
        engine.state.objects[&looked[3]].counter_count(tricerules_cards::CounterKind::Loyalty),
        5
    );
    let library = engine.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(&library[..suffix.len()], &suffix);
    let mut rest = library[suffix.len()..].to_vec();
    rest.sort_unstable();
    let mut expected = vec![looked[0], looked[2], looked[4], looked[5], looked[6]];
    expected.sort_unstable();
    assert_eq!(rest, expected);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    let logs = done
        .events
        .iter()
        .filter_map(|event| {
            if let Some(Ev::Log(log)) = &event.ev {
                Some(log)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        logs.iter()
            .filter(|log| log.text.contains("in a random order"))
            .count(),
        1
    );
    assert!(!logs
        .iter()
        .any(|log| log.text.contains("shuffles") || log.text.contains("randomly bottoms")));
}
