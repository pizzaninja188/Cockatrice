//! Actual-card scenarios for the queue-v22 printed `+X/-Y` flying self-pump
//! cohort.
//!
//! Expectations are printed Oracle values. CR 702.9 gives flying evasion;
//! CR 602.2 requires the mana cost; CR 611.2a/613.4c apply the pump until
//! cleanup; CR 514.2 expires it; zero toughness is lethal by state-based action.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::RuledCommand;

struct Case {
    id: &'static str,
    power: i32,
    toughness: i32,
    delta_power: i32,
    delta_toughness: i32,
    mana: ManaGift,
}

fn gift(w: u32, u: u32, b: u32, r: u32) -> ManaGift {
    ManaGift {
        w,
        u,
        b,
        r,
        ..Default::default()
    }
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "darklit_gargoyle",
            power: 1,
            toughness: 2,
            delta_power: 2,
            delta_toughness: -1,
            mana: gift(0, 0, 1, 0),
        },
        Case {
            id: "flowstone_wyvern",
            power: 3,
            toughness: 3,
            delta_power: 2,
            delta_toughness: -2,
            mana: gift(0, 0, 0, 1),
        },
        Case {
            id: "metropolis_sprite",
            power: 1,
            toughness: 2,
            delta_power: 1,
            delta_toughness: -1,
            mana: gift(0, 1, 0, 0),
        },
        Case {
            id: "talonrend",
            power: 0,
            toughness: 5,
            delta_power: 1,
            delta_toughness: -1,
            mana: gift(0, 0, 0, 0),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("forest", &["grizzly_bears"]),
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

fn pump_command(e: &GameEngine, source: u32) -> RuledCommand {
    activate_ability_for(e, source, 0, vec![])
}

fn activate_pump(e: &mut GameEngine, case: &Case, hybrid_blue: bool) {
    let source = move_ready_to_battlefield(e, 0, case.id);
    give_mana(
        e,
        0,
        if case.id == "talonrend" && hybrid_blue {
            ManaGift {
                u: 1,
                ..Default::default()
            }
        } else if case.id == "talonrend" {
            ManaGift {
                r: 1,
                ..Default::default()
            }
        } else {
            gift(case.mana.w, case.mana.u, case.mana.b, case.mana.r)
        },
    );
    let command = pump_command(e, source);
    e.apply_scenario_command(0, &command)
        .expect("activate the printed self-pump");
    resolve_entire_stack_two_player(e);
}

#[test]
fn flying_neg_pump_batch_canary_cards_apply_the_printed_pump() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "metropolis_sprite" | "talonrend"))
    {
        let mut e = engine(11_600_000, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            if case.id == "talonrend" {
                ManaGift {
                    u: 1,
                    ..Default::default()
                }
            } else {
                gift(case.mana.w, case.mana.u, case.mana.b, case.mana.r)
            },
        );
        let command = pump_command(&e, source);
        e.apply_scenario_command(0, &command).expect("activate");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some((case.power + case.delta_power) as u32),
                Some((case.toughness + case.delta_toughness) as u32)
            ),
            "{}",
            case.id
        );
    }
}

#[test]
fn flying_neg_pump_batch_cards_apply_the_exact_pump_to_every_card() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_610_000 + index as u64, case.id);
        if case.id == "talonrend" {
            activate_pump(&mut e, &case, true);
        } else {
            activate_pump(&mut e, &case, false);
        }
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some((case.power + case.delta_power) as u32),
                Some((case.toughness + case.delta_toughness) as u32)
            ),
            "{}",
            case.id
        );
    }
}

#[test]
fn flying_neg_pump_batch_cards_die_at_zero_toughness() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_620_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let activations =
            ((case.toughness - case.delta_toughness - 1) / -case.delta_toughness) as u32;
        give_mana(
            &mut e,
            0,
            ManaGift {
                u: if case.id == "talonrend" || case.id == "metropolis_sprite" {
                    activations
                } else {
                    0
                },
                b: if case.id == "darklit_gargoyle" {
                    activations
                } else {
                    0
                },
                r: if case.id == "flowstone_wyvern" || case.id == "talonrend" {
                    activations
                } else {
                    0
                },
                c: 0,
                w: 0,
                g: 0,
            },
        );
        for _ in 0..activations {
            let command = pump_command(&e, source);
            e.apply_scenario_command(0, &command)
                .expect("activate to zero toughness");
            resolve_entire_stack_two_player(&mut e);
        }
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: zero toughness is lethal",
            case.id
        );
    }
}

#[test]
fn flying_neg_pump_batch_cards_expire_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_630_000 + index as u64, case.id);
        if case.id == "talonrend" {
            activate_pump(&mut e, &case, true);
        } else {
            activate_pump(&mut e, &case, false);
        }
        let source = battlefield_object_for_card(&e, 0, case.id);
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power as u32), Some(case.toughness as u32)),
            "{}: the pump expired",
            case.id
        );
    }
}

#[test]
fn flying_neg_pump_batch_cards_keep_flying_evasion() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_640_000 + index as u64, case.id);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let other_attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        activate_pump(&mut e, &case, false);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert!(
            e.effective_has_keyword(source, tricerules_cards::Keyword::Flying),
            "{}: flying remains",
            case.id
        );
        assert_eq!(
            e.state.objects[&blocker].zone,
            Zone::Battlefield,
            "{}: a grounded blocker exists",
            case.id
        );
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![source, other_attacker]))
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
                .any(|pair| { pair.attacker_id == other_attacker && pair.blocker_id == blocker }),
            "{}: the grounded blocker may block the grounded attacker",
            case.id
        );
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| { pair.attacker_id == source && pair.blocker_id == blocker }),
            "{}: the grounded creature cannot block the flier",
            case.id
        );
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: source,
                    blocker_id: blocker,
                }]),
            )
            .is_err(),
            "{}: the forbidden flier block is rejected",
            case.id
        );
    }
}

#[test]
fn flying_neg_pump_batch_cards_reject_activation_without_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_650_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let command_index = e.state.command_index;
        let command = pump_command(&e, source);
        assert!(
            e.apply_scenario_command(0, &command).is_err(),
            "{}",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power as u32), Some(case.toughness as u32)),
            "{}",
            case.id
        );
    }
}
