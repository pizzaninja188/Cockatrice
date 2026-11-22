//! Actual-card scenarios for the queue-v21 negative-toughness self-pump cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 activates the ability and
//! pays its printed mana; CR 611.2a/613.4c apply the modification; CR 704.5f kills a
//! creature at zero toughness; CR 514.2 restores the printed body at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    activation: ManaGift,
    body: (u32, u32),
    pump: (i32, i32),
}

fn cases() -> [Case; 10] {
    [
        Case {
            id: "breathstealer",
            activation: ManaGift {
                b: 1,
                ..Default::default()
            },
            body: (2, 2),
            pump: (1, -1),
        },
        Case {
            id: "flowstone_crusher",
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (4, 4),
            pump: (1, -1),
        },
        Case {
            id: "flowstone_giant",
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (3, 3),
            pump: (2, -2),
        },
        Case {
            id: "flowstone_shambler",
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (2, 2),
            pump: (1, -1),
        },
        Case {
            id: "frostburn_weird",
            activation: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: (1, 4),
            pump: (1, -1),
        },
        Case {
            id: "retrieval_agent",
            activation: ManaGift {
                c: 2,
                ..Default::default()
            },
            body: (2, 5),
            pump: (1, -1),
        },
        Case {
            id: "scion_of_glaciers",
            activation: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: (2, 5),
            pump: (1, -1),
        },
        Case {
            id: "viashino_slasher",
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (1, 2),
            pump: (1, -1),
        },
        Case {
            id: "watercourser",
            activation: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: (2, 3),
            pump: (1, -1),
        },
        Case {
            id: "yavimaya_ancients",
            activation: ManaGift {
                g: 1,
                ..Default::default()
            },
            body: (2, 7),
            pump: (1, -2),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn activate(e: &mut GameEngine, source: u32, case: &Case) {
    give_mana(e, 0, gift(&case.activation));
    e.apply_scenario_command(0, &activate_ability_for(e, source, 0, vec![]))
        .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    resolve_entire_stack_two_player(e);
    assert!(e.state.stack.is_empty(), "{}", case.id);
    assert!(!e.state.objects[&source].tapped, "{}: no tap cost", case.id);
}

#[test]
fn self_pump_negative_batch_canary_cards_apply_the_exact_printed_pump() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "breathstealer" | "frostburn_weird"))
    {
        let mut e = engine(10_040_000, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate(&mut e, source, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some((case.body.0 as i32 + case.pump.0) as u32),
                Some((case.body.1 as i32 + case.pump.1) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}: no tap cost", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    }
}

#[test]
fn self_pump_negative_batch_cards_apply_the_exact_printed_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_041_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate(&mut e, source, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some((case.body.0 as i32 + case.pump.0) as u32),
                Some((case.body.1 as i32 + case.pump.1) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert_eq!(
            (e.effective_power(opponent), e.effective_toughness(opponent)),
            (Some(2), Some(2)),
            "{}: only the source is affected",
            case.id
        );
    }
}

#[test]
fn self_pump_negative_batch_cards_stack_two_activations() {
    for (index, case) in cases().into_iter().enumerate() {
        if case.body.1 as i32 + 2 * case.pump.1 <= 0 {
            continue;
        }
        let mut e = engine(10_042_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate(&mut e, source, &case);
        activate(&mut e, source, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some((case.body.0 as i32 + 2 * case.pump.0) as u32),
                Some((case.body.1 as i32 + 2 * case.pump.1) as u32)
            ),
            "{}: two activations accumulate",
            case.id
        );
    }
}

#[test]
fn self_pump_negative_batch_cards_die_at_zero_toughness() {
    for (index, case) in cases().into_iter().enumerate() {
        let activations = (1..=16)
            .find(|count| case.body.1 as i32 + count * case.pump.1 <= 0)
            .expect("printed toughness reaches zero");
        let mut e = engine(10_043_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        for activation in 0..activations {
            activate(&mut e, source, &case);
            if activation + 1 < activations {
                assert!(
                    case.body.1 as i32 + (activation + 1) * case.pump.1 > 0,
                    "{}: activates only while toughness is positive",
                    case.id
                );
                assert_eq!(
                    e.state.objects[&source].zone,
                    Zone::Battlefield,
                    "{}",
                    case.id
                );
            }
        }
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: CR 704.5f sends a zero-toughness creature to the graveyard after {activations} activations",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&source),
            "{}",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: a dead source cannot be activated again",
            case.id
        );
    }
}

#[test]
fn self_pump_negative_batch_cards_expire_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        if case.body.1 as i32 + case.pump.1 <= 0 {
            continue;
        }
        let mut e = engine(10_044_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate(&mut e, source, &case);
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}

#[test]
fn self_pump_negative_batch_cards_reject_activation_without_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_045_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: the printed mana is required",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: no pump is applied",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
    }
}

#[test]
fn self_pump_negative_batch_frostburn_weird_accepts_either_hybrid_color() {
    for (index, gift) in [
        ManaGift {
            u: 1,
            ..Default::default()
        },
        ManaGift {
            r: 1,
            ..Default::default()
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(10_046_000 + index as u64, "frostburn_weird");
        let source = inject_permanent_on_battlefield(&mut e, 0, "frostburn_weird");
        give_mana(&mut e, 0, gift);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect("activate with either hybrid color");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(2), Some(3)),
            "hybrid option {index}"
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
    }
}
