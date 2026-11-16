//! Actual-card scenarios for the queue-v21 basic-type tapped dual lands.
//!
//! Expectations are printed Oracle values. CR 305.6/305.8 make the basic land
//! subtypes grant the intrinsic mana ability; CR 614.1d/614.12 make the
//! enters-tapped clause a replacement effect; CR 605.1a/605.3b make the
//! targetless mana ability resolve immediately without using the stack.
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
            id: "contaminated_aquifer",
            first: (0, 1, 0, 0, 0, 0),
            second: (0, 0, 1, 0, 0, 0),
        },
        Case {
            id: "geothermal_bog",
            first: (0, 0, 1, 0, 0, 0),
            second: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "haunted_mire",
            first: (0, 0, 1, 0, 0, 0),
            second: (0, 0, 0, 0, 1, 0),
        },
        Case {
            id: "idyllic_beachfront",
            first: (1, 0, 0, 0, 0, 0),
            second: (0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "molten_tributary",
            first: (0, 1, 0, 0, 0, 0),
            second: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "radiant_grove",
            first: (0, 0, 0, 0, 1, 0),
            second: (1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "sacred_peaks",
            first: (0, 0, 0, 1, 0, 0),
            second: (1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "sunlit_marsh",
            first: (1, 0, 0, 0, 0, 0),
            second: (0, 0, 1, 0, 0, 0),
        },
        Case {
            id: "tangled_islet",
            first: (0, 0, 0, 0, 1, 0),
            second: (0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "wooded_ridgeline",
            first: (0, 0, 0, 1, 0, 0),
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
fn tapped_dual_basic_batch_canary_cards_enter_and_tap_for_their_pair() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "contaminated_aquifer" | "wooded_ridgeline"))
    {
        let mut e = engine(9_969_000, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_index = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &play_land(hand_index))
            .expect("play the canary land");
        let land = battlefield_object_for_card(&e, 0, case.id);
        assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
        let mut e = engine(9_969_100, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &mana_command(&e, land, 1))
            .expect("activate the second printed option");
        assert_eq!(pool(&e), case.second, "{}: second option", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn tapped_dual_basic_batch_cards_enter_tapped_and_reject_immediate_tap() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_970_000 + index as u64, case.id);
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
fn tapped_dual_basic_batch_cards_tap_for_each_printed_option_in_order() {
    for (index, case) in cases().into_iter().enumerate() {
        for (option, expected) in [(0u32, case.first), (1u32, case.second)] {
            let mut e = engine(9_971_000 + index as u64 * 10 + u64::from(option), case.id);
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
fn tapped_dual_basic_batch_cards_credit_the_activating_player_only() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_972_000 + index as u64, case.id);
        end_active_turn(&mut e, 0);
        advance_to_main1_from_game_start(&mut e);
        assert_eq!(e.state.active_player_id(), 1, "{}", case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, case.id);
        e.apply_scenario_command(1, &mana_command(&e, land, 0))
            .expect("player one activates the mana ability");
        let one = &e.state.players[1].mana_pool;
        let zero = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                one.white,
                one.blue,
                one.black,
                one.red,
                one.green,
                one.colorless
            ),
            case.first,
            "{}: the activating player receives the mana",
            case.id
        );
        assert_eq!(
            (
                zero.white,
                zero.blue,
                zero.black,
                zero.red,
                zero.green,
                zero.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{}: the other player receives nothing",
            case.id
        );
    }
}
