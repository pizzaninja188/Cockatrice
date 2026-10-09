use crate::helpers::*;
use prost::Message;
use tricerules_cards::CounterKind;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChooseTriggerTarget, RuledCommand, TargetRef,
    TargetRefKind,
};

fn wizard_engine(seed: u64) -> GameEngine {
    GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["wizard_class"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("Wizard Class decks are valid")
}

fn trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                expected_zone_change_generation: None,
                object_id,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
            ..Default::default()
        })),
    }
}

fn trigger_stack_target(stack_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                object_id: stack_id,
                group_index: 0,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    }
}

fn level_up(engine: &mut GameEngine, class: u32, generic: u32) {
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            c: generic,
            ..Default::default()
        },
    );
    let ability_index = active_class_ability_index(engine, class);
    apply_ability(engine, 0, class, ability_index, vec![])
        .expect("activate the currently available level");
    resolve_entire_stack_two_player(engine);
}

fn active_class_ability_index(engine: &mut GameEngine, class: u32) -> u32 {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .and_then(|view| view.per_player.first())
        .into_iter()
        .flat_map(|player| player.battlefield_objects.iter())
        .find(|object| object.object_id == class)
        .and_then(|object| object.activated_abilities.first())
        .map(|ability| ability.ability_index)
        .expect("Class exposes its current level ability through the public zone view")
}

#[test]
fn wizard_class_level_two_draws_two_and_ignores_the_hand_size_limit() {
    let mut engine = wizard_engine(716_001);
    let class = inject_permanent_on_battlefield(&mut engine, 0, "wizard_class");
    assert_eq!(engine.state.class_level(class), 1);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, class),
        ["Level 1"]
    );

    // Level abilities have sorcery timing and their mana costs are paid only when legal.
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(zone_view_ability_flags(&mut engine, 0, class), [false]);
    let level_two_index = active_class_ability_index(&mut engine, class);
    assert!(apply_ability(&mut engine, 0, class, level_two_index, vec![]).is_err());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    assert!(engine.state.stack.is_empty());

    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(zone_view_ability_flags(&mut engine, 0, class), [true]);
    let hand_before_level_two = engine.state.players[0].hand.len();
    apply_ability(&mut engine, 0, class, level_two_index, vec![])
        .expect("level up to 2 in main phase");
    assert_eq!(engine.state.class_level(class), 1);
    assert_eq!(engine.state.players[0].hand.len(), hand_before_level_two);
    assert_eq!(engine.state.stack.len(), 1, "leveling uses the stack");
    let mana_after_first_activation = engine.state.players[0].mana_pool;
    assert!(apply_ability(&mut engine, 0, class, level_two_index, vec![]).is_err());
    assert_eq!(engine.state.class_level(class), 1);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.players[0].mana_pool,
        mana_after_first_activation
    );
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.class_level(class), 2);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, class),
        ["Level 2"]
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_level_two + 2,
        "the newly active level-2 trigger draws two cards"
    );
    assert_eq!(zone_view_ability_flags(&mut engine, 0, class), [true]);
    let level_three_index = active_class_ability_index(&mut engine, class);
    assert_ne!(level_three_index, level_two_index);
    assert!(apply_ability(&mut engine, 0, class, level_two_index, vec![]).is_err());

    let hand_before_cleanup = engine.state.players[0].hand.len();
    assert!(hand_before_cleanup > 7);
    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_cleanup,
        "Wizard Class's top ability removes the maximum hand size"
    );
}

#[test]
fn wizard_class_level_three_counters_only_its_controllers_draws() {
    let mut engine = wizard_engine(716_002);
    advance_to_main1_from_game_start(&mut engine);
    let class = inject_permanent_on_battlefield(&mut engine, 0, "wizard_class");
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    level_up(&mut engine, class, 2);
    assert_eq!(engine.state.class_level(class), 2);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 4,
            ..Default::default()
        },
    );
    assert_eq!(zone_view_ability_flags(&mut engine, 0, class), [true]);
    let level_three_index = active_class_ability_index(&mut engine, class);
    apply_ability(&mut engine, 0, class, level_three_index, vec![]).expect("activate level 3");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.class_level(class), 3);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, class),
        ["Level 3"]
    );
    assert!(zone_view_ability_flags(&mut engine, 0, class).is_empty());

    // The opponent's normal draw does not satisfy "Whenever you draw a card".
    end_active_turn(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
    assert_eq!(
        engine.state.objects[&creature]
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        None
    );

    // On its controller's next draw, the trigger requires that controller's creature as target.
    end_active_turn(&mut engine, 1);
    pass_priority_round(&mut engine); // P0 upkeep -> draw step
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(engine
        .apply_command(0, &trigger_target(opponent_creature))
        .is_err());
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "an illegal opponent target leaves the trigger choice pending"
    );
    engine
        .apply_command(0, &trigger_target(creature))
        .expect("choose a creature you control");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.class_level(class), 3);
    assert_eq!(
        engine.state.objects[&creature]
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&1)
    );

    // A single spell instruction to draw two cards creates two draw events, and each event
    // independently triggers the active level-three ability.
    pass_priority_round(&mut engine); // P0 draw step -> main 1
    inject_card_into_hand(&mut engine, 0, "divination");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let divination_index = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(divination_index, vec![]))
        .expect("cast a spell that draws two cards");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    for _ in 0..2 {
        assert_eq!(engine.state.pending_triggers.len(), 1);
        engine
            .apply_command(0, &trigger_target(creature))
            .expect("each card draw creates a separate targeted trigger");
    }
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&creature]
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&3)
    );
}

#[test]
fn wizard_class_level_up_ability_can_be_countered() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        716_003,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["wizard_class", "tishanas_tidebinder"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("Wizard Class and its ability counter are valid");
    advance_to_main1_from_game_start(&mut engine);
    let class = inject_permanent_on_battlefield(&mut engine, 0, "wizard_class");
    ensure_in_hand(&mut engine, 0, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );

    let level_two_index = active_class_ability_index(&mut engine, class);
    apply_ability(&mut engine, 0, class, level_two_index, vec![])
        .expect("activate Wizard Class's level-2 ability");
    let level_up_ability_id = engine
        .state
        .stack
        .last()
        .expect("level ability on stack")
        .id;
    assert_eq!(engine.state.class_level(class), 1);

    let tidebinder_index = hand_index_for_card(&engine, 0, "tishanas_tidebinder");
    engine
        .apply_command(0, &cast_spell(tidebinder_index, vec![]))
        .expect("cast Tishana's Tidebinder with flash in response");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &trigger_stack_target(level_up_ability_id))
        .expect("target the level-up ability with Tishana's Tidebinder");
    pass_both_players(&mut engine);

    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == level_up_ability_id));
    assert_eq!(engine.state.class_level(class), 1);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, class),
        ["Level 1"]
    );
}

#[test]
fn wizard_class_level_assignment_replays_serialized_accepted_commands() {
    fn fixture(seed: u64) -> (GameEngine, u32) {
        let mut engine = wizard_engine(seed);
        advance_to_main1_from_game_start(&mut engine);
        let class = inject_permanent_on_battlefield(&mut engine, 0, "wizard_class");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
        );
        (engine, class)
    }

    let (mut engine, class) = fixture(716_004);
    let (mut replay, replay_class) = fixture(716_004);
    assert_eq!(class, replay_class);
    let ability_index = active_class_ability_index(&mut engine, class);
    assert_eq!(
        ability_index,
        active_class_ability_index(&mut replay, replay_class)
    );

    let mut accepted = Vec::new();
    let command = activate_ability_for(&engine, class, ability_index, vec![]);
    let batch = engine.apply_command(0, &command).unwrap();
    accepted.push((0, command, batch));
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        let command = pass();
        let batch = engine.apply_command(actor, &command).unwrap();
        accepted.push((actor, command, batch));
    }

    assert_eq!(engine.state.class_level(class), 2);
    for (actor, command, expected_batch) in accepted {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(
            replay.apply_command(actor, &decoded).unwrap(),
            expected_batch
        );
    }
    assert_eq!(replay.state.class_level(replay_class), 2);
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap()
    );
}
