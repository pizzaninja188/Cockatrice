//! Actual-card scenarios for the queue-v23 flying `{T}: Add one mana of any color` cohort.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3b make the targetless
//! mana ability resolve immediately; CR 302.6 makes a summoning-sick creature
//! unable to pay its `{T}` cost; CR 702.9a-b restricts blocking by fliers.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::BlockPair;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "birds_of_paradise",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            body: (0, 1),
        },
        Case {
            id: "lotus_guardian",
            mana: ManaGift {
                c: 7,
                ..Default::default()
            },
            body: (4, 4),
        },
        Case {
            id: "ornithopter_of_paradise",
            mana: ManaGift {
                c: 2,
                ..Default::default()
            },
            body: (0, 2),
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
    let decks = Some(vec![
        deck_with("forest", &[card, "grizzly_bears"]),
        deck_with("island", &["grizzly_bears"]),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cast cost", case.id);
    resolve_entire_stack_two_player(e);
    let source = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.body.0), Some(case.body.1)),
        "{}: printed body",
        case.id
    );
    assert!(e.state.objects[&source].summoning_sick, "{}", case.id);
    source
}

#[test]
fn flying_any_mana_batch_canary_cards_cast_and_offer_every_color() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "birds_of_paradise" | "lotus_guardian"))
    {
        for option in 0u32..5 {
            let mut e = engine(12_300_000 + option as u64, case.id);
            let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
            assert_eq!(
                (e.effective_power(source), e.effective_toughness(source)),
                (Some(case.body.0), Some(case.body.1)),
                "{}: printed body",
                case.id
            );
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

#[test]
fn flying_any_mana_batch_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_310_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        assert_eq!(e.state.objects[&source].controller, 0, "{}", case.id);
    }
}

#[test]
fn flying_any_mana_batch_cards_apply_summoning_sickness_to_the_tap_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_320_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        let before = pool(&e);
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
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn flying_any_mana_batch_cards_offer_all_five_colors() {
    for (index, case) in cases().into_iter().enumerate() {
        for option in 0u32..5 {
            let mut e = engine(12_335_000 + index as u64 * 10 + u64::from(option), case.id);
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

#[test]
fn flying_any_mana_batch_cards_are_fliers_that_ground_creatures_cannot_block() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_330_000 + index as u64, case.id);
        let attacker = inject_creature_on_battlefield(&mut e, 0, case.id);
        let other_attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let ground = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker, other_attacker]))
            .expect("declare the flier and a ground attacker");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::DeclareBlockers,
            "{}: the ground attacker keeps the block step open",
            case.id
        );
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            legal
                .legal_block_pairs
                .iter()
                .any(|pair| { pair.attacker_id == other_attacker && pair.blocker_id == ground }),
            "{}: the grounded blocker may block the grounded attacker",
            case.id
        );
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| { pair.attacker_id == attacker && pair.blocker_id == ground }),
            "{}: the grounded creature cannot block the flier",
            case.id
        );
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: ground,
                }]),
            )
            .is_err(),
            "{}: the forbidden flier block is rejected",
            case.id
        );
    }
}

#[test]
fn flying_any_mana_batch_cards_remain_on_the_battlefield_after_resolution() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_340_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
        assert_eq!(count_card_id_in_graveyard(&e, 0, case.id), 0, "{}", case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            tricerules_core::Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
