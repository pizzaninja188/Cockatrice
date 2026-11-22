//! Actual-card scenarios for the queue-v55 batch B cohorts.
//!
//! Expectations are printed Oracle values. Relevant rules: CR 701.6 (counter), CR 118.12a
//! (optional payment on resolution), CR 701.22 (scry), CR 701.25 (surveil), CR 702.40a (storm),
//! CR 115.1a/115.3 (zero to two targets), CR 701.26 (tap), CR 701.16 (investigate),
//! CR 502.3/611.2a (skip the next untap step), CR 702.34a-d (flashback) and CR 608.2b (an illegal
//! sole target fizzles the spell, so no rider happens).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, CastMethod, CastSpell, ChoiceKind, ResolutionChoiceDecision,
    SubmitResolutionChoice, TargetRef,
};

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn choice(objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            chosen_object_ids: objects,
            chosen_player_ids: players,
            ..Default::default()
        })),
    }
}

fn hand_ability(e: &GameEngine, source: u32, ability_index: u32) -> RuledCommand {
    use tricerules_proto::ruled::v1::{AbilitySourceZone, ActivateAbility};
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            ..Default::default()
        })),
    }
}

fn spell_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        ..Default::default()
    }
}

fn bolt_at_opponent(e: &mut GameEngine) -> u32 {
    inject_card_into_hand(e, 0, "lightning_bolt");
    give_mana(
        e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "lightning_bolt");
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Lightning Bolt");
    let bolt = e.state.stack.last().expect("bolt on stack").id;
    e.apply_scenario_command(0, &pass())
        .expect("active player passes");
    bolt
}

fn remove_from_stack(e: &mut GameEngine, owner: usize, object: u32) {
    e.state.stack.retain(|item| item.id != object);
    e.state.players[owner].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn move_battlefield_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn cast_counter(e: &mut GameEngine, card: &str, mana: ManaGift, bolt: u32) -> u32 {
    inject_card_into_hand(e, 1, card);
    give_mana(e, 1, mana);
    let slot = hand_index_for_card(e, 1, card);
    e.apply_scenario_command(1, &cast_spell(slot, vec![spell_target(bolt)]))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
    e.state.stack.last().expect("counter on stack").id
}

fn park_counter(e: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    e.apply_scenario_command(1, &pass())
        .expect("counter controller passes");
    e.apply_scenario_command(0, &pass())
        .expect("target controller passes to resolve the counter")
}

// ---------------------------------------------------------------------------
// 04-soft-counter-riders

#[test]
fn q55_b_canary_simplest_miscast_counters_an_instant_without_a_rider() {
    let mut e = engine(55_200_001);
    let bolt = bolt_at_opponent(&mut e);
    let miscast = cast_counter(
        &mut e,
        "miscast",
        ManaGift {
            u: 1,
            ..Default::default()
        },
        bolt,
    );
    let parked = park_counter(&mut e);
    let request = find_resolution_choice(&parked).expect("payment choice");
    assert_eq!(request.generic_mana_cost, 3);
    assert_eq!(request.deciding_player_id, 0);
    submit_mana_resolution_decision(&mut e, 0, ResolutionChoiceDecision::Decline)
        .expect("decline payment");
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&miscast].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 20, "the bolt never resolves");
    assert_eq!(e.state.players[0].life, 20);
}

#[test]
fn q55_b_miscast_rejects_a_creature_spell() {
    let mut e = engine(55_201_001);
    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let bears_slot = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(bears_slot, vec![]))
        .expect("cast Grizzly Bears");
    let bears = e.state.stack.last().expect("bears on stack").id;
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 1, "miscast");
    let slot = hand_index_for_card(&e, 1, "miscast");
    let error = e
        .apply_scenario_command(1, &cast_spell(slot, vec![spell_target(bears)]))
        .expect_err("Miscast can only target an instant or sorcery spell");
    assert!(
        matches!(error, tricerules_core::EngineError::Illegal(_)),
        "the rejection must be a target-filter error, not missing card data: {error:?}"
    );
    assert_eq!(pool(&e, 1), (0, 1, 0, 0, 0, 0), "rejected cast keeps {{U}}");
    assert_eq!(e.state.stack.len(), 1, "only Grizzly Bears remains");
}

#[test]
fn q55_b_soft_counter_riders_apply_when_the_payment_is_declined() {
    // Runeboggle: draw even though the target is countered.
    let mut runeboggle = engine(55_202_001);
    let bolt = bolt_at_opponent(&mut runeboggle);
    let counter = cast_counter(
        &mut runeboggle,
        "runeboggle",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        bolt,
    );
    let library_before = runeboggle.state.players[1].library.len();
    let parked = park_counter(&mut runeboggle);
    assert_eq!(
        find_resolution_choice(&parked)
            .expect("payment")
            .generic_mana_cost,
        1
    );
    submit_mana_resolution_decision(&mut runeboggle, 0, ResolutionChoiceDecision::Decline)
        .expect("decline payment");
    assert_eq!(runeboggle.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(runeboggle.state.objects[&counter].zone, Zone::Graveyard);
    assert_eq!(
        runeboggle.state.players[1].library.len(),
        library_before - 1
    );
    assert_eq!(runeboggle.state.players[1].life, 20);

    // Stymied Hopes: scry 1 even though the target is countered.
    let mut stymied = engine(55_202_002);
    let bolt = bolt_at_opponent(&mut stymied);
    cast_counter(
        &mut stymied,
        "stymied_hopes",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        bolt,
    );
    let parked = park_counter(&mut stymied);
    assert_eq!(
        find_resolution_choice(&parked)
            .expect("payment")
            .generic_mana_cost,
        1
    );
    let batch = submit_mana_resolution_decision(&mut stymied, 0, ResolutionChoiceDecision::Decline)
        .expect("decline payment");
    let scry = find_resolution_choice(&batch).expect("scry parks");
    assert_eq!(scry.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(stymied.state.objects[&bolt].zone, Zone::Graveyard);

    // Offering to Asha: gain 4 even though the target is countered.
    let mut offering = engine(55_202_003);
    let bolt = bolt_at_opponent(&mut offering);
    cast_counter(
        &mut offering,
        "offering_to_asha",
        ManaGift {
            w: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
        bolt,
    );
    let parked = park_counter(&mut offering);
    assert_eq!(
        find_resolution_choice(&parked)
            .expect("payment")
            .generic_mana_cost,
        4
    );
    submit_mana_resolution_decision(&mut offering, 0, ResolutionChoiceDecision::Decline)
        .expect("decline payment");
    assert_eq!(offering.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(offering.state.players[1].life, 24);
    assert_eq!(offering.state.players[0].life, 20);
}

#[test]
fn q55_b_soft_counter_riders_apply_when_the_payment_is_made() {
    // Runeboggle: the bolt resolves, then the counter's controller draws.
    let mut runeboggle = engine(55_203_001);
    let bolt = bolt_at_opponent(&mut runeboggle);
    cast_counter(
        &mut runeboggle,
        "runeboggle",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        bolt,
    );
    let library_before = runeboggle.state.players[1].library.len();
    let parked = park_counter(&mut runeboggle);
    assert!(find_resolution_choice(&parked).is_some());
    give_mana(
        &mut runeboggle,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut runeboggle, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {{1}}");
    resolve_entire_stack_two_player(&mut runeboggle);
    assert_eq!(runeboggle.state.players[1].life, 17, "the bolt resolves");
    assert_eq!(
        runeboggle.state.players[1].library.len(),
        library_before - 1
    );

    // Offering to Asha: gain 4, then take the 3 damage from the bolt.
    let mut offering = engine(55_203_002);
    let bolt = bolt_at_opponent(&mut offering);
    cast_counter(
        &mut offering,
        "offering_to_asha",
        ManaGift {
            w: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
        bolt,
    );
    let parked = park_counter(&mut offering);
    assert!(find_resolution_choice(&parked).is_some());
    give_mana(
        &mut offering,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut offering, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {{4}}");
    resolve_entire_stack_two_player(&mut offering);
    assert_eq!(offering.state.players[1].life, 21, "gain 4 then take 3");
    assert_eq!(offering.state.players[0].life, 20);
}

#[test]
fn q55_b_soft_counters_fizzle_when_the_target_spell_left_the_stack() {
    for (index, (id, mana)) in [
        (
            "miscast",
            ManaGift {
                u: 1,
                ..Default::default()
            },
        ),
        (
            "runeboggle",
            ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        ),
        (
            "stymied_hopes",
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "offering_to_asha",
            ManaGift {
                w: 1,
                u: 1,
                c: 2,
                ..Default::default()
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(55_205_000 + index as u64);
        let bolt = bolt_at_opponent(&mut e);
        let counter = cast_counter(&mut e, id, mana, bolt);
        let library_before = e.state.players[1].library.len();
        // The targeted spell leaves the stack before the soft counter resolves.
        remove_from_stack(&mut e, 0, bolt);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&counter].zone,
            Zone::Graveyard,
            "{id}: the fizzled counter goes to its owner's graveyard"
        );
        assert!(
            e.state.pending_resolution.is_none(),
            "{id}: no payment or scry choice follows a fizzled counter"
        );
        assert_eq!(
            e.state.players[1].library.len(),
            library_before,
            "{id}: no draw"
        );
        assert_eq!(e.state.players[1].life, 20, "{id}: no life gain");
        assert_eq!(e.state.players[0].life, 20, "{id}: no bolt damage");
    }
}

#[test]
fn q55_b_hindering_touch_storm_copies_counter_each_target() {
    let mut e = engine(55_204_001);
    let bolt = bolt_at_opponent(&mut e);
    cast_counter(
        &mut e,
        "hindering_touch",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        bolt,
    );
    pass_both_players(&mut e);
    let copy_choice = e
        .state
        .pending_resolution
        .as_ref()
        .expect("storm copy target choice");
    assert_eq!(copy_choice.deciding_player, 1);
    assert!(copy_choice.presentation.candidates.contains(&bolt));
    e.apply_scenario_command(1, &choice(vec![bolt], vec![]))
        .expect("choose the bolt for the storm copy");
    assert_eq!(
        e.state.stack.iter().filter(|item| item.is_copy).count(),
        1,
        "one storm copy was created"
    );
    pass_both_players(&mut e);
    let payment = e
        .state
        .pending_resolution
        .as_ref()
        .expect("storm copy's conditional payment");
    assert_eq!(payment.deciding_player, 0);
    submit_mana_resolution_decision(&mut e, 0, ResolutionChoiceDecision::Decline)
        .expect("decline the copy's payment");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 20, "the bolt never resolves");
}

// ---------------------------------------------------------------------------
// 05-surveil-spells

fn seat_on_top(e: &mut GameEngine, player: usize, count: usize) -> Vec<u32> {
    let mut cards = Vec::new();
    for _ in 0..count {
        let object = inject_library_card(e, player, "grizzly_bears");
        e.state.players[player].library.retain(|oid| *oid != object);
        e.state.players[player].library.push_front(object);
        cards.push(object);
    }
    cards
}

#[test]
fn q55_b_canary_simplest_contingency_plan_surveils_five() {
    let mut e = engine(55_210_001);
    let cards = seat_on_top(&mut e, 0, 5);
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "contingency_plan");
    let slot = hand_index_for_card(&e, 0, "contingency_plan");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Contingency Plan");
    let batch = {
        e.apply_scenario_command(0, &pass()).expect("pass");
        e.apply_scenario_command(1, &pass()).expect("resolve")
    };
    let request = find_resolution_choice(&batch).expect("surveil parks");
    assert_eq!(request.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((request.min, request.max), (0, 5));
    e.apply_scenario_command(0, &choice(cards.clone(), vec![]))
        .expect("put all five cards into the graveyard");
    for card in &cards {
        assert_eq!(e.state.objects[card].zone, Zone::Graveyard);
    }
    assert_eq!(e.state.players[0].library.len(), library_before - 5);
    assert_eq!(
        e.state.players[0]
            .graveyard
            .iter()
            .filter(|oid| e.state.objects[oid].card_id == "contingency_plan")
            .count(),
        1,
        "the spell finished resolving into the graveyard"
    );
}

#[test]
fn q55_b_taigams_scheming_surveils_five_and_orders_the_rest_on_top() {
    let mut e = engine(55_211_001);
    let cards = seat_on_top(&mut e, 0, 5);
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "taigams_scheming");
    let slot = hand_index_for_card(&e, 0, "taigams_scheming");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Taigam's Scheming");
    let batch = {
        e.apply_scenario_command(0, &pass()).expect("pass");
        e.apply_scenario_command(1, &pass()).expect("resolve")
    };
    let request = find_resolution_choice(&batch).expect("surveil parks");
    assert_eq!(request.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((request.min, request.max), (0, 5));
    let to_graveyard = vec![cards[0], cards[1]];
    let ordering = e
        .apply_scenario_command(0, &choice(to_graveyard.clone(), vec![]))
        .expect("put two cards into the graveyard");
    for card in &to_graveyard {
        assert_eq!(e.state.objects[card].zone, Zone::Graveyard);
    }
    let order = find_resolution_choice(&ordering).expect("the kept cards need an order");
    assert_eq!(order.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((order.min, order.max), (3, 3));
    let mut expected_candidates = cards[2..5].to_vec();
    expected_candidates.reverse();
    assert_eq!(order.candidate_object_ids, expected_candidates);
    let candidates = order.candidate_object_ids.clone();
    let submitted = vec![candidates[1], candidates[2], candidates[0]];
    e.apply_scenario_command(0, &choice(submitted.clone(), vec![]))
        .expect("order the cards kept on top");
    let top: Vec<u32> = e.state.players[0].library.iter().take(3).copied().collect();
    let expected_top: Vec<u32> = submitted.iter().rev().copied().collect();
    assert_eq!(
        top, expected_top,
        "the submitted permutation sets the top order"
    );
    assert_eq!(e.state.players[0].library.len(), library_before - 2);
    assert_eq!(
        e.state.players[0]
            .graveyard
            .iter()
            .filter(|oid| e.state.objects[oid].card_id == "taigams_scheming")
            .count(),
        1,
        "the spell finished resolving into the graveyard"
    );
}

#[test]
fn q55_b_otherworldly_gaze_surveils_three_and_flashes_back() {
    let mut e = engine(55_212_001);
    let cards = seat_on_top(&mut e, 0, 3);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "otherworldly_gaze");
    let slot = hand_index_for_card(&e, 0, "otherworldly_gaze");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Otherworldly Gaze");
    let batch = {
        e.apply_scenario_command(0, &pass()).expect("pass");
        e.apply_scenario_command(1, &pass()).expect("resolve")
    };
    let request = find_resolution_choice(&batch).expect("surveil parks");
    assert_eq!(request.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((request.min, request.max), (0, 3));
    e.apply_scenario_command(0, &choice(cards.clone(), vec![]))
        .expect("put all three cards into the graveyard");
    assert!(cards
        .iter()
        .all(|card| e.state.objects[card].zone == Zone::Graveyard));
    let gaze = e.state.players[0]
        .graveyard
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == "otherworldly_gaze")
        .unwrap_or_else(|| {
            panic!(
                "Otherworldly Gaze not in graveyard; graveyard={:?}; stack={:?}; zones={:?}",
                e.state.players[0].graveyard,
                e.state
                    .stack
                    .iter()
                    .map(|s| (s.id, s.card_id.clone()))
                    .collect::<Vec<_>>(),
                e.state
                    .objects
                    .iter()
                    .filter(|(_, o)| o.card_id == "otherworldly_gaze")
                    .map(|(id, o)| (*id, o.zone))
                    .collect::<Vec<_>>()
            )
        });
    let generation = e
        .state
        .zone_change_generation
        .get(&gaze)
        .copied()
        .unwrap_or(0);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                cast_method: CastMethod::Flashback as i32,
                source: Some(graveyard_cast_source(gaze, generation)),
                ..Default::default()
            })),
        },
    )
    .expect("flash back Otherworldly Gaze");
    let batch = {
        e.apply_scenario_command(0, &pass()).expect("pass");
        e.apply_scenario_command(1, &pass()).expect("resolve")
    };
    let request = find_resolution_choice(&batch).expect("flashback surveil parks");
    assert_eq!((request.min, request.max), (0, 3));
    let looked_at = request.candidate_object_ids.clone();
    e.apply_scenario_command(0, &choice(looked_at, vec![]))
        .expect("put all three looked-at cards into the graveyard");
    assert_eq!(
        e.state.objects[&gaze].zone,
        Zone::Exile,
        "a flashed-back spell is exiled"
    );
}

// ---------------------------------------------------------------------------
// 06-tap-two-riders

#[test]
fn q55_b_canary_simplest_repel_the_darkness_draws_without_targets() {
    let mut e = engine(55_220_001);
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "repel_the_darkness");
    let slot = hand_index_for_card(&e, 0, "repel_the_darkness");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast with zero targets");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].library.len(), library_before - 1);
}

#[test]
fn q55_b_tap_two_spells_tap_each_chosen_creature_and_apply_their_riders() {
    // Repel the Darkness: two targets, both tapped, draw one.
    let mut repel = engine(55_221_001);
    let first = inject_creature_on_battlefield(&mut repel, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut repel, 1, "grizzly_bears");
    let library_before = repel.state.players[0].library.len();
    give_mana(
        &mut repel,
        0,
        ManaGift {
            w: 2,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut repel, 0, "repel_the_darkness");
    let slot = hand_index_for_card(&repel, 0, "repel_the_darkness");
    repel
        .apply_scenario_command(
            0,
            &cast_spell(
                slot,
                vec![target_object(first)[0], target_object(second)[0]],
            ),
        )
        .expect("cast with two targets");
    resolve_entire_stack_two_player(&mut repel);
    assert!(repel.state.objects[&first].tapped);
    assert!(repel.state.objects[&second].tapped);
    assert_eq!(repel.state.players[0].library.len(), library_before - 1);

    // Djeru's Renunciation: two targets tapped; cycling draws.
    let mut djeru = engine(55_221_002);
    let first = inject_creature_on_battlefield(&mut djeru, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut djeru, 1, "grizzly_bears");
    give_mana(
        &mut djeru,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut djeru, 0, "djerus_renunciation");
    let slot = hand_index_for_card(&djeru, 0, "djerus_renunciation");
    djeru
        .apply_scenario_command(
            0,
            &cast_spell(
                slot,
                vec![target_object(first)[0], target_object(second)[0]],
            ),
        )
        .expect("cast Djeru's Renunciation");
    resolve_entire_stack_two_player(&mut djeru);
    assert!(djeru.state.objects[&first].tapped);
    assert!(djeru.state.objects[&second].tapped);
    let cycle_card = inject_card_into_hand(&mut djeru, 0, "djerus_renunciation");
    let drawn = seat_on_top(&mut djeru, 0, 1)[0];
    give_mana(
        &mut djeru,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    djeru
        .apply_scenario_command(0, &hand_ability(&djeru, cycle_card, 0))
        .expect("cycle Djeru's Renunciation");
    resolve_entire_stack_two_player(&mut djeru);
    assert_eq!(djeru.state.objects[&cycle_card].zone, Zone::Graveyard);
    assert_eq!(djeru.state.objects[&drawn].zone, Zone::Hand);
}

#[test]
fn q55_b_repel_the_darkness_handles_partial_and_all_illegal_targets() {
    // One of two targets illegal: the remaining creature taps and Repel draws.
    let mut partial = engine(55_225_001);
    let first = inject_creature_on_battlefield(&mut partial, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut partial, 1, "grizzly_bears");
    let library_before = partial.state.players[0].library.len();
    give_mana(
        &mut partial,
        0,
        ManaGift {
            w: 2,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut partial, 0, "repel_the_darkness");
    let slot = hand_index_for_card(&partial, 0, "repel_the_darkness");
    partial
        .apply_scenario_command(
            0,
            &cast_spell(
                slot,
                vec![target_object(first)[0], target_object(second)[0]],
            ),
        )
        .expect("cast with two targets");
    move_battlefield_to_graveyard(&mut partial, 1, second);
    resolve_entire_stack_two_player(&mut partial);
    assert!(partial.state.objects[&first].tapped);
    assert_eq!(partial.state.players[0].library.len(), library_before - 1);

    // The sole target is illegal: the spell fizzles and does not draw.
    let mut sole = engine(55_225_002);
    let target = inject_creature_on_battlefield(&mut sole, 1, "grizzly_bears");
    let library_before = sole.state.players[0].library.len();
    give_mana(
        &mut sole,
        0,
        ManaGift {
            w: 2,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut sole, 0, "repel_the_darkness");
    let slot = hand_index_for_card(&sole, 0, "repel_the_darkness");
    sole.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast with one target");
    move_battlefield_to_graveyard(&mut sole, 1, target);
    resolve_entire_stack_two_player(&mut sole);
    assert_eq!(sole.state.players[0].library.len(), library_before);

    // Both targets illegal: the spell fizzles and does not draw.
    let mut both = engine(55_225_003);
    let first = inject_creature_on_battlefield(&mut both, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut both, 1, "grizzly_bears");
    let library_before = both.state.players[0].library.len();
    give_mana(
        &mut both,
        0,
        ManaGift {
            w: 2,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut both, 0, "repel_the_darkness");
    let slot = hand_index_for_card(&both, 0, "repel_the_darkness");
    both.apply_scenario_command(
        0,
        &cast_spell(
            slot,
            vec![target_object(first)[0], target_object(second)[0]],
        ),
    )
    .expect("cast with two targets");
    move_battlefield_to_graveyard(&mut both, 1, first);
    move_battlefield_to_graveyard(&mut both, 1, second);
    resolve_entire_stack_two_player(&mut both);
    assert_eq!(both.state.players[0].library.len(), library_before);
}

#[test]
fn q55_b_feeling_of_dread_keeps_the_legal_target_and_flashes_back() {
    let mut e = engine(55_222_001);
    let first = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "feeling_of_dread");
    let slot = hand_index_for_card(&e, 0, "feeling_of_dread");
    e.apply_scenario_command(
        0,
        &cast_spell(
            slot,
            vec![target_object(first)[0], target_object(second)[0]],
        ),
    )
    .expect("cast Feeling of Dread");
    // The second target leaves; the remaining legal target is still tapped (pinned ruling).
    e.state.players[1]
        .battlefield
        .retain(|object| *object != second);
    e.state.players[1].graveyard.push(second);
    e.state.objects.get_mut(&second).expect("second").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(second).or_default() += 1;
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&first].tapped);
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);

    // Flashback from the graveyard.
    let dread = e.state.players[0]
        .graveyard
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == "feeling_of_dread")
        .expect("Feeling of Dread in graveyard");
    let generation = e
        .state
        .zone_change_generation
        .get(&dread)
        .copied()
        .unwrap_or(0);
    let third = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                cast_method: CastMethod::Flashback as i32,
                source: Some(graveyard_cast_source(dread, generation)),
                targets: target_object(third),
                ..Default::default()
            })),
        },
    )
    .expect("flash back Feeling of Dread");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&third].tapped);
    assert_eq!(e.state.objects[&dread].zone, Zone::Exile);
}

#[test]
fn q55_b_expose_evil_investigates_with_or_without_targets() {
    // Zero targets: investigate only.
    let mut e = engine(55_223_001);
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "expose_evil");
    let slot = hand_index_for_card(&e, 0, "expose_evil");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast with zero targets");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 1);

    // All targets illegal: the spell fizzles and no Clue is created.
    let mut stale = engine(55_223_002);
    let target = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
    give_mana(
        &mut stale,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut stale, 0, "expose_evil");
    let slot = hand_index_for_card(&stale, 0, "expose_evil");
    stale
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast with one target");
    stale.state.players[1]
        .battlefield
        .retain(|object| *object != target);
    stale.state.players[1].graveyard.push(target);
    stale.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
    *stale
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut stale);
    assert!(
        battlefield_token_oids(&stale, 0, "clue").is_empty(),
        "no Clue when the only target is illegal"
    );
}

#[test]
fn q55_b_decision_paralysis_taps_two_and_skips_their_next_untap() {
    let mut e = engine(55_224_001);
    let first = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "decision_paralysis");
    let slot = hand_index_for_card(&e, 0, "decision_paralysis");
    e.apply_scenario_command(
        0,
        &cast_spell(
            slot,
            vec![target_object(first)[0], target_object(second)[0]],
        ),
    )
    .expect("cast Decision Paralysis");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&first].tapped);
    assert!(e.state.objects[&second].tapped);
    assert_eq!(e.state.skip_next_untap.len(), 2);
}
