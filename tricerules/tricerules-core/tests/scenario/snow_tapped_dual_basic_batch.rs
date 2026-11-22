//! Actual-card scenarios for the queue-v21 snow basic-type tapped dual lands.
//!
//! Expectations are printed Oracle values. CR 205.4g makes Snow a supertype with
//! no rules meaning of its own; CR 305.6/305.8 make the basic land subtypes
//! grant the intrinsic mana ability; CR 614.1d/614.12 apply the enter-tapped
//! replacement; CR 605.1a/605.3b make the targetless mana ability resolve
//! immediately. These cards print no {S} symbol and add no snow-mana contract.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    first: (u32, u32, u32, u32, u32, u32),
    second: (u32, u32, u32, u32, u32, u32),
}

fn cases() -> [Case; 10] {
    [
        Case {
            id: "alpine_meadow",
            first: (0, 0, 0, 1, 0, 0),
            second: (1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "arctic_treeline",
            first: (0, 0, 0, 0, 1, 0),
            second: (1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "glacial_floodplain",
            first: (1, 0, 0, 0, 0, 0),
            second: (0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "highland_forest",
            first: (0, 0, 0, 1, 0, 0),
            second: (0, 0, 0, 0, 1, 0),
        },
        Case {
            id: "ice_tunnel",
            first: (0, 1, 0, 0, 0, 0),
            second: (0, 0, 1, 0, 0, 0),
        },
        Case {
            id: "rimewood_falls",
            first: (0, 0, 0, 0, 1, 0),
            second: (0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "snowfield_sinkhole",
            first: (1, 0, 0, 0, 0, 0),
            second: (0, 0, 1, 0, 0, 0),
        },
        Case {
            id: "sulfurous_mire",
            first: (0, 0, 1, 0, 0, 0),
            second: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "volatile_fjord",
            first: (0, 1, 0, 0, 0, 0),
            second: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "woodland_chasm",
            first: (0, 0, 1, 0, 0, 0),
            second: (0, 0, 0, 0, 1, 0),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn mana_command(e: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}

#[test]
fn snow_tapped_dual_basic_batch_canary_cards_enter_and_tap_for_their_pair() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "alpine_meadow" | "woodland_chasm"))
    {
        let mut e = engine(9_972_000, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_index = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &play_land(hand_index))
            .expect("play the canary land");
        let land = battlefield_object_for_card(&e, 0, case.id);
        assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
        let mut e = engine(9_972_100, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &mana_command(&e, land, 1))
            .expect("activate the second printed option");
        assert_eq!(pool(&e), case.second, "{}: second option", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn snow_tapped_dual_basic_batch_cards_enter_tapped_and_reject_immediate_tap() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_973_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_index = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &play_land(hand_index))
            .expect("play the land");
        let land = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
        let before = pool(&e);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &mana_command(&e, land, 0))
                .is_err(),
            "{}: a tapped land cannot pay its tap cost",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected activation adds no mana",
            case.id
        );
        assert!(e.state.objects[&land].tapped, "{}", case.id);
    }
}

#[test]
fn snow_tapped_dual_basic_batch_cards_tap_for_each_printed_option_in_order() {
    for (index, case) in cases().into_iter().enumerate() {
        for (option, expected) in [(0u32, case.first), (1u32, case.second)] {
            let mut e = engine(9_974_000 + index as u64 * 10 + u64::from(option), case.id);
            let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
            e.apply_scenario_command(0, &mana_command(&e, land, option))
                .expect("activate the selected mana option");
            assert_eq!(pool(&e), expected, "{} option {option}", case.id);
            assert!(e.state.objects[&land].tapped, "{} option {option}", case.id);
            assert!(
                e.state.stack.is_empty(),
                "{} option {option}: mana abilities do not use the stack",
                case.id
            );
        }
    }
}

#[test]
fn snow_tapped_dual_basic_batch_cards_produce_ordinary_colored_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_975_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &mana_command(&e, land, 0))
            .expect("activate the snow land for its first color");
        let pool = pool(&e);
        assert_eq!(
            pool.0 + pool.1 + pool.2 + pool.3 + pool.4,
            1,
            "{}: exactly one colored mana unit, with no separate snow counter",
            case.id
        );
        assert_eq!(pool.5, 0, "{}: the mana is colored, not colorless", case.id);
        assert!(
            e.state.stack.is_empty(),
            "{}: the mana ability resolves without the stack",
            case.id
        );
    }
}
