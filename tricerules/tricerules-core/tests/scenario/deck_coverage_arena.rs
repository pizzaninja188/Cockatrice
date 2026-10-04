//! Actual registered Arena; expectations come from its Oracle/rulings, not emitted events.
use super::helpers::*;
use prost::Message;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

fn fixture(seed: u64, own_card: &str) -> (GameEngine, u32, u32, u32) {
    let decks = Some(vec![
        deck_with("forest", &["arena", own_card]),
        deck_with("forest", &["hill_giant"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = move_ready_to_battlefield(&mut engine, 0, "arena");
    let own = move_ready_to_battlefield(&mut engine, 0, own_card);
    let opponent = move_ready_to_battlefield(&mut engine, 1, "hill_giant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    (engine, source, own, opponent)
}

fn begin(engine: &GameEngine, source: u32, own: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
            source_object_id: source,
            expected_zone_change_generation: engine.state.zone_change_generation[&source],
            ability_index: 0,
            own_target: Some(rv1::AbilityActivationTarget {
                object_id: own,
                zone_change_generation: engine.state.zone_change_generation[&own],
                group_index: 0,
            }),
            ..Default::default()
        })),
    }
}

fn answer(engine: &GameEngine, opponent: u32) -> RuledCommand {
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    RuledCommand {
        cmd: Some(Cmd::SubmitAbilityActivationChoice(
            rv1::SubmitAbilityActivationChoice {
                transaction_id: pending.transaction_id,
                expected_revision: pending.revision,
                target: Some(rv1::AbilityActivationTarget {
                    object_id: opponent,
                    zone_change_generation: engine.state.zone_change_generation[&opponent],
                    group_index: 1,
                }),
                ..Default::default()
            },
        )),
    }
}

fn commit(engine: &GameEngine) -> RuledCommand {
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    RuledCommand {
        cmd: Some(Cmd::CommitAbilityActivation(rv1::CommitAbilityActivation {
            transaction_id: pending.transaction_id,
            expected_revision: pending.revision,
            ..Default::default()
        })),
    }
}

fn pay(engine: &mut GameEngine, source: u32, own: u32, opponent: u32) {
    engine
        .apply_command(0, &begin(engine, source, own))
        .unwrap();
    assert!(engine.state.stack.is_empty());
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    engine.apply_command(1, &answer(engine, opponent)).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    let mut command = commit(engine);
    authoring_actions::pay(engine, 0, &mut command).unwrap();
    engine.apply_command(0, &command).unwrap();
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].targets.len(), 2);
}

fn resolve(engine: &mut GameEngine) {
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn arena_announces_both_targets_before_payment_and_fights_tapped_creatures() {
    for tapped in [false, true] {
        let (mut engine, source, own, opponent) = fixture(602_401, "grizzly_bears");
        engine.state.objects.get_mut(&own).unwrap().tapped = tapped;
        engine.state.objects.get_mut(&opponent).unwrap().tapped = tapped;
        pay(&mut engine, source, own, opponent);
        resolve(&mut engine);
        assert_eq!(engine.state.objects[&own].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&opponent].zone, Zone::Battlefield);
        assert!(engine.state.objects[&opponent].tapped);
        assert_eq!(engine.state.objects[&opponent].damage, 2);
    }
}

#[test]
fn arena_fight_uses_current_power_and_normal_lifelink_deathtouch() {
    let (mut engine, source, own, opponent) = fixture(602_402, "grizzly_bears");
    pay(&mut engine, source, own, opponent);
    engine
        .state
        .objects
        .get_mut(&own)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    resolve(&mut engine);
    assert_eq!(engine.state.objects[&own].zone, Zone::Battlefield);
    assert!(engine.state.objects[&own].tapped);
    assert_eq!(engine.state.objects[&own].damage, 3);
    assert_eq!(engine.state.objects[&opponent].zone, Zone::Graveyard);

    let (mut engine, source, own, opponent) = fixture(602_403, "vampire_nighthawk");
    pay(&mut engine, source, own, opponent);
    resolve(&mut engine);
    assert_eq!(engine.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opponent].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, 22);
}

#[test]
fn arena_partial_illegality_taps_the_remaining_target_without_fighting_and_all_illegal_fizzles() {
    for all_illegal in [false, true] {
        let (mut engine, source, own, opponent) = fixture(602_404, "grizzly_bears");
        pay(&mut engine, source, own, opponent);
        engine
            .state
            .zone_change_generation
            .entry(own)
            .and_modify(|generation| *generation += 1);
        if all_illegal {
            engine
                .state
                .zone_change_generation
                .entry(opponent)
                .and_modify(|generation| *generation += 1);
        }
        resolve(&mut engine);
        assert_eq!(engine.state.objects[&own].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&opponent].zone, Zone::Battlefield);
        assert!(!engine.state.objects[&own].tapped);
        assert_eq!(engine.state.objects[&opponent].tapped, !all_illegal);
        assert_eq!(engine.state.objects[&own].damage, 0);
        assert_eq!(engine.state.objects[&opponent].damage, 0);
        assert!(engine.state.objects[&source].tapped);
    }
}

#[test]
fn arena_rejected_payment_and_cancel_do_not_pay_costs_or_lose_the_target_receipts() {
    let (mut engine, source, own, opponent) = fixture(602_405, "grizzly_bears");
    engine
        .apply_command(0, &begin(&engine, source, own))
        .unwrap();
    engine.apply_command(1, &answer(&engine, opponent)).unwrap();
    engine.state.players[0].mana_pool.colorless = 2;
    let snapshot = format!("{:?}", engine.state);
    engine
        .apply_command(0, &commit(&engine))
        .expect_err("insufficient mana");
    assert_eq!(format!("{:?}", engine.state), snapshot);
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    let cancel = RuledCommand {
        cmd: Some(Cmd::CancelAbilityActivation(rv1::CancelAbilityActivation {
            transaction_id: pending.transaction_id,
            expected_revision: pending.revision,
        })),
    };
    engine.apply_command(0, &cancel).unwrap();
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
}

#[test]
fn arena_paid_announcement_and_resolution_replay_serialized_commands_identically() {
    let (mut engine, source, own, opponent) = fixture(602_406, "grizzly_bears");
    let (mut replay, replay_source, replay_own, replay_opponent) =
        fixture(602_406, "grizzly_bears");
    assert_eq!(
        (source, own, opponent),
        (replay_source, replay_own, replay_opponent)
    );
    let mut commands = vec![];
    let command = begin(&engine, source, own);
    let batch = engine.apply_command(0, &command).unwrap();
    commands.push((0, command, batch));
    let command = answer(&engine, opponent);
    let batch = engine.apply_command(1, &command).unwrap();
    commands.push((1, command, batch));
    let mut command = commit(&engine);
    authoring_actions::pay(&engine, 0, &mut command).unwrap();
    let batch = engine.apply_command(0, &command).unwrap();
    commands.push((0, command, batch));
    for actor in [0, 1] {
        let command = pass();
        let batch = engine.apply_command(actor, &command).unwrap();
        commands.push((actor, command, batch));
    }
    for (actor, command, expected) in commands {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), expected);
    }
    assert_eq!(engine.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(engine.state.command_index, replay.state.command_index);
    assert_eq!(
        engine.state.objects[&opponent].damage,
        replay.state.objects[&opponent].damage
    );
    assert!(replay.state.pending_ability_activation.is_none());
}
