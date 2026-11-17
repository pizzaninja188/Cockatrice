//! Actual-card scenarios for the queue-v26 Banner sacrifice-draw cycle.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3a-b make the
//! targetless mana ability resolve immediately; CR 118.3 and 701.21a require the
//! printed tap and sacrifice costs; CR 121.1 draws exactly one card.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

type Pool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    colors: [char; 3],
}

const CASES: [Case; 5] = [
    Case {
        id: "abzan_banner",
        colors: ['w', 'b', 'g'],
    },
    Case {
        id: "jeskai_banner",
        colors: ['u', 'r', 'w'],
    },
    Case {
        id: "mardu_banner",
        colors: ['r', 'w', 'b'],
    },
    Case {
        id: "sultai_banner",
        colors: ['b', 'g', 'u'],
    },
    Case {
        id: "temur_banner",
        colors: ['g', 'u', 'r'],
    },
];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine) -> Pool {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn mask(color: char) -> Pool {
    match color {
        'w' => (1, 0, 0, 0, 0, 0),
        'u' => (0, 1, 0, 0, 0, 0),
        'b' => (0, 0, 1, 0, 0, 0),
        'r' => (0, 0, 0, 1, 0, 0),
        'g' => (0, 0, 0, 0, 1, 0),
        _ => unreachable!("printed color"),
    }
}

fn gift(colors: &[char]) -> ManaGift {
    let mut gift = ManaGift::default();
    for color in colors {
        match color {
            'w' => gift.w += 1,
            'u' => gift.u += 1,
            'b' => gift.b += 1,
            'r' => gift.r += 1,
            'g' => gift.g += 1,
            _ => unreachable!("printed color"),
        }
    }
    gift
}

fn mana_command(
    e: &GameEngine,
    artifact: u32,
    option: u32,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(e, artifact, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn banners_batch_cards_produce_each_printed_color_and_tap() {
    for (case_index, case) in CASES.iter().enumerate() {
        for (option, color) in case.colors.iter().enumerate() {
            let mut e = engine(26_040_000 + (case_index * 3 + option) as u64);
            let artifact = inject_permanent_on_battlefield(&mut e, 0, case.id);
            e.apply_scenario_command(0, &mana_command(&e, artifact, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            assert_eq!(pool(&e), mask(*color), "{} option {option}", case.id);
            assert!(e.state.objects[&artifact].tapped, "{}", case.id);
            assert!(e.state.stack.is_empty(), "{}", case.id);
        }
    }
}

#[test]
fn banners_batch_cards_sacrifice_for_exactly_one_card() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_041_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.colors));
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();

        e.apply_scenario_command(0, &activate_ability_for(&e, artifact, 1, vec![]))
            .unwrap_or_else(|error| panic!("{}: activate draw: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact cost consumed",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
    }
}

#[test]
fn banners_batch_cards_reject_the_draw_without_all_three_colors() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_042_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.colors[..2]));
        let hand_before = e.state.players[0].hand.len();
        let before = e.state.command_index;
        e.apply_scenario_command(0, &activate_ability_for(&e, artifact, 1, vec![]))
            .expect_err("two of the three printed colors cannot pay the cost");
        assert_eq!(e.state.command_index, before, "{}", case.id);
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(!e.state.objects[&artifact].tapped, "{}", case.id);

        let mut colorless = engine(26_043_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut colorless, 0, case.id);
        give_mana(
            &mut colorless,
            0,
            ManaGift {
                c: 3,
                ..ManaGift::default()
            },
        );
        let before = colorless.state.command_index;
        colorless
            .apply_scenario_command(0, &activate_ability_for(&colorless, artifact, 1, vec![]))
            .expect_err("three colorless mana cannot pay three colored symbols");
        assert_eq!(colorless.state.command_index, before, "{}", case.id);
        assert_eq!(pool(&colorless), (0, 0, 0, 0, 0, 3), "{}", case.id);
        assert_eq!(
            colorless.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(!colorless.state.objects[&artifact].tapped, "{}", case.id);
    }
}
