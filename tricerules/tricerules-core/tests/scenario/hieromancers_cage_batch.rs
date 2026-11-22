//! Actual-card scenarios for the reviewed Hieromancer's Cage linked-exile family.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the entry trigger on the stack;
//! CR 115.1 and 601.2c govern the target; CR 610.3 links the exiled object to the source;
//! CR 608.2b removes a trigger whose only target is illegal; CR 400.7 gives the returned
//! object a new object identity when it changes zones.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone, RuledCommand};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(
            tricerules_proto::ruled::v1::ChooseTriggerTarget {
                decline: false,
                selected_modes: Vec::new(),
                targets: target_object(object_id),
            },
        )),
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn move_card_with_dev(e: &mut GameEngine, owner: i32, card: &str, zone: DevZone) {
    e.enable_dev_commands();
    e.apply_scenario_command(
        owner,
        &RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: owner,
                dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                    card_name: tricerules_cards::registry::global()
                        .get(card)
                        .expect("known card")
                        .name
                        .clone(),
                    zone: zone as i32,
                    ready: false,
                })),
            })),
        },
    )
    .expect("dev move");
}

fn cast_cage(e: &mut GameEngine, id: &str, mana: ManaGift) -> u32 {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = hand_index_for_card(e, 0, id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast the linked-exile enchantment");
    pass_priority_round(e);
    let _source = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == id)
        .expect("the enchantment entered the battlefield");
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "{id}: entry trigger waits"
    );
    slot
}

#[test]
fn hieromancers_cage_batch_cards_exile_and_return_the_chosen_permanent() {
    for (index, (id, mana)) in [
        (
            "borrowed_time",
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        ),
        (
            "hieromancers_cage",
            ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "memory_trap",
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(760_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let bystander = inject_creature_on_battlefield(&mut e, 1, "hill_giant");
        let source = {
            give_mana(&mut e, 0, gift(&mana));
            let slot = inject_card_into_hand(&mut e, 0, id);
            let hand_slot = hand_index_for_card(&e, 0, id);
            e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
                .expect("cast");
            pass_priority_round(&mut e);
            let source = e.state.players[0]
                .battlefield
                .iter()
                .copied()
                .find(|oid| e.state.objects[oid].card_id == id)
                .expect("entered");
            assert_eq!(e.state.pending_triggers.len(), 1);
            let _ = slot;
            source
        };
        e.apply_scenario_command(0, &choose_trigger_target(target))
            .expect("choose the opponent's creature");
        pass_priority_round(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{id}: exiled");
        assert_eq!(e.state.objects[&bystander].zone, Zone::Battlefield, "{id}");
        assert_eq!(e.state.objects[&source].zone, Zone::Battlefield, "{id}");

        let generation = e
            .state
            .zone_change_generation
            .get(&target)
            .copied()
            .unwrap_or(0);
        move_card_with_dev(&mut e, 0, id, DevZone::Graveyard);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{id}: source left"
        );
        pass_priority_round(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{id}: returned"
        );
        assert!(
            e.state.zone_change_generation[&target] > generation,
            "{id}: the returned object has a new incarnation"
        );
        assert!(e.state.players[1].battlefield.contains(&target), "{id}");
        assert!(!e.state.players[0].battlefield.contains(&target), "{id}");
        assert_eq!(
            e.state.objects[&target].controller, e.state.players[1].id,
            "{id}: the returned card is controlled by its owner"
        );
        assert_eq!(e.state.objects[&bystander].zone, Zone::Battlefield, "{id}");
    }
}

#[test]
fn hieromancers_cage_batch_cards_reject_land_and_own_permanent_targets() {
    for (index, id) in ["borrowed_time", "hieromancers_cage", "memory_trap"]
        .into_iter()
        .enumerate()
    {
        for (kind, target_owner, card) in [
            ("land", 1usize, "forest"),
            ("own permanent", 0usize, "grizzly_bears"),
        ] {
            let mut e = engine(761_000 + index as u64 * 10 + u64::from(target_owner as u32));
            let _decoy = inject_creature_on_battlefield(&mut e, 1, "hill_giant");
            let target = if target_owner == 0 {
                inject_creature_on_battlefield(&mut e, 0, card)
            } else {
                inject_permanent_on_battlefield(&mut e, 1, card)
            };
            let _source = cast_cage(
                &mut e,
                id,
                ManaGift {
                    w: 1,
                    c: if id == "hieromancers_cage" { 3 } else { 2 },
                    ..Default::default()
                },
            );
            assert!(
                e.apply_scenario_command(0, &choose_trigger_target(target))
                    .is_err(),
                "{id}: {kind} is not a legal trigger target"
            );
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Battlefield,
                "{id}: {kind}"
            );
            assert_eq!(e.state.pending_triggers.len(), 1, "{id}: trigger remains");
        }
    }
}

#[test]
fn hieromancers_cage_batch_cards_do_not_exile_when_the_source_left_before_the_trigger() {
    for (index, id) in ["borrowed_time", "hieromancers_cage", "memory_trap"]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(763_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let source = cast_cage(
            &mut e,
            id,
            ManaGift {
                w: 1,
                c: if id == "hieromancers_cage" { 3 } else { 2 },
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &choose_trigger_target(target))
            .expect("choose the target");
        let generation = e
            .state
            .zone_change_generation
            .get(&target)
            .copied()
            .unwrap_or(0);
        move_card_with_dev(&mut e, 0, id, DevZone::Graveyard);
        assert_eq!(e.state.objects[&source].zone, Zone::Graveyard, "{id}");
        pass_priority_round(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{id}: ruling: the target is not exiled when the source left first"
        );
        assert_eq!(
            e.state
                .zone_change_generation
                .get(&target)
                .copied()
                .unwrap_or(0),
            generation,
            "{id}: the ruling says it is never exiled, so it never changes zones"
        );
        assert!(e.state.pending_triggers.is_empty(), "{id}: trigger removed");
    }
}

#[test]
fn hieromancers_cage_batch_cards_fizzle_when_the_target_left_before_the_trigger() {
    for (index, id) in ["borrowed_time", "hieromancers_cage", "memory_trap"]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(762_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let source = cast_cage(
            &mut e,
            id,
            ManaGift {
                w: 1,
                c: if id == "hieromancers_cage" { 3 } else { 2 },
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &choose_trigger_target(target))
            .expect("choose the target");
        move_card_with_dev(&mut e, 1, "grizzly_bears", DevZone::Graveyard);
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard, "{id}");
        pass_priority_round(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{id}: fizzled"
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{id}: source stays"
        );
        assert!(e.state.pending_triggers.is_empty(), "{id}: trigger removed");
    }
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}
