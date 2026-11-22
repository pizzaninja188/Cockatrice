//! Actual-card scenarios for the reviewed Cycling {2} land cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d/614.12 apply the tapped entry;
//! CR 605.1a/605.3b make the mana ability immediate; CR 702.29a defines Cycling
//! as a hand-only ability paid with {2} and discarding the card, followed by a
//! draw.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

struct Case {
    id: &'static str,
    color: (u32, u32, u32, u32, u32, u32),
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "remote_isle",
            color: (0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "polluted_mire",
            color: (0, 0, 1, 0, 0, 0),
        },
        Case {
            id: "smoldering_crater",
            color: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "slippery_karst",
            color: (0, 0, 0, 0, 1, 0),
        },
        Case {
            id: "drifting_meadow",
            color: (1, 0, 0, 0, 0, 0),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn put_on_top(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
}

fn hand_ability(e: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 1,
            ..Default::default()
        })),
    }
}

#[test]
fn cycling_lands_batch_cards_enter_tapped_add_color_and_cycle_for_two() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut entered = engine(10_300_000 + index as u64, case.id);
        ensure_in_hand(&mut entered, 0, case.id);
        let slot = hand_index_for_card(&entered, 0, case.id);
        entered
            .apply_scenario_command(0, &play_land(slot))
            .expect("play the land");
        let entered_land = battlefield_object_for_card(&entered, 0, case.id);
        assert!(
            entered.state.objects[&entered_land].tapped,
            "{}: the printed entry replacement taps it",
            case.id
        );
        assert!(
            apply_ability(&mut entered, 0, entered_land, 0, vec![]).is_err(),
            "{}: the entering land cannot pay {{T}} immediately",
            case.id
        );

        let mut mana = engine(10_310_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut mana, 0, case.id);
        apply_ability(&mut mana, 0, source, 0, vec![]).expect("tap for the printed color");
        assert_eq!(pool(&mana, 0), case.color, "{}", case.id);
        assert!(mana.state.stack.is_empty(), "{}", case.id);

        let mut cycling = engine(10_320_000 + index as u64, case.id);
        ensure_in_hand(&mut cycling, 0, case.id);
        let card = cycling.state.players[0].hand[hand_index_for_card(&cycling, 0, case.id)];
        let drawn = put_on_top(&mut cycling, 0, "grizzly_bears");
        give_mana(
            &mut cycling,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        cycling
            .apply_scenario_command(0, &hand_ability(&cycling, card))
            .expect("activate Cycling {2}");
        assert_eq!(
            cycling.state.objects[&card].zone,
            Zone::Graveyard,
            "{}: cycling discards itself as a cost",
            case.id
        );
        assert_eq!(pool(&cycling, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);
        resolve_entire_stack_two_player(&mut cycling);
        assert_eq!(
            cycling.state.objects[&drawn].zone,
            Zone::Hand,
            "{}: cycling draws exactly one card",
            case.id
        );
    }
}

#[test]
fn cycling_lands_batch_cards_reject_cycling_without_the_printed_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_330_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
        let command = hand_ability(&e, card);
        e.apply_scenario_command(0, &command)
            .expect_err("Cycling {2} needs two mana");
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{}", case.id);

        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &command)
            .expect_err("one mana is not enough");
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 1), "{}", case.id);

        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &command)
            .expect("exactly {2} is enough");
        assert_eq!(e.state.objects[&card].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn cycling_lands_batch_cards_cycle_only_from_hand() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_340_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        assert!(
            apply_ability(&mut e, 0, source, 1, vec![]).is_err(),
            "{}: the battlefield permanent has no cycling ability",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 2), "{}", case.id);
    }
}
