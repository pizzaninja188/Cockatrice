//! Actual-card scenarios for the queue-v22 "each player sacrifices a creature of
//! their choice" entry cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a makes the entry trigger;
//! CR 101.4/101.4b gives each player their own choice in APNAP order; CR 701.21a
//! sacrifices all chosen creatures simultaneously.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    generic: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "fleshbag_marauder",
            generic: 2,
        },
        Case {
            id: "merciless_executioner",
            generic: 2,
        },
        Case {
            id: "slum_reaper",
            generic: 3,
        },
    ]
}

fn engine(seed: u64, card: &str, players: &[i32]) -> GameEngine {
    let decks = (0..players.len())
        .map(|seat| {
            if seat == 0 {
                deck_with("swamp", &[card, "grizzly_bears", "grizzly_bears"])
            } else {
                deck_with("forest", &["grizzly_bears", "grizzly_bears"])
            }
        })
        .collect();
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        players,
        20,
        Some(decks),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast_card(e: &mut GameEngine, case: &Case) {
    give_mana(
        e,
        0,
        ManaGift {
            b: 1,
            c: case.generic,
            ..Default::default()
        },
    );
    ensure_in_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the entry creature");
    pass_priority_round(e); // the creature spell resolves and the entry trigger stacks
    pass_priority_round(e); // the trigger reaches its first sacrifice choice
}

fn choose(e: &mut GameEngine, victim: u32) {
    let deciding = e
        .state
        .pending_resolution
        .as_ref()
        .expect("a sacrifice choice is pending")
        .deciding_player;
    let seat = e.state.player_idx(deciding).expect("known deciding player");
    let player = e.state.players[seat].id;
    e.apply_scenario_command(player, &submit_resolution_choice(vec![victim]))
        .expect("choose the sacrificed creature");
}

#[test]
fn etb_edict_batch_canary_each_player_sacrifices_their_own_chosen_creature() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "fleshbag_marauder" | "slum_reaper"))
    {
        let mut e = engine(11_200_000, case.id, &[0, 1]);
        let own_bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let other_bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_card(&mut e, &case);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        while e.state.pending_resolution.is_some() {
            let deciding = e.state.pending_resolution.as_ref().unwrap().deciding_player;
            let seat = e.state.player_idx(deciding).expect("known deciding player");
            let victim = if seat == 0 { own_bear } else { other_bear };
            let player = e.state.players[seat].id;
            e.apply_scenario_command(player, &submit_resolution_choice(vec![victim]))
                .expect("choose a creature");
        }
        assert_eq!(
            e.state.objects[&own_bear].zone,
            Zone::Graveyard,
            "{}: the controller's chosen creature is sacrificed",
            case.id
        );
        assert_eq!(
            e.state.objects[&other_bear].zone,
            Zone::Graveyard,
            "{}: the opponent's chosen creature is sacrificed",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: the source survives when not chosen",
            case.id
        );
    }
}

#[test]
fn etb_edict_batch_cards_make_each_player_choose_independently() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_210_000 + index as u64, case.id, &[0, 1]);
        let own_bears: Vec<u32> = (0..2)
            .map(|_| inject_creature_on_battlefield(&mut e, 0, "grizzly_bears"))
            .collect();
        let other_bears: Vec<u32> = (0..2)
            .map(|_| inject_creature_on_battlefield(&mut e, 1, "grizzly_bears"))
            .collect();
        cast_card(&mut e, &case);
        let first = e.state.pending_resolution.as_ref().unwrap().deciding_player;
        let first_seat = e.state.player_idx(first).expect("known deciding player");
        assert_eq!(
            first_seat, 0,
            "{}: APNAP starts with the controller",
            case.id
        );
        let first_player = e.state.players[first_seat].id;
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(
                first_player,
                &submit_resolution_choice(vec![other_bears[0]])
            )
            .is_err(),
            "{}: a player cannot choose another player's creature",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        choose(&mut e, own_bears[0]);
        choose(&mut e, other_bears[0]);
        assert_eq!(
            e.state.objects[&own_bears[0]].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&other_bears[0]].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&own_bears[1]].zone,
            Zone::Battlefield,
            "{}: the unchosen creature stays",
            case.id
        );
        assert_eq!(
            e.state.objects[&other_bears[1]].zone,
            Zone::Battlefield,
            "{}: the unchosen creature stays",
            case.id
        );
    }
}

#[test]
fn etb_edict_batch_controller_sacrifices_the_source_when_it_is_the_only_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_220_000 + index as u64, case.id, &[0, 1]);
        cast_card(&mut e, &case);
        assert!(
            e.state.pending_resolution.is_none(),
            "{}: one candidate needs no choice and the opponent has none",
            case.id
        );
        assert!(
            e.state
                .objects
                .values()
                .any(|object| { object.card_id == case.id && object.zone == Zone::Graveyard }),
            "{}: the source is the only creature and is sacrificed",
            case.id
        );
    }
}

#[test]
fn etb_edict_batch_cards_include_every_player_in_a_three_player_game() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_230_000 + index as u64, case.id, &[0, 1, 2]);
        let bears: Vec<u32> = (0..3)
            .map(|seat| inject_creature_on_battlefield(&mut e, seat, "grizzly_bears"))
            .collect();
        cast_card(&mut e, &case);
        while e.state.pending_resolution.is_some() {
            let deciding = e.state.pending_resolution.as_ref().unwrap().deciding_player;
            let seat = e.state.player_idx(deciding).expect("known deciding player");
            let player = e.state.players[seat].id;
            e.apply_scenario_command(player, &submit_resolution_choice(vec![bears[seat]]))
                .expect("choose a creature");
        }
        for (seat, bear) in bears.iter().enumerate() {
            assert_eq!(
                e.state.objects[bear].zone,
                Zone::Graveyard,
                "{}: seat {seat} sacrificed its chosen creature",
                case.id
            );
        }
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
