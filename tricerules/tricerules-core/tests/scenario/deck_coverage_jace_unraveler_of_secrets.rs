//! Jace's first two loyalty abilities reuse Scry, Draw, and targeted owner-hand return.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    dev_command, ChoiceKind, DevCommand, DevMoveCard, DevZone, SubmitResolutionChoice,
};

const JACE: &str = "jace,_unraveler_of_secrets";

fn setup_jace(seed: u64) -> (GameEngine, u32) {
    let registry = tricerules_cards::registry::global();
    assert!(
        registry.get(JACE).is_some(),
        "Jace, Unraveler of Secrets must be registered"
    );
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("create game");
    advance_to_main1_from_game_start(&mut engine);
    let jace = inject_permanent_on_battlefield(&mut engine, 0, JACE);
    engine
        .state
        .objects
        .get_mut(&jace)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    (engine, jace)
}

fn place_on_top(engine: &mut GameEngine, card_ids: &[&str]) -> Vec<u32> {
    let objects: Vec<_> = card_ids
        .iter()
        .map(|id| inject_library_card(engine, 0, id))
        .collect();
    engine.state.players[0]
        .library
        .retain(|oid| !objects.contains(oid));
    for &object in objects.iter().rev() {
        engine.state.players[0].library.push_front(object);
    }
    objects
}

fn move_jace_to_graveyard(engine: &mut GameEngine) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Jace, Unraveler of Secrets".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the physical Jace through the normal zone-change path");
}

fn choice_with_prompt(prompt_id: u64, chosen: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: prompt_id,
            chosen_object_ids: chosen,
            ..Default::default()
        })),
    }
}

#[test]
fn jace_plus_one_scries_then_draws_once_after_a_private_choice() {
    for bottom in [false, true] {
        let (mut engine, jace) = setup_jace(261_027_100 + u64::from(bottom));
        let top = place_on_top(&mut engine, &["grizzly_bears", "storm_crow"]);
        let hand_before = engine.state.players[0].hand.len();
        let library_before = engine.state.players[0].library.clone();
        let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
        apply_ability(&mut engine, 0, jace, 0, vec![]).expect("activate +1");
        assert_eq!(
            engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
            6
        );
        if bottom {
            move_jace_to_graveyard(&mut engine);
            assert_eq!(engine.state.objects[&jace].zone, Zone::Graveyard);
        }
        let batch = pass_both_players_with_batch(&mut engine);
        let choice = find_resolution_choice(&batch).expect("Scry pauses before drawing");
        assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!((choice.min, choice.max), (0, 1));
        assert_eq!(choice.candidate_object_ids, [top[0]]);
        assert!(
            choice.public_reveal.is_none(),
            "Scry does not publicly reveal the card"
        );
        assert_eq!(engine.state.players[0].library, library_before);
        assert_eq!(engine.state.players[0].hand.len(), hand_before);
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before
        );

        let prompt = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        for (player, rejected) in [(1, vec![top[0]]), (0, vec![top[1]])] {
            let before = format!("{:?}", engine.state);
            engine
                .apply_scenario_command(player, &choice_with_prompt(prompt, rejected))
                .expect_err("only the chooser can select an offered Scry card");
            assert_eq!(format!("{:?}", engine.state), before);
        }
        let selected = if bottom { vec![top[0]] } else { vec![] };
        engine
            .apply_scenario_command(0, &choice_with_prompt(prompt, selected.clone()))
            .expect("complete Scry and resume the draw");
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before + 1
        );
        assert!(engine.state.players[0]
            .hand
            .contains(&top[usize::from(bottom)]));
        if bottom {
            assert_eq!(engine.state.players[0].library.back(), Some(&top[0]));
        }
        let after = format!("{:?}", engine.state);
        engine
            .apply_scenario_command(0, &choice_with_prompt(prompt, selected))
            .expect_err("a consumed Scry choice cannot be replayed");
        assert_eq!(format!("{:?}", engine.state), after);
    }
}

#[test]
fn jace_plus_one_handles_empty_and_one_card_libraries() {
    for remaining in 0..=1 {
        let (mut engine, jace) = setup_jace(261_027_102 + remaining as u64);
        while engine.state.players[0].library.len() > remaining {
            let object = engine.state.players[0].library.pop_back().unwrap();
            engine.state.objects.get_mut(&object).unwrap().zone = Zone::Graveyard;
            engine.state.players[0].graveyard.push(object);
        }
        let only = engine.state.players[0].library.front().copied();
        let hand_before = engine.state.players[0].hand.len();
        apply_ability(&mut engine, 0, jace, 0, vec![]).expect("activate +1");
        let batch = pass_both_players_with_batch(&mut engine);
        if let Some(card) = only {
            let choice = find_resolution_choice(&batch).expect("one card can be scried");
            assert_eq!(choice.candidate_object_ids, [card]);
            engine
                .apply_scenario_command(0, &submit_resolution_choice(vec![card]))
                .expect("bottom the only card, then draw it");
            assert!(engine.state.players[0].hand.contains(&card));
            assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
            assert!(!engine.state.players[0].has_lost);
        } else {
            assert!(find_resolution_choice(&batch).is_none());
            assert!(
                engine.state.players[0].has_lost,
                "the draw after empty Scry still happens"
            );
        }
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn jace_minus_two_returns_a_foreign_owned_creature_after_jace_leaves() {
    let (mut engine, jace) = setup_jace(261_027_104);
    let creature = inject_creature_under_foreign_control(&mut engine, 1, 0, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "island");
    apply_ability(&mut engine, 0, jace, 1, target_object(land))
        .expect_err("a noncreature cannot be targeted");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    apply_ability(&mut engine, 0, jace, 1, target_object(creature)).expect("activate -2");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        3
    );
    move_jace_to_graveyard(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&jace].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Hand);
    assert!(
        engine.state.players[1].hand.contains(&creature),
        "return to owner, not controller"
    );
    assert!(!engine.state.players[0].hand.contains(&creature));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn jace_minus_two_does_not_return_a_stale_creature_incarnation() {
    let (mut engine, jace) = setup_jace(261_027_105);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    apply_ability(&mut engine, 0, jace, 1, target_object(creature)).expect("activate -2");
    // Simulate a later battlefield incarnation, as in the existing Daretti stale-target test.
    *engine
        .state
        .zone_change_generation
        .entry(creature)
        .or_default() += 2;
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert!(!engine.state.players[1].hand.contains(&creature));
    assert!(engine.state.stack.is_empty());
}

fn setup_three_player_jace(seed: u64) -> (GameEngine, u32) {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("create three-player game");
    advance_to_main1_from_game_start(&mut engine);
    let jace = inject_permanent_on_battlefield(&mut engine, 0, JACE);
    engine
        .state
        .objects
        .get_mut(&jace)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 8);
    (engine, jace)
}

fn create_jace_emblem(engine: &mut GameEngine, jace: u32) -> u32 {
    apply_ability(engine, 0, jace, 2, vec![]).expect("activate Jace's -8");
    assert_eq!(
        engine.state.objects[&jace].zone,
        Zone::Graveyard,
        "the last eight loyalty are paid before the ultimate resolves"
    );
    assert_eq!(engine.state.stack.len(), 1);
    pass_priority_round(engine);
    let marker = engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.controller == 0)
        .expect("the ability creates its emblem after Jace has left")
        .object_id;
    assert!(engine.initial_response_batch().events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::ZoneView(view)) if view.per_player.iter().any(|player| {
            player.player_id == 0 && player.static_emblems.iter().any(|emblem| emblem.object_id == marker)
        }))
    }));
    marker
}

fn cast_life_goes_on(engine: &mut GameEngine, seat: usize) -> (u32, RuledEventBatch) {
    let actor = engine.state.players[seat].id;
    for _ in 0..engine.state.players.len() {
        if engine.state.priority_player_id() == actor {
            break;
        }
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("pass priority to caster");
    }
    assert_eq!(engine.state.priority_player_id(), actor);
    give_mana(
        engine,
        actor,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let spell = inject_card_into_hand(engine, seat, "life_goes_on");
    let slot = hand_index_for_card(engine, seat, "life_goes_on");
    let batch = engine
        .apply_scenario_command(actor, &cast_spell(slot, vec![]))
        .expect("cast the untargeted instant");
    (spell, batch)
}

fn assert_untargeted_emblem_trigger(engine: &GameEngine, batch: &RuledEventBatch, marker: u32) {
    let trigger = engine.state.stack.last().expect("emblem trigger on stack");
    assert!(trigger.is_triggered);
    assert_eq!(trigger.source_permanent_id, Some(marker));
    assert!(trigger.targets.is_empty());
    assert!(!batch
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::TriggerNeedsTarget(_)))));
    let pushed = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(pushed)) if pushed.object_id == trigger.id => Some(pushed),
            _ => None,
        })
        .expect("the trigger has a public stack presentation");
    assert!(pushed.is_triggered && pushed.targets.is_empty());
    let presentation = pushed
        .primary_presentation
        .as_ref()
        .expect("captured emblem presentation");
    assert_eq!(presentation.card_id, JACE);
    assert_eq!(presentation.face_id, "jace_unraveler_of_secrets");
    assert_eq!(presentation.oracle_line_indices, [3]);
    assert_eq!(
        presentation
            .path
            .iter()
            .map(|part| part.id.as_str())
            .collect::<Vec<_>>(),
        ["activated_03", "emblem_01"]
    );
}

fn pass_round_with_batch(engine: &mut GameEngine) -> RuledEventBatch {
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
        - engine.state.passes_since_stack_change as usize;
    let mut batch = RuledEventBatch::default();
    for _ in 0..count {
        let priority = engine.state.priority_player_id();
        batch = engine
            .apply_scenario_command(priority, &pass())
            .expect("pass priority");
    }
    batch
}

fn assert_countered_by_emblem(engine: &mut GameEngine, spell: u32) {
    let batch = pass_round_with_batch(engine);
    assert!(batch.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::StackObjectCountered(countered)) if countered.object_id == spell)
    }), "the exact triggering spell is countered");
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
}

fn advance_to_main_for(engine: &mut GameEngine, player: i32) {
    for _ in 0..150 {
        if engine.state.active_player_id() == player
            && engine.state.turn_step == tricerules_core::TurnStep::Main1
        {
            return;
        }
        if let Some(actor) = engine.state.cleanup_discard_player {
            let seat = engine.state.player_idx(actor).unwrap();
            let excess = engine.state.players[seat].hand.len() - 7;
            engine
                .apply_scenario_command(actor, &discard_cleanup_batch((0..excess as u32).collect()))
                .expect("finish cleanup");
        } else {
            pass_priority_round(engine);
        }
    }
    panic!("did not reach player {player}'s main phase");
}

#[test]
fn jace_emblem_counters_each_opponents_first_spell_without_targeting() {
    let (mut engine, jace) = setup_three_player_jace(261_027_110);
    let marker = create_jace_emblem(&mut engine, jace);

    let (first, first_batch) = cast_life_goes_on(&mut engine, 1);
    assert_untargeted_emblem_trigger(&engine, &first_batch, marker);
    assert_countered_by_emblem(&mut engine, first);

    let (second, second_batch) = cast_life_goes_on(&mut engine, 1);
    assert!(!second_batch
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::TriggerNeedsTarget(_)))));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the same opponent's second spell does not trigger"
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[1].life, 24,
        "the second spell resolves"
    );

    let (other_first, other_batch) = cast_life_goes_on(&mut engine, 2);
    assert_eq!(
        engine.state.active_player_id(),
        0,
        "both opponents cast during another player's turn"
    );
    assert_untargeted_emblem_trigger(&engine, &other_batch, marker);
    assert_countered_by_emblem(&mut engine, other_first);
    assert_eq!(engine.state.players[2].life, 20);
}

#[test]
fn jace_emblem_respects_prior_casts_and_resets_each_turn() {
    let (mut engine, jace) = setup_three_player_jace(261_027_111);
    let (before_emblem, _) = cast_life_goes_on(&mut engine, 1);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&before_emblem].zone, Zone::Graveyard);
    let marker = create_jace_emblem(&mut engine, jace);

    let (second, _) = cast_life_goes_on(&mut engine, 1);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the pre-emblem cast already used the first ordinal"
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, 28);

    let (other_first, other_batch) = cast_life_goes_on(&mut engine, 2);
    assert_untargeted_emblem_trigger(&engine, &other_batch, marker);
    assert_countered_by_emblem(&mut engine, other_first);

    advance_to_main_for(&mut engine, 1);
    let (next_turn, next_batch) = cast_life_goes_on(&mut engine, 1);
    assert_untargeted_emblem_trigger(&engine, &next_batch, marker);
    assert_countered_by_emblem(&mut engine, next_turn);
    assert_eq!(engine.state.players[1].life, 28);
}

fn pass_to(engine: &mut GameEngine, actor: i32) {
    for _ in 0..engine.state.players.len() {
        if engine.state.priority_player_id() == actor {
            return;
        }
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("pass priority");
    }
    panic!("did not reach player {actor}");
}

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

fn create_emblem_for_active_player(engine: &mut GameEngine, seat: usize) -> u32 {
    let actor = engine.state.players[seat].id;
    assert_eq!(engine.state.active_player_id(), actor);
    let existing: Vec<_> = engine
        .state
        .static_emblems
        .iter()
        .map(|emblem| emblem.object_id)
        .collect();
    let jace = inject_permanent_on_battlefield(engine, seat, JACE);
    engine
        .state
        .objects
        .get_mut(&jace)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 8);
    apply_ability(engine, actor, jace, 2, vec![]).expect("activate the additional Jace ultimate");
    pass_priority_round(engine);
    engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.controller == actor && !existing.contains(&emblem.object_id))
        .expect("new emblem")
        .object_id
}

#[test]
fn jace_emblem_does_not_get_another_chance_after_an_uncounterable_first_spell() {
    let (mut engine, jace) = setup_three_player_jace(261_027_112);
    let marker = create_jace_emblem(&mut engine, jace);
    advance_to_main_for(&mut engine, 1);
    pass_to(&mut engine, 1);
    let surrak = inject_card_into_hand(&mut engine, 1, "surrak,_elusive_hunter");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "surrak,_elusive_hunter");
    let cast = engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("cast Surrak");
    assert_untargeted_emblem_trigger(&engine, &cast, marker);
    let failed_counter = pass_round_with_batch(&mut engine);
    assert!(!failed_counter
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::StackObjectCountered(_)))));
    assert!(engine.state.stack.iter().any(|item| item.id == surrak));
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&surrak].zone, Zone::Battlefield);

    let (second, _) = cast_life_goes_on(&mut engine, 1);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the uncounterable first cast consumed its ordinal"
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, 24);
}

#[test]
fn jace_emblems_and_a_real_cast_observer_share_apnap_and_controller_ordering() {
    let (mut engine, jace) = setup_three_player_jace(261_027_113);
    let first = create_jace_emblem(&mut engine, jace);
    let second = create_emblem_for_active_player(&mut engine, 0);
    advance_to_main_for(&mut engine, 1);
    let other = create_emblem_for_active_player(&mut engine, 1);
    let lizard = inject_creature_on_battlefield(&mut engine, 1, "magebane_lizard");
    advance_to_main_for(&mut engine, 0);

    let (spell, _) = cast_life_goes_on(&mut engine, 2);
    let first_order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("active player orders its emblems");
    assert_eq!(first_order.deciding_player, 0);
    assert_eq!(first_order.candidates.len(), 2);
    while engine
        .state
        .pending_trigger_order
        .as_ref()
        .is_some_and(|order| order.deciding_player == 0)
    {
        let candidate = engine
            .state
            .pending_trigger_order
            .as_ref()
            .unwrap()
            .candidates[0]
            .object_id;
        engine
            .apply_scenario_command(0, &submit_trigger_order(candidate))
            .expect("order active triggers");
    }
    let next_order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("next player orders emblem and Lizard");
    assert_eq!(next_order.deciding_player, 1);
    assert_eq!(next_order.candidates.len(), 2);
    answer_trigger_order_in_engine_order(&mut engine);
    assert!(engine.state.pending_trigger_order.is_none());
    let triggers: Vec<_> = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect();
    assert_eq!(triggers.len(), 4);
    assert_eq!(
        triggers
            .iter()
            .map(|item| item.controller)
            .collect::<Vec<_>>(),
        [0, 0, 1, 1]
    );
    let sources: Vec<_> = triggers
        .iter()
        .map(|item| item.source_permanent_id.unwrap())
        .collect();
    assert!(sources.contains(&first) && sources.contains(&second));
    assert!(sources.contains(&other) && sources.contains(&lizard));
    let mut counter_events = 0;
    while !engine.state.stack.is_empty() {
        let batch = pass_round_with_batch(&mut engine);
        counter_events += batch.events.iter().filter(|event| {
            matches!(&event.ev, Some(Ev::StackObjectCountered(countered)) if countered.object_id == spell)
        }).count();
    }
    assert_eq!(
        counter_events, 1,
        "multiple emblems counter this occurrence at most once"
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[2].life, 19,
        "the independently staged Lizard trigger resolves"
    );
}

#[test]
fn jace_emblem_trigger_can_be_countered_without_refunding_the_first_cast() {
    let (mut engine, jace) = setup_three_player_jace(261_027_114);
    let marker = create_jace_emblem(&mut engine, jace);
    let (spell, _) = cast_life_goes_on(&mut engine, 1);
    let trigger = engine.state.stack.last().expect("emblem trigger").id;
    pass_to(&mut engine, 1);
    inject_card_into_hand(&mut engine, 1, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "tishanas_tidebinder");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("cast Tidebinder");
    pass_priority_round(&mut engine);
    let choice = RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: stack_target(trigger),
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(1, &choice)
        .expect("target the emblem trigger");
    let counter = pass_round_with_batch(&mut engine);
    assert!(counter.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::StackObjectCountered(item)) if item.object_id == trigger)
    }));
    assert!(engine.state.stack.iter().any(|item| item.id == spell));
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[1].life, 24);
    assert!(engine
        .state
        .static_emblems
        .iter()
        .any(|emblem| emblem.object_id == marker));
    let (later, _) = cast_life_goes_on(&mut engine, 1);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "countering the trigger did not refund the ordinal"
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&later].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, 28);
}

#[test]
fn jace_emblem_keeps_the_old_occurrence_after_the_same_card_is_recast() {
    let (mut engine, jace) = setup_three_player_jace(261_027_115);
    create_jace_emblem(&mut engine, jace);
    let (spell, _) = cast_life_goes_on(&mut engine, 1);
    let trigger = engine.state.stack.last().unwrap().id;
    let old_generation = engine.state.zone_change_generation[&spell];
    pass_to(&mut engine, 0);
    inject_card_into_hand(&mut engine, 0, "counterspell");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "counterspell");
    engine
        .apply_scenario_command(0, &cast_spell(slot, stack_target(spell)))
        .expect("counter the original spell before the emblem trigger resolves");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.stack.iter().any(|item| item.id == trigger));

    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 1,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Life Goes On".into(),
                        zone: DevZone::Hand as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("return the same physical card to its owner's hand");
    pass_to(&mut engine, 1);
    give_mana(
        &mut engine,
        1,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "life_goes_on");
    assert_eq!(engine.state.players[1].hand[slot], spell);
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("recast that same card");
    assert!(engine.state.zone_change_generation[&spell] > old_generation);
    assert_eq!(engine.state.stack.len(), 2);
    // Synthetic ordering exposes both incarnations' identity check while the recast is live.
    let old_trigger = engine
        .state
        .stack
        .iter()
        .position(|item| item.id == trigger)
        .unwrap();
    let recast = engine
        .state
        .stack
        .iter()
        .position(|item| item.id == spell)
        .unwrap();
    engine.state.stack.swap(old_trigger, recast);
    let stale = pass_round_with_batch(&mut engine);
    assert!(!stale.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::StackObjectCountered(item)) if item.object_id == spell)
    }));
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    assert!(engine.state.stack.iter().any(|item| item.id == spell));
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[1].life, 24);
}

#[test]
fn jace_emblem_does_not_treat_an_opponents_twincast_copy_as_a_cast() {
    let (mut engine, jace) = setup_three_player_jace(261_027_116);
    create_jace_emblem(&mut engine, jace);
    let base = inject_card_into_hand(&mut engine, 0, "life_goes_on");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "life_goes_on");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast controller's instant");

    let (opponent_first, _) = cast_life_goes_on(&mut engine, 1);
    let counter = pass_round_with_batch(&mut engine);
    assert!(counter.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::StackObjectCountered(item)) if item.object_id == opponent_first)
    }));
    assert!(engine.state.stack.iter().any(|item| item.id == base));
    pass_to(&mut engine, 1);
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
        .apply_scenario_command(1, &cast_spell(slot, stack_target(base)))
        .expect("cast Twincast as that opponent's second spell");
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_triggered)
            .count(),
        0
    );
    let history = engine.state.turn_history.current.spell_casts.clone();
    pass_priority_round(&mut engine);
    let copy = engine
        .state
        .stack
        .last()
        .expect("Twincast created the copy");
    assert!(copy.is_copy && copy.cast_occurrence.is_none());
    assert_eq!(copy.controller, 1);
    assert_eq!(engine.state.turn_history.current.spell_casts, history);
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_triggered)
            .count(),
        0
    );
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[1].life, 24);
    assert_eq!(engine.state.players[0].life, 24);
}

#[test]
fn jace_emblem_counters_an_actually_cast_preparation_copy() {
    use tricerules_proto::ruled::v1 as rv1;
    let (mut engine, jace) = setup_three_player_jace(261_027_117);
    advance_to_main_for(&mut engine, 1);
    let healer = inject_card_into_hand(&mut engine, 1, "infirmary_healer_stream_of_life");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "infirmary_healer_stream_of_life");
    engine
        .apply_scenario_command(1, &cast_spell_face(slot, vec![], 0))
        .expect("cast Infirmary Healer");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&healer].zone, Zone::Battlefield);
    let prepared = engine.state.prepared_permanents[&healer];

    advance_to_main_for(&mut engine, 0);
    let marker = create_jace_emblem(&mut engine, jace);
    advance_to_main_for(&mut engine, 1);
    give_mana(
        &mut engine,
        1,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let offer = engine.initial_response_batch();
    let action = offer.legal_by_player[&1]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == prepared)
        .expect("the real prepared face is offered");
    let command = RuledCommand {
        cmd: Some(Cmd::CastSpell(rv1::CastSpell {
            source: Some(rv1::CastSource {
                location: Some(rv1::cast_source::Location::ExileObjectId(prepared)),
                expected_zone_change_generation: Some(action.zone_change_generation),
            }),
            face_index: action.face_index,
            casting_permission_id: action.casting_permission_id,
            cast_method: action.cast_method,
            targets: target_player(1),
            x_value: 0,
            ..Default::default()
        })),
    };
    let cast = engine
        .apply_scenario_command(1, &command)
        .expect("cast the preparation copy");
    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| !item.is_triggered)
        .expect("cast copy");
    assert!(copy.is_copy && copy.cast_occurrence.is_some());
    assert_eq!(copy.cast_by, Some(1));
    let copy_id = copy.id;
    assert_untargeted_emblem_trigger(&engine, &cast, marker);
    let counter = pass_round_with_batch(&mut engine);
    assert!(counter.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::StackObjectCountered(item)) if item.object_id == copy_id)
    }));
    assert!(!engine.state.stack.iter().any(|item| item.id == copy_id));
}

#[test]
fn jace_emblem_controller_departure_discards_unbegun_triggers_and_marker() {
    for stage in 0..=2 {
        let (mut engine, jace) = setup_three_player_jace(261_027_118 + stage);
        let first = create_jace_emblem(&mut engine, jace);
        let second = if stage == 2 {
            Some(create_emblem_for_active_player(&mut engine, 0))
        } else {
            None
        };
        let pending_spell = if stage == 0 {
            None
        } else {
            let (spell, _) = cast_life_goes_on(&mut engine, 1);
            if stage == 1 {
                assert!(engine
                    .state
                    .stack
                    .iter()
                    .any(|item| item.source_permanent_id == Some(first)));
            } else {
                assert_eq!(
                    engine
                        .state
                        .pending_trigger_order
                        .as_ref()
                        .unwrap()
                        .deciding_player,
                    0
                );
            }
            Some(spell)
        };
        engine
            .apply_scenario_command(0, &concede())
            .expect("emblem controller leaves");
        assert!(!engine
            .state
            .static_emblems
            .iter()
            .any(|emblem| emblem.object_id == first || Some(emblem.object_id) == second));
        assert!(!engine
            .state
            .stack
            .iter()
            .any(|item| item.source_permanent_id == Some(first)
                || item.source_permanent_id == second && second.is_some()));
        assert!(engine.state.pending_trigger_order.is_none());
        let spell = if let Some(spell) = pending_spell {
            spell
        } else {
            let (spell, _) = cast_life_goes_on(&mut engine, 1);
            spell
        };
        assert_eq!(
            engine
                .state
                .stack
                .iter()
                .filter(|item| item.is_triggered)
                .count(),
            0
        );
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert_eq!(engine.state.players[1].life, 24);
    }
}

#[test]
fn jace_emblem_survives_its_original_sources_owner_departing() {
    for owner_leaves_before_ultimate in [false, true] {
        let (mut engine, jace) =
            setup_three_player_jace(261_027_121 + u64::from(owner_leaves_before_ultimate));
        advance_to_main_for(&mut engine, 1);
        // Synthetic control change of the real planeswalker; its owner remains player 0.
        engine.state.players[0].battlefield.retain(|id| *id != jace);
        engine.state.players[1].battlefield.push(jace);
        let object = engine.state.objects.get_mut(&jace).unwrap();
        object.base_controller = 1;
        object.controller = 1;
        engine.initial_response_batch();
        apply_ability(&mut engine, 1, jace, 2, vec![])
            .expect("borrowed Jace's controller activates -8");
        assert_eq!(engine.state.objects[&jace].zone, Zone::Graveyard);
        if owner_leaves_before_ultimate {
            engine
                .apply_scenario_command(0, &concede())
                .expect("source owner leaves before resolution");
        }
        pass_priority_round(&mut engine);
        let marker = engine
            .state
            .static_emblems
            .iter()
            .find(|emblem| emblem.controller == 1)
            .expect("the surviving controller gets the emblem")
            .object_id;
        let (spell, cast) = cast_life_goes_on(&mut engine, 2);
        assert_untargeted_emblem_trigger(&engine, &cast, marker);
        if !owner_leaves_before_ultimate {
            engine
                .apply_scenario_command(0, &concede())
                .expect("source owner leaves after trigger creation");
        }
        assert!(!engine.state.objects.contains_key(&jace));
        assert!(engine
            .state
            .static_emblems
            .iter()
            .any(|emblem| emblem.object_id == marker));
        assert_countered_by_emblem(&mut engine, spell);
    }
}

#[test]
fn jace_emblem_keeps_its_spell_receipt_after_synthetic_controller_change() {
    let (mut engine, jace) = setup_three_player_jace(261_027_123);
    let marker = create_jace_emblem(&mut engine, jace);
    let (spell, cast) = cast_life_goes_on(&mut engine, 1);
    assert_untargeted_emblem_trigger(&engine, &cast, marker);
    // A synthetic stack-controller change isolates event-time identity from later control.
    engine
        .state
        .stack
        .iter_mut()
        .find(|item| item.id == spell)
        .unwrap()
        .controller = 2;
    assert_countered_by_emblem(&mut engine, spell);
    assert!(engine.state.players[1].graveyard.contains(&spell));
}

#[test]
fn jace_emblem_cast_and_counter_replay_accepted_wire_commands() {
    use prost::Message;
    fn fixture() -> (GameEngine, u32, u32) {
        let (mut engine, jace) = setup_three_player_jace(261_027_124);
        let spell = inject_card_into_hand(&mut engine, 1, "life_goes_on");
        give_mana(
            &mut engine,
            1,
            ManaGift {
                g: 1,
                ..Default::default()
            },
        );
        (engine, jace, spell)
    }
    fn record_and_replay(
        engine: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
    ) -> RuledEventBatch {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let actual = engine
            .apply_scenario_command(actor, &command)
            .expect("accept command");
        let repeated = replay
            .apply_scenario_command(actor, &decoded)
            .expect("replay accepted command");
        assert_eq!(actual, repeated);
        actual
    }
    let (mut engine, jace, spell) = fixture();
    let (mut replay, other_jace, other_spell) = fixture();
    assert_eq!((jace, spell), (other_jace, other_spell));
    let activation = activate_ability_for(&engine, jace, 2, vec![]);
    record_and_replay(&mut engine, &mut replay, 0, activation);
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        record_and_replay(&mut engine, &mut replay, actor, pass());
    }
    let actor = engine.state.priority_player_id();
    assert_eq!(actor, 0);
    record_and_replay(&mut engine, &mut replay, actor, pass());
    let slot = hand_index_for_card(&engine, 1, "life_goes_on");
    record_and_replay(&mut engine, &mut replay, 1, cast_spell(slot, vec![]));
    let mut last = RuledEventBatch::default();
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        last = record_and_replay(&mut engine, &mut replay, actor, pass());
    }
    assert!(last.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::StackObjectCountered(item)) if item.object_id == spell)
    }));
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    let actor = engine.state.priority_player_id();
    record_and_replay(&mut engine, &mut replay, actor, pass());
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}
