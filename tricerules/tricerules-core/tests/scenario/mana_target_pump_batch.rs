//! Actual-card scenarios for the queue-v23 mana-activated target-pump cohort.
//!
//! Expectations are printed Oracle values. CR 602.2 governs activation, CR 115.1
//! fixes the target at activation, CR 611.2c/613.4c apply the until-end-of-turn
//! modifier, and CR 514.2 ends it during cleanup.
use super::helpers::*;

struct Case {
    id: &'static str,
    activation: ManaGift,
    pump: (i32, i32),
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "ghitu_war_cry",
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            pump: (1, 0),
        },
        Case {
            id: "captive_flame",
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            pump: (1, 0),
        },
        Case {
            id: "sacred_armory",
            activation: ManaGift {
                c: 2,
                ..Default::default()
            },
            pump: (1, 0),
        },
        Case {
            id: "ursapine",
            activation: ManaGift {
                g: 1,
                ..Default::default()
            },
            pump: (1, 1),
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
        deck_with("mountain", &[card]),
        deck_with("forest", &["grizzly_bears", "accorders_shield"]),
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

fn activate(
    e: &mut GameEngine,
    source: u32,
    target: u32,
    case: &Case,
) -> Result<(), tricerules_core::EngineError> {
    give_mana(e, 0, gift(&case.activation));
    let result = apply_ability(e, 0, source, 0, target_object(target)).map(|_| ());
    if result.is_ok() {
        resolve_entire_stack_two_player(e);
    }
    result
}

#[test]
fn mana_target_pump_batch_canary_cards_pump_a_chosen_creature() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "sacred_armory" | "ursapine"))
    {
        let mut e = engine(12_500_000, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate(&mut e, source, target, &case).expect("activate the pump");
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(2 + case.pump.0 as u32), Some(2 + case.pump.1 as u32)),
            "{}: printed pump on the chosen creature",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn mana_target_pump_batch_cards_target_any_creature_and_leave_others_unchanged() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_510_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate(&mut e, source, own, &case).expect("pump an own creature");
        assert_eq!(
            (e.effective_power(own), e.effective_toughness(own)),
            (Some(2 + case.pump.0 as u32), Some(2 + case.pump.1 as u32)),
            "{}: own target",
            case.id
        );
        assert_eq!(
            (e.effective_power(opponent), e.effective_toughness(opponent)),
            (Some(2), Some(2)),
            "{}: the untargeted creature is unchanged",
            case.id
        );
        activate(&mut e, source, opponent, &case).expect("pump an opponent's creature");
        assert_eq!(
            (e.effective_power(opponent), e.effective_toughness(opponent)),
            (Some(2 + case.pump.0 as u32), Some(2 + case.pump.1 as u32)),
            "{}: any creature is a legal target",
            case.id
        );
    }
}

#[test]
fn mana_target_pump_batch_cards_stack_repeated_activations() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_520_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        activate(&mut e, source, target, &case).expect("first activation");
        activate(&mut e, source, target, &case).expect("second activation");
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some(2 + (case.pump.0 * 2) as u32),
                Some(2 + (case.pump.1 * 2) as u32)
            ),
            "{}: both activations apply",
            case.id
        );
    }
}

#[test]
fn mana_target_pump_batch_cards_reject_illegal_targets_without_paying() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_530_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "accorders_shield");
        give_mana(&mut e, 0, gift(&case.activation));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(artifact)).is_err(),
            "{}: a noncreature is not a legal target",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        give_mana(&mut e, 0, gift(&case.activation));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the pump needs a target",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
    }
}

#[test]
fn mana_target_pump_batch_cards_activate_through_the_stack() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_525_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, target_object(target)).expect("activate the pump");
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: the pump uses the stack",
            case.id
        );
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed activation payment",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(2 + case.pump.0 as u32), Some(2 + case.pump.1 as u32)),
            "{}",
            case.id
        );
    }
}

#[test]
fn mana_target_pump_batch_cards_fizzle_a_target_that_left_before_resolution() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_535_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let bystander = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let before = (e.effective_power(source), e.effective_toughness(source));
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, target_object(target)).expect("activate the pump");
        assert_eq!(e.state.stack.len(), 1, "{}: still on the stack", case.id);
        e.state.players[1].battlefield.retain(|id| *id != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = tricerules_core::Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            before,
            "{}: the source is not pumped by a fizzled target",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: no other creature is pumped",
            case.id
        );
        assert!(
            e.state.continuous_effects.is_empty(),
            "{}: the fizzled pump installs no effect",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn mana_target_pump_batch_cards_expire_the_pump_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_540_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        activate(&mut e, source, target, &case).expect("activate the pump");
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(2), Some(2)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}

#[test]
fn mana_target_pump_batch_ursapine_can_target_itself() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "ursapine")
        .expect("ursapine case");
    let mut e = engine(12_550_000, case.id);
    let source = inject_creature_with_stats(&mut e, 0, case.id, 3, 3);
    activate(&mut e, source, source, &case).expect("Ursapine targets itself");
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(4), Some(4)),
        "Ursapine's printed 3/3 plus +1/+1"
    );
}
