//! Actual-card scenarios for the reviewed "{T}: Add {C}; {1}, {T}: Add one mana
//! of any color" family.
//!
//! Expectations are printed Oracle values. CR 605.1a and 605.3b make these
//! targetless mana abilities resolve immediately without the stack; CR 118.3
//! requires every printed cost to be fully payable.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    is_land: bool,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "mana_prism",
            is_land: false,
        },
        Case {
            id: "painted_bluffs",
            is_land: true,
        },
        Case {
            id: "shimmering_grotto",
            is_land: true,
        },
        Case {
            id: "unknown_shores",
            is_land: true,
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

#[test]
fn mana_any_color_batch_cards_produce_colorless_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_280_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            tricerules_core::Zone::Battlefield,
            "{}",
            case.id
        );
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect("activate the colorless mana ability");
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 1),
            "{}: one colorless mana",
            case.id
        );
        assert!(
            e.state.objects[&source].tapped,
            "{}: the tap cost is paid",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: mana abilities do not use the stack",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].controller, 0,
            "{}: the source stays controlled by its activator",
            case.id
        );
        let _ = case.is_land;
    }
}

#[test]
fn mana_any_color_batch_cards_offer_all_five_colors() {
    for (index, case) in cases().into_iter().enumerate() {
        for option in 0u32..5 {
            let mut e = engine(9_281_000 + index as u64 * 10 + u64::from(option), case.id);
            let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
            give_mana(
                &mut e,
                0,
                ManaGift {
                    c: 1,
                    ..Default::default()
                },
            );
            let mut command = activate_ability_for(&e, source, 1, vec![]);
            let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
                unreachable!()
            };
            ability.mana_option_index = option;
            e.apply_scenario_command(0, &command)
                .expect("activate the selected mana option");
            let mut expected = [0u32; 6];
            expected[option as usize] = 1;
            assert_eq!(
                pool(&e),
                (
                    expected[0],
                    expected[1],
                    expected[2],
                    expected[3],
                    expected[4],
                    expected[5]
                ),
                "{} option {option}",
                case.id
            );
            assert!(
                e.state.objects[&source].tapped,
                "{} option {option}: the tap cost is paid",
                case.id
            );
            assert!(
                e.state.stack.is_empty(),
                "{} option {option}: mana abilities do not use the stack",
                case.id
            );
        }
    }
}

#[test]
fn mana_any_color_batch_cards_require_the_printed_costs() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_282_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 1, vec![]))
                .is_err(),
            "{}: the any-color ability requires {{1}}",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(!e.state.objects[&source].tapped, "{}", case.id);

        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect("the first activation is legal");
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: a tapped source cannot pay {{T}} again",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 1, vec![]))
                .is_err(),
            "{}: a tapped source cannot activate the any-color ability",
            case.id
        );
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 1),
            "{}: rejected activations add no mana",
            case.id
        );
        let mut foreign = engine(9_283_000 + index as u64, case.id);
        let foreign_source = inject_permanent_on_battlefield(&mut foreign, 0, case.id);
        assert!(
            foreign
                .apply_scenario_command(
                    1,
                    &activate_ability_for(&foreign, foreign_source, 0, vec![])
                )
                .is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert_eq!(
            pool(&foreign),
            (0, 0, 0, 0, 0, 0),
            "{}: the foreign activation adds no mana",
            case.id
        );
        let _ = case.is_land;
    }
}
