//! Actual-card scenarios for the reviewed `Counter target creature spell.` family.
//!
//! Expectations are printed Oracle values. CR 701.6 counters a spell; CR 115.1a
//! and 601.2c choose the creature spell as the counter is cast; CR 608.2b leaves
//! a stale target alone. A countered creature spell never enters the battlefield.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "false_summoning",
        },
        Case {
            id: "preemptive_strike",
        },
        Case { id: "remove_soul" },
    ]
}

fn engine(seed: u64, counter: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["grizzly_bears", "bonesplitter", "ornithopter"]),
        deck_with("island", &[counter]),
    ]);
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

fn cast_from_hand(e: &mut GameEngine, player: usize, card: &str, mana: ManaGift) -> u32 {
    ensure_in_hand(e, player, card);
    give_mana(e, player as i32, mana);
    let slot = hand_index_for_card(e, player, card);
    e.apply_scenario_command(player as i32, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card}: {error}"));
    e.state.stack.last().expect("spell on stack").id
}

#[test]
fn counter_creature_spell_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_030_000 + index as u64, case.id);
        let creature_spell = cast_from_hand(
            &mut e,
            0,
            "grizzly_bears",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        ensure_in_hand(&mut e, 1, case.id);
        give_mana(
            &mut e,
            1,
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 1, case.id);
        e.apply_scenario_command(1, &cast_spell(slot, target_object(creature_spell)))
            .expect("counter the creature spell");
        assert_eq!(e.state.stack.len(), 2, "{}", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&creature_spell].zone,
            Zone::Graveyard,
            "{}: the countered creature spell is in its owner's graveyard",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&creature_spell),
            "{}",
            case.id
        );
        assert!(
            !e.state.players[0]
                .battlefield
                .iter()
                .any(|oid| e.state.objects[oid].card_id == "grizzly_bears"),
            "{}: the countered creature never enters",
            case.id
        );
        assert!(
            e.state.players[1]
                .graveyard
                .iter()
                .any(|oid| e.state.objects[oid].card_id == case.id),
            "{}: the counter spell resolves to its owner's graveyard",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);

        let mut artifact_e = engine(9_030_500 + index as u64, case.id);
        let artifact_creature_spell =
            cast_from_hand(&mut artifact_e, 0, "ornithopter", ManaGift::default());
        artifact_e
            .apply_scenario_command(0, &pass())
            .expect("pass priority");
        ensure_in_hand(&mut artifact_e, 1, case.id);
        give_mana(
            &mut artifact_e,
            1,
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&artifact_e, 1, case.id);
        artifact_e
            .apply_scenario_command(1, &cast_spell(slot, target_object(artifact_creature_spell)))
            .expect("counter the artifact creature spell");
        resolve_entire_stack_two_player(&mut artifact_e);
        assert_eq!(
            artifact_e.state.objects[&artifact_creature_spell].zone,
            Zone::Graveyard,
            "{}: an artifact creature is a creature spell",
            case.id
        );
        assert!(
            !artifact_e.state.players[0]
                .battlefield
                .iter()
                .any(|oid| artifact_e.state.objects[oid].card_id == "ornithopter"),
            "{}: the artifact creature never enters",
            case.id
        );
    }
}

#[test]
fn counter_creature_spell_batch_cards_reject_illegal_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_031_000 + index as u64, case.id);
        let artifact_spell = cast_from_hand(
            &mut e,
            0,
            "bonesplitter",
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        ensure_in_hand(&mut e, 1, case.id);
        give_mana(
            &mut e,
            1,
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        );
        let before = e.state.players[1].mana_pool.blue;
        let slot = hand_index_for_card(&e, 1, case.id);
        assert!(
            e.apply_scenario_command(1, &cast_spell(slot, target_object(artifact_spell)))
                .is_err(),
            "{}: a noncreature spell is not a legal target",
            case.id
        );
        assert_eq!(
            e.state.players[1].mana_pool.blue, before,
            "{}: rejected cast spends no mana",
            case.id
        );
        assert_eq!(e.state.stack.len(), 1, "{}", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.players[0]
                .battlefield
                .iter()
                .any(|oid| e.state.objects[oid].card_id == "bonesplitter"),
            "{}: the uncountered artifact spell resolves",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn counter_creature_spell_batch_cards_handle_stale_or_expired_state() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_032_000 + index as u64, case.id);
        let creature_spell = cast_from_hand(
            &mut e,
            0,
            "grizzly_bears",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        ensure_in_hand(&mut e, 1, case.id);
        give_mana(
            &mut e,
            1,
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 1, case.id);
        e.apply_scenario_command(1, &cast_spell(slot, target_object(creature_spell)))
            .expect("counter the creature spell");
        e.state.stack.retain(|entry| entry.id != creature_spell);
        e.state
            .objects
            .get_mut(&creature_spell)
            .expect("spell")
            .zone = Zone::Graveyard;
        e.state.players[0].graveyard.push(creature_spell);
        *e.state
            .zone_change_generation
            .entry(creature_spell)
            .or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&creature_spell].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            e.state.players[1]
                .graveyard
                .iter()
                .any(|oid| e.state.objects[oid].card_id == case.id),
            "{}: the counter spell fizzles to its owner's graveyard",
            case.id
        );
        assert!(
            !e.state.players[0]
                .battlefield
                .iter()
                .any(|oid| e.state.objects[oid].card_id == "grizzly_bears"),
            "{}: no creature entered",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
