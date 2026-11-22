//! Actual-card scenarios for the queue-v43 activated target pump cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the activation; CR 118.3 pays
//! the printed mana; CR 115.1 chooses one target creature; CR 611.2a/613.4c apply the temporary
//! pump and CR 514.2 ends it at cleanup.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    dp: i32,
    dt: i32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "flowstone_overseer",
            mana: ManaGift {
                r: 2,
                ..Default::default()
            },
            dp: 1,
            dt: -1,
        },
        Case {
            id: "devotee_of_strength",
            mana: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            dp: 2,
            dt: 2,
        },
        Case {
            id: "hagra_sharpshooter",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            dp: -1,
            dt: -1,
        },
        Case {
            id: "smokespew_invoker",
            mana: ManaGift {
                b: 1,
                c: 7,
                ..Default::default()
            },
            dp: -3,
            dt: -3,
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

fn activate(
    e: &mut GameEngine,
    source: u32,
    target: u32,
    mana: &ManaGift,
) -> Result<(), tricerules_core::EngineError> {
    give_mana(e, 0, gift(mana));
    let result = apply_ability(e, 0, source, 0, target_object(target)).map(|_| ());
    if result.is_ok() {
        resolve_entire_stack_two_player(e);
    }
    result
}

#[test]
fn q43_activated_target_pump_canary_pumps_a_chosen_creature() {
    let case = &cases()[0];
    let mut e = engine(43_030_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let bystander = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
    activate(&mut e, source, target, &case.mana).expect("activate the pump");
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(1)),
        "the printed +1/-1 applies"
    );
    assert_eq!(
        (
            e.effective_power(bystander),
            e.effective_toughness(bystander)
        ),
        (Some(2), Some(2)),
        "only the chosen creature is pumped"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact two red");
}

#[test]
fn q43_activated_target_pump_cards_apply_their_printed_values() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_031_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
        activate(&mut e, source, target, &case.mana).expect("activate the pump");
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some((4 + case.dp) as u32), Some((4 + case.dt) as u32)),
            "{}: printed pump",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: the source stays",
            case.id
        );
    }
}

#[test]
fn q43_activated_target_pump_cards_expire_and_reject_illegal_activation() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_032_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 4, 4);
        activate(&mut e, source, target, &case.mana).expect("activate the pump");
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(4), Some(4)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}: the target survives the negative pump and reaches cleanup",
            case.id
        );

        let mut reject = engine(43_033_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut reject, 0, case.id);
        let legal = inject_creature_on_battlefield(&mut reject, 0, "grizzly_bears");
        let artifact = inject_permanent_on_battlefield(&mut reject, 1, "accorders_shield");
        assert!(
            apply_ability(&mut reject, 0, source, 0, target_object(legal)).is_err(),
            "{}: the activation requires its printed mana",
            case.id
        );
        assert_eq!(
            pool(&reject),
            (0, 0, 0, 0, 0, 0),
            "{}: no mana spent",
            case.id
        );
        give_mana(&mut reject, 0, gift(&case.mana));
        let before = pool(&reject);
        assert!(
            apply_ability(&mut reject, 0, source, 0, target_object(artifact)).is_err(),
            "{}: a noncreature is not a legal target",
            case.id
        );
        assert_eq!(pool(&reject), before, "{}: no mana spent", case.id);
        assert!(
            apply_ability(&mut reject, 0, source, 0, vec![]).is_err(),
            "{}: the pump needs a target",
            case.id
        );
        assert_eq!(pool(&reject), before, "{}: no mana spent", case.id);
    }
}

#[test]
fn q43_activated_target_pump_cards_fizzle_a_target_that_left() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_034_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let before = (e.effective_power(source), e.effective_toughness(source));
        give_mana(&mut e, 0, gift(&case.mana));
        apply_ability(&mut e, 0, source, 0, target_object(target)).expect("activate");
        e.state.players[1].battlefield.retain(|id| *id != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            before,
            "{}: the source is not pumped",
            case.id
        );
        assert!(
            e.state.continuous_effects.is_empty(),
            "{}: the fizzled pump installs no effect",
            case.id
        );
    }
}
