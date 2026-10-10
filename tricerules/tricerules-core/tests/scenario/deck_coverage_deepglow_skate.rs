//! Deepglow Skate: resolution-time counter bags, additional counter placement, optional targets.
use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_cards::CounterKind;
use tricerules_core::state::CopiableValues;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{dev_command::Dev, DevCommand, DevMoveCard, DevZone};
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, TargetRef, TargetRefKind};

fn setup() -> GameEngine {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_201,
        &[0, 1],
        20,
        Some(vec![deck; 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast(engine: &mut GameEngine) -> u32 {
    let skate = inject_card_into_hand(engine, 0, "deepglow_skate");
    give_mana(
        engine,
        0,
        ManaGift {
            c: 4,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "deepglow_skate");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&skate].zone, Zone::Battlefield);
    assert_eq!(engine.characteristics(skate).unwrap().power, Some(3));
    assert_eq!(engine.characteristics(skate).unwrap().toughness, Some(3));
    skate
}

fn choose(ids: &[u32]) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: ids
                .iter()
                .map(|&object_id| TargetRef {
                    kind: TargetRefKind::Permanent as i32,
                    object_id,
                    group_index: 0,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })),
    }
}

#[test]
fn deepglow_skate_paid_cast_doubles_each_current_kind_on_own_and_opposing_targets() {
    let mut engine = setup();
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let empty = inject_creature_on_battlefield(&mut engine, 1, "forest");
    engine
        .state
        .objects
        .get_mut(&own)
        .unwrap()
        .counters
        .insert(CounterKind::PlusOnePlusOne, 2);
    engine
        .state
        .objects
        .get_mut(&own)
        .unwrap()
        .counters
        .insert(CounterKind::Stun, 3);
    engine
        .state
        .objects
        .get_mut(&own)
        .unwrap()
        .counters
        .insert(CounterKind::Time, 6);
    engine
        .state
        .objects
        .get_mut(&other)
        .unwrap()
        .counters
        .insert(CounterKind::Charge, 4);
    let skate = cast(&mut engine);
    engine
        .apply_command(0, &choose(&[own, other, empty]))
        .unwrap();
    // Information is read on resolution rather than on entry or target selection.
    engine
        .state
        .objects
        .get_mut(&own)
        .unwrap()
        .counters
        .insert(CounterKind::Stun, 5);
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&own].counters[&CounterKind::PlusOnePlusOne],
        4
    );
    assert_eq!(engine.state.objects[&own].counters[&CounterKind::Stun], 10);
    assert_eq!(engine.state.objects[&own].counters[&CounterKind::Time], 12);
    assert_eq!(
        engine.state.objects[&other].counters[&CounterKind::Charge],
        8
    );
    assert!(engine.state.objects[&empty].counters.is_empty());
    assert!(engine.state.objects[&skate].counters.is_empty());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
}

#[test]
fn deepglow_skate_can_choose_zero_targets_and_finishes_without_placing_counters() {
    let mut engine = setup();
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .counters
        .insert(CounterKind::Quest, 2);
    cast(&mut engine);
    engine.apply_command(0, &choose(&[])).unwrap();
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&target].counters[&CounterKind::Quest],
        2
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
}

#[test]
fn deepglow_skate_rejects_wrong_actor_duplicate_foreign_and_player_targets_atomically() {
    let mut engine = setup();
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast(&mut engine);
    let mut player = choose(&[target]);
    let Some(Cmd::ChooseTriggerTarget(choice)) = &mut player.cmd else {
        unreachable!()
    };
    choice.targets[0].kind = TargetRefKind::Player as i32;
    choice.targets[0].object_id = 1;
    for (actor, command) in [
        (1, choose(&[target])),
        (0, choose(&[target, target])),
        (0, choose(&[999_999])),
        (0, player),
    ] {
        let before = format!("{:?}", engine.state);
        assert!(engine.apply_command(actor, &command).is_err());
        assert_eq!(format!("{:?}", engine.state), before);
    }
    engine.apply_command(0, &choose(&[target])).unwrap();
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
}

fn move_card(engine: &mut GameEngine, name: &str, seat: i32, zone: DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: seat,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
}

#[test]
fn deepglow_skate_partial_illegal_target_and_departed_source_leave_other_target_valid() {
    let mut engine = setup();
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    engine
        .state
        .objects
        .get_mut(&own)
        .unwrap()
        .counters
        .insert(CounterKind::Quest, 2);
    engine
        .state
        .objects
        .get_mut(&other)
        .unwrap()
        .counters
        .insert(CounterKind::Charge, 3);
    let source = cast(&mut engine);
    engine.apply_command(0, &choose(&[own, other])).unwrap();
    move_card(&mut engine, "Grizzly Bears", 0, DevZone::Graveyard);
    move_card(&mut engine, "Deepglow Skate", 0, DevZone::Graveyard);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&own].zone, Zone::Graveyard);
    assert!(engine.state.objects[&own].counters.is_empty());
    assert_eq!(
        engine.state.objects[&other].counters[&CounterKind::Charge],
        6
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn deepglow_skate_all_illegal_blinked_target_does_not_receive_counters_as_new_incarnation() {
    let mut engine = setup();
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&own)
        .unwrap()
        .counters
        .insert(CounterKind::Quest, 2);
    cast(&mut engine);
    engine.apply_command(0, &choose(&[own])).unwrap();
    move_card(&mut engine, "Grizzly Bears", 0, DevZone::Graveyard);
    move_card(&mut engine, "Grizzly Bears", 0, DevZone::Battlefield);
    engine
        .state
        .objects
        .get_mut(&own)
        .unwrap()
        .counters
        .insert(CounterKind::Quest, 7);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&own].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&own].counters[&CounterKind::Quest], 7);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn deepglow_skate_uses_effect_placement_replacement_and_prohibition_for_each_kind() {
    let mut engine = setup();
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let prohibited = inject_creature_on_battlefield(&mut engine, 0, "tatterkite");
    let replacement = inject_creature_on_battlefield(&mut engine, 0, "forest");
    let registry = CardRegistry::from_chunks_and_tokens(
        &[r#"(
      id: "deepglow_replacement_fixture", name: "Deepglow Replacement Fixture",
      face_id: "deepglow_replacement_fixture", types: ["Enchantment"],
      static_abilities: [(ability_id: "static_01", presentation: Fallback,
        definition: DoubleEffectCountersPlacedOnPermanentsYouControl)],
    )"#],
        &[],
    )
    .unwrap();
    engine
        .state
        .objects
        .get_mut(&replacement)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        source_card_id: "deepglow_replacement_fixture".into(),
        source_face_index: 0,
        face: registry
            .get("deepglow_replacement_fixture")
            .unwrap()
            .primary_face()
            .clone(),
        room_faces: None,
        display_name: "Deepglow Replacement Fixture".into(),
    });
    for oid in [own, other, prohibited] {
        engine
            .state
            .objects
            .get_mut(&oid)
            .unwrap()
            .counters
            .insert(CounterKind::Quest, 2);
        engine
            .state
            .objects
            .get_mut(&oid)
            .unwrap()
            .counters
            .insert(CounterKind::Stun, 3);
    }
    cast(&mut engine);
    engine
        .apply_command(0, &choose(&[own, other, prohibited]))
        .unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&own].counters[&CounterKind::Quest], 6);
    assert_eq!(engine.state.objects[&own].counters[&CounterKind::Stun], 9);
    assert_eq!(
        engine.state.objects[&other].counters[&CounterKind::Quest],
        4
    );
    assert_eq!(engine.state.objects[&other].counters[&CounterKind::Stun], 6);
    assert_eq!(
        engine.state.objects[&prohibited].counters[&CounterKind::Quest],
        2
    );
    assert_eq!(
        engine.state.objects[&prohibited].counters[&CounterKind::Stun],
        3
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn deepglow_skate_paid_cast_and_target_command_replay_identically() {
    use prost::Message;
    fn fresh() -> (GameEngine, u32) {
        let mut engine = setup();
        let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        engine
            .state
            .objects
            .get_mut(&target)
            .unwrap()
            .counters
            .insert(CounterKind::Quest, 3);
        inject_card_into_hand(&mut engine, 0, "deepglow_skate");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 4,
                u: 1,
                ..Default::default()
            },
        );
        (engine, target)
    }
    let (mut engine, target) = fresh();
    let slot = hand_index_for_card(&engine, 0, "deepglow_skate");
    let commands = vec![
        (0, cast_spell(slot, vec![])),
        (0, pass()),
        (1, pass()),
        (0, choose(&[target])),
        (0, pass()),
        (1, pass()),
    ];
    let mut expected = Vec::new();
    for (actor, command) in &commands {
        expected.push(engine.apply_command(*actor, command).unwrap());
    }
    assert_eq!(
        engine.state.objects[&target].counters[&CounterKind::Quest],
        6
    );
    let (mut replay, _) = fresh();
    for ((actor, command), batch) in commands.into_iter().zip(expected) {
        let encoded = command.encode_to_vec();
        let decoded = RuledCommand::decode(encoded.as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), batch);
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn deepglow_skate_three_nonconsecutive_players_use_published_permanent_ids() {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_209,
        &[4, 9, 27],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut engine, 2, "hill_giant");
    for oid in [own, other] {
        engine
            .state
            .objects
            .get_mut(&oid)
            .unwrap()
            .counters
            .insert(CounterKind::Quest, 2);
    }
    inject_card_into_hand(&mut engine, 0, "deepglow_skate");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            c: 4,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "deepglow_skate");
    engine.apply_command(4, &cast_spell(slot, vec![])).unwrap();
    for _ in 0..3 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    engine.apply_command(4, &choose(&[own, other])).unwrap();
    for _ in 0..3 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    for oid in [own, other] {
        assert_eq!(engine.state.objects[&oid].counters[&CounterKind::Quest], 4);
    }
    assert!(engine.state.stack.is_empty());
}
