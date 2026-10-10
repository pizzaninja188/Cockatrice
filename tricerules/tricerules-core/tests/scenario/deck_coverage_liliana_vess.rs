use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{ruled_event::Ev, ChoiceKind, RuledCommand};

const LILIANA: &str = "liliana_vess";

#[test]
fn liliana_vess_has_its_exact_printed_identity_and_three_loyalty_abilities() {
    use tricerules_cards::primitives::AbilityCost;

    let card = tricerules_cards::registry::global()
        .get(LILIANA)
        .expect("Liliana Vess needs a complete ruled definition");
    assert_eq!(card.name, "Liliana Vess");
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{B}{B}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Liliana"]);
    assert_eq!(face.loyalty, Some(5));
    assert_eq!(face.activated_abilities.len(), 3);
    let loyalty_costs = face
        .activated_abilities
        .iter()
        .map(|ability| match ability.costs.as_slice() {
            [AbilityCost::Loyalty(amount)] => *amount,
            costs => panic!("expected one loyalty cost, found {costs:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(loyalty_costs, [1, -2, -8]);
}

fn engine_with_players(seed: u64, player_ids: &[i32]) -> GameEngine {
    let deck = deck_with("swamp", &[LILIANA]);
    let decks = Some(vec![deck; player_ids.len()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        player_ids,
        20,
        decks,
        true,
    )
    .expect("Liliana Vess scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn put_liliana_on_battlefield(engine: &mut GameEngine, player: usize) -> u32 {
    move_ready_to_battlefield(engine, player, LILIANA)
}

fn pass_until_resolution_choice(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    for _ in 0..engine.state.players.len() * 2 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("pass priority toward Liliana's resolving ability");
        if engine.state.pending_resolution.is_some() {
            return batch;
        }
    }
    panic!("Liliana's ability did not reach its resolution choice");
}

fn apply_logged_to_pair(
    original: &mut GameEngine,
    replay: &mut GameEngine,
    actor: i32,
    command: RuledCommand,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    use prost::Message;

    let decoded = RuledCommand::decode(command.encode_to_vec().as_slice())
        .expect("accepted command serializes and decodes");
    let observed = original
        .apply_command(actor, &command)
        .expect("original command is accepted");
    let reproduced = replay
        .apply_command(actor, &decoded)
        .expect("replayed command is accepted");
    assert_eq!(observed, reproduced);
    assert_eq!(
        original.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    observed
}

fn pass_until_returned(
    engine: &mut GameEngine,
    object_ids: &[u32],
) -> Vec<tricerules_proto::ruled::v1::RuledEventBatch> {
    let mut batches = Vec::new();
    for _ in 0..engine.state.players.len() * 2 {
        batches.extend(
            answer_simultaneous_entry_order_in_engine_order(engine)
                .into_iter()
                .map(|(_, _, batch)| batch),
        );
        answer_trigger_order_in_engine_order(engine);
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("pass priority toward Liliana's graveyard return");
        batches.push(batch);
        if object_ids
            .iter()
            .all(|object_id| engine.state.objects[object_id].zone == Zone::Battlefield)
        {
            return batches;
        }
    }
    let remaining = object_ids
        .iter()
        .map(|object_id| {
            let object = &engine.state.objects[object_id];
            (*object_id, object.card_id.as_str(), object.zone)
        })
        .collect::<Vec<_>>();
    panic!(
        "Liliana's graveyard cohort did not enter: remaining={remaining:?}, priority={}, stack={}, pending={:?}",
        engine.state.priority_player_id(),
        engine.state.stack.len(),
        engine.state.pending_resolution
    );
}

fn resolve_current_stack(engine: &mut GameEngine) {
    for _ in 0..128 {
        answer_simultaneous_entry_order_in_engine_order(engine);
        answer_trigger_order_in_engine_order(engine);
        if engine.state.pending_resolution.is_some() || engine.state.stack.is_empty() {
            break;
        }
        pass_priority_round(engine);
    }
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn liliana_vess_plus_one_targeted_player_chooses_the_discard() {
    let mut engine = engine_with_players(513_001, &[10, 20]);
    let source = put_liliana_on_battlefield(&mut engine, 0);
    let discarded = inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    let retained = inject_card_into_hand(&mut engine, 1, "storm_crow");

    engine
        .apply_command(
            10,
            &activate_ability_for(&engine, source, 0, target_player(20)),
        )
        .expect("target an opponent with Liliana's +1");
    let batch = pass_until_resolution_choice(&mut engine);
    let choice = find_resolution_choice(&batch).expect("targeted player discard choice");
    assert_eq!(choice.deciding_player_id, 20);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(choice.candidate_object_ids.contains(&discarded));
    assert!(engine
        .apply_command(10, &submit_resolution_choice(vec![discarded]))
        .is_err());
    engine
        .apply_command(20, &submit_resolution_choice(vec![discarded]))
        .expect("affected player chooses which card to discard");

    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&retained].zone, Zone::Hand);
}

#[test]
fn liliana_vess_minus_two_shuffles_and_puts_the_private_search_result_on_top() {
    let mut engine = engine_with_players(513_002, &[10, 20]);
    let source = put_liliana_on_battlefield(&mut engine, 0);
    let chosen = inject_library_card(&mut engine, 0, "mind_stone");
    let other = inject_library_card(&mut engine, 0, "storm_crow");

    engine
        .apply_command(10, &activate_ability_for(&engine, source, 1, Vec::new()))
        .expect("activate Liliana's -2");
    let batch = pass_until_resolution_choice(&mut engine);
    let choice = find_resolution_choice(&batch).expect("private library search");
    assert_eq!(choice.deciding_player_id, 10);
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(choice.candidate_object_ids.contains(&chosen));
    assert!(engine
        .apply_command(20, &submit_resolution_choice(vec![chosen]))
        .is_err());

    let completion = engine
        .apply_command(10, &submit_resolution_choice(vec![chosen]))
        .expect("controller chooses a library card");
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert!(completion.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "P10 shuffles their library."
    )));
    assert!(completion.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log))
            if log.text.contains("Mind Stone") && log.visible_to_player_id == Some(10)
    )));
    assert!(completion.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log))
            if log.text == "P10 puts a card on top of their library."
                && log.hidden_from_player_id == Some(10)
    )));
    assert_eq!(engine.state.players[0].library.front(), Some(&chosen));
    assert!(engine.state.players[0].library.contains(&other));
}

#[test]
fn liliana_vess_minus_two_replays_the_search_choice_and_shuffle_from_logged_commands() {
    fn fixture() -> (GameEngine, u32, u32) {
        let mut engine = engine_with_players(513_008, &[10, 20]);
        let source = put_liliana_on_battlefield(&mut engine, 0);
        let chosen = inject_library_card(&mut engine, 0, "mind_stone");
        inject_library_card(&mut engine, 0, "storm_crow");
        (engine, source, chosen)
    }

    let (mut original, source, chosen) = fixture();
    let (mut replay, _, _) = fixture();

    let activation = activate_ability_for(&original, source, 1, Vec::new());
    apply_logged_to_pair(&mut original, &mut replay, 10, activation);
    let mut choice_batch = None;
    for _ in 0..original.state.players.len() * 2 {
        let actor = original.state.priority_player_id();
        let batch = apply_logged_to_pair(&mut original, &mut replay, actor, pass());
        if original.state.pending_resolution.is_some() {
            choice_batch = Some(batch);
            break;
        }
    }
    let choice_batch = choice_batch.expect("priority passes reach the library search choice");
    let choice = find_resolution_choice(&choice_batch).expect("private search choice");
    assert_eq!(choice.deciding_player_id, 10);
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);

    let completion = apply_logged_to_pair(
        &mut original,
        &mut replay,
        10,
        submit_resolution_choice(vec![chosen]),
    );
    assert!(completion.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "P10 shuffles their library."
    )));
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert_eq!(original.state.players[0].library.front(), Some(&chosen));
    assert_eq!(original.state.stack.len(), 0);
    assert!(original.state.pending_resolution.is_none());
}

#[test]
fn liliana_vess_minus_one_skips_a_target_with_an_empty_hand() {
    let mut engine = engine_with_players(513_003, &[10, 20, 30]);
    let source = put_liliana_on_battlefield(&mut engine, 0);
    let target_hand = std::mem::take(&mut engine.state.players[1].hand);
    for object_id in target_hand {
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
        engine.state.players[1].graveyard.push(object_id);
    }

    engine
        .apply_command(
            10,
            &activate_ability_for(&engine, source, 0, target_player(20)),
        )
        .expect("target a player with no cards");
    pass_priority_round(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.players[1].hand.is_empty());
}

#[test]
fn liliana_vess_minus_one_does_nothing_if_its_target_leaves_before_resolution() {
    let mut engine = engine_with_players(513_004, &[10, 20, 30]);
    let source = put_liliana_on_battlefield(&mut engine, 0);
    let target_card = inject_card_into_hand(&mut engine, 1, "grizzly_bears");

    engine
        .apply_command(
            10,
            &activate_ability_for(&engine, source, 0, target_player(20)),
        )
        .expect("target the second player");
    engine
        .apply_command(20, &concede())
        .expect("the targeted player concedes before resolution");
    pass_priority_round(&mut engine);

    assert!(engine.state.players[1].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(
        !engine.state.objects.contains_key(&target_card),
        "the conceded player's owned card leaves the game"
    );
}

#[test]
fn liliana_vess_minus_two_shuffles_an_empty_library_without_a_choice() {
    let mut engine = engine_with_players(513_005, &[10, 20]);
    let source = put_liliana_on_battlefield(&mut engine, 0);
    let library = std::mem::take(&mut engine.state.players[0].library);
    for object_id in library {
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
        engine.state.players[0].graveyard.push(object_id);
    }

    engine
        .apply_command(10, &activate_ability_for(&engine, source, 1, Vec::new()))
        .expect("activate Liliana's -2 with an empty library");
    let mut completion = None;
    for _ in 0..engine.state.players.len() * 2 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("pass priority toward the empty library search");
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            let deciding_player = pending.deciding_player;
            completion = Some(
                engine
                    .apply_command(deciding_player, &submit_resolution_choice(Vec::new()))
                    .expect("decline the empty library search choice"),
            );
            break;
        }
        completion = Some(batch);
        if engine.state.stack.is_empty() {
            break;
        }
    }
    let batch = completion.expect("empty library search completion");

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.players[0].library.is_empty());
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "P10 shuffles their library."
    )));
}

#[test]
fn liliana_vess_minus_eight_assigns_cross_owner_entry_choice_to_destination_controller() {
    let mut engine = engine_with_players(513_008, &[10, 20, 30]);
    let source = put_liliana_on_battlefield(&mut engine, 0);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 8);
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    engine.state.turn_history.current.creatures_died = 1;
    let paladin = inject_graveyard_card(&mut engine, 1, "bloodcrazed_paladin");

    engine
        .apply_command(10, &activate_ability_for(&engine, source, 2, Vec::new()))
        .expect("activate Liliana's -8");
    let choice_batch = loop {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("pass priority toward the entry replacement choice");
        if engine.state.pending_resolution.is_some() {
            break batch;
        }
    };

    let choice = find_resolution_choice(&choice_batch).expect("entry replacement-order choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::ReplacementEffect);
    assert_eq!(choice.deciding_player_id, 10);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the entry choice remains pending");
    assert_eq!(pending.deciding_player, 10);

    let application = pending.presentation.candidates[0];
    engine
        .apply_command(10, &submit_resolution_choice(vec![application]))
        .expect("Liliana's controller chooses the entry replacement order");
    assert_eq!(engine.state.objects[&paladin].owner, 20);
    assert_eq!(engine.state.objects[&paladin].controller, 10);
    assert_eq!(engine.state.objects[&paladin].zone, Zone::Battlefield);
}

#[test]
fn liliana_vess_minus_eight_returns_the_live_three_player_creature_cohort_together() {
    let mut engine = engine_with_players(513_006, &[10, 20, 30]);
    let source = put_liliana_on_battlefield(&mut engine, 0);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 8);
    let warden = inject_permanent_on_battlefield(&mut engine, 0, "soul_warden");
    let own_creature = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let artifact_creature = inject_graveyard_card(&mut engine, 1, "darksteel_myr");
    let enchantment_creature = inject_graveyard_card(&mut engine, 2, "nyxborn_courser");
    let noncreature_artifact = inject_graveyard_card(&mut engine, 1, "mind_stone");
    let land = inject_graveyard_card(&mut engine, 2, "forest");
    let spell = inject_graveyard_card(&mut engine, 2, "demonic_tutor");

    engine
        .apply_command(10, &activate_ability_for(&engine, source, 2, Vec::new()))
        .expect("activate Liliana's -8");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(
        !engine.state.stack.is_empty(),
        "the ability survives its source"
    );
    let live_creature = inject_graveyard_card(&mut engine, 2, "storm_crow");

    let returning = [
        own_creature,
        artifact_creature,
        enchantment_creature,
        live_creature,
    ];
    let batches = pass_until_returned(&mut engine, &returning);
    assert!(batches.iter().any(|batch| {
        let moved = batch
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(Ev::PermanentMoved(moved)) if returning.contains(&moved.object_id) => {
                    Some(moved.object_id)
                }
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        returning.iter().all(|object_id| moved.contains(object_id))
    }));

    for (object_id, owner) in [
        (own_creature, 10),
        (artifact_creature, 20),
        (enchantment_creature, 30),
        (live_creature, 30),
    ] {
        let object = &engine.state.objects[&object_id];
        assert_eq!(object.zone, Zone::Battlefield);
        assert_eq!(object.owner, owner);
        assert_eq!(object.controller, 10);
        assert!(engine.state.players[0].battlefield.contains(&object_id));
    }
    for object_id in [noncreature_artifact, land, spell] {
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Graveyard);
    }

    resolve_current_stack(&mut engine);
    assert_eq!(engine.state.objects[&warden].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].life, 24);
}

#[test]
fn liliana_vess_minus_eight_finishes_with_an_empty_or_noncreature_only_cohort() {
    let mut engine = engine_with_players(513_007, &[10, 20, 30]);
    let source = put_liliana_on_battlefield(&mut engine, 0);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 8);
    let artifact = inject_graveyard_card(&mut engine, 1, "mind_stone");
    let land = inject_graveyard_card(&mut engine, 2, "forest");
    let spell = inject_graveyard_card(&mut engine, 2, "demonic_tutor");

    engine
        .apply_command(10, &activate_ability_for(&engine, source, 2, Vec::new()))
        .expect("activate Liliana's -8");
    pass_priority_round(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}
