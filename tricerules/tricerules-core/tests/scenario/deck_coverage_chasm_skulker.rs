//! Exact Chasm Skulker: draw, generation-bound counter LKI, death, Squid characteristics/combat.
use super::helpers::*;
use prost::Message;
use tricerules_cards::{
    Color, ContinuousEffectKind, ControllerReference, CounterKind, EffectDuration, Evasion,
};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const CARD: &str = "chasm_skulker";
const TOKEN: &str = "squid_u_1_1_islandwalk";

fn setup(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut e = GameEngine::new(seed, &[10, 20, 30], 20, Some(vec![deck; 3]), true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    e.enable_dev_commands();
    e
}

fn finish(e: &mut GameEngine) {
    for _ in 0..32 {
        answer_simultaneous_entry_order_in_engine_order(e);
        answer_trigger_order_in_engine_order(e);
        if e.state.stack.is_empty() && e.state.blocking_choice().is_none() {
            return;
        }
        pass_priority_round(e);
    }
    panic!("Chasm resolution exceeded 32 rounds");
}

fn cast(e: &mut GameEngine, card: &str, blue: u32, colorless: u32, targets: Vec<TargetRef>) -> u32 {
    let id = inject_card_into_hand(e, 0, card);
    give_mana(
        e,
        10,
        ManaGift {
            u: blue,
            c: colorless,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 10, &cast_spell(slot, targets));
    id
}

fn source(e: &mut GameEngine) -> u32 {
    let id = cast(e, CARD, 1, 2, vec![]);
    assert_eq!(e.state.objects[&id].zone, Zone::Stack);
    assert_eq!(e.state.players[0].mana_pool.blue, 0);
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    finish(e);
    assert_eq!(e.state.objects[&id].zone, Zone::Battlefield);
    assert_eq!(e.characteristics(id).unwrap().power, Some(1));
    id
}

fn squids(e: &GameEngine) -> Vec<u32> {
    let mut ids: Vec<_> = e
        .state
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && o.card_id == TOKEN)
        .map(|o| o.id)
        .collect();
    ids.sort_unstable();
    ids
}

fn assert_squids(e: &GameEngine, count: usize, controller: i32) {
    let ids = squids(e);
    assert_eq!(ids.len(), count);
    for id in ids {
        let o = &e.state.objects[&id];
        assert_eq!((o.owner, o.controller), (controller, controller));
        assert!(o.is_token());
        let c = e.characteristics(id).unwrap();
        assert!(c.is_creature() && c.has_type("Squid"));
        assert_eq!((c.power, c.toughness), (Some(1), Some(1)));
        assert_eq!(c.colors, [Color::Blue]);
        assert_eq!(
            c.evasions,
            [Evasion::Landwalk {
                land_subtype: "Island".into()
            }]
        );
    }
}

fn move_command(zone: DevZone) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 10,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: "Chasm Skulker".into(),
                zone: zone as i32,
                ready: false,
            })),
        })),
    }
}

fn kill(e: &mut GameEngine, id: u32) {
    e.state.objects.get_mut(&id).unwrap().damage = 20;
    let actor = e.state.priority_player_id();
    semantic::accepted(e, actor, &pass());
}

#[test]
fn chasm_paid_cast_draws_individually_and_only_for_current_controller() {
    let mut e = setup(49801);
    let id = source(&mut e);
    let before: Vec<_> = e.state.players.iter().map(|p| p.hand.len()).collect();
    cast(&mut e, "vision_skeins", 1, 1, vec![]);
    pass_priority_round(&mut e);
    answer_trigger_order_in_engine_order(&mut e);
    assert_eq!(
        e.state.stack.len(),
        2,
        "two own draws; four opposing draws do not trigger"
    );
    assert_eq!(
        e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(
        e.state
            .players
            .iter()
            .map(|p| p.hand.len())
            .collect::<Vec<_>>(),
        before.iter().map(|n| n + 2).collect::<Vec<_>>()
    );
    pass_priority_round(&mut e);
    assert_eq!(
        e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    finish(&mut e);
    assert_eq!(
        e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        (
            e.characteristics(id).unwrap().power,
            e.characteristics(id).unwrap().toughness
        ),
        (Some(3), Some(3))
    );
    assert_squids(&e, 0, 10);
}

#[test]
fn chasm_draw_and_death_use_derived_controller_while_owner_keeps_card() {
    let mut e = setup(49802);
    let id = source(&mut e);
    e.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(id),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(20),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: e.state.command_index,
        trigger_grant_origin: None,
    });
    let hands: Vec<_> = e.state.players.iter().map(|p| p.hand.len()).collect();
    let controller_library = e.state.players[1].library.len();
    inject_card_into_hand(&mut e, 0, "blue_suns_zenith");
    give_mana(
        &mut e,
        10,
        ManaGift {
            u: 3,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "blue_suns_zenith");
    semantic::accepted(&mut e, 10, &cast_spell_x(slot, target_player(20), 2));
    finish(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hands[0],
        "owner did not draw"
    );
    assert_eq!(e.state.players[1].hand.len(), hands[1] + 2);
    assert_eq!(e.state.players[1].library.len(), controller_library - 2);
    assert_eq!(e.state.players[2].hand.len(), hands[2]);
    assert_eq!(
        e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "only the current controller drew"
    );
    cast(&mut e, "divination", 1, 2, vec![]);
    finish(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hands[0] + 2);
    assert_eq!(e.state.players[1].hand.len(), hands[1] + 2);
    assert_eq!(e.state.players[1].library.len(), controller_library - 2);
    assert_eq!(
        e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "owner-only draws cannot trigger the stolen source"
    );
    assert_eq!(e.state.objects[&id].controller, 20);
    kill(&mut e, id);
    assert_eq!(e.state.objects[&id].zone, Zone::Graveyard);
    assert!(e.state.players[0].graveyard.contains(&id));
    assert!(!e.state.players[1].graveyard.contains(&id));
    assert_squids(&e, 0, 20);
    finish(&mut e);
    assert_squids(&e, 2, 20);
}

#[test]
fn chasm_death_counts_only_plus_counters_including_zero() {
    for plus in [0, 2] {
        let mut e = setup(49810 + plus as u64);
        let id = source(&mut e);
        let o = e.state.objects.get_mut(&id).unwrap();
        o.set_counter(CounterKind::PlusOnePlusOne, plus);
        o.set_counter(CounterKind::Charge, 7);
        let generation = semantic::generation(&e, id);
        kill(&mut e, id);
        semantic::assert_object(&e, id, CARD, 10, 10, Zone::Graveyard, generation + 1);
        assert!(e.state.objects[&id].counters.is_empty());
        assert_squids(&e, 0, 10);
        finish(&mut e);
        assert_squids(&e, plus as usize, 10);
    }
}

#[test]
fn chasm_lethal_simultaneous_cancellation_uses_pre_sba_counters() {
    let mut e = setup(49820);
    let id = source(&mut e);
    let o = e.state.objects.get_mut(&id).unwrap();
    o.set_counter(CounterKind::PlusOnePlusOne, 2);
    o.set_counter(CounterKind::MinusOneMinusOne, 3);
    semantic::accepted(&mut e, 10, &pass());
    assert_eq!(e.state.objects[&id].zone, Zone::Graveyard);
    finish(&mut e);
    assert_squids(&e, 2, 10);
}

#[test]
fn chasm_nonlethal_cancellation_changes_later_death_count() {
    let mut e = setup(49821);
    let id = source(&mut e);
    let o = e.state.objects.get_mut(&id).unwrap();
    o.set_counter(CounterKind::PlusOnePlusOne, 2);
    o.set_counter(CounterKind::MinusOneMinusOne, 1);
    semantic::accepted(&mut e, 10, &pass());
    assert_eq!(e.state.objects[&id].zone, Zone::Battlefield);
    assert_eq!(
        e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        e.state.objects[&id].counter_count(CounterKind::MinusOneMinusOne),
        0
    );
    kill(&mut e, id);
    finish(&mut e);
    assert_squids(&e, 1, 10);
}

#[test]
fn chasm_death_trigger_reads_old_generation_after_source_reentry() {
    let mut e = setup(49822);
    let id = source(&mut e);
    e.state
        .objects
        .get_mut(&id)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    let generation = semantic::generation(&e, id);
    kill(&mut e, id);
    assert_eq!(e.state.stack.len(), 1);
    semantic::accepted(&mut e, 10, &move_command(DevZone::Battlefield));
    assert_eq!(semantic::generation(&e, id), generation + 2);
    e.state
        .objects
        .get_mut(&id)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 7);
    finish(&mut e);
    assert_squids(&e, 2, 10);
    assert_eq!(
        e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
        7
    );
}

#[test]
fn chasm_old_draw_triggers_do_not_add_counters_to_reentered_source() {
    let mut e = setup(49823);
    let id = source(&mut e);
    cast(&mut e, "vision_skeins", 1, 1, vec![]);
    pass_priority_round(&mut e);
    answer_trigger_order_in_engine_order(&mut e);
    assert_eq!(e.state.stack.len(), 2);
    semantic::accepted(&mut e, 10, &move_command(DevZone::Graveyard));
    semantic::accepted(&mut e, 10, &move_command(DevZone::Battlefield));
    finish(&mut e);
    assert_eq!(e.state.objects[&id].zone, Zone::Battlefield);
    assert_eq!(
        e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_squids(&e, 0, 10);
}

#[test]
fn chasm_token_copy_death_retains_counter_lki_after_cessation() {
    let mut e = setup(49824);
    let original = source(&mut e);
    cast(
        &mut e,
        "cackling_counterpart",
        2,
        1,
        target_object(original),
    );
    finish(&mut e);
    let token = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|id| e.state.objects[id].is_token())
        .unwrap();
    e.state
        .objects
        .get_mut(&token)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    kill(&mut e, token);
    assert!(
        !e.state.objects.contains_key(&token),
        "CR111.7 deletes the copied source before resolution"
    );
    assert_eq!(e.state.objects[&original].zone, Zone::Battlefield);
    finish(&mut e);
    assert_squids(&e, 2, 10);
}

#[test]
fn chasm_exile_replacement_suppresses_death_trigger() {
    let mut e = setup(49825);
    let id = source(&mut e);
    e.state
        .objects
        .get_mut(&id)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    e.state
        .death_replacement_effects
        .push(tricerules_core::state::ActiveDeathReplacement {
            object_id: id,
            zone_change_generation: semantic::generation(&e, id),
        });
    kill(&mut e, id);
    assert_eq!(e.state.objects[&id].zone, Zone::Exile);
    assert!(e.state.stack.is_empty());
    assert_squids(&e, 0, 10);
}

#[test]
fn chasm_squids_islandwalk_uses_actual_defender_and_rejects_block_atomically() {
    for defender_has_island in [false, true] {
        let mut e = setup(49830 + u64::from(defender_has_island));
        let id = source(&mut e);
        e.state
            .objects
            .get_mut(&id)
            .unwrap()
            .set_counter(CounterKind::PlusOnePlusOne, 1);
        kill(&mut e, id);
        finish(&mut e);
        assert_squids(&e, 1, 10);
        let squid = squids(&e)[0];
        e.state.objects.get_mut(&squid).unwrap().summoning_sick = false;
        let vanilla = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_permanent_on_battlefield(&mut e, if defender_has_island { 1 } else { 2 }, "island");
        inject_permanent_on_battlefield(&mut e, 1, "forest");
        semantic::accepted(&mut e, 10, &primitive_yield());
        pass_priority_round(&mut e);
        assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
        let legal = e.initial_response_batch().legal_by_player[&10]
            .legal_attack_assignments
            .clone();
        let assignments = [squid, vanilla]
            .iter()
            .map(|id| {
                *legal
                    .iter()
                    .find(|a| a.attacker_object_id == *id && a.defending_player_id == 20)
                    .unwrap()
            })
            .collect();
        semantic::accepted(
            &mut e,
            10,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers { assignments })),
            },
        );
        pass_priority_round(&mut e);
        assert_eq!(e.state.turn_step, TurnStep::DeclareBlockers);
        let command = declare_blockers(vec![BlockPair {
            attacker_id: squid,
            blocker_id: blocker,
        }]);
        if defender_has_island {
            let before = e.diagnostic_snapshot().unwrap();
            assert!(e.apply_command(20, &command).is_err());
            assert_eq!(e.diagnostic_snapshot().unwrap(), before);
            semantic::accepted(
                &mut e,
                20,
                &declare_blockers(vec![BlockPair {
                    attacker_id: vanilla,
                    blocker_id: blocker,
                }]),
            );
        } else {
            semantic::accepted(&mut e, 20, &command);
        }
    }
}

#[test]
fn chasm_actual_draw_death_tokens_and_serialized_commands_replay_identically() {
    fn fresh() -> GameEngine {
        let mut e = setup(49840);
        inject_card_into_hand(&mut e, 0, CARD);
        inject_card_into_hand(&mut e, 0, "vision_skeins");
        inject_card_into_hand(&mut e, 0, "lightning_bolt");
        give_mana(
            &mut e,
            10,
            ManaGift {
                u: 2,
                c: 3,
                r: 1,
                ..Default::default()
            },
        );
        e
    }
    fn apply(
        e: &mut GameEngine,
        commands: &mut Vec<(i32, RuledCommand)>,
        events: &mut Vec<RuledEventBatch>,
        actor: i32,
        command: RuledCommand,
    ) {
        let encoded = command.encode_to_vec();
        let decoded = RuledCommand::decode(encoded.as_slice()).unwrap();
        events.push(semantic::accepted(e, actor, &decoded));
        commands.push((actor, decoded));
    }
    fn drain(
        e: &mut GameEngine,
        commands: &mut Vec<(i32, RuledCommand)>,
        events: &mut Vec<RuledEventBatch>,
    ) {
        for _ in 0..64 {
            if let Some(pending) = e.state.pending_resolution.as_ref() {
                assert_eq!(
                    pending.presentation.choice_kind,
                    ChoiceKind::SimultaneousEntryOrder
                );
                let actor = pending.deciding_player;
                let command = submit_resolution_choice(pending.presentation.candidates.clone());
                apply(e, commands, events, actor, command);
            } else if let Some(order) = e.state.pending_trigger_order.as_ref() {
                let actor = order.deciding_player;
                let command = submit_trigger_order(order.candidates[0].object_id);
                apply(e, commands, events, actor, command);
            } else if e.state.stack.is_empty() && e.state.blocking_choice().is_none() {
                return;
            } else {
                assert!(
                    e.state.blocking_choice().is_none(),
                    "unexpected unrecorded choice"
                );
                let actor = e.state.priority_player_id();
                apply(e, commands, events, actor, pass());
            }
        }
        panic!("replay drain exhausted");
    }
    let mut e = fresh();
    let mut commands = vec![];
    let mut events = vec![];
    let slot = hand_index_for_card(&e, 0, CARD);
    apply(
        &mut e,
        &mut commands,
        &mut events,
        10,
        cast_spell(slot, vec![]),
    );
    drain(&mut e, &mut commands, &mut events);
    let slot = hand_index_for_card(&e, 0, "vision_skeins");
    apply(
        &mut e,
        &mut commands,
        &mut events,
        10,
        cast_spell(slot, vec![]),
    );
    drain(&mut e, &mut commands, &mut events);
    let source = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|id| e.state.objects[id].card_id == CARD)
        .unwrap();
    let slot = hand_index_for_card(&e, 0, "lightning_bolt");
    apply(
        &mut e,
        &mut commands,
        &mut events,
        10,
        cast_spell(slot, target_object(source)),
    );
    drain(&mut e, &mut commands, &mut events);
    assert_squids(&e, 2, 10);
    // Public presentation must carry the same printed evasion as the real token.
    // Without it, the exact blue 1/1 Islandwalk token database entry is rejected.
    let assert_identity = |identity: &tricerules_proto::ruled::v1::TokenIdentity| {
        assert_eq!(identity.name, "Squid");
        assert_eq!(identity.pt, "1/1");
        assert_eq!(identity.color, "u");
        assert!(identity.is_creature);
        assert!(identity.types.iter().any(|kind| kind == "Squid"));
        assert_eq!(identity.keywords, ["Islandwalk"]);
    };
    let mut proposed = 0;
    let mut created = 0;
    for event in events.iter().flat_map(|batch| &batch.events) {
        match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice))
                if choice.choice_kind == ChoiceKind::SimultaneousEntryOrder as i32 =>
            {
                for identity in &choice.candidate_token_identities {
                    assert_identity(identity);
                    proposed += 1;
                }
            }
            Some(Ev::TokenCreated(token)) => {
                assert_identity(token.identity.as_ref().expect("created token identity"));
                created += 1;
            }
            _ => {}
        }
    }
    assert_eq!((proposed, created), (2, 2));
    let snapshot = e.initial_response_batch();
    let mut resynced = vec![];
    for event in &snapshot.events {
        if let Some(Ev::ZoneView(view)) = &event.ev {
            for object in view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
            {
                if squids(&e).contains(&object.object_id) {
                    assert_identity(
                        object
                            .token_identity
                            .as_ref()
                            .expect("resynced token identity"),
                    );
                    resynced.push(object.object_id);
                }
            }
        }
    }
    resynced.sort_unstable();
    assert_eq!(resynced, squids(&e));
    let mut replay = fresh();
    let replayed: Vec<_> = commands
        .iter()
        .map(|(actor, command)| replay.apply_command(*actor, command).unwrap())
        .collect();
    assert_eq!(events, replayed);
    assert_eq!(
        e.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}
