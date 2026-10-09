//! Exact Into the Wilds upkeep, private look and optional land entry contract.
use super::helpers::*;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::ChoiceKind;

fn own_upkeep(seed: u64) -> GameEngine {
    own_upkeep_for(seed, &[0, 1])
}

fn own_upkeep_for(seed: u64, players: &[i32]) -> GameEngine {
    assert!(
        tricerules_cards::registry::global()
            .get("into_the_wilds")
            .is_some(),
        "actual Into the Wilds must be registered"
    );
    let decks = players.iter().map(|_| deck_with("forest", &[])).collect();
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        players,
        20,
        Some(decks),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "into_the_wilds");
    end_active_turn(&mut engine, players[0]);
    for &opponent in &players[1..] {
        assert!(
            engine.state.stack.is_empty(),
            "opponent upkeep does not trigger"
        );
        pass_priority_round(&mut engine);
        pass_priority_round(&mut engine);
        end_active_turn(&mut engine, opponent);
    }
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "controller upkeep triggers before draw"
    );
    engine
}

fn set_current_top(engine: &mut GameEngine, card: &str) -> u32 {
    let oid = inject_library_card(engine, 0, card);
    engine.state.players[0].library.retain(|id| *id != oid);
    engine.state.players[0].library.push_front(oid);
    oid
}

fn resolve_look(engine: &mut GameEngine) {
    pass_priority_round(engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("private look remains pending");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryLook);
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.min, 0);
}

#[test]
fn actual_into_the_wilds_accepts_resolution_time_top_without_using_land_play() {
    let mut engine = own_upkeep(202_610_061);
    let top = set_current_top(&mut engine, "forest");
    let before = engine
        .state
        .zone_change_generation
        .get(&top)
        .copied()
        .unwrap_or(0);
    resolve_look(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.presentation.candidates, vec![top]);
    assert_eq!(pending.presentation.max, 1);
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![top]))
        .unwrap();
    assert_eq!(engine.state.objects[&top].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&top].tapped);
    assert_eq!(engine.state.zone_change_generation[&top], before + 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(!batch.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::Log(log)) if log.text.contains("shuffles"))));
    assert_eq!(engine.state.lands_played_this_turn, 0);
}

#[test]
fn actual_into_the_wilds_decline_and_nonland_acknowledgement_keep_exact_next_draw() {
    for (seed, card, max) in [(202_610_062, "forest", 1), (202_610_063, "divination", 0)] {
        let mut engine = own_upkeep(seed);
        let top = set_current_top(&mut engine, card);
        let order = engine.state.players[0].library.clone();
        let generations = engine.state.zone_change_generation.clone();
        resolve_look(&mut engine);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .max,
            max
        );
        engine
            .apply_command(0, &submit_resolution_choice(vec![]))
            .unwrap();
        assert_eq!(engine.state.players[0].library, order);
        assert_eq!(engine.state.zone_change_generation, generations);
        pass_both_players(&mut engine);
        assert_eq!(engine.state.turn_step, TurnStep::Draw);
        assert_eq!(engine.state.objects[&top].zone, Zone::Hand);
        assert!(engine.state.players[0].hand.contains(&top));
    }
}

#[test]
fn actual_into_the_wilds_wrong_actor_and_duplicate_answer_are_atomic() {
    let mut engine = own_upkeep(202_610_064);
    let top = set_current_top(&mut engine, "forest");
    resolve_look(&mut engine);
    for (actor, selected) in [(1, vec![top]), (0, vec![top, top]), (0, vec![999_999])] {
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(actor, &submit_resolution_choice(selected))
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    engine
        .apply_command(0, &submit_resolution_choice(vec![top]))
        .unwrap();
    assert_eq!(engine.state.objects[&top].zone, Zone::Battlefield);
}

#[test]
fn actual_into_the_wilds_empty_library_is_a_noop_without_a_picker() {
    let mut engine = own_upkeep(202_610_065);
    for oid in engine.state.players[0]
        .library
        .drain(..)
        .collect::<Vec<_>>()
    {
        engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Exile;
        engine.state.players[0].exile.push(oid);
    }
    pass_both_players(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.lands_played_this_turn, 0);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
}

#[test]
fn actual_into_the_wilds_stale_top_generation_zone_owner_and_land_type_are_atomic() {
    for case in 0..6 {
        let mut engine = own_upkeep(202_610_066 + case);
        let top = set_current_top(&mut engine, "forest");
        resolve_look(&mut engine);
        match case {
            0 => {
                *engine.state.zone_change_generation.entry(top).or_default() += 1;
            }
            1 => {
                set_current_top(&mut engine, "forest");
            }
            2 => {
                engine.state.objects.get_mut(&top).unwrap().zone = Zone::Hand;
            }
            3 => {
                engine.state.objects.get_mut(&top).unwrap().owner = 1;
            }
            4 => {
                engine.state.objects.get_mut(&top).unwrap().card_id = "divination".into();
            }
            5 => {
                engine.state.players[0].library.retain(|id| *id != top);
            }
            _ => unreachable!(),
        }
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(
            engine
                .apply_command(0, &submit_resolution_choice(vec![top]))
                .is_err(),
            "case {case}"
        );
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before, "case {case}");
        if case == 4 {
            engine
                .apply_command(0, &submit_resolution_choice(vec![]))
                .expect("changed land can still be declined");
            assert_eq!(engine.state.players[0].library.front(), Some(&top));
        }
    }
}

#[test]
fn actual_into_the_wilds_nonland_cannot_be_accepted_but_can_be_acknowledged() {
    let mut engine = own_upkeep(202_610_072);
    let top = set_current_top(&mut engine, "divination");
    resolve_look(&mut engine);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![top]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    assert_eq!(engine.state.players[0].library.front(), Some(&top));
}

#[test]
fn actual_into_the_wilds_source_departure_preserves_frozen_trigger_controller() {
    let mut engine = own_upkeep(202_610_073);
    let source = engine.state.stack[0].source_permanent_id.unwrap();
    // Explicit source-departure fixture; the actual printed trigger is already on the stack.
    engine.state.players[0]
        .battlefield
        .retain(|id| *id != source);
    engine.state.players[0].graveyard.push(source);
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_default() += 1;
    let top = set_current_top(&mut engine, "forest");
    resolve_look(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![top]))
        .unwrap();
    assert_eq!(engine.state.objects[&top].controller, 0);
    assert_eq!(engine.state.objects[&top].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn actual_into_the_wilds_ordinary_tapped_entry_and_etb_are_preserved() {
    let mut engine = own_upkeep(202_610_074);
    let top = set_current_top(&mut engine, "jungle_hollow");
    resolve_look(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![top]))
        .unwrap();
    assert_eq!(engine.state.objects[&top].zone, Zone::Battlefield);
    assert!(engine.state.objects[&top].tapped);
    assert_eq!(engine.state.lands_played_this_turn, 0);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "normal Jungle Hollow ETB trigger"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 21);
    pass_both_players(&mut engine); // upkeep to draw
    pass_both_players(&mut engine); // draw to main
    let hand = hand_index_for_card(&engine, 0, "forest");
    engine
        .apply_command(0, &play_land(hand))
        .expect("ordinary land play remains available");
    assert_eq!(engine.state.lands_played_this_turn, 1);
}

#[test]
fn actual_into_the_wilds_controller_departure_clears_the_private_picker() {
    let mut engine = own_upkeep_for(202_610_075, &[0, 4, 9]);
    set_current_top(&mut engine, "forest");
    resolve_look(&mut engine);
    engine.apply_command(0, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn actual_into_the_wilds_replacement_pause_and_controller_departure_are_safe() {
    for leave in [false, true] {
        let mut engine = own_upkeep_for(202_610_076, &[0, 4, 9]);
        inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
        inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
        let top = set_current_top(&mut engine, "forest");
        resolve_look(&mut engine);
        engine
            .apply_command(0, &submit_resolution_choice(vec![top]))
            .unwrap();
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("replacement order parks entry");
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ReplacementEffect
        );
        assert_eq!(engine.state.objects[&top].zone, Zone::Library);
        if leave {
            engine.apply_command(0, &concede()).unwrap();
            assert!(engine.state.pending_resolution.is_none());
            assert!(
                engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"]
                    .is_null()
            );
        } else {
            let order = pending.presentation.candidates[0];
            engine
                .apply_command(0, &submit_resolution_choice(vec![order]))
                .unwrap();
            assert_eq!(engine.state.objects[&top].zone, Zone::Battlefield);
            assert!(engine.state.objects[&top].tapped);
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.is_empty());
        }
    }
}

#[test]
fn actual_into_the_wilds_logged_choice_replays_identically() {
    let mut first = own_upkeep(202_610_077);
    let mut second = own_upkeep(202_610_077);
    let top = set_current_top(&mut first, "forest");
    assert_eq!(top, set_current_top(&mut second, "forest"));
    resolve_look(&mut first);
    resolve_look(&mut second);
    let command = submit_resolution_choice(vec![top]);
    assert_eq!(
        first.apply_command(0, &command).unwrap(),
        second.apply_command(0, &command).unwrap()
    );
    assert_eq!(
        first.diagnostic_snapshot().unwrap(),
        second.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn actual_into_the_wilds_real_cast_pays_exact_green_cost() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_078,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["into_the_wilds"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "into_the_wilds");
    let index = hand_index_for_card(&engine, 0, "into_the_wilds");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
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
            g: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(index, vec![])).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert_eq!(
        engine.state.players[0]
            .battlefield
            .iter()
            .filter(|id| engine.state.objects[id].card_id == "into_the_wilds")
            .count(),
        1
    );
}

#[test]
fn actual_into_the_wilds_private_image_and_public_wording_do_not_leak_land_eligibility() {
    let mut public_logs = Vec::new();
    let mut prompts = Vec::new();
    for card in ["forest", "divination"] {
        let mut engine = own_upkeep(202_610_079);
        let top = set_current_top(&mut engine, card);
        let first = engine.state.priority_player_id();
        engine.apply_command(first, &pass()).unwrap();
        let batch = engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
        let choice = find_resolution_choice(&batch).unwrap();
        assert_eq!(choice.candidate_object_ids, [top]);
        assert_eq!(choice.candidate_card_ids, [card]);
        assert_eq!(choice.candidate_selectable, [card == "forest"]);
        assert_eq!(choice.min, 0);
        assert_eq!(choice.max, u32::from(card == "forest"));
        assert!(choice.public_reveal.is_none());
        prompts.push(choice.prompt_text.clone());
        let mut public = Vec::new();
        let mut private = Vec::new();
        for event in &batch.events {
            if let Some(Ev::Log(log)) = &event.ev {
                if log.visible_to_player_id.is_none() {
                    public.push(log.text.clone());
                } else {
                    assert_eq!(log.visible_to_player_id, Some(0));
                    private.push(log.text.clone());
                }
            }
        }
        assert!(private
            .iter()
            .any(|text| text.contains(if card == "forest" {
                "Forest"
            } else {
                "Divination"
            })));
        public_logs.push(public);
    }
    assert_eq!(prompts[0], prompts[1]);
    assert_eq!(public_logs[0], public_logs[1]);
}
