//! Actual-card scenarios for the reviewed `{T}: Add one mana of any color` family.
//!
//! Expectations are printed Oracle values. CR 605.1a and 605.3b make the
//! targetless mana ability resolve immediately without using the stack. A
//! creature source still obeys the CR 302.6 summoning-sickness rule for its
//! `{T}` cost, while a noncreature artifact does not.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    mana: ManaGift,
    creature: bool,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "lifespring_druid",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            creature: true,
        },
        Case {
            id: "utopia_tree",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            creature: true,
        },
        Case {
            id: "xenobotanist",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            creature: true,
        },
        Case {
            id: "manalith",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            creature: false,
        },
        Case {
            id: "opaline_unicorn",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            creature: true,
        },
        Case {
            id: "alloy_myr",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            creature: true,
        },
    ]
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
fn any_color_mana_batch_cards_cast_and_apply_summoning_sickness() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(850_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the mana source");
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(e.state.objects[&source].controller, 0, "{}", case.id);

        let before = pool(&e);
        if case.creature {
            assert!(
                e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                    .is_err(),
                "{}: a summoning-sick creature cannot pay {{T}}",
                case.id
            );
            assert_eq!(
                pool(&e),
                before,
                "{}: rejected activation adds no mana",
                case.id
            );
            assert!(!e.state.objects[&source].tapped, "{}", case.id);
        } else {
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .expect("artifact mana ability");
            assert!(e.state.objects[&source].tapped, "{}", case.id);
        }
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn any_color_mana_batch_cards_offer_all_five_colors() {
    for (index, case) in cases().into_iter().enumerate() {
        for option in 0u32..5 {
            let mut e = engine(851_000 + index as u64 * 10 + u64::from(option), case.id);
            let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
            let mut command = activate_ability_for(&e, source, 0, vec![]);
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
                "{} option {option}",
                case.id
            );
            assert!(
                e.state.stack.is_empty(),
                "{}: mana abilities do not use the stack",
                case.id
            );
        }
    }
}
