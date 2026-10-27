//! Actual paid Carth casts and entry triggers; hand, mana and library order are fixture setup.
use std::collections::BTreeSet;

use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::Zone;

const CARTH: &str = "carth_the_lion";
const ASHIOK: &str = "ashiok,_dream_render";

fn game(seed: u64, opposing_ashiok: bool) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    assert!(
        registry.get(CARTH).is_some(),
        "Carth the Lion must be registered"
    );
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]), deck_with("island", &[])]),
        true,
    )
    .expect("create a two-player game");
    advance_to_main1_from_game_start(&mut engine);
    if opposing_ashiok {
        // Synthetic hand placement followed by a real engine entry registers Ashiok's static.
        let ashiok = inject_card_into_hand(&mut engine, 1, ASHIOK);
        assert_eq!(move_ready_to_battlefield(&mut engine, 1, ASHIOK), ashiok);
        assert_eq!(engine.state.objects[&ashiok].zone, Zone::Battlefield);
    }
    engine
}

fn put_on_top(engine: &mut GameEngine, ids: &[&str]) -> Vec<u32> {
    // Synthetic library order gives the real entry trigger a known physical cohort.
    let objects: Vec<_> = ids
        .iter()
        .map(|id| inject_library_card(engine, 0, id))
        .collect();
    engine.state.players[0]
        .library
        .retain(|id| !objects.contains(id));
    for &id in objects.iter().rev() {
        engine.state.players[0].library.push_front(id);
    }
    objects
}

fn cast_and_resolve_entry(engine: &mut GameEngine) -> (u32, u32, RuledEventBatch) {
    // Only the injected hand card and mana are synthetic; cast, payment and entry are real.
    let carth = inject_card_into_hand(engine, 0, CARTH);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 2,
            b: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, CARTH);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Carth for {2}{B}{G}");
    let pool = &engine.state.players[0].mana_pool;
    assert_eq!((pool.colorless, pool.black, pool.green), (0, 0, 0));
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&carth].zone, Zone::Battlefield);
    let trigger = engine
        .state
        .stack
        .last()
        .expect("Carth's entry trigger is on the stack")
        .id;
    let batch = pass_both_players_with_batch(engine);
    (carth, trigger, batch)
}

fn assert_no_reveal(batch: &RuledEventBatch) {
    assert!(!batch
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
}

fn assert_finished_once(engine: &GameEngine, batch: &RuledEventBatch, trigger: u32) {
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger
            ))
            .count(),
        1
    );
}

fn assert_random_bottom_cohort(engine: &GameEngine, expected: &[u32]) {
    let library: Vec<_> = engine.state.players[0].library.iter().copied().collect();
    let bottom: BTreeSet<_> = library[library.len() - expected.len()..]
        .iter()
        .copied()
        .collect();
    assert_eq!(bottom, expected.iter().copied().collect());
}

#[test]
fn carth_entry_looks_at_seven_and_reveals_only_the_selected_planeswalker() {
    let mut engine = game(2_610_271_000, false);
    let top = put_on_top(
        &mut engine,
        &[
            "forest",
            "jace,_unraveler_of_secrets",
            "island",
            "daretti,_scrap_savant",
            "swamp",
            "grizzly_bears",
            "sol_ring",
            "mountain",
        ],
    );
    let (_, trigger, batch) = cast_and_resolve_entry(&mut engine);
    let choice = find_resolution_choice(&batch).expect("entry ability offers the private look");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.candidate_object_ids, top[..7]);
    assert_eq!(
        choice.candidate_selectable,
        [false, true, false, true, false, false, false]
    );
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.public_reveal.is_none());
    assert_no_reveal(&batch);
    let result = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[1]]))
        .expect("select Jace from the looked-at cards");
    let reveals: Vec<_> = result
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .collect();
    assert_eq!(reveals.len(), 1);
    assert_eq!(reveals[0].cards.len(), 1);
    assert_eq!(reveals[0].cards[0].object_id, top[1]);
    assert_eq!(reveals[0].cards[0].card_name, "Jace, Unraveler of Secrets");
    assert!(engine.state.players[0].hand.contains(&top[1]));
    assert_eq!(
        engine.state.players[0].library.front().copied(),
        Some(top[7])
    );
    assert_random_bottom_cohort(&engine, &[top[0], top[2], top[3], top[4], top[5], top[6]]);
    assert_finished_once(&engine, &result, trigger);
}

#[test]
fn carth_entry_may_decline_and_handles_no_planeswalker_hit() {
    for has_hit in [true, false] {
        let mut engine = game(2_610_271_001 + u64::from(has_hit), false);
        let top = put_on_top(
            &mut engine,
            &[
                if has_hit {
                    "jace,_unraveler_of_secrets"
                } else {
                    "forest"
                },
                "island",
                "swamp",
                "mountain",
                "forest",
                "sol_ring",
                "grizzly_bears",
                "plains",
            ],
        );
        let (_, trigger, batch) = cast_and_resolve_entry(&mut engine);
        let choice = find_resolution_choice(&batch).expect("the nonempty cohort is looked at");
        assert_eq!(choice.candidate_object_ids, top[..7]);
        assert_eq!((choice.min, choice.max), (0, u32::from(has_hit)));
        assert_eq!(
            choice
                .candidate_selectable
                .iter()
                .filter(|&&can_select| can_select)
                .count(),
            usize::from(has_hit)
        );
        assert!(choice.public_reveal.is_none());
        assert_no_reveal(&batch);
        let result = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("choose no card");
        assert_no_reveal(&result);
        assert_eq!(
            engine.state.players[0].library.front().copied(),
            Some(top[7])
        );
        assert_random_bottom_cohort(&engine, &top[..7]);
        assert_finished_once(&engine, &result, trigger);
    }
}

#[test]
fn carth_entry_handles_short_and_empty_libraries() {
    for empty in [false, true] {
        let mut engine = game(2_610_271_003 + u64::from(empty), false);
        // Synthetic shortening happens after the normal opening draw, before the real cast.
        engine.state.players[0].library.clear();
        let top = if empty {
            Vec::new()
        } else {
            put_on_top(
                &mut engine,
                &["forest", "jace,_unraveler_of_secrets", "island"],
            )
        };
        let (_, trigger, batch) = cast_and_resolve_entry(&mut engine);
        if empty {
            assert!(find_resolution_choice(&batch).is_none());
            assert_no_reveal(&batch);
            assert_finished_once(&engine, &batch, trigger);
        } else {
            let choice = find_resolution_choice(&batch).expect("look at all three remaining cards");
            assert_eq!(choice.candidate_object_ids, top);
            assert_eq!(choice.candidate_selectable, [false, true, false]);
            let result = engine
                .apply_scenario_command(0, &submit_resolution_choice(vec![top[1]]))
                .expect("choose the planeswalker from the short library");
            assert!(engine.state.players[0].hand.contains(&top[1]));
            assert_random_bottom_cohort(&engine, &[top[0], top[2]]);
            assert_finished_once(&engine, &result, trigger);
        }
    }
}

#[test]
fn carth_entry_looks_despite_an_opponents_ashiok() {
    let mut engine = game(2_610_271_005, true);
    let top = put_on_top(
        &mut engine,
        &["forest", "jace,_unraveler_of_secrets", "island"],
    );
    let (_, trigger, batch) = cast_and_resolve_entry(&mut engine);
    let choice = find_resolution_choice(&batch).expect("Ashiok prohibits searching, not looking");
    assert_eq!(&choice.candidate_object_ids[..3], top);
    assert_eq!(&choice.candidate_selectable[..3], [false, true, false]);
    assert!(choice.public_reveal.is_none());
    let result = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[1]]))
        .expect("choose the looked-at planeswalker under Ashiok");
    assert!(engine.state.players[0].hand.contains(&top[1]));
    assert_finished_once(&engine, &result, trigger);
}

#[test]
fn carth_random_bottom_does_not_disclose_the_resulting_order() {
    let mut engine = game(2_610_271_006, false);
    let top = put_on_top(
        &mut engine,
        &[
            "forest",
            "jace,_unraveler_of_secrets",
            "island",
            "daretti,_scrap_savant",
            "swamp",
            "grizzly_bears",
            "sol_ring",
            "mountain",
        ],
    );
    let (_, trigger, batch) = cast_and_resolve_entry(&mut engine);
    let choice = find_resolution_choice(&batch).expect("the real entry trigger looks at seven");
    assert_eq!(choice.candidate_object_ids, top[..7]);
    assert!(choice.public_reveal.is_none());
    assert_no_reveal(&batch);
    let result = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[1]]))
        .expect("select Jace and randomly bottom the remaining cards");
    let reveals: Vec<_> = result
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .collect();
    assert_eq!(reveals.len(), 1);
    assert_eq!(reveals[0].cards.len(), 1);
    assert_eq!(reveals[0].cards[0].object_id, top[1]);
    assert_finished_once(&engine, &result, trigger);
    assert_random_bottom_cohort(&engine, &[top[0], top[2], top[3], top[4], top[5], top[6]]);

    let library: Vec<_> = engine.state.players[0].library.iter().copied().collect();
    let actual_order = &library[library.len() - 6..];
    let original_order = [top[0], top[2], top[3], top[4], top[5], top[6]];
    assert_ne!(
        actual_order, original_order,
        "fixture must distinguish the known look order from the new random order"
    );
    let names = actual_order
        .iter()
        .map(|id| {
            let card_id = &engine.state.objects[id].card_id;
            tricerules_cards::registry::global()
                .get(card_id)
                .unwrap()
                .name
                .clone()
        })
        .collect::<Vec<_>>()
        .join(", ");
    for event in &result.events {
        if let Some(Ev::Log(log)) = &event.ev {
            assert!(
                !log.text.contains(&names),
                "new random library order leaked to {:?}: {}",
                log.visible_to_player_id,
                log.text
            );
        }
    }
}

#[test]
fn carth_random_bottom_replays_accepted_wire_commands_deterministically() {
    use prost::Message;

    fn fixture() -> (GameEngine, Vec<u32>) {
        let mut engine = game(2_610_271_007, false);
        let top = put_on_top(
            &mut engine,
            &[
                "forest",
                "jace,_unraveler_of_secrets",
                "island",
                "daretti,_scrap_savant",
                "swamp",
                "grizzly_bears",
                "sol_ring",
                "mountain",
            ],
        );
        inject_card_into_hand(&mut engine, 0, CARTH);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                b: 1,
                g: 1,
                ..Default::default()
            },
        );
        (engine, top)
    }
    fn replay_one(
        original: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
    ) -> RuledEventBatch {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let actual = original
            .apply_scenario_command(actor, &command)
            .expect("accepted command");
        let repeated = replay
            .apply_scenario_command(actor, &decoded)
            .expect("replayed accepted wire command");
        assert_eq!(actual, repeated);
        actual
    }

    let (mut original, top) = fixture();
    let (mut replay, replay_top) = fixture();
    assert_eq!(top, replay_top);
    let slot = hand_index_for_card(&original, 0, CARTH);
    replay_one(&mut original, &mut replay, 0, cast_spell(slot, vec![]));
    let mut batch = RuledEventBatch::default();
    for _ in 0..4 {
        let actor = original.state.priority_player_id();
        batch = replay_one(&mut original, &mut replay, actor, pass());
    }
    let choice = find_resolution_choice(&batch).expect("real entry trigger offers the look");
    assert_eq!(choice.candidate_object_ids, top[..7]);
    assert_ne!(choice.resolution_prompt_id, 0);
    let mut answer = submit_resolution_choice(vec![top[1]]);
    let Some(Cmd::SubmitResolutionChoice(submission)) = answer.cmd.as_mut() else {
        unreachable!()
    };
    submission.resolution_prompt_id = choice.resolution_prompt_id;
    let completed = replay_one(&mut original, &mut replay, 0, answer);
    assert!(completed
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert_random_bottom_cohort(&original, &[top[0], top[2], top[3], top[4], top[5], top[6]]);
    assert!(original.state.pending_resolution.is_none());
    assert_eq!(
        original.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    let actor = original.state.priority_player_id();
    replay_one(&mut original, &mut replay, actor, pass());
    assert_eq!(
        original.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

fn settled_carth(seed: u64) -> (GameEngine, u32) {
    let mut engine = game(seed, false);
    let (carth, trigger, batch) = cast_and_resolve_entry(&mut engine);
    assert!(find_resolution_choice(&batch).is_some());
    let result = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("decline a selection from Carth's completed entry look");
    assert_finished_once(&engine, &result, trigger);
    (engine, carth)
}

fn fixture_walker(engine: &mut GameEngine, owner: usize, controller: i32, card: &str) -> u32 {
    use tricerules_cards::{
        ContinuousEffectKind, ControllerReference, CounterKind, EffectDuration,
    };
    use tricerules_core::{AffectedScope, ContinuousEffect};
    // Placement, loyalty and any control change are synthetic. Damage and death below are real.
    let walker = inject_permanent_on_battlefield(engine, owner, card);
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    if engine.state.players[owner].id != controller {
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            trigger_grant_origin: None,
            affected: AffectedScope::Single(walker),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(controller),
            },
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: engine.state.command_index + 1,
        });
    }
    assert_eq!(
        engine.characteristics(walker).unwrap().controller,
        controller
    );
    assert!(engine
        .characteristics(walker)
        .unwrap()
        .has_type("Planeswalker"));
    walker
}

fn bolt_walker(engine: &mut GameEngine, walker: u32) {
    // Synthetic hand and mana; Lightning Bolt, damage and zero-loyalty death are real commands.
    inject_card_into_hand(engine, 0, "lightning_bolt");
    give_mana(
        engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "lightning_bolt");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(walker)))
        .expect("cast Lightning Bolt targeting the planeswalker");
    pass_both_players(engine);
}

fn assert_captured_walker_trigger(engine: &GameEngine, carth: u32, walker: u32, generation: u64) {
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one Carth trigger for the walker that died"
    );
    let item = engine.state.stack.last().unwrap();
    assert!(item.is_triggered);
    assert_eq!(
        (
            item.card_id.as_str(),
            item.controller,
            item.source_permanent_id
        ),
        (CARTH, 0, Some(carth))
    );
    let observed = item
        .trigger_context
        .observed_object
        .as_ref()
        .expect("captured event-time walker");
    assert_eq!(
        (
            observed.object_id,
            observed.zone_change_generation,
            observed.controller_at_event
        ),
        (walker, generation, 0)
    );
}

#[test]
fn carth_watches_death_of_own_and_foreign_owned_controlled_walkers() {
    for owner in [0, 1] {
        let (mut engine, carth) = settled_carth(2_610_271_010 + owner as u64);
        let top = put_on_top(
            &mut engine,
            &["forest", "jace,_unraveler_of_secrets", "island"],
        );
        let walker = fixture_walker(&mut engine, owner, 0, "daretti,_scrap_savant");
        let generation = engine
            .state
            .zone_change_generation
            .get(&walker)
            .copied()
            .unwrap_or(0);
        bolt_walker(&mut engine, walker);
        assert_eq!(engine.state.objects[&walker].zone, Zone::Graveyard);
        assert!(engine.state.players[owner].graveyard.contains(&walker));
        assert_captured_walker_trigger(&engine, carth, walker, generation);
        let trigger = engine.state.stack.last().unwrap().id;
        let batch = pass_both_players_with_batch(&mut engine);
        let choice = find_resolution_choice(&batch).expect("death trigger offers a private look");
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(&choice.candidate_object_ids[..3], top);
        assert!(choice.public_reveal.is_none());
        assert_no_reveal(&batch);
        let result = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![top[1]]))
            .expect("select a planeswalker for hand");
        assert!(result.events.iter().any(|event| matches!(&event.ev,
            Some(Ev::CardsRevealed(reveal)) if reveal.cards.len() == 1 && reveal.cards[0].object_id == top[1])));
        assert_finished_once(&engine, &result, trigger);
    }
}

#[test]
fn carth_does_not_watch_walkers_controlled_by_an_opponent() {
    for owner in [0, 1] {
        let (mut engine, _) = settled_carth(2_610_271_012 + owner as u64);
        let walker = fixture_walker(&mut engine, owner, 1, "daretti,_scrap_savant");
        bolt_walker(&mut engine, walker);
        assert_eq!(engine.state.objects[&walker].zone, Zone::Graveyard);
        assert!(engine.state.players[owner].graveyard.contains(&walker));
        assert!(
            engine.state.stack.is_empty(),
            "former controller, not owner, controls eligibility"
        );
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn carth_does_not_trigger_when_a_walker_is_exiled_instead_of_dying() {
    use tricerules_core::state::ActiveDeathReplacement;
    let (mut engine, _) = settled_carth(2_610_271_014);
    let walker = fixture_walker(&mut engine, 0, 0, "daretti,_scrap_savant");
    let generation = engine
        .state
        .zone_change_generation
        .get(&walker)
        .copied()
        .unwrap_or(0);
    // Synthetic existing replacement isolates the destination; damage and replacement are real.
    engine
        .state
        .death_replacement_effects
        .push(ActiveDeathReplacement {
            object_id: walker,
            zone_change_generation: generation,
        });
    bolt_walker(&mut engine, walker);
    assert_eq!(engine.state.objects[&walker].zone, Zone::Exile);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn carth_and_two_walkers_dying_together_produce_two_captured_looks() {
    let (mut engine, carth) = settled_carth(2_610_271_015);
    let top = put_on_top(
        &mut engine,
        &[
            "jace,_unraveler_of_secrets",
            "forest",
            "forest",
            "forest",
            "forest",
            "forest",
            "forest",
            "daretti,_scrap_savant",
            "forest",
            "forest",
            "forest",
            "forest",
            "forest",
            "forest",
        ],
    );
    let first = fixture_walker(&mut engine, 0, 0, "jace,_unraveler_of_secrets");
    let second = fixture_walker(&mut engine, 0, 0, "daretti,_scrap_savant");
    let generations: Vec<_> = [first, second]
        .iter()
        .map(|id| {
            engine
                .state
                .zone_change_generation
                .get(id)
                .copied()
                .unwrap_or(0)
        })
        .collect();
    inject_card_into_hand(&mut engine, 0, "all_is_dust");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "all_is_dust");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast All Is Dust");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&carth].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::GraveyardCards);
        let owner = pending.deciding_player;
        let order = pending.presentation.candidates.clone();
        engine
            .apply_scenario_command(owner, &submit_resolution_choice(order))
            .expect("owner orders simultaneous graveyard arrivals");
    }
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "one captured trigger for each controlled walker"
    );
    let mut observed = BTreeSet::new();
    for item in &engine.state.stack {
        assert!(item.is_triggered);
        assert_eq!(
            (
                item.card_id.as_str(),
                item.controller,
                item.source_permanent_id
            ),
            (CARTH, 0, Some(carth))
        );
        let fact = item
            .trigger_context
            .observed_object
            .as_ref()
            .expect("captured death fact");
        observed.insert((
            fact.object_id,
            fact.zone_change_generation,
            fact.controller_at_event,
        ));
    }
    assert_eq!(
        observed,
        [(first, generations[0], 0), (second, generations[1], 0)]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    for selected in [top[0], top[7]] {
        let trigger = engine.state.stack.last().unwrap().id;
        let batch = pass_both_players_with_batch(&mut engine);
        let choice =
            find_resolution_choice(&batch).expect("each captured trigger looks independently");
        assert!(choice.candidate_object_ids.contains(&selected));
        assert!(choice.public_reveal.is_none());
        let result = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
            .expect("select one planeswalker from this look");
        assert!(engine.state.players[0].hand.contains(&selected));
        assert_eq!(
            result
                .events
                .iter()
                .filter(|event| matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger))
                .count(),
            1
        );
        assert!(engine.state.pending_resolution.is_none());
    }
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_trigger_order.is_none());
}

#[test]
fn carth_watches_a_token_planeswalker_die_before_the_token_ceases() {
    use tricerules_cards::CounterKind;
    let (mut engine, carth) = settled_carth(2_610_271_016);
    // Synthetic copiable face makes an existing token a planeswalker; the token identity remains.
    let token = inject_creature_on_battlefield(&mut engine, 0, "soldier_w_1_1");
    let face = tricerules_cards::registry::global()
        .get("jace,_unraveler_of_secrets")
        .unwrap()
        .primary_face()
        .clone();
    let object = engine.state.objects.get_mut(&token).unwrap();
    object.copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: "jace,_unraveler_of_secrets".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Jace, Unraveler of Secrets".into(),
    });
    object.set_counter(CounterKind::Loyalty, 3);
    assert!(engine
        .characteristics(token)
        .unwrap()
        .has_type("Planeswalker"));
    let generation = engine
        .state
        .zone_change_generation
        .get(&token)
        .copied()
        .unwrap_or(0);
    bolt_walker(&mut engine, token);
    assert!(
        !engine.state.objects.contains_key(&token),
        "the dead token ceases at the SBA"
    );
    assert_captured_walker_trigger(&engine, carth, token, generation);
    let trigger = engine.state.stack.last().unwrap().id;
    let batch = pass_both_players_with_batch(&mut engine);
    let choice = find_resolution_choice(&batch).expect("captured token death still resolves");
    assert_eq!(choice.deciding_player_id, 0);
    let result = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("decline the optional card selection");
    assert_no_reveal(&result);
    assert_finished_once(&engine, &result, trigger);
}

fn loyalty_ability_info(
    engine: &mut GameEngine,
    walker: u32,
    index: u32,
) -> tricerules_proto::ruled::v1::AbilityInfo {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| {
            let Some(Ev::ZoneView(view)) = &event.ev else {
                return None;
            };
            view.per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == walker)
                .and_then(|object| {
                    object
                        .activated_abilities
                        .iter()
                        .find(|ability| ability.ability_index == index)
                })
                .cloned()
        })
        .expect("the engine publishes the real loyalty ability")
}

fn with_loyalty(engine: &mut GameEngine, card: &str, count: u32) -> u32 {
    use tricerules_cards::CounterKind;
    let walker = fixture_walker(engine, 0, 0, card);
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, count);
    walker
}

#[test]
fn carth_loyalty_offers_report_the_combined_signed_cost() {
    use tricerules_cards::primitives::AbilityCost;
    let cases = [
        ("jace,_unraveler_of_secrets", 0, 1, "+2"),
        ("garruk,_cursed_huntsman", 0, 0, "+1"),
        ("ashiok,_dream_render", 0, -1, "+0"),
        ("garruk,_cursed_huntsman", 1, -3, "-2"),
    ];
    for (case, &(card, index, printed, expected)) in cases.iter().enumerate() {
        let (mut engine, _) = settled_carth(2_610_271_020 + case as u64);
        let walker = with_loyalty(&mut engine, card, 5);
        if card == "garruk,_cursed_huntsman" && index == 1 {
            inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        }
        let info = loyalty_ability_info(&mut engine, walker, index);
        assert!(
            info.activatable,
            "the real ability is offered for case {case}"
        );
        assert_eq!(
            info.cost_label, expected,
            "the offered cost is the combined cost"
        );
        let face = tricerules_cards::registry::global()
            .get(card)
            .unwrap()
            .primary_face();
        assert_eq!(
            face.activated_abilities[index as usize].costs,
            [AbilityCost::Loyalty(printed)]
        );
    }
}

#[test]
fn carth_positive_and_zero_loyalty_costs_commit_one_combined_placement() {
    use tricerules_cards::primitives::{AbilityCost, CounterKind};
    for (case, &(card, printed, expected)) in [
        ("jace,_unraveler_of_secrets", 1, 5),
        ("garruk,_cursed_huntsman", 0, 4),
    ]
    .iter()
    .enumerate()
    {
        let (mut engine, _) = settled_carth(2_610_271_024 + case as u64);
        let walker = with_loyalty(&mut engine, card, 3);
        apply_ability(&mut engine, 0, walker, 0, vec![])
            .expect("activate the real loyalty ability");
        assert_eq!(
            engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
            expected
        );
        let ability = engine
            .state
            .stack
            .last()
            .unwrap()
            .activated_ability
            .as_ref()
            .unwrap();
        assert_eq!(
            ability.costs,
            [AbilityCost::Loyalty(printed)],
            "the captured printed definition stays unchanged"
        );
    }
}

fn attach_synthetic_counter_prohibition(engine: &mut GameEngine, walker: u32, card: &str) {
    // Copy the real walker's face and add Tatterkite's existing self-prohibition as test setup.
    let registry = tricerules_cards::registry::global();
    let mut face = registry.get(card).unwrap().primary_face().clone();
    let mut prohibition = registry
        .get("tatterkite")
        .unwrap()
        .primary_face()
        .static_abilities[0]
        .clone();
    prohibition.ability_id = tricerules_cards::AbilityId::new("fixture_no_counters").unwrap();
    face.static_abilities.push(prohibition);
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: card.into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: registry.get(card).unwrap().name.clone(),
    });
}

#[test]
fn carth_minus_one_becomes_zero_even_when_counter_placement_is_prohibited() {
    use tricerules_cards::CounterKind;
    let (mut engine, _) = settled_carth(2_610_271_026);
    let jace = with_loyalty(&mut engine, "jace,_unraveler_of_secrets", 3);
    attach_synthetic_counter_prohibition(&mut engine, jace, "jace,_unraveler_of_secrets");
    assert!(
        !loyalty_ability_info(&mut engine, jace, 0).activatable,
        "the copied self-prohibition actually prevents a positive loyalty cost"
    );
    let ashiok = with_loyalty(&mut engine, ASHIOK, 1);
    attach_synthetic_counter_prohibition(&mut engine, ashiok, ASHIOK);
    apply_ability(&mut engine, 0, ashiok, 0, target_player(1))
        .expect("a combined zero loyalty cost needs no counter placement");
    assert_eq!(engine.state.objects[&ashiok].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&ashiok].counter_count(CounterKind::Loyalty),
        1
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        loyalty_ability_info(&mut engine, ashiok, 0).cost_label,
        "+0"
    );
}

#[test]
fn carth_minus_three_is_payable_at_two_and_death_trigger_does_not_replace_the_activation() {
    use tricerules_cards::CounterKind;
    let (mut engine, carth) = settled_carth(2_610_271_027);
    let walker = with_loyalty(&mut engine, "garruk,_cursed_huntsman", 1);
    let victim = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    assert!(!loyalty_ability_info(&mut engine, walker, 1).activatable);
    let before = engine.diagnostic_snapshot().unwrap();
    apply_ability(&mut engine, 0, walker, 1, target_object(victim))
        .expect_err("one counter is below the net cost");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 2);
    let info = loyalty_ability_info(&mut engine, walker, 1);
    assert!(info.activatable, "two counters pay the combined -2 cost");
    assert_eq!(info.cost_label, "-2");
    apply_ability(&mut engine, 0, walker, 1, target_object(victim))
        .expect("activate at the exact threshold");
    assert_eq!(engine.state.objects[&walker].zone, Zone::Graveyard);
    let ability = engine
        .state
        .stack
        .iter()
        .find(|item| !item.is_triggered && item.source_permanent_id == Some(walker))
        .expect("the paid Garruk ability remains on the stack")
        .id;
    assert!(
        engine
            .state
            .stack
            .iter()
            .any(|item| item.is_triggered && item.source_permanent_id == Some(carth)),
        "Carth separately sees the zero-loyalty walker die"
    );
    let look = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&look).is_some());
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("finish Carth's death look");
    let completion = pass_both_players_with_batch(&mut engine);
    assert_eq!(engine.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == ability))
            .count(),
        1
    );
    assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
}

#[test]
fn carth_loyalty_cost_tracks_live_suppression_and_control() {
    use tricerules_cards::{ContinuousEffectKind, EffectDuration};
    use tricerules_core::{AffectedScope, ContinuousEffect};
    let (mut engine, carth) = settled_carth(2_610_271_028);
    let walker = with_loyalty(&mut engine, "jace,_unraveler_of_secrets", 3);
    assert_eq!(
        loyalty_ability_info(&mut engine, walker, 0).cost_label,
        "+2"
    );
    // Synthetic layer-6 suppression tests the actual Carth static's live availability.
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(carth),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
    assert_eq!(
        loyalty_ability_info(&mut engine, walker, 0).cost_label,
        "+1"
    );
    engine
        .state
        .continuous_effects
        .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities));
    assert_eq!(
        loyalty_ability_info(&mut engine, walker, 0).cost_label,
        "+2"
    );
    // Synthetic current-control transfer preserves Carth's owner and exact incarnation.
    engine.state.players[0]
        .battlefield
        .retain(|id| *id != carth);
    engine.state.players[1].battlefield.push(carth);
    let source = engine.state.objects.get_mut(&carth).unwrap();
    source.base_controller = 1;
    source.controller = 1;
    assert_eq!(source.owner, 0);
    assert_eq!(
        loyalty_ability_info(&mut engine, walker, 0).cost_label,
        "+1"
    );
}

#[test]
fn carth_loyalty_cost_adds_multiple_nonlegendary_fixture_copies() {
    use tricerules_cards::CounterKind;
    let (mut engine, _) = settled_carth(2_610_271_029);
    let copy = inject_creature_on_battlefield(&mut engine, 0, CARTH);
    // A nonlegendary copied face is synthetic so two real Carth statics can coexist.
    let mut face = tricerules_cards::registry::global()
        .get(CARTH)
        .unwrap()
        .primary_face()
        .clone();
    face.name = "Carth fixture copy".into();
    face.supertypes.clear();
    engine.state.objects.get_mut(&copy).unwrap().copiable_values =
        Some(tricerules_core::state::CopiableValues {
            prototyped: false,
            source_card_id: CARTH.into(),
            source_face_index: 0,
            face,
            room_faces: None,
            display_name: "Carth fixture copy".into(),
        });
    let walker = with_loyalty(&mut engine, "jace,_unraveler_of_secrets", 3);
    assert_eq!(
        loyalty_ability_info(&mut engine, walker, 0).cost_label,
        "+3"
    );
    apply_ability(&mut engine, 0, walker, 0, vec![]).expect("activate with two applicable sources");
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6
    );
}

#[test]
fn carth_does_not_modify_a_loyalty_ability_on_a_nonplaneswalker_source() {
    use tricerules_cards::{
        ContinuousEffectKind, CounterKind, EffectDuration, PermanentTypeFilter, TypeLineReplacement,
    };
    use tricerules_core::{AffectedScope, ContinuousEffect};
    let (mut engine, _) = settled_carth(2_610_271_030);
    let source = with_loyalty(&mut engine, "jace,_unraveler_of_secrets", 3);
    // Synthetic type change keeps the printed Jace ability but removes the planeswalker type.
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
            land_types: vec![],
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
    assert!(!engine
        .characteristics(source)
        .unwrap()
        .has_type("Planeswalker"));
    assert_eq!(
        loyalty_ability_info(&mut engine, source, 0).cost_label,
        "+1"
    );
    apply_ability(&mut engine, 0, source, 0, vec![])
        .expect("activate the real ability of a nonplaneswalker");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        4
    );
}

#[test]
fn carth_does_not_modify_an_opponents_loyalty_activation() {
    use tricerules_cards::CounterKind;
    use tricerules_core::TurnStep;
    let (mut engine, _) = settled_carth(2_610_271_031);
    let walker = fixture_walker(&mut engine, 1, 1, "jace,_unraveler_of_secrets");
    let starting_turn = engine.state.turn_instance;
    for _ in 0..100 {
        if engine.state.turn_instance > starting_turn
            && engine.state.active_player_id() == 1
            && engine.state.turn_step == TurnStep::Main1
        {
            break;
        }
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(player) => {
                let seat = engine.state.player_idx(player).unwrap();
                let excess = engine.state.players[seat].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("advance through the real turn");
    }
    assert_eq!(
        (engine.state.active_player_id(), engine.state.turn_step),
        (1, TurnStep::Main1)
    );
    assert_eq!(
        loyalty_ability_info(&mut engine, walker, 0).cost_label,
        "+1"
    );
    apply_ability(&mut engine, 1, walker, 0, vec![]).expect("opponent activates their own walker");
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        4
    );
}

fn cast_oath_for_carth_interaction(engine: &mut GameEngine) {
    // The Forest, hand card and mana are fixture setup; casting and Oath's ETB are real.
    let forest = inject_permanent_on_battlefield(engine, 0, "forest");
    let oath = inject_card_into_hand(engine, 0, "oath_of_teferi");
    give_mana(
        engine,
        0,
        ManaGift {
            c: 3,
            w: 1,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "oath_of_teferi");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Oath");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&oath].zone, Zone::Battlefield);
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(forest),
                })),
            },
        )
        .expect("choose Oath's other controlled permanent");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Exile);
    assert!(engine.state.stack.is_empty());
}

fn carth_interaction_loyalty_uses(engine: &GameEngine, walker: u32) -> u32 {
    let key = tricerules_core::state::ActivationUseKey {
        object_id: walker,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&walker)
            .copied()
            .unwrap_or(0),
        identity: tricerules_core::state::ActivationUseIdentity::Loyalty,
    };
    engine
        .state
        .activation_uses_this_turn
        .get(&key)
        .copied()
        .unwrap_or_default()
}

#[test]
fn carth_adjusts_each_oath_and_veil_authorized_loyalty_activation() {
    use tricerules_cards::{AbilityCost, CounterKind};
    let (mut engine, _) = settled_carth(2_610_271_100);
    cast_oath_for_carth_interaction(&mut engine);
    // Hand and mana are fixtures; Veil's cast, paid activation and grant are actual commands.
    let veil = inject_card_into_hand(&mut engine, 0, "the_chain_veil");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "the_chain_veil");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Veil");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&veil].zone, Zone::Battlefield);
    let walker = with_loyalty(&mut engine, "jace_beleren", 3);

    for used in 1..=2 {
        let offer = loyalty_ability_info(&mut engine, walker, 0);
        assert!(offer.activatable);
        assert_eq!(offer.cost_label, "+3");
        apply_ability(&mut engine, 0, walker, 0, vec![]).expect("Oath allows the activation");
        assert_eq!(
            engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
            3 + 3 * used
        );
        assert_eq!(carth_interaction_loyalty_uses(&engine, walker), used);
        assert_eq!(
            engine
                .state
                .stack
                .last()
                .unwrap()
                .activated_ability
                .as_ref()
                .unwrap()
                .costs,
            [AbilityCost::Loyalty(2)]
        );
        pass_both_players(&mut engine);
    }
    assert!(!loyalty_ability_info(&mut engine, walker, 0).activatable);
    apply_ability(&mut engine, 0, walker, 0, vec![]).expect_err("Oath's two uses are exhausted");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, veil, 0, vec![]).expect("pay and tap Veil for its permission");
    assert!(engine.state.objects[&veil].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    pass_both_players(&mut engine);
    let offer = loyalty_ability_info(&mut engine, walker, 0);
    assert!(offer.activatable);
    assert_eq!(offer.cost_label, "+3");
    apply_ability(&mut engine, 0, walker, 0, vec![]).expect("use Veil's additional permission");
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        12
    );
    assert_eq!(carth_interaction_loyalty_uses(&engine, walker), 3);
    pass_both_players(&mut engine);
    assert!(!loyalty_ability_info(&mut engine, walker, 0).activatable);
    apply_ability(&mut engine, 0, walker, 0, vec![])
        .expect_err("all three permissions are exhausted");
    assert_eq!(carth_interaction_loyalty_uses(&engine, walker), 3);
}

#[test]
fn carth_rings_copy_neither_repays_loyalty_nor_uses_oaths_second_activation() {
    use tricerules_cards::{AbilityCost, CounterKind};
    let (mut engine, _) = settled_carth(2_610_271_101);
    cast_oath_for_carth_interaction(&mut engine);
    // Hand and mana are fixtures; Rings is cast and its actual triggered payment is resolved.
    let rings = inject_card_into_hand(&mut engine, 0, "rings_of_brighthearth");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "rings_of_brighthearth");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Rings");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&rings].zone, Zone::Battlefield);
    let walker = with_loyalty(&mut engine, "jace_beleren", 3);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(
        loyalty_ability_info(&mut engine, walker, 0).cost_label,
        "+3"
    );
    apply_ability(&mut engine, 0, walker, 0, vec![]).expect("pay modified cost and trigger Rings");
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(carth_interaction_loyalty_uses(&engine, walker), 1);
    assert_eq!(engine.state.stack.len(), 2);
    assert_eq!(
        engine.state.stack[0]
            .activated_ability
            .as_ref()
            .unwrap()
            .costs,
        [AbilityCost::Loyalty(2)]
    );
    let prompt = pass_both_players_with_batch(&mut engine);
    assert_eq!(
        find_resolution_choice(&prompt).unwrap().choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose the Rings payment");
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {2} to copy Jace's ability");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.stack.last().unwrap().is_copy);
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .unwrap()
            .activated_ability
            .as_ref()
            .unwrap()
            .costs,
        [AbilityCost::Loyalty(2)]
    );
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(carth_interaction_loyalty_uses(&engine, walker), 1);
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(carth_interaction_loyalty_uses(&engine, walker), 1);
    assert!(loyalty_ability_info(&mut engine, walker, 0).activatable);
    apply_ability(&mut engine, 0, walker, 0, vec![])
        .expect("Oath permits the second real activation");
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        9
    );
    assert_eq!(carth_interaction_loyalty_uses(&engine, walker), 2);
    let prompt = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&prompt).is_some());
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the second Rings copy");
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(!loyalty_ability_info(&mut engine, walker, 0).activatable);
}

fn assert_complete_loyalty_fallback(
    ability: &tricerules_proto::ruled::v1::AbilityInfo,
    cost: &str,
) {
    assert_eq!(ability.cost_label, cost);
    assert_eq!(
        ability.text.split_once(':').map(|(prefix, _)| prefix),
        Some(cost),
        "the engine fallback must attest the full effective cost"
    );
    assert_eq!(
        ability.presentation.as_ref().unwrap().fallback_text,
        ability.text
    );
}

#[test]
fn carth_publishes_complete_cost_for_actual_jace_beleren() {
    let (mut engine, _) = settled_carth(2_610_271_110);
    let walker = with_loyalty(&mut engine, "jace_beleren", 3);
    // Draw(EachPlayer) uses the existing generic identifier fallback, not simple generated prose.
    let ability = loyalty_ability_info(&mut engine, walker, 0);
    assert_complete_loyalty_fallback(&ability, "+3");
}

#[test]
fn carth_publishes_complete_cost_for_actual_jace_unraveler_program() {
    let (mut engine, _) = settled_carth(2_610_271_111);
    let walker = with_loyalty(&mut engine, "jace,_unraveler_of_secrets", 5);
    // The actual Program's existing fallback is an identifier with no colon.
    let ability = loyalty_ability_info(&mut engine, walker, 0);
    assert_complete_loyalty_fallback(&ability, "+2");
}

#[test]
fn carth_preserves_colons_inside_a_generic_ability_identifier() {
    let (mut engine, _) = settled_carth(2_610_271_118);
    let walker = with_loyalty(&mut engine, "jace_beleren", 3);
    // Synthetic copied name checks the identifier boundary, not a new card definition.
    let mut face = tricerules_cards::registry::global()
        .get("jace_beleren")
        .unwrap()
        .primary_face()
        .clone();
    face.name = "Loyalty: fixture".into();
    let original = face.activated_abilities[0].fallback_text(&face.name);
    assert!(original.starts_with("Loyalty: fixture"));
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: "jace_beleren".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Loyalty: fixture".into(),
    });
    let ability = loyalty_ability_info(&mut engine, walker, 0);
    assert_complete_loyalty_fallback(&ability, "+3");
    assert_eq!(ability.text, format!("+3: {original}"));
}

#[test]
fn carth_publishes_zero_positive_and_negative_effective_loyalty_prefixes() {
    for (index, card, ability_index, expected) in [
        (0, "garruk,_cursed_huntsman", 0, "+1"),
        (1, "jace_beleren", 1, "+0"),
        (2, "garruk,_cursed_huntsman", 1, "-2"),
    ] {
        let (mut engine, _) = settled_carth(2_610_271_112 + index);
        let walker = with_loyalty(&mut engine, card, 5);
        let ability = loyalty_ability_info(&mut engine, walker, ability_index);
        assert_complete_loyalty_fallback(&ability, expected);
    }
}

#[test]
fn carth_replaces_the_generated_simple_loyalty_cost_prefix() {
    use tricerules_cards::{Amount, SpellEffectKind};
    let (mut engine, _) = settled_carth(2_610_271_115);
    let walker = with_loyalty(&mut engine, "jace_beleren", 3);
    // Synthetic simple flat effect exercises the existing generated-colon fallback branch.
    let mut face = tricerules_cards::registry::global()
        .get("jace_beleren")
        .unwrap()
        .primary_face()
        .clone();
    face.activated_abilities[0].effect = vec![SpellEffectKind::GainLife {
        amount: Amount::Fixed(1),
        who: tricerules_cards::primitives::PlayerRecipient::Controller,
    }];
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: "jace_beleren".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Simple loyalty fixture".into(),
    });
    let ability = loyalty_ability_info(&mut engine, walker, 0);
    assert_complete_loyalty_fallback(&ability, "+3");
    assert_eq!(ability.text, "+3: You gain 1 life.");
}

fn carth_arena_loyalty_face(engine: &mut GameEngine, modified_mana: bool) -> (u32, String) {
    use tricerules_cards::primitives::ActivatedCostModifier;
    use tricerules_cards::{AbilityCost, Amount, CounterKind};
    // Arena's admitted staged target profile with a synthetic planeswalker face and +1 cost.
    let source = inject_permanent_on_battlefield(engine, 0, "arena");
    let mut face = tricerules_cards::registry::global()
        .get("arena")
        .unwrap()
        .primary_face()
        .clone();
    face.types = vec!["Planeswalker".into(), "Jace".into()];
    face.activated_abilities[0]
        .costs
        .push(AbilityCost::Loyalty(1));
    if modified_mana {
        face.activated_abilities[0]
            .cost_modifiers
            .push(ActivatedCostModifier::GenericReduction {
                amount: Amount::Fixed(1),
            });
    }
    let fallback = face.activated_abilities[0].fallback_text(&face.name);
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: "arena".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Arena loyalty fixture".into(),
    });
    object.set_counter(CounterKind::Loyalty, 3);
    (source, fallback)
}

#[test]
fn carth_publishes_complete_compound_arena_loyalty_cost() {
    let (mut engine, _) = settled_carth(2_610_271_116);
    let (source, _) = carth_arena_loyalty_face(&mut engine, false);
    let ability = loyalty_ability_info(&mut engine, source, 0);
    assert_complete_loyalty_fallback(&ability, "{3}, {T}, +2");
}

#[test]
fn carth_does_not_attest_modified_mana_or_restricted_partial_costs() {
    use tricerules_cards::{AbilityCost, CounterKind};
    let (mut engine, _) = settled_carth(2_610_271_117);
    let (arena, fallback) = carth_arena_loyalty_face(&mut engine, true);
    let ability = loyalty_ability_info(&mut engine, arena, 0);
    assert_eq!(ability.cost_label, "{3}, {T}, +2");
    assert_eq!(
        ability.text, fallback,
        "a separate mana modifier makes the raw label incomplete"
    );

    // Synthetic planeswalker face keeps Fountainport's real token-only sacrifice restriction.
    let source = inject_permanent_on_battlefield(&mut engine, 0, "fountainport");
    let mut face = tricerules_cards::registry::global()
        .get("fountainport")
        .unwrap()
        .primary_face()
        .clone();
    face.types = vec!["Planeswalker".into(), "Jace".into()];
    face.activated_abilities[1]
        .costs
        .push(AbilityCost::Loyalty(1));
    let fallback = face.activated_abilities[1].fallback_text(&face.name);
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: "fountainport".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Restricted loyalty fixture".into(),
    });
    object.set_counter(CounterKind::Loyalty, 3);
    let ability = loyalty_ability_info(&mut engine, source, 1);
    assert!(ability.cost_label.contains("Sacrifice a permanent"));
    assert_eq!(
        ability.text, fallback,
        "the label omits the token-only restriction"
    );
}
