//! Memory Jar's face-down hands stay linked to one resolving activation.
//! CR 101.4, 406.3, 603.7, 608.2, and 701.9 govern these scenarios.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::state::EventObserverPayload;
use tricerules_core::{AttachmentRecipient, EngineDeck, TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

const JAR: &str = "memory_jar";

fn same_hand(left: &[u32], right: &[u32]) -> bool {
    let mut left = left.to_vec();
    let mut right = right.to_vec();
    left.sort_unstable();
    right.sort_unstable();
    left == right
}

fn same_hands(left: &[Vec<u32>], right: &[Vec<u32>]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left_hand, right_hand)| same_hand(left_hand, right_hand))
}

fn game(seed: u64, players: &[i32], hands: &[&[&str]]) -> (GameEngine, Vec<Vec<u32>>) {
    let jar = tricerules_cards::registry::global()
        .get(JAR)
        .expect("Memory Jar must be registered");
    assert_eq!(jar.name, "Memory Jar");
    assert_eq!(players.len(), hands.len());
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        players,
        20,
        None,
        true,
    )
    .expect("create game");
    advance_to_main1_from_game_start(&mut engine);
    for seat in 0..players.len() {
        let old_hand = std::mem::take(&mut engine.state.players[seat].hand);
        for object in old_hand {
            engine.state.objects.get_mut(&object).unwrap().zone = Zone::Library;
            engine.state.players[seat].library.push_back(object);
        }
    }
    let originals = hands
        .iter()
        .enumerate()
        .map(|(seat, cards)| {
            cards
                .iter()
                .map(|card| inject_card_into_hand(&mut engine, seat, card))
                .collect()
        })
        .collect();
    (engine, originals)
}

fn pass_round_with_events(engine: &mut GameEngine) -> Vec<rv1::RuledEvent> {
    let remaining = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
        .checked_sub(engine.state.passes_since_stack_change as usize)
        .expect("consistent priority pass count");
    let mut events = Vec::new();
    for _ in 0..remaining {
        let player = engine.state.priority_player_id();
        events.extend(
            engine
                .apply_scenario_command(player, &pass())
                .expect("priority holder passes")
                .events,
        );
    }
    events
}

fn advance_to_end_step(engine: &mut GameEngine) {
    for _ in 0..10 {
        if engine.state.turn_step == TurnStep::EndStep {
            return;
        }
        let active = engine.state.active_player_id();
        engine
            .apply_scenario_command(active, &primitive_yield())
            .expect("advance to the end step");
    }
    panic!("the game did not reach the end step");
}

fn choose_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn assert_no_public_old_hand_names(events: &[rv1::RuledEvent]) {
    for event in events {
        let text = match &event.ev {
            Some(Ev::Log(log)) if log.visible_to_player_id.is_none() => Some(log.text.as_str()),
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice.prompt_text.as_str()),
            _ => None,
        };
        if let Some(text) = text {
            assert!(
                !text.contains("Forest") && !text.contains("Mountain"),
                "{text}"
            );
        }
    }
}

fn jar_with_commander_in_hand(seed: u64) -> (GameEngine, u32, [u32; 2]) {
    let decks = vec![
        EngineDeck {
            mainboard: deck_with("island", &[]),
            commanders: Vec::new(),
        },
        EngineDeck {
            mainboard: deck_with("island", &[]),
            commanders: vec!["kami_of_the_crescent_moon".into()],
        },
    ];
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        seed,
        &[4, 9],
        20,
        Some(decks),
        true,
    )
    .expect("create a Commander fixture");
    advance_to_main1_from_game_start(&mut engine);
    for player in &mut engine.state.players {
        for id in std::mem::take(&mut player.hand) {
            engine.state.objects.get_mut(&id).unwrap().zone = Zone::Library;
            player.library.push_back(id);
        }
    }
    let commander = engine.state.players[1].command_zone[0];
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            9,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(rv1::DevCommand {
                    target_player_id: 9,
                    dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                        card_name: "Kami of the Crescent Moon".into(),
                        zone: rv1::DevZone::Hand as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the declared commander into its owner's hand");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Hand);
    let ordinary = [
        inject_card_into_hand(&mut engine, 0, "forest"),
        inject_card_into_hand(&mut engine, 1, "mountain"),
    ];
    let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
    apply_ability(&mut engine, 4, jar, 0, vec![]).expect("activate Memory Jar");
    (engine, commander, ordinary)
}

fn resolve_jar_and_choose_commander_sba(
    engine: &mut GameEngine,
    commander: u32,
    ordinary: [u32; 2],
    initial_branch: u32,
) -> Vec<Vec<u32>> {
    let events = pass_round_with_events(engine);
    let draws: Vec<Vec<u32>> = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect();
    assert!(
        draws.iter().all(|hand| hand.len() == 7),
        "all draws finish before the SBA offer"
    );
    assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
    assert!(engine.state.objects[&commander].face_down);
    for id in ordinary {
        assert_eq!(engine.state.objects[&id].zone, Zone::Exile);
        assert!(engine.state.objects[&id].face_down);
    }
    let choice = events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice)) if choice.source_object_id == commander => {
                Some(choice)
            }
            _ => None,
        })
        .expect("the owner receives the post-resolution commander SBA choice");
    assert_eq!(choice.deciding_player_id, 9);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert!(choice.prompt_text.contains("903.9a"));
    assert_no_public_old_hand_names(&events);
    engine
        .apply_scenario_command(9, &choose_branch(initial_branch))
        .expect("answer the commander SBA choice");
    draws
}

fn jar_cohort_is_stored(engine: &GameEngine) -> bool {
    // RON preserves the other generation-keyed maps that cannot be represented as JSON keys.
    let state = ron::to_string(&engine.state).expect("inspect rules-only cohort bookkeeping");
    !state
        .split_once("memory_jar_cohorts:")
        .expect("cohort bookkeeping is serialized")
        .1
        .starts_with("{}")
}

fn install_departing_aura_observer(engine: &mut GameEngine) -> (u32, u32) {
    let creature = inject_creature_on_battlefield(engine, 1, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(engine, 1, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(creature));

    // P36 casts an actual Banishing Light while taking the fixture's Main1 priority.
    engine.state.active_player_idx = 3;
    engine.state.priority_idx = 3;
    let light = inject_card_into_hand(engine, 3, "banishing_light");
    give_mana(
        engine,
        36,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 3, "banishing_light");
    engine
        .apply_scenario_command(36, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&light].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "the real entry trigger needs its target"
    );
    engine
        .apply_scenario_command(
            36,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(aura),
                })),
            },
        )
        .expect("choose the Aura for Banishing Light");
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    assert_eq!(
        engine.state.active_event_observers.len(),
        1,
        "the real Light created its return observer"
    );
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    (aura, creature)
}

type ParkedJarDiscardFixture = (
    GameEngine,
    Vec<Vec<u32>>,
    Vec<Vec<u32>>,
    u32,
    Option<(u32, u32)>,
);

fn parked_jar_discard(seed: u64, observer: bool) -> ParkedJarDiscardFixture {
    let (mut engine, originals) = game(
        seed,
        &[4, 9, 27, 36],
        &[&["mountain"], &["forest"], &["swamp"], &["island"]],
    );
    let aura = observer.then(|| install_departing_aura_observer(&mut engine));
    inject_permanent_on_battlefield(&mut engine, 1, "library_of_leng");
    let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
    apply_ability(&mut engine, 4, jar, 0, vec![]).expect("activate Memory Jar");
    pass_round_with_events(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    let draws = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.clone())
        .collect();
    advance_to_end_step(&mut engine);
    let trigger = engine
        .state
        .stack
        .last()
        .expect("the delayed trigger is on the stack")
        .id;
    let offered = pass_round_with_events(&mut engine);
    let choice = offered
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .expect("Library of Leng offers a discard destination");
    assert_eq!(choice.deciding_player_id, 9);
    assert_eq!(choice.choice_kind(), ChoiceKind::PrivateReplacement);
    assert!(
        !offered.iter().any(|event| matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger
        )),
        "the delayed child has not completed while discard is parked"
    );
    for id in originals.iter().flatten() {
        assert_eq!(engine.state.objects[id].zone, Zone::Exile);
        assert!(engine.state.objects[id].face_down);
    }
    assert!(jar_cohort_is_stored(&engine));
    (engine, originals, draws, trigger, aura)
}

fn finish_leng_discard(engine: &mut GameEngine, events: &mut Vec<rv1::RuledEvent>) {
    for _ in 0..16 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            break;
        };
        assert_eq!(pending.deciding_player, 9);
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::PrivateReplacement
        );
        assert!(!pending.presentation.ordered);
        assert!(pending.presentation.candidates.contains(&0));
        events.extend(
            engine
                .apply_scenario_command(9, &submit_resolution_choice(vec![0]))
                .expect("choose the graveyard for this discard")
                .events,
        );
    }
    assert!(
        engine.state.pending_resolution.is_none(),
        "discard replacement completes"
    );
}

fn assert_surviving_jar_return(
    engine: &GameEngine,
    originals: &[Vec<u32>],
    draws: &[Vec<u32>],
    trigger: u32,
    events: &[rv1::RuledEvent],
) {
    assert!(
        !jar_cohort_is_stored(engine),
        "the finished child retires its private cohort"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger))
            .count(),
        1
    );
    let first_return = events
        .iter()
        .position(|event| {
            matches!(&event.ev,
                Some(Ev::PermanentMoved(moved))
                    if originals.iter().flatten().any(|id| *id == moved.object_id)
                        && moved.destination == rv1::permanent_moved::Destination::Hand as i32
            )
        })
        .expect("a surviving original hand returns");
    for (seat, player) in engine.state.players.iter().enumerate() {
        if player.has_lost {
            continue;
        }
        assert!(same_hand(&player.hand, &originals[seat]));
        for id in &draws[seat] {
            assert_eq!(engine.state.objects[id].zone, Zone::Graveyard);
            let discard = events.iter().position(|event| matches!(&event.ev,
                Some(Ev::PermanentMoved(moved))
                    if moved.object_id == *id
                        && moved.destination == rv1::permanent_moved::Destination::Graveyard as i32
            )).expect("each surviving new hand card was discarded");
            assert!(
                discard < first_return,
                "every surviving discard precedes every return"
            );
        }
        for id in &originals[seat] {
            assert_eq!(engine.state.objects[id].zone, Zone::Hand);
            assert!(!engine.state.objects[id].face_down);
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(&event.ev,
                        Some(Ev::PermanentMoved(moved)) if moved.object_id == *id
                            && moved.destination == rv1::permanent_moved::Destination::Hand as i32
                    ))
                    .count(),
                1,
                "each exact old hand card returns once"
            );
        }
    }
}

#[test]
fn memory_jar_initial_draw_finishes_after_controller_departs_without_future_trigger() {
    let (mut engine, originals) = game(
        261_027_211,
        &[4, 9, 27],
        &[&["mountain"], &["forest"], &["swamp"]],
    );
    inject_permanent_on_battlefield(&mut engine, 1, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 1, "thought_reflection");
    let prior_draws = [4, 9, 27].map(|id| engine.state.turn_history.current.player(id).cards_drawn);
    let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
    apply_ability(&mut engine, 4, jar, 0, vec![]).expect("activate Memory Jar");
    let activation = engine
        .state
        .stack
        .last()
        .expect("the activation is on the stack")
        .id;
    let offered = pass_round_with_events(&mut engine);
    let choice = offered
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .expect("P9 orders the real draw replacements");
    assert_eq!(choice.deciding_player_id, 9);
    assert_eq!(choice.choice_kind(), ChoiceKind::ReplacementEffect);
    assert_eq!(choice.replacement_options.len(), 2);
    assert_eq!(
        engine.state.players[0].hand.len(),
        7,
        "P4 has already drawn"
    );
    assert!(jar_cohort_is_stored(&engine));

    let departure = engine
        .apply_scenario_command(4, &concede())
        .expect("P4 concedes");
    assert!(
        engine.state.outcome.is_none(),
        "P9 and P27 remain in the game"
    );
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[1].has_lost && !engine.state.players[2].has_lost);
    let mut choice = find_resolution_choice(&departure)
        .expect("the surviving drawer receives refreshed replacement choices");
    let mut events = offered;
    events.extend(departure.events);
    let mut completed = false;
    for _ in 0..80 {
        assert_eq!(choice.deciding_player_id, 9);
        assert_eq!(choice.choice_kind(), ChoiceKind::ReplacementEffect);
        let option = choice
            .replacement_options
            .first()
            .expect("replacement option")
            .application_id;
        let batch = engine
            .apply_scenario_command(9, &submit_resolution_choice(vec![option]))
            .expect("choose a surviving draw replacement");
        let next_choice = find_resolution_choice(&batch);
        events.extend(batch.events);
        match next_choice {
            Some(next) => choice = next,
            None => {
                completed = true;
                break;
            }
        }
    }
    assert!(completed, "all replacement choices finish");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == activation))
            .count(),
        1,
        "the begun activation completes once"
    );
    assert_eq!(engine.state.players[1].hand.len(), 28);
    assert_eq!(engine.state.players[2].hand.len(), 7);
    assert_eq!(
        engine.state.turn_history.current.player(4).cards_drawn,
        prior_draws[0] + 7
    );
    assert_eq!(
        engine.state.turn_history.current.player(9).cards_drawn,
        prior_draws[1] + 28
    );
    assert_eq!(
        engine.state.turn_history.current.player(27).cards_drawn,
        prior_draws[2] + 7
    );
    for id in originals[1].iter().chain(&originals[2]) {
        assert_eq!(engine.state.objects[id].zone, Zone::Exile);
        assert!(engine.state.objects[id].face_down);
    }
    assert!(
        !engine
            .state
            .active_event_observers
            .iter()
            .any(|observer| matches!(
                &observer.payload,
                EventObserverPayload::StageDelayedTrigger(payload) if payload.card_id == JAR
            )),
        "a departed controller cannot own a future delayed trigger"
    );
    assert!(
        !jar_cohort_is_stored(&engine),
        "the unavailable future trigger retires its cohort"
    );
}

#[test]
fn memory_jar_delayed_discard_and_return_finish_when_controller_departs() {
    let (mut engine, originals, draws, trigger, _) = parked_jar_discard(261_027_212, false);
    let departure = engine
        .apply_scenario_command(4, &concede())
        .expect("P4 concedes");
    assert!(
        engine.state.outcome.is_none(),
        "P9, P27 and P36 remain in the game"
    );
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.players[1..]
        .iter()
        .all(|player| !player.has_lost));
    assert!(
        jar_cohort_is_stored(&engine),
        "the begun child retains its exact cohort"
    );
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the discard choice survives");
    assert_eq!(pending.deciding_player, 9);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::PrivateReplacement
    );
    let mut events = departure.events;
    finish_leng_discard(&mut engine, &mut events);
    assert_surviving_jar_return(&engine, &originals, &draws, trigger, &events);
}

#[test]
fn memory_jar_child_restores_after_observer_and_controller_departures() {
    let (mut engine, originals, draws, trigger, aura) = parked_jar_discard(261_027_213, true);
    let (aura, creature) = aura.expect("the fixture installed a real Banishing Light observer");
    let source_departure = engine
        .apply_scenario_command(36, &concede())
        .expect("P36 concedes");
    assert!(engine.state.outcome.is_none());
    assert!(engine.state.players[3].has_lost);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the Aura needs an attachment");
    assert_eq!(pending.deciding_player, 9);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(pending.presentation.candidates.contains(&creature));
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    assert!(
        jar_cohort_is_stored(&engine),
        "the suspended child retains its cohort"
    );
    assert!(
        !source_departure
            .events
            .iter()
            .any(|event| matches!(&event.ev,
                Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger
            )),
        "the Jar child is still suspended"
    );

    let controller_departure = engine
        .apply_scenario_command(4, &concede())
        .expect("P4 concedes");
    assert!(
        engine.state.outcome.is_none(),
        "P9 and P27 remain in the game"
    );
    assert!(engine.state.players[0].has_lost && engine.state.players[3].has_lost);
    assert!(!engine.state.players[1].has_lost && !engine.state.players[2].has_lost);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the Aura choice survives");
    assert_eq!(pending.deciding_player, 9);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(
        jar_cohort_is_stored(&engine),
        "the suspended begun child keeps its cohort"
    );
    assert!(
        !controller_departure
            .events
            .iter()
            .any(|event| matches!(&event.ev,
                Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger
            )),
        "the Jar child does not complete while the Aura choice is pending"
    );

    let resumed = engine
        .apply_scenario_command(9, &submit_resolution_choice(vec![creature]))
        .expect("attach the returned Aura and restore the Jar child");
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the Jar discard choice is restored");
    assert_eq!(pending.deciding_player, 9);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::PrivateReplacement
    );
    assert!(
        jar_cohort_is_stored(&engine),
        "the restored child still has its cohort"
    );
    let mut events = source_departure.events;
    events.extend(controller_departure.events);
    events.extend(resumed.events);
    finish_leng_discard(&mut engine, &mut events);
    assert_surviving_jar_return(&engine, &originals, &draws, trigger, &events);
}

#[test]
fn memory_jar_exiles_each_hand_face_down_draws_in_turn_order_and_restores_its_cohort() {
    let (mut engine, old_hands) = game(
        261_027_200,
        &[4, 9, 27],
        &[&["forest", "mountain"], &[], &["swamp"]],
    );
    engine.state.active_player_idx = 2;
    engine.state.priority_idx = 2;
    let expected_draws: Vec<Vec<u32>> = engine
        .state
        .players
        .iter()
        .map(|player| player.library.iter().take(7).copied().collect())
        .collect();
    assert!(expected_draws.iter().all(|cards| cards.len() == 7));
    let old_generations: Vec<Vec<u64>> = old_hands
        .iter()
        .map(|hand| {
            hand.iter()
                .map(|id| {
                    engine
                        .state
                        .zone_change_generation
                        .get(id)
                        .copied()
                        .unwrap_or(0)
                })
                .collect()
        })
        .collect();
    let jar = inject_permanent_on_battlefield(&mut engine, 2, JAR);
    apply_ability(&mut engine, 27, jar, 0, vec![]).expect("activate Memory Jar");
    assert_eq!(engine.state.objects[&jar].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);

    let events = pass_round_with_events(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    let first_draw = events
        .iter()
        .position(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains(" draws ")))
        .expect("the draws are published");
    for (seat, hand) in old_hands.iter().enumerate() {
        assert!(same_hand(
            &engine.state.players[seat].hand,
            &expected_draws[seat]
        ));
        for (index, id) in hand.iter().enumerate() {
            let object = &engine.state.objects[id];
            assert_eq!(object.zone, Zone::Exile);
            assert!(object.face_down);
            assert_eq!(
                engine.state.zone_change_generation[id],
                old_generations[seat][index] + 1
            );
            let move_index = events
                .iter()
                .position(|event| {
                    matches!(&event.ev, Some(Ev::PermanentMoved(moved))
                    if moved.object_id == *id
                        && moved.destination == rv1::permanent_moved::Destination::Exile as i32
                        && moved.face_down)
                })
                .expect("the committed exile is already face down");
            assert!(
                move_index < first_draw,
                "every hand is exiled before any draw"
            );
        }
    }
    let draws: Vec<_> = events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::Log(log)) if log.text.contains(" draws ") => Some(log.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        draws,
        [
            "P27 draws 7 cards (Memory Jar).",
            "P4 draws 7 cards (Memory Jar).",
            "P9 draws 7 cards (Memory Jar).",
        ]
    );
    let view = events
        .iter()
        .rev()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("the completed move publishes a zone view");
    for player in &view.per_player {
        assert!(player
            .hideaway_exile_cards
            .iter()
            .all(|entry| { !old_hands.iter().flatten().any(|id| *id == entry.object_id) }));
    }

    advance_to_end_step(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the delayed trigger uses the stack"
    );
    assert!(engine.state.stack[0].is_triggered);
    pass_round_with_events(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    for (seat, hand) in old_hands.iter().enumerate() {
        assert!(same_hand(&engine.state.players[seat].hand, hand));
        for id in hand {
            assert_eq!(engine.state.objects[id].zone, Zone::Hand);
            assert!(!engine.state.objects[id].face_down);
        }
        for id in &expected_draws[seat] {
            assert_eq!(engine.state.objects[id].zone, Zone::Graveyard);
        }
    }
}

#[test]
fn memory_jar_cannot_be_activated_when_tapped() {
    let (mut engine, _) = game(261_027_201, &[0, 1], &[&[], &[]]);
    let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
    engine.state.objects.get_mut(&jar).unwrap().tapped = true;
    apply_ability(&mut engine, 0, jar, 0, vec![]).expect_err("the tap cost is mandatory");
    assert_eq!(engine.state.objects[&jar].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn memory_jar_copied_by_rings_tracks_two_independent_hand_cohorts() {
    let (mut engine, original_hands) = game(261_027_202, &[0, 1], &[&["mountain"], &["forest"]]);
    inject_permanent_on_battlefield(&mut engine, 0, "rings_of_brighthearth");
    let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, jar, 0, vec![]).expect("activate the Jar");
    assert_eq!(engine.state.objects[&jar].zone, Zone::Graveyard);
    let offer = pass_both_players_with_batch(&mut engine);
    let choice = find_resolution_choice(&offer).expect("Rings offers its payment");
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    let payment = engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose to pay for the copy");
    let choice = find_resolution_choice(&payment).expect("Rings asks for mana");
    assert_eq!(choice.choice_kind(), ChoiceKind::ManaPayment);
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay for the copy");
    assert_eq!(engine.state.stack.len(), 2);

    pass_priority_round(&mut engine);
    let middle_hands: Vec<Vec<u32>> = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect();
    assert!(middle_hands.iter().all(|hand| hand.len() == 7));
    for id in original_hands.iter().flatten() {
        assert_eq!(engine.state.objects[id].zone, Zone::Exile);
        assert!(engine.state.objects[id].face_down);
    }
    pass_priority_round(&mut engine);
    let final_draws: Vec<Vec<u32>> = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect();
    assert!(final_draws.iter().all(|hand| hand.len() == 7));
    for id in original_hands.iter().chain(&middle_hands).flatten() {
        assert_eq!(engine.state.objects[id].zone, Zone::Exile);
        assert!(engine.state.objects[id].face_down);
    }

    advance_to_end_step(&mut engine);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 2);
    pass_priority_round(&mut engine);
    let first_return: Vec<Vec<u32>> = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect();
    let original_returned_first = same_hands(&first_return, &original_hands);
    assert!(original_returned_first || same_hands(&first_return, &middle_hands));
    for id in final_draws.iter().flatten() {
        assert_eq!(engine.state.objects[id].zone, Zone::Graveyard);
    }
    pass_priority_round(&mut engine);
    let second_return: Vec<Vec<u32>> = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect();
    if original_returned_first {
        assert!(same_hands(&second_return, &middle_hands));
    } else {
        assert!(same_hands(&second_return, &original_hands));
    }
    for id in first_return.iter().flatten() {
        assert_eq!(engine.state.objects[id].zone, Zone::Graveyard);
    }
}

#[test]
fn memory_jar_commander_sba_after_draw_can_remove_it_from_the_delayed_cohort() {
    let (mut engine, commander, ordinary) = jar_with_commander_in_hand(261_027_203);
    let draws = resolve_jar_and_choose_commander_sba(&mut engine, commander, ordinary, 0);
    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
    assert!(!engine.state.objects[&commander].face_down);

    advance_to_end_step(&mut engine);
    let events = pass_round_with_events(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
    for (seat, id) in ordinary.into_iter().enumerate() {
        assert_eq!(engine.state.objects[&id].zone, Zone::Hand);
        assert!(engine.state.players[seat].hand.contains(&id));
    }
    for id in draws.iter().flatten() {
        assert_eq!(engine.state.objects[id].zone, Zone::Graveyard);
    }
    assert_no_public_old_hand_names(&events);
}

#[test]
fn memory_jar_delayed_return_offers_owner_command_or_hand_before_the_batch() {
    for destination in [0, 1] {
        let (mut engine, commander, ordinary) =
            jar_with_commander_in_hand(261_027_204 + destination as u64);
        let draws = resolve_jar_and_choose_commander_sba(&mut engine, commander, ordinary, 1);
        assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
        assert!(engine.state.objects[&commander].face_down);

        advance_to_end_step(&mut engine);
        let first = engine.state.priority_player_id();
        engine
            .apply_scenario_command(first, &pass())
            .expect("first player passes");
        let last = engine.state.priority_player_id();
        let offered = engine
            .apply_scenario_command(last, &pass())
            .expect("the delayed return must offer the commander replacement");
        let choice = find_resolution_choice(&offered).expect("the owner chooses before any return");
        assert_eq!(choice.deciding_player_id, 9);
        assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
        assert!(choice.prompt_text.contains("903.9b"));
        assert_eq!(
            choice
                .resolution_branches
                .iter()
                .map(|branch| branch.label.as_str())
                .collect::<Vec<_>>(),
            ["Command zone", "Hand"]
        );
        for id in ordinary.into_iter().chain([commander]) {
            assert_eq!(
                engine.state.objects[&id].zone,
                Zone::Exile,
                "no member returns before the owner's choice"
            );
        }
        for id in draws.iter().flatten() {
            assert_eq!(
                engine.state.objects[id].zone,
                Zone::Graveyard,
                "all new hands have already been discarded"
            );
        }
        assert_no_public_old_hand_names(&offered.events);

        let completed = engine
            .apply_scenario_command(9, &choose_branch(destination))
            .expect("finish the simultaneous return after the owner's choice");
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.objects[&commander].zone,
            if destination == 0 {
                Zone::Command
            } else {
                Zone::Hand
            }
        );
        assert!(!engine.state.objects[&commander].face_down);
        for (seat, id) in ordinary.into_iter().enumerate() {
            assert_eq!(engine.state.objects[&id].zone, Zone::Hand);
            assert!(engine.state.players[seat].hand.contains(&id));
            assert!(!engine.state.objects[&id].face_down);
        }
        assert_no_public_old_hand_names(&completed.events);
    }
}

#[test]
fn memory_jar_with_every_hand_empty_still_discards_the_new_hands() {
    let (mut engine, originals) = game(261_027_220, &[4, 9], &[&[], &[]]);
    let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
    apply_ability(&mut engine, 4, jar, 0, vec![]).expect("activate the Jar");
    pass_round_with_events(&mut engine);
    let draws: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.clone())
        .collect();
    assert!(draws.iter().all(|hand| hand.len() == 7));
    assert!(originals.iter().all(Vec::is_empty));
    assert!(
        jar_cohort_is_stored(&engine),
        "even an empty cohort owns a delayed trigger"
    );
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    pass_round_with_events(&mut engine);
    assert!(engine
        .state
        .players
        .iter()
        .all(|player| player.hand.is_empty()));
    assert!(draws
        .iter()
        .flatten()
        .all(|id| engine.state.objects[id].zone == Zone::Graveyard));
    assert!(!jar_cohort_is_stored(&engine));
}

#[test]
fn memory_jar_short_library_loss_does_not_skip_other_players_draws_or_returns() {
    let (mut engine, originals) = game(
        261_027_221,
        &[4, 9, 27],
        &[&["mountain"], &["forest"], &["swamp"]],
    );
    engine.state.players[1].library.truncate(3);
    let prior = [4, 9, 27].map(|id| engine.state.turn_history.current.player(id).cards_drawn);
    let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
    apply_ability(&mut engine, 4, jar, 0, vec![]).expect("activate the Jar");
    pass_round_with_events(&mut engine);
    assert!(
        engine.state.players[1].has_lost,
        "P9 attempted to draw from an empty library"
    );
    assert!(engine.state.outcome.is_none(), "P4 and P27 remain");
    assert_eq!(
        engine.state.turn_history.current.player(4).cards_drawn,
        prior[0] + 7
    );
    assert_eq!(
        engine.state.turn_history.current.player(9).cards_drawn,
        prior[1] + 3
    );
    assert_eq!(
        engine.state.turn_history.current.player(27).cards_drawn,
        prior[2] + 7
    );
    let draws: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.clone())
        .collect();
    assert!(jar_cohort_is_stored(&engine));
    assert!(!engine.state.objects.contains_key(&originals[1][0]));
    advance_to_end_step(&mut engine);
    pass_round_with_events(&mut engine);
    for seat in [0, 2] {
        assert!(same_hand(
            &engine.state.players[seat].hand,
            &originals[seat]
        ));
        assert!(draws[seat]
            .iter()
            .all(|id| engine.state.objects[id].zone == Zone::Graveyard));
    }
    assert!(!jar_cohort_is_stored(&engine));
}

#[test]
fn memory_jar_activated_during_end_step_waits_for_the_next_one() {
    let (mut engine, originals) = game(261_027_222, &[4, 9], &[&["mountain"], &["forest"]]);
    advance_to_end_step(&mut engine);
    let turn = engine.state.turn_instance;
    let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
    apply_ability(&mut engine, 4, jar, 0, vec![]).expect("activate during the end step");
    pass_round_with_events(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "the current end-step beginning has passed"
    );
    assert!(jar_cohort_is_stored(&engine));
    assert!(originals
        .iter()
        .flatten()
        .all(|id| engine.state.objects[id].zone == Zone::Exile));
    engine
        .apply_scenario_command(4, &primitive_yield())
        .expect("finish the current end step");
    assert!(engine.state.turn_instance > turn);
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "the next end step triggers");
    pass_round_with_events(&mut engine);
    for (seat, hand) in originals.iter().enumerate() {
        assert!(same_hand(&engine.state.players[seat].hand, hand));
    }
    assert!(!jar_cohort_is_stored(&engine));
}

fn counter_jar_ability_with_tidebinder(engine: &mut GameEngine, ability: u32) {
    if engine.state.priority_player_id() == 0 {
        engine
            .apply_scenario_command(0, &pass())
            .expect("pass to the opponent");
    }
    inject_card_into_hand(engine, 1, "tishanas_tidebinder");
    give_mana(
        engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 1, "tishanas_tidebinder");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("flash Tidebinder");
    pass_both_players(engine);
    let choice = RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                object_id: ability,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(1, &choice)
        .expect("target the Jar ability");
    pass_both_players(engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == ability));
}

#[test]
fn memory_jar_activation_and_delayed_trigger_can_each_be_countered() {
    for delayed in [false, true] {
        let (mut engine, originals) = game(
            261_027_223 + u64::from(delayed),
            &[0, 1],
            &[&["mountain"], &["forest"]],
        );
        let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
        apply_ability(&mut engine, 0, jar, 0, vec![]).expect("activate the Jar");
        if delayed {
            pass_both_players(&mut engine);
            assert!(jar_cohort_is_stored(&engine));
            advance_to_end_step(&mut engine);
        }
        let ability = engine
            .state
            .stack
            .last()
            .expect("the ability is on the stack")
            .id;
        let hands_before: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|player| player.hand.clone())
            .collect();
        counter_jar_ability_with_tidebinder(&mut engine, ability);
        assert!(
            !jar_cohort_is_stored(&engine),
            "countering retires any owned cohort"
        );
        assert!(same_hands(
            &hands_before,
            &engine
                .state
                .players
                .iter()
                .map(|p| p.hand.clone())
                .collect::<Vec<_>>()
        ));
        for id in originals.iter().flatten() {
            assert_eq!(
                engine.state.objects[id].zone,
                if delayed { Zone::Exile } else { Zone::Hand }
            );
        }
        assert!(!engine
            .state
            .active_event_observers
            .iter()
            .any(|observer| matches!(
                &observer.payload,
                EventObserverPayload::StageDelayedTrigger(payload) if payload.card_id == JAR
            )));
    }
}

#[test]
fn memory_jar_current_discard_chooser_can_depart_without_stranding_survivors() {
    let (mut engine, originals, draws, trigger, _) = parked_jar_discard(261_027_225, false);
    let departure = engine
        .apply_scenario_command(9, &concede())
        .expect("P9 concedes at their choice");
    assert!(engine.state.outcome.is_none());
    assert!(engine.state.players[1].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.objects.contains_key(&originals[1][0]));
    assert_surviving_jar_return(&engine, &originals, &draws, trigger, &departure.events);
}

#[test]
fn memory_jar_keeps_live_controller_when_source_owner_or_other_player_departs() {
    for (departing_seat, after_trigger) in [(1, false), (1, true), (2, false), (2, true)] {
        let (mut engine, originals) = game(
            261_027_226 + departing_seat as u64 * 2 + u64::from(after_trigger),
            &[4, 9, 27],
            &[&["mountain"], &["forest"], &["swamp"]],
        );
        // Synthetic control change: P9 owns the Jar, while P4 controls and activates it.
        let jar = inject_permanent_on_battlefield(&mut engine, 1, JAR);
        engine.state.players[1].battlefield.retain(|id| *id != jar);
        engine.state.players[0].battlefield.push(jar);
        let object = engine.state.objects.get_mut(&jar).unwrap();
        object.base_controller = 4;
        object.controller = 4;
        apply_ability(&mut engine, 4, jar, 0, vec![]).expect("P4 activates the borrowed Jar");
        pass_round_with_events(&mut engine);
        let draws: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|player| player.hand.clone())
            .collect();
        if after_trigger {
            advance_to_end_step(&mut engine);
            assert_eq!(engine.state.stack.len(), 1);
        }
        let player = engine.state.players[departing_seat].id;
        engine
            .apply_scenario_command(player, &concede())
            .expect("the other player concedes");
        assert!(engine.state.outcome.is_none());
        assert!(
            jar_cohort_is_stored(&engine),
            "the live controller's delayed work survives"
        );
        if !after_trigger {
            advance_to_end_step(&mut engine);
        }
        assert_eq!(engine.state.stack.len(), 1);
        pass_round_with_events(&mut engine);
        for seat in 0..3 {
            if seat == departing_seat {
                assert!(!engine.state.objects.contains_key(&originals[seat][0]));
                continue;
            }
            assert!(same_hand(
                &engine.state.players[seat].hand,
                &originals[seat]
            ));
            assert!(draws[seat]
                .iter()
                .all(|id| engine.state.objects[id].zone == Zone::Graveyard));
        }
        assert!(!jar_cohort_is_stored(&engine));
    }
}

#[test]
fn memory_jar_does_not_return_a_later_exile_incarnation() {
    let (mut engine, originals) = game(261_027_231, &[4, 9], &[&["sol_ring"], &["forest"]]);
    let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
    apply_ability(&mut engine, 4, jar, 0, vec![]).expect("activate the Jar");
    pass_round_with_events(&mut engine);
    let old = originals[0][0];
    let captured_generation = engine.state.zone_change_generation[&old];
    engine.enable_dev_commands();
    for zone in [rv1::DevZone::Hand, rv1::DevZone::Exile] {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(rv1::DevCommand {
                        target_player_id: 4,
                        dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                            card_name: "Sol Ring".into(),
                            zone: zone as i32,
                            ready: false,
                        })),
                    })),
                },
            )
            .expect("move the same card through the real zone-change funnel");
    }
    assert_eq!(engine.state.objects[&old].zone, Zone::Exile);
    assert_eq!(
        engine.state.zone_change_generation[&old],
        captured_generation + 2
    );
    advance_to_end_step(&mut engine);
    pass_round_with_events(&mut engine);
    assert_eq!(engine.state.objects[&old].zone, Zone::Exile);
    assert_eq!(
        engine.state.zone_change_generation[&old],
        captured_generation + 2
    );
    assert!(engine.state.players[0].hand.is_empty());
    assert!(same_hand(&engine.state.players[1].hand, &originals[1]));
    assert!(!jar_cohort_is_stored(&engine));
}

#[test]
fn memory_jar_commander_return_choice_survives_controller_or_owner_departure() {
    for departing in [4, 9] {
        let decks = vec![
            EngineDeck {
                mainboard: deck_with("island", &[]),
                commanders: Vec::new(),
            },
            EngineDeck {
                mainboard: deck_with("island", &[]),
                commanders: vec!["kami_of_the_crescent_moon".into()],
            },
            EngineDeck {
                mainboard: deck_with("island", &[]),
                commanders: Vec::new(),
            },
        ];
        let mut engine = GameEngine::new_with_commander_decks(
            tricerules_cards::registry::global(),
            261_027_232 + departing as u64,
            &[4, 9, 27],
            20,
            Some(decks),
            true,
        )
        .expect("create a three-player Commander fixture");
        advance_to_main1_from_game_start(&mut engine);
        for player in &mut engine.state.players {
            for id in std::mem::take(&mut player.hand) {
                engine.state.objects.get_mut(&id).unwrap().zone = Zone::Library;
                player.library.push_back(id);
            }
        }
        let commander = engine.state.players[1].command_zone[0];
        engine.enable_dev_commands();
        engine
            .apply_scenario_command(
                9,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(rv1::DevCommand {
                        target_player_id: 9,
                        dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                            card_name: "Kami of the Crescent Moon".into(),
                            zone: rv1::DevZone::Hand as i32,
                            ready: false,
                        })),
                    })),
                },
            )
            .expect("put P9's declared commander into hand");
        let originals = vec![
            vec![inject_card_into_hand(&mut engine, 0, "forest")],
            vec![commander, inject_card_into_hand(&mut engine, 1, "mountain")],
            vec![inject_card_into_hand(&mut engine, 2, "swamp")],
        ];
        let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
        apply_ability(&mut engine, 4, jar, 0, vec![]).expect("activate the Jar");
        let offered = pass_round_with_events(&mut engine);
        assert!(offered.iter().any(|event| matches!(&event.ev,
            Some(Ev::ResolutionChoiceRequired(choice)) if choice.deciding_player_id == 9
                && choice.prompt_text.contains("903.9a")
        )));
        let draws: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|player| player.hand.clone())
            .collect();
        engine
            .apply_scenario_command(9, &choose_branch(1))
            .expect("keep the commander in exile");
        advance_to_end_step(&mut engine);
        let trigger = engine
            .state
            .stack
            .last()
            .expect("the delayed trigger is on the stack")
            .id;
        let offered = pass_round_with_events(&mut engine);
        assert!(offered.iter().any(|event| matches!(&event.ev,
            Some(Ev::ResolutionChoiceRequired(choice)) if choice.deciding_player_id == 9
                && choice.prompt_text.contains("903.9b")
        )));
        assert!(originals
            .iter()
            .flatten()
            .all(|id| engine.state.objects[id].zone == Zone::Exile));
        assert!(jar_cohort_is_stored(&engine));
        let departure = engine
            .apply_scenario_command(departing, &concede())
            .expect("player concedes");
        assert!(engine.state.outcome.is_none());
        let mut events = offered;
        events.extend(departure.events);
        if departing == 4 {
            assert!(jar_cohort_is_stored(&engine));
            let pending = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("the owner's choice survives");
            assert_eq!(pending.deciding_player, 9);
            let finished = engine
                .apply_scenario_command(9, &choose_branch(1))
                .expect("the surviving owner chooses hand");
            events.extend(finished.events);
        } else {
            assert!(!engine.state.objects.contains_key(&commander));
        }
        assert!(engine.state.pending_resolution.is_none());
        assert_surviving_jar_return(&engine, &originals, &draws, trigger, &events);
    }
}

#[test]
fn memory_jar_replays_private_discard_choice_and_controller_departure() {
    use prost::Message;

    fn fixture() -> (GameEngine, Vec<Vec<u32>>, u32) {
        let (mut engine, originals) = game(
            261_027_234,
            &[4, 9, 27, 36],
            &[&["mountain"], &["forest"], &["swamp"], &["island"]],
        );
        inject_permanent_on_battlefield(&mut engine, 1, "library_of_leng");
        let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
        (engine, originals, jar)
    }

    fn record_and_replay(
        engine: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice())
            .expect("decode the recorded command");
        let actual = engine
            .apply_scenario_command(actor, &command)
            .expect("record command");
        let replayed = replay
            .apply_scenario_command(actor, &decoded)
            .expect("replay command");
        assert_eq!(
            actual, replayed,
            "each authoritative event batch is deterministic"
        );
        events.extend(actual.events);
    }

    let (mut engine, originals, jar) = fixture();
    let (mut replay, replay_originals, replay_jar) = fixture();
    assert_eq!((jar, &originals), (replay_jar, &replay_originals));
    let mut events = Vec::new();
    let activate = activate_ability_for(&engine, jar, 0, vec![]);
    record_and_replay(&mut engine, &mut replay, 4, activate, &mut events);
    for _ in 0..4 {
        let actor = engine.state.priority_player_id();
        record_and_replay(&mut engine, &mut replay, actor, pass(), &mut events);
    }
    let draws: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.clone())
        .collect();
    for _ in 0..10 {
        if engine.state.turn_step == TurnStep::EndStep {
            break;
        }
        let actor = engine.state.active_player_id();
        record_and_replay(
            &mut engine,
            &mut replay,
            actor,
            primitive_yield(),
            &mut events,
        );
    }
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
    let trigger = engine
        .state
        .stack
        .last()
        .expect("the delayed trigger is on the stack")
        .id;
    for _ in 0..4 {
        let actor = engine.state.priority_player_id();
        record_and_replay(&mut engine, &mut replay, actor, pass(), &mut events);
    }
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P9 has a discard choice");
    assert_eq!(pending.deciding_player, 9);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::PrivateReplacement
    );
    record_and_replay(
        &mut engine,
        &mut replay,
        9,
        submit_resolution_choice(vec![0]),
        &mut events,
    );
    record_and_replay(&mut engine, &mut replay, 4, concede(), &mut events);
    for _ in 0..16 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            break;
        };
        assert_eq!(pending.deciding_player, 9);
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::PrivateReplacement
        );
        record_and_replay(
            &mut engine,
            &mut replay,
            9,
            submit_resolution_choice(vec![0]),
            &mut events,
        );
    }
    assert!(engine.state.pending_resolution.is_none());
    assert!(replay.state.pending_resolution.is_none());
    assert_surviving_jar_return(&engine, &originals, &draws, trigger, &events);
    for seat in 1..4 {
        assert!(same_hand(
            &engine.state.players[seat].hand,
            &replay.state.players[seat].hand
        ));
        for id in originals[seat].iter().chain(&draws[seat]) {
            assert_eq!(engine.state.objects[id].zone, replay.state.objects[id].zone);
            assert_eq!(
                engine.state.zone_change_generation[id],
                replay.state.zone_change_generation[id]
            );
        }
    }
    assert!(!jar_cohort_is_stored(&replay));
}

#[test]
fn memory_jar_unbegun_work_ends_when_its_controller_departs() {
    // Concede with the activation still stacked, with its delayed observer waiting, or with
    // the delayed trigger stacked. None of these occurrences has begun resolving yet.
    for stage in 0..3 {
        let (mut engine, originals) = game(
            261_027_240 + stage,
            &[4, 9, 27],
            &[&["mountain"], &["forest"], &["swamp"]],
        );
        let jar = inject_permanent_on_battlefield(&mut engine, 0, JAR);
        apply_ability(&mut engine, 4, jar, 0, vec![]).expect("activate the Jar");
        if stage > 0 {
            pass_round_with_events(&mut engine);
            assert!(jar_cohort_is_stored(&engine));
        }
        if stage == 2 {
            advance_to_end_step(&mut engine);
            assert_eq!(
                engine.state.stack.len(),
                1,
                "the delayed trigger is waiting on the stack"
            );
        }
        let survivor_hands: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|player| player.hand.clone())
            .collect();
        engine
            .apply_scenario_command(4, &concede())
            .expect("the controller concedes");
        assert!(engine.state.outcome.is_none());
        assert!(!engine.state.players[1].has_lost && !engine.state.players[2].has_lost);
        assert!(engine.state.stack.is_empty());
        assert!(!jar_cohort_is_stored(&engine));
        assert!(!engine
            .state
            .active_event_observers
            .iter()
            .any(|observer| matches!(
                &observer.payload,
                EventObserverPayload::StageDelayedTrigger(payload) if payload.card_id == JAR
            )));
        for seat in [1, 2] {
            assert!(same_hand(
                &engine.state.players[seat].hand,
                &survivor_hands[seat]
            ));
            for id in &originals[seat] {
                assert_eq!(
                    engine.state.objects[id].zone,
                    if stage == 0 { Zone::Hand } else { Zone::Exile }
                );
            }
        }
        if stage == 1 {
            // The active player has left; advance this turn using live priority holders rather
            // than asking the departed active player to submit a primitive yield.
            for _ in 0..40 {
                if engine.state.turn_step == TurnStep::EndStep {
                    break;
                }
                let actor = engine.state.priority_player_id();
                engine
                    .apply_scenario_command(actor, &pass())
                    .expect("a survivor passes priority");
            }
            assert_eq!(engine.state.turn_step, TurnStep::EndStep);
            assert!(
                engine.state.stack.is_empty(),
                "no unavailable delayed trigger appears"
            );
            for seat in [1, 2] {
                assert!(same_hand(
                    &engine.state.players[seat].hand,
                    &survivor_hands[seat]
                ));
                assert_eq!(engine.state.objects[&originals[seat][0]].zone, Zone::Exile);
            }
        }
    }
}

#[test]
fn memory_jar_source_owner_can_depart_before_borrowed_activation_resolves() {
    let (mut engine, originals) = game(
        261_027_243,
        &[4, 9, 27],
        &[&["mountain"], &["forest"], &["swamp"]],
    );
    // The physical source belongs to P9, but P4 controls and activates it.
    let jar = inject_permanent_on_battlefield(&mut engine, 1, JAR);
    engine.state.players[1].battlefield.retain(|id| *id != jar);
    engine.state.players[0].battlefield.push(jar);
    let object = engine.state.objects.get_mut(&jar).unwrap();
    object.base_controller = 4;
    object.controller = 4;
    apply_ability(&mut engine, 4, jar, 0, vec![]).expect("activate the borrowed Jar");
    let activation = engine.state.stack.last().unwrap().id;
    engine
        .apply_scenario_command(9, &concede())
        .expect("the physical source owner concedes");
    assert!(engine.state.outcome.is_none());
    assert!(!engine.state.objects.contains_key(&jar));
    assert!(engine.state.stack.iter().any(|item| item.id == activation));
    pass_round_with_events(&mut engine);
    assert!(jar_cohort_is_stored(&engine));
    for seat in [0, 2] {
        assert_eq!(engine.state.players[seat].hand.len(), 7);
        assert_eq!(engine.state.objects[&originals[seat][0]].zone, Zone::Exile);
    }
    advance_to_end_step(&mut engine);
    pass_round_with_events(&mut engine);
    for seat in [0, 2] {
        assert!(same_hand(
            &engine.state.players[seat].hand,
            &originals[seat]
        ));
    }
    assert!(!jar_cohort_is_stored(&engine));
}
