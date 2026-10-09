use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

fn setup() -> GameEngine {
    assert!(
        tricerules_cards::registry::global()
            .get("blue_suns_zenith")
            .is_some(),
        "complete Blue Sun definition"
    );
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        504_001,
        &[0, 1, 2],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_one(engine: &mut GameEngine, oid: u32) -> Vec<rv1::RuledEvent> {
    let mut events = Vec::new();
    for _ in 0..12 {
        if !engine.state.stack.iter().any(|item| item.id == oid) {
            return events;
        }
        let actor = engine.state.priority_player_id();
        events.extend(engine.apply_command(actor, &pass()).unwrap().events);
    }
    panic!("stack object did not resolve");
}

fn cast_blue(engine: &mut GameEngine, caster: usize, owner: usize, target: i32, x: u32) -> u32 {
    let oid = inject_card_into_hand(engine, owner, "blue_suns_zenith");
    if owner != caster {
        engine.state.players[owner].hand.retain(|id| *id != oid);
        engine.state.players[caster].hand.push(oid);
        engine.state.objects.get_mut(&oid).unwrap().controller = engine.state.players[caster].id;
    }
    let actor = engine.state.players[caster].id;
    give_mana(
        engine,
        actor,
        ManaGift {
            u: 10,
            c: 10,
            ..Default::default()
        },
    );
    for _ in 0..engine.state.players.len() {
        if engine.state.priority_player_id() == actor {
            break;
        }
        let priority = engine.state.priority_player_id();
        engine.apply_command(priority, &pass()).unwrap();
    }
    assert_eq!(engine.state.priority_player_id(), actor);
    let slot = hand_index_for_card(engine, caster, "blue_suns_zenith");
    engine
        .apply_command(actor, &cast_spell_x(slot, target_player(target), x))
        .unwrap();
    assert_eq!(engine.state.objects[&oid].zone, Zone::Stack);
    oid
}

fn logs(events: &[rv1::RuledEvent], text: &str) -> usize {
    events
        .iter()
        .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == text))
        .count()
}

fn response(engine: &mut GameEngine, seat: usize, card: &str, target: u32) -> u32 {
    let actor = engine.state.players[seat].id;
    inject_card_into_hand(engine, seat, card);
    give_mana(
        engine,
        actor,
        ManaGift {
            u: 10,
            ..Default::default()
        },
    );
    for _ in 0..engine.state.players.len() {
        if engine.state.priority_player_id() == actor {
            break;
        }
        let priority = engine.state.priority_player_id();
        engine.apply_command(priority, &pass()).unwrap();
    }
    let slot = hand_index_for_card(engine, seat, card);
    engine
        .apply_command(actor, &cast_spell(slot, target_object(target)))
        .unwrap();
    engine.state.stack.last().unwrap().id
}

#[test]
fn blue_actual_twincast_copies_x_and_shuffles_only_its_copy_owner() {
    let mut engine = setup();
    let source = cast_blue(&mut engine, 0, 0, 2, 2);
    let original_library = engine.state.players[0].library.clone();
    let copy_library = engine.state.players[1].library.clone();
    let target_hand = engine.state.players[2].hand.len();
    let twincast = response(&mut engine, 1, "twincast", source);
    resolve_one(&mut engine, twincast);
    assert!(engine.state.pending_resolution.is_some());
    let before = serde_json::to_vec(&engine.state).unwrap();
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![2]))
        .is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    engine
        .apply_command(1, &submit_resolution_choice(vec![2]))
        .unwrap();
    let copy = engine.state.stack.last().unwrap().clone();
    assert!(copy.is_copy && copy.id != source);
    assert_eq!(copy.chosen_x, 2);
    assert!(!engine.state.objects.contains_key(&copy.id));
    let copy_events = resolve_one(&mut engine, copy.id);
    assert_eq!(logs(&copy_events, "P1 shuffles their library."), 1);
    assert_eq!(logs(&copy_events, "P0 shuffles their library."), 0);
    assert_eq!(engine.state.objects[&source].zone, Zone::Stack);
    assert_eq!(engine.state.players[0].library, original_library);
    assert_eq!(engine.state.players[1].library.len(), copy_library.len());
    let exits = copy_events
        .iter()
        .filter(|event| {
            matches!(&event.ev,
        Some(Ev::StackResolved(exit)) if exit.object_id == copy.id)
        })
        .count();
    assert_eq!(exits, 1);
    assert_eq!(engine.state.players[2].hand.len(), target_hand + 2);
    let original_events = resolve_one(&mut engine, source);
    assert_eq!(logs(&original_events, "P0 shuffles their library."), 1);
    assert_eq!(engine.state.players[2].hand.len(), target_hand + 4);
    assert_eq!(engine.state.objects[&source].zone, Zone::Library);
}

#[test]
fn blue_cast_method_exit_replacement_still_shuffles_physical_owner() {
    use tricerules_core::state::SpellCastMethod;
    // Primitive exit-method fixture on an actual normally cast Blue Sun. This does not claim
    // an unsupported actual Blue Sun Flashback or Harmonize cast procedure.
    for method in [SpellCastMethod::Flashback, SpellCastMethod::Harmonize] {
        let mut engine = setup();
        let source = cast_blue(&mut engine, 1, 0, 2, 1);
        engine.state.stack.last_mut().unwrap().cast_method = method;
        let library = engine.state.players[0].library.len();
        let events = resolve_one(&mut engine, source);
        assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
        assert!(engine.state.players[0].exile.contains(&source));
        assert!(!engine.state.players[1].exile.contains(&source));
        assert_eq!(engine.state.players[0].library.len(), library);
        assert_eq!(logs(&events, "P0 shuffles their library."), 1);
        let exits: Vec<_> = events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(Ev::StackResolved(exit)) if exit.object_id == source => Some(exit),
                _ => None,
            })
            .collect();
        assert_eq!(exits.len(), 1);
        assert_eq!(
            exits[0].destination,
            rv1::StackResolveDestination::Exile as i32
        );
        assert_eq!(exits[0].owner_player_id, Some(0));
    }
}

#[test]
fn blue_failed_target_and_countered_spell_skip_draw_and_shuffle() {
    for countered in [false, true] {
        let mut engine = setup();
        let source = cast_blue(&mut engine, 1, 0, 2, 2);
        let library = engine.state.players[0].library.clone();
        let hand = engine.state.players[2].hand.clone();
        let events = if countered {
            let counter = response(&mut engine, 0, "counterspell", source);
            resolve_one(&mut engine, counter)
        } else {
            engine.apply_command(2, &concede()).unwrap();
            resolve_one(&mut engine, source)
        };
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert!(engine.state.players[0].graveyard.contains(&source));
        assert!(!engine.state.players[1].graveyard.contains(&source));
        assert_eq!(engine.state.players[0].library, library);
        assert_eq!(logs(&events, "P0 shuffles their library."), 0);
        if countered {
            assert_eq!(engine.state.players[2].hand, hand);
            assert!(events.iter().any(|event| matches!(&event.ev,
                Some(Ev::StackObjectCountered(exit)) if exit.object_id == source)));
        } else {
            assert!(events.iter().any(|event| matches!(&event.ev,
                Some(Ev::StackResolved(exit)) if exit.object_id == source
                    && exit.destination == rv1::StackResolveDestination::Graveyard as i32
                    && exit.owner_player_id == Some(0))));
        }
    }
}

#[test]
fn blue_failed_draw_is_not_undone_by_returning_source_to_library() {
    let mut engine = setup();
    let source = cast_blue(&mut engine, 0, 0, 0, 1);
    let empty: Vec<_> = engine.state.players[0].library.drain(..).collect();
    for oid in empty {
        engine.state.objects.remove(&oid);
    }
    let events = resolve_one(&mut engine, source);
    assert_eq!(logs(&events, "P0 shuffles their library."), 1);
    assert!(engine.state.players[0].has_lost);
    assert!(events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackResolved(exit)) if exit.object_id == source
            && exit.destination == rv1::StackResolveDestination::Library as i32)));
}

#[test]
fn blue_illegal_commands_are_atomic_and_accepted_draw_shuffle_replays() {
    fn prepared() -> GameEngine {
        let mut engine = setup();
        inject_card_into_hand(&mut engine, 0, "blue_suns_zenith");
        engine
    }
    let mut engine = prepared();
    let slot = hand_index_for_card(&engine, 0, "blue_suns_zenith");
    let cast = cast_spell_x(slot, target_player(2), 2);
    let before = serde_json::to_vec(&engine.state).unwrap();
    assert!(engine.apply_command(0, &cast).is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            c: 2,
            ..Default::default()
        },
    );
    let before = serde_json::to_vec(&engine.state).unwrap();
    for (actor, command) in [
        (1, cast.clone()),
        (0, cast_spell_x(slot, target_player(99), 2)),
        (0, cast_spell_x(slot, vec![], 0)),
    ] {
        assert!(engine.apply_command(actor, &command).is_err());
        assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    }
    let mut commands = vec![(0, cast)];
    let mut batches = vec![engine.apply_command(0, &commands[0].1).unwrap()];
    for _ in 0..12 {
        if engine.state.stack.is_empty() {
            break;
        }
        let actor = engine.state.priority_player_id();
        let command = pass();
        batches.push(engine.apply_command(actor, &command).unwrap());
        commands.push((actor, command));
    }
    assert!(engine.state.stack.is_empty());
    let mut replay = prepared();
    give_mana(
        &mut replay,
        0,
        ManaGift {
            u: 3,
            c: 2,
            ..Default::default()
        },
    );
    for ((actor, command), batch) in commands.into_iter().zip(batches) {
        assert_eq!(replay.apply_command(actor, &command).unwrap(), batch);
    }
    assert_eq!(
        serde_json::to_value(&replay.state).unwrap(),
        serde_json::to_value(&engine.state).unwrap()
    );
}

#[test]
fn blue_real_x_draws_then_shuffles_exact_source_to_foreign_owner() {
    for x in [0, 2] {
        let mut engine = setup();
        let known: Vec<_> = engine.state.players[2]
            .library
            .iter()
            .take(x as usize)
            .copied()
            .collect();
        let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
        let library_before = engine.state.players[0].library.len();
        let source = cast_blue(&mut engine, 1, 0, 2, x);
        let cast_generation = engine.state.zone_change_generation[&source];
        let events = resolve_one(&mut engine, source);
        assert_eq!(engine.state.players[2].hand.len(), hands[2] + x as usize);
        assert_eq!(engine.state.players[0].hand.len(), hands[0]);
        assert_eq!(engine.state.players[1].hand.len(), hands[1]);
        assert_eq!(&engine.state.players[2].hand[hands[2]..], known);
        assert!(!engine.state.players[2].hand.contains(&source));
        assert_eq!(engine.state.objects[&source].zone, Zone::Library);
        assert_eq!(engine.state.players[0].library.len(), library_before + 1);
        assert_eq!(
            engine.state.players[0]
                .library
                .iter()
                .filter(|id| **id == source)
                .count(),
            1
        );
        assert!(!engine.state.players[1].library.contains(&source));
        assert_eq!(
            engine.state.zone_change_generation[&source],
            cast_generation + 1
        );
        let exits: Vec<_> = events
            .iter()
            .filter_map(|e| match &e.ev {
                Some(Ev::StackResolved(exit)) if exit.object_id == source => Some(exit),
                _ => None,
            })
            .collect();
        assert_eq!(exits.len(), 1);
        assert_eq!(exits[0].owner_player_id, Some(0));
        assert_eq!(
            exits[0].destination,
            rv1::StackResolveDestination::Library as i32
        );
    }
}
