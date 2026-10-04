//! Actual-card Multikicker, charge-entry, live colorless mana and zone-history coverage.
use super::helpers::*;
use prost::Message;
use tricerules_cards::CounterKind;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    dev_command::Dev, DevCommand, DevMoveCard, DevPutCardInZone, DevZone,
};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, CastCostGroupSelection};

fn game() -> GameEngine {
    let mut e = GameEngine::new(
        704_100,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    e.enable_dev_commands();
    e
}

fn cast_chalice(e: &mut GameEngine, n: u32) -> u32 {
    let oid = inject_card_into_hand(e, 0, "everflowing_chalice");
    give_mana(
        e,
        0,
        ManaGift {
            c: 2 * n,
            ..Default::default()
        },
    );
    let selections = if n == 0 {
        vec![]
    } else {
        vec![CastCostGroupSelection {
            repetitions: Some(n),
            ..Default::default()
        }]
    };
    let command = cast_spell_with_cast_cost_groups(
        hand_index_for_card(e, 0, "everflowing_chalice"),
        vec![],
        selections,
    );
    let batch = semantic::accepted(e, 0, &command);
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    let item = e.state.stack.last().unwrap();
    assert_eq!(item.card_id, "everflowing_chalice");
    assert_eq!(item.chosen_x, 0);
    // The committed stack is public, while cast offers remain caster-only.
    assert!(batch.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.card_id == "everflowing_chalice")
    }));
    semantic::complete(e, 8, |_| None).require_exercised();
    assert_eq!(e.state.objects[&oid].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&oid].counter_count(CounterKind::Charge), n);
    oid
}

#[test]
fn everflowing_chalice_casts_zero_one_three_and_produces_live_colorless_mana() {
    for n in [0, 1, 3] {
        let mut e = game();
        let oid = cast_chalice(&mut e, n);
        let command = activate_ability_for(&e, oid, 0, vec![]);
        semantic::accepted(&mut e, 0, &command);
        let pool = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            (0, 0, 0, 0, 0, n)
        );
        assert!(e.state.objects[&oid].tapped);
        assert!(e.state.stack.is_empty());
        assert!(e.state.pending_resolution.is_none());
    }
}

#[test]
fn everflowing_chalice_reads_live_counters_and_rejects_invalid_mana_choice() {
    let mut e = game();
    let oid = cast_chalice(&mut e, 1);
    e.state
        .objects
        .get_mut(&oid)
        .unwrap()
        .set_counter(CounterKind::Charge, 4);
    let mut invalid = activate_ability_for(&e, oid, 0, vec![]);
    let Some(Cmd::ActivateAbility(command)) = invalid.cmd.as_mut() else {
        unreachable!()
    };
    command.mana_option_index = 1;
    let before = format!("{:?}", e.state);
    assert!(e.apply_command(0, &invalid).is_err());
    assert_eq!(format!("{:?}", e.state), before);
    let command = activate_ability_for(&e, oid, 0, vec![]);
    semantic::accepted(&mut e, 0, &command);
    assert_eq!(e.state.players[0].mana_pool.colorless, 4);
}

#[test]
fn everflowing_chalice_direct_entry_and_leave_return_have_no_cast_payment_history() {
    let mut e = game();
    let oid = cast_chalice(&mut e, 3);
    dev(
        &mut e,
        Dev::MoveCard(DevMoveCard {
            card_name: "Everflowing Chalice".into(),
            zone: DevZone::Graveyard as i32,
            ready: false,
        }),
    );
    dev(
        &mut e,
        Dev::MoveCard(DevMoveCard {
            card_name: "Everflowing Chalice".into(),
            zone: DevZone::Battlefield as i32,
            ready: false,
        }),
    );
    assert_eq!(e.state.objects[&oid].counter_count(CounterKind::Charge), 0);
    dev(
        &mut e,
        Dev::PutCardInZone(DevPutCardInZone {
            card_name: "Everflowing Chalice".into(),
            zone: DevZone::Battlefield as i32,
            ready: true,
        }),
    );
    let fresh = e.state.next_object_id - 1;
    assert_eq!(
        e.state.objects[&fresh].counter_count(CounterKind::Charge),
        0
    );
}

fn dev(e: &mut GameEngine, dev: Dev) {
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: 0,
                dev: Some(dev),
            })),
        },
    )
    .unwrap();
}

#[test]
fn everflowing_chalice_replays_serialized_count_payment_entry_and_mana_identically() {
    let mut engine = game();
    let mut replay = game();
    let oid = inject_card_into_hand(&mut engine, 0, "everflowing_chalice");
    assert_eq!(
        oid,
        inject_card_into_hand(&mut replay, 0, "everflowing_chalice")
    );
    for e in [&mut engine, &mut replay] {
        give_mana(
            e,
            0,
            ManaGift {
                c: 6,
                ..Default::default()
            },
        );
    }
    let cast = cast_spell_with_cast_cost_groups(
        hand_index_for_card(&engine, 0, "everflowing_chalice"),
        vec![],
        vec![CastCostGroupSelection {
            repetitions: Some(3),
            ..Default::default()
        }],
    );
    let mut commands = vec![];
    let batch = engine.apply_command(0, &cast).unwrap();
    commands.push((0, cast, batch));
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        let command = pass();
        let batch = engine.apply_command(actor, &command).unwrap();
        commands.push((actor, command, batch));
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&oid].counter_count(CounterKind::Charge),
        3
    );
    let command = activate_ability_for(&engine, oid, 0, vec![]);
    let batch = engine.apply_command(0, &command).unwrap();
    commands.push((0, command, batch));
    for (actor, command, expected) in commands {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), expected);
    }
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert_eq!(
        serde_json::to_value(&engine.state).unwrap(),
        serde_json::to_value(&replay.state).unwrap()
    );
}
