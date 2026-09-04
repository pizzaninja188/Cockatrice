use super::helpers::*;
use tricerules_cards::{primitives::CounterKind, CardRegistry, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    self as rv1, dev_command, ruled_command::Cmd, ruled_event::Ev, CastMethod, CastSpell,
    DevCommand, DevMoveCard, DevZone, RuledCommand,
};

const ESPER: &str = "esper_origins_summon:_esper_maduin";

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[ESPER]), forest_only_deck()]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn move_esper_to_graveyard(engine: &mut GameEngine) -> u32 {
    ensure_in_hand(engine, 0, ESPER);
    let oid = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == ESPER)
        .expect("Esper in hand");
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Esper Origins".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move Esper Origins to graveyard");
    oid
}

fn resolve_surveil_putting_everything_in_graveyard(
    engine: &mut GameEngine,
) -> rv1::RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine.apply_command(first, &pass()).expect("first pass");
    let choice_batch = engine
        .apply_command(second, &pass())
        .expect("resolve into surveil");
    let choice = find_resolution_choice(&choice_batch).expect("surveil choice");
    engine
        .apply_command(
            choice.deciding_player_id,
            &submit_resolution_choice(choice.candidate_object_ids.clone()),
        )
        .expect("put surveilled cards into graveyard")
}

fn flashback_esper(engine: &mut GameEngine, esper: u32) {
    give_mana(
        engine,
        0,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    cast_method: CastMethod::Flashback as i32,
                    source: Some(graveyard_cast_source(
                        esper,
                        engine.state.zone_change_generation[&esper],
                    )),
                    ..Default::default()
                })),
            },
        )
        .expect("cast Esper Origins with flashback");
}

#[test]
fn issue_213_graveyard_cast_returns_transformed_with_finality_and_lore() {
    let mut engine = engine(213_001);
    let esper = move_esper_to_graveyard(&mut engine);
    let graveyard_generation = engine.state.zone_change_generation[&esper];
    flashback_esper(&mut engine, esper);
    let completion = resolve_surveil_putting_everything_in_graveyard(&mut engine);

    let object = &engine.state.objects[&esper];
    assert_eq!(object.zone, Zone::Battlefield);
    assert_eq!(object.controller, 0);
    assert_eq!(object.face_up_index, 1);
    assert_eq!(object.counter_count(CounterKind::Finality), 1);
    assert_eq!(object.counter_count(CounterKind::Lore), 1);
    assert_eq!(
        engine.state.zone_change_generation[&esper],
        graveyard_generation + 3
    );

    let moves = completion
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::PermanentMoved(moved)) if moved.object_id == esper => Some(moved.destination),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        moves,
        vec![
            rv1::permanent_moved::Destination::Exile as i32,
            rv1::permanent_moved::Destination::Battlefield as i32,
        ]
    );
    assert!(completion.events.iter().any(|event| matches!(
        event.ev,
        Some(Ev::StackResolved(ref resolved))
            if resolved.object_id == esper
                && resolved.destination == rv1::StackResolveDestination::Exile as i32
    )));
}

#[test]
fn issue_213_normal_cast_resolves_to_graveyard_without_returning() {
    let mut engine = engine(213_002);
    ensure_in_hand(&mut engine, 0, ESPER);
    let esper = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == ESPER)
        .expect("Esper in hand");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, ESPER);
    engine
        .apply_command(0, &cast_spell(hand_index, vec![]))
        .expect("cast Esper Origins from hand");
    resolve_surveil_putting_everything_in_graveyard(&mut engine);

    assert_eq!(engine.state.objects[&esper].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&esper].face_up_index, 0);
}

#[test]
fn issue_213_chapter_one_publicly_reveals_and_moves_a_permanent_to_hand() {
    let mut engine = engine(213_003);
    let esper = move_esper_to_graveyard(&mut engine);
    flashback_esper(&mut engine, esper);
    resolve_surveil_putting_everything_in_graveyard(&mut engine);

    let revealed = inject_library_card(&mut engine, 0, "forest");
    engine.state.players[0]
        .library
        .retain(|oid| *oid != revealed);
    engine.state.players[0].library.push_front(revealed);
    answer_trigger_order_in_engine_order(&mut engine);
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine.apply_command(first, &pass()).expect("first pass");
    let batch = engine
        .apply_command(second, &pass())
        .expect("resolve chapter I");

    assert_eq!(engine.state.objects[&revealed].zone, Zone::Hand);
    let reveal = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .expect("public reveal event");
    assert_eq!(reveal.zone_owner_player_id, 0);
    assert_eq!(reveal.cards.len(), 1);
    assert_eq!(reveal.cards[0].object_id, revealed);
    assert_eq!(reveal.cards[0].card_id, "forest");
}

#[test]
fn issue_213_chapter_one_leaves_a_revealed_nonpermanent_on_top() {
    let mut engine = engine(213_005);
    let esper = move_esper_to_graveyard(&mut engine);
    flashback_esper(&mut engine, esper);
    resolve_surveil_putting_everything_in_graveyard(&mut engine);

    let revealed = inject_library_card(&mut engine, 0, "lightning_bolt");
    engine.state.players[0]
        .library
        .retain(|oid| *oid != revealed);
    engine.state.players[0].library.push_front(revealed);
    answer_trigger_order_in_engine_order(&mut engine);
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine.apply_command(first, &pass()).expect("first pass");
    let batch = engine
        .apply_command(second, &pass())
        .expect("resolve chapter I");

    assert_eq!(engine.state.objects[&revealed].zone, Zone::Library);
    assert_eq!(engine.state.players[0].library.front(), Some(&revealed));
    assert!(batch.events.iter().any(|event| matches!(
        event.ev,
        Some(Ev::CardsRevealed(ref reveal))
            if reveal.cards.len() == 1 && reveal.cards[0].object_id == revealed
    )));
    assert!(!batch.events.iter().any(|event| matches!(
        event.ev,
        Some(Ev::PermanentMoved(ref moved)) if moved.object_id == revealed
    )));
}

#[test]
fn issue_213_finality_replaces_battlefield_to_graveyard_with_exile() {
    let mut engine = engine(213_004);
    let esper = move_esper_to_graveyard(&mut engine);
    flashback_esper(&mut engine, esper);
    resolve_surveil_putting_everything_in_graveyard(&mut engine);
    engine
        .state
        .objects
        .get_mut(&esper)
        .expect("Esper permanent")
        .add_counters(CounterKind::Finality, 1, engine.state.command_index);

    engine.enable_dev_commands();
    let moved = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Esper Origins".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move finality permanent toward graveyard");

    assert_eq!(engine.state.objects[&esper].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&esper].counter_count(CounterKind::Finality),
        0
    );
    assert!(moved.events.iter().any(|event| matches!(
        event.ev,
        Some(Ev::PermanentMoved(ref permanent))
            if permanent.object_id == esper
                && permanent.destination == rv1::permanent_moved::Destination::Exile as i32
    )));
}

#[test]
fn issue_213_finality_applies_to_noncreatures_but_not_to_bounce() {
    let mut graveyard_move = engine(213_006);
    inject_library_card(&mut graveyard_move, 0, "howling_mine");
    let artifact = move_ready_to_battlefield(&mut graveyard_move, 0, "howling_mine");
    graveyard_move
        .state
        .objects
        .get_mut(&artifact)
        .expect("artifact permanent")
        .add_counters(CounterKind::Finality, 1, graveyard_move.state.command_index);
    graveyard_move.enable_dev_commands();
    graveyard_move
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Howling Mine".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move finality land toward graveyard");
    assert_eq!(graveyard_move.state.objects[&artifact].zone, Zone::Exile);

    let mut bounce = engine(213_007);
    inject_library_card(&mut bounce, 0, "howling_mine");
    let artifact = move_ready_to_battlefield(&mut bounce, 0, "howling_mine");
    bounce
        .state
        .objects
        .get_mut(&artifact)
        .expect("artifact permanent")
        .add_counters(CounterKind::Finality, 1, bounce.state.command_index);
    bounce.enable_dev_commands();
    bounce
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Howling Mine".into(),
                        zone: DevZone::Hand as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("bounce finality land");
    assert_eq!(bounce.state.objects[&artifact].zone, Zone::Hand);
}

#[test]
fn issue_213_chapter_two_uses_the_stack_and_adds_green_mana() {
    let decks = Some(vec![deck_with("forest", &[ESPER]), forest_only_deck()]);
    let mut engine = GameEngine::new(213_008, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let saga = relocate_to_battlefield(&mut engine, 0, ESPER, false);
    {
        let object = engine.state.objects.get_mut(&saga).expect("Maduin");
        object.face_up_index = 1;
        object.set_counter(CounterKind::Lore, 1);
    }
    engine.state.turn_step = TurnStep::Draw;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&saga].counter_count(CounterKind::Lore),
        2
    );
    assert_eq!(engine.state.stack.len(), 1, "chapter II must use the stack");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].mana_pool.green, 2);
    assert_eq!(engine.state.objects[&saga].zone, Zone::Battlefield);
}

#[test]
fn issue_213_chapter_three_buffs_only_other_controlled_creatures_then_finality_exiles_saga() {
    let decks = Some(vec![
        deck_with("forest", &[ESPER, "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(213_009, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let saga = relocate_to_battlefield(&mut engine, 0, ESPER, false);
    let ours = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let theirs = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    {
        let object = engine.state.objects.get_mut(&saga).expect("Maduin");
        object.face_up_index = 1;
        object.set_counter(CounterKind::Lore, 2);
        object.add_counters(CounterKind::Finality, 1, 0);
    }
    engine.state.turn_step = TurnStep::Draw;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "chapter III must use the stack"
    );
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.effective_power(ours), Some(4));
    assert_eq!(engine.effective_toughness(ours), Some(4));
    assert!(engine.effective_has_keyword(ours, Keyword::Trample));
    assert_eq!(engine.effective_power(theirs), Some(2));
    assert!(!engine.effective_has_keyword(theirs, Keyword::Trample));
    assert_eq!(engine.state.objects[&saga].zone, Zone::Exile);
}

#[test]
fn issue_213_esper_origins_is_fully_registered() {
    let card = CardRegistry::global()
        .get(ESPER)
        .expect("Esper Origins // Summon: Esper Maduin must be supported");
    assert_eq!(card.faces.len(), 2);
}
