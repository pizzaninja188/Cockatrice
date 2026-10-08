//! Guardian Project's creature-name uniqueness condition.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

fn engine_with_cards(seed: u64, player_zero: &[&str], player_one: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", player_zero),
        deck_with("island", player_one),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn move_card_to_zone(
    engine: &mut GameEngine,
    player: i32,
    card_name: &str,
    zone: DevZone,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    engine.enable_dev_commands();
    engine
        .apply_command(
            player,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap_or_else(|error| panic!("move {card_name} to {zone:?}: {error:?}"))
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids: Vec<_> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for &object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(object_id);
    }
    object_ids
}

fn manifest_grizzly_bears(engine: &mut GameEngine) -> (u32, usize) {
    let top = seat_on_top(engine, 0, &["grizzly_bears", "grizzly_bears"]);
    ensure_in_hand(engine, 0, "manifest_dread");
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, "manifest_dread");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Manifest Dread");
    engine.apply_command(0, &pass()).expect("caster passes");
    let parked = engine
        .apply_command(1, &pass())
        .expect("Manifest Dread resolves");
    let choice = find_resolution_choice(&parked).expect("Manifest Dread choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::ManifestDread);
    engine
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("manifest a Grizzly Bears card face down");
    let hand_before_trigger = engine.state.players[0].hand.len();
    resolve_entire_stack_two_player(engine);
    (top[0], hand_before_trigger)
}

#[test]
fn guardian_project_does_not_trigger_for_a_name_already_on_the_battlefield() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &["guardian_project", "llanowar_elves", "llanowar_elves"],
        ),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(9401, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    relocate_to_battlefield(&mut e, 0, "llanowar_elves", false);
    ensure_in_hand(&mut e, 0, "guardian_project");
    grant_pool(&mut e, 0);
    let guardian_index = hand_index_for_card(&e, 0, "guardian_project");
    e.apply_command(0, &cast_spell(guardian_index, vec![]))
        .expect("cast Guardian Project");
    pass_both_players(&mut e);
    assert!(e.state.stack.is_empty(), "Guardian Project resolved");

    ensure_in_hand(&mut e, 0, "llanowar_elves");
    let creature_index = hand_index_for_card(&e, 0, "llanowar_elves");
    e.apply_command(0, &cast_spell(creature_index, vec![]))
        .expect("cast the duplicate creature");
    pass_both_players(&mut e);

    assert!(
        e.state.stack.is_empty(),
        "a matching name at entry prevents Guardian Project's trigger"
    );
}

#[test]
fn guardian_project_does_not_trigger_for_a_name_in_its_controllers_graveyard() {
    let mut e = engine_with_cards(9402, &["guardian_project", "llanowar_elves"], &[]);
    inject_graveyard_card(&mut e, 0, "llanowar_elves");
    move_ready_to_battlefield(&mut e, 0, "guardian_project");
    ensure_in_hand(&mut e, 0, "llanowar_elves");
    grant_pool(&mut e, 0);
    let hand_before_cast = e.state.players[0].hand.len();
    let creature_index = hand_index_for_card(&e, 0, "llanowar_elves");
    e.apply_command(0, &cast_spell(creature_index, vec![]))
        .expect("cast the creature matching a graveyard card");
    pass_both_players(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast - 1,
        "the creature was cast without a Guardian Project draw"
    );
}

#[test]
fn guardian_project_draws_for_a_unique_name_even_if_an_opponent_controls_that_name() {
    let mut e = engine_with_cards(
        9403,
        &["guardian_project", "grizzly_bears"],
        &["grizzly_bears"],
    );
    move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut e, 0, "guardian_project");
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    let creature_index = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(creature_index, vec![]))
        .expect("cast the creature");
    pass_both_players(&mut e);

    assert_eq!(e.state.stack.len(), 1, "the entry trigger is on the stack");
    let hand_before_trigger = e.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_trigger + 1,
        "an opponent's same-named creature does not block the draw"
    );
}

#[test]
fn guardian_project_rechecks_controlled_creatures_when_its_trigger_resolves() {
    let mut e = engine_with_cards(9404, &["guardian_project", "grizzly_bears"], &[]);
    move_ready_to_battlefield(&mut e, 0, "guardian_project");
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    let creature_index = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(creature_index, vec![]))
        .expect("cast the first creature");
    pass_both_players(&mut e);
    assert_eq!(e.state.stack.len(), 1, "the unique entry created a trigger");
    let hand_before_trigger = e.state.players[0].hand.len();

    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_trigger,
        "a new controlled duplicate makes the intervening-if false"
    );
}

#[test]
fn guardian_project_uses_the_departed_name_when_the_creature_is_exiled() {
    let mut e = engine_with_cards(9408, &["guardian_project", "grizzly_bears"], &[]);
    move_ready_to_battlefield(&mut e, 0, "guardian_project");
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    let creature_index = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(creature_index, vec![]))
        .expect("cast the creature");
    pass_both_players(&mut e);
    assert_eq!(e.state.stack.len(), 1, "the unique entry created a trigger");
    let observed = e.state.stack[0]
        .trigger_context
        .observed_object
        .expect("entry trigger retains its observed generation");
    let hand_before_trigger = e.state.players[0].hand.len();

    move_card_to_zone(&mut e, 0, "Grizzly Bears", DevZone::Exile);
    assert_eq!(
        e.state.last_known_names_by_generation
            [&(observed.object_id, observed.zone_change_generation)],
        vec!["Grizzly Bears"]
    );
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_trigger + 1,
        "without a duplicate, the departed creature's last-known name still permits the draw"
    );
}

#[test]
fn guardian_project_counts_the_observed_creature_in_its_graveyard() {
    let mut e = engine_with_cards(9409, &["guardian_project", "grizzly_bears"], &[]);
    move_ready_to_battlefield(&mut e, 0, "guardian_project");
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    let creature_index = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(creature_index, vec![]))
        .expect("cast the creature");
    pass_both_players(&mut e);
    assert_eq!(e.state.stack.len(), 1, "the unique entry created a trigger");
    let observed = e.state.stack[0]
        .trigger_context
        .observed_object
        .expect("entry trigger retains its observed generation");
    let hand_before_trigger = e.state.players[0].hand.len();

    move_card_to_zone(&mut e, 0, "Grizzly Bears", DevZone::Graveyard);
    assert_eq!(e.state.objects[&observed.object_id].zone, Zone::Graveyard);
    assert_eq!(
        e.state.last_known_names_by_generation
            [&(observed.object_id, observed.zone_change_generation)],
        vec!["Grizzly Bears"]
    );
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_trigger,
        "the observed card itself is now the matching creature card in its owner's graveyard"
    );
}

#[test]
fn guardian_project_uses_the_departed_generation_name_after_a_creature_reenters() {
    let mut e = engine_with_cards(9405, &["guardian_project", "grizzly_bears"], &[]);
    move_ready_to_battlefield(&mut e, 0, "guardian_project");
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    let creature_index = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(creature_index, vec![]))
        .expect("cast the creature");
    pass_both_players(&mut e);
    assert_eq!(e.state.stack.len(), 1, "the first entry created a trigger");
    let observed = e.state.stack[0]
        .trigger_context
        .observed_object
        .expect("entry trigger retains its observed generation");
    let hand_before_triggers = e.state.players[0].hand.len();

    move_card_to_zone(&mut e, 0, "Grizzly Bears", DevZone::Exile);
    assert_eq!(e.state.objects[&observed.object_id].zone, Zone::Exile);
    assert_eq!(
        e.state.last_known_names_by_generation
            [&(observed.object_id, observed.zone_change_generation)],
        vec!["Grizzly Bears"]
    );
    let reentry_batch = move_card_to_zone(&mut e, 0, "Grizzly Bears", DevZone::Battlefield);
    let reentry_triggers = reentry_batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::StackPushed(pushed)) if pushed.is_triggered => Some(pushed),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reentry_triggers.len(), 1, "reentry creates its own trigger");
    assert_ne!(
        e.state.zone_change_generation[&observed.object_id], observed.zone_change_generation,
        "the reentered permanent is a new object"
    );
    pass_both_players(&mut e);
    assert_eq!(
        e.state.stack.len(),
        1,
        "the older trigger remains on the stack"
    );
    assert_eq!(
        e.state.stack[0]
            .trigger_context
            .observed_object
            .expect("older entry trigger")
            .zone_change_generation,
        observed.zone_change_generation,
        "the generation-bound trigger survives reentry"
    );

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_triggers + 1,
        "only the reentry's trigger draws; the old generation sees its same-named successor"
    );
}

#[test]
fn guardian_project_uses_a_copied_name_after_the_observed_card_leaves() {
    let mut e = engine_with_cards(9406, &["guardian_project", "clone"], &["grizzly_bears"]);
    let opponent_bear = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut e, 0, "guardian_project");
    ensure_in_hand(&mut e, 0, "clone");
    grant_pool(&mut e, 0);
    let clone_index = hand_index_for_card(&e, 0, "clone");
    e.apply_command(0, &cast_spell(clone_index, vec![]))
        .expect("cast Clone");
    pass_both_players(&mut e);
    e.apply_command(0, &submit_resolution_choice(vec![opponent_bear]))
        .expect("copy the opponent's Grizzly Bears");

    let clone = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == "clone")
        .expect("Clone permanent");
    let generation = e.state.zone_change_generation[&clone];
    assert_eq!(
        e.characteristics(clone).unwrap().names,
        vec!["Grizzly Bears"]
    );
    assert_eq!(
        e.state.stack.len(),
        1,
        "the copied name was unique to its controller"
    );
    let hand_before_trigger = e.state.players[0].hand.len();

    move_card_to_zone(&mut e, 0, "Clone", DevZone::Graveyard);
    assert_eq!(e.state.objects[&clone].zone, Zone::Graveyard);
    assert_eq!(
        e.state.last_known_names_by_generation[&(clone, generation)],
        vec!["Grizzly Bears"]
    );
    inject_graveyard_card(&mut e, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_trigger,
        "the copied name is compared with a creature card in the controller's graveyard"
    );
}

#[test]
fn guardian_project_treats_face_down_creature_names_as_empty() {
    let mut e = engine_with_cards(
        9407,
        &["guardian_project", "manifest_dread", "manifest_dread"],
        &[],
    );
    move_ready_to_battlefield(&mut e, 0, "guardian_project");

    for _ in 0..2 {
        let (face_down_creature, hand_before_trigger) = manifest_grizzly_bears(&mut e);
        assert_eq!(e.state.objects[&face_down_creature].zone, Zone::Battlefield);
        assert!(e.state.objects[&face_down_creature].face_down);
        let characteristics = e
            .characteristics(face_down_creature)
            .expect("face-down creature characteristics");
        assert!(characteristics.is_creature());
        assert!(characteristics.names.is_empty());
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before_trigger + 1,
            "Guardian Project draws once after each Manifest Dread cast"
        );
    }
}
