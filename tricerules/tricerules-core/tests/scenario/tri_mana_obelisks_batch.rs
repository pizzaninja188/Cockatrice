//! Actual-card scenarios for the reviewed three-color mana cohort.
//!
//! Expectations are printed Oracle values. CR 605.1a and 605.3a-b make the targetless mana
//! abilities resolve immediately; CR 106.1a-b produce one mana of the chosen
//! printed color; CR 302.6 and 602.5a deny a summoning-sick creature's tap cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: Option<(u32, u32)>,
    options: [(u32, u32, u32, u32, u32, u32); 3],
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "obelisk_of_bant",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            body: None,
            options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
        },
        Case {
            id: "obelisk_of_esper",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            body: None,
            options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
        },
        Case {
            id: "obelisk_of_grixis",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            body: None,
            options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
        },
        Case {
            id: "obelisk_of_jund",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            body: None,
            options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
        },
        Case {
            id: "obelisk_of_naya",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            body: None,
            options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
        },
        Case {
            id: "druid_of_the_anima",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            body: Some((1, 1)),
            options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
        },
        Case {
            id: "urborg_elf",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            body: Some((1, 1)),
            options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0), (0, 1, 0, 0, 0, 0)],
        },
    ]
}

fn canaries() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|case| matches!(case.id, "obelisk_of_bant" | "druid_of_the_anima"))
        .collect()
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

fn clear_summoning_sickness(e: &mut GameEngine) {
    end_active_turn(e, 0);
    advance_to_main1_from_game_start(e);
    end_active_turn(e, 1);
    advance_to_main1_from_game_start(e);
}

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(
        e,
        0,
        ManaGift {
            w: case.mana.w,
            u: case.mana.u,
            b: case.mana.b,
            r: case.mana.r,
            g: case.mana.g,
            c: case.mana.c,
        },
    );
    inject_card_into_hand(e, 0, case.id);
    let hand = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
    let source = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    match case.body {
        Some((power, toughness)) => assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(power), Some(toughness)),
            "{}: printed body",
            case.id
        ),
        None => assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (None, None),
            "{}: an artifact has no body",
            case.id
        ),
    }
    source
}

#[test]
fn tri_mana_obelisks_batch_canary_cards_cast_and_offer_each_printed_color() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(880_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        assert!(
            e.state.stack.is_empty(),
            "{}: nothing left on the stack",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        for (option, expected) in case.options.iter().enumerate() {
            let mut e = engine(881_000 + index as u64 * 10 + option as u64, case.id);
            let source = cast_source(&mut e, &case);
            if case.body.is_some() {
                clear_summoning_sickness(&mut e);
            }
            e.apply_scenario_command(0, &mana_command(&e, source, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            assert_eq!(
                pool(&e),
                *expected,
                "{} option {option}: printed color",
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

#[test]
fn tri_mana_obelisks_batch_cards_tap_for_each_printed_option_in_order() {
    for (index, case) in cases().into_iter().enumerate() {
        for (option, expected) in case.options.iter().enumerate() {
            let mut e = engine(882_000 + index as u64 * 10 + option as u64, case.id);
            let source = move_ready_to_battlefield(&mut e, 0, case.id);
            e.apply_scenario_command(0, &mana_command(&e, source, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            assert_eq!(
                pool(&e),
                *expected,
                "{} option {option}: printed color",
                case.id
            );
            assert!(
                e.state.objects[&source].tapped,
                "{} option {option}",
                case.id
            );
        }
    }
}

#[test]
fn tri_mana_obelisks_batch_cards_reject_activation_when_already_tapped() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(883_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &mana_command(&e, source, 0))
            .expect("first activation");
        let before = pool(&e);
        assert!(
            e.apply_scenario_command(0, &mana_command(&e, source, 1))
                .is_err(),
            "{}: a tapped source cannot pay the tap cost",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected activation adds no mana",
            case.id
        );
    }
}

#[test]
fn tri_mana_obelisks_batch_artifacts_ignore_summoning_sickness_but_creatures_do_not() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(884_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        let before = pool(&e);
        let result = e.apply_scenario_command(0, &mana_command(&e, source, 0));
        match case.body {
            None => {
                result.expect("an artifact can tap immediately");
                assert_eq!(pool(&e), case.options[0], "{}", case.id);
            }
            Some(_) => {
                assert!(
                    result.is_err(),
                    "{}: a summoning-sick creature cannot tap",
                    case.id
                );
                assert_eq!(pool(&e), before, "{}", case.id);
                assert!(!e.state.objects[&source].tapped, "{}", case.id);
                end_active_turn(&mut e, 0);
                advance_to_main1_from_game_start(&mut e);
                end_active_turn(&mut e, 1);
                advance_to_main1_from_game_start(&mut e);
                e.apply_scenario_command(0, &mana_command(&e, source, 0))
                    .expect("the creature taps once the sickness clears");
                assert_eq!(pool(&e), case.options[0], "{}", case.id);
            }
        }
    }
}
