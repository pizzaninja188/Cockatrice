//! Actual-card scenarios for the queue-v41 activated pump, shrink and enchantment-removal
//! cohorts.
//!
//! Expectations are printed Oracle values. CR 602.2 governs activation, CR 118.3 fixes the
//! payment, CR 701.21a sacrifices as a cost, CR 701.8 destroys, and CR 611.2c/613.4c expire the
//! until-end-of-turn modifier at cleanup (CR 514.2).
use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::TargetRef;

struct PumpCase {
    id: &'static str,
    mana: ManaGift,
    dp: i32,
    dt: i32,
}

fn pump_cases() -> [PumpCase; 5] {
    [
        PumpCase {
            id: "wyluli_wolf",
            mana: ManaGift::default(),
            dp: 1,
            dt: 1,
        },
        PumpCase {
            id: "ghost_warden",
            mana: ManaGift::default(),
            dp: 1,
            dt: 1,
        },
        PumpCase {
            id: "nantuko_disciple",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            dp: 2,
            dt: 2,
        },
        PumpCase {
            id: "thriss,_nantuko_primus",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            dp: 5,
            dt: 5,
        },
        PumpCase {
            id: "tower_of_champions",
            mana: ManaGift {
                c: 8,
                ..Default::default()
            },
            dp: 6,
            dt: 6,
        },
    ]
}

struct ShrinkCase {
    id: &'static str,
    mana: ManaGift,
    dp: i32,
    dt: i32,
    sacrifices: bool,
}

fn shrink_cases() -> [ShrinkCase; 6] {
    [
        ShrinkCase {
            id: "ghosts_of_the_damned",
            mana: ManaGift::default(),
            dp: -1,
            dt: 0,
            sacrifices: false,
        },
        ShrinkCase {
            id: "saltfield_recluse",
            mana: ManaGift::default(),
            dp: -2,
            dt: 0,
            sacrifices: false,
        },
        ShrinkCase {
            id: "phyrexian_denouncer",
            mana: ManaGift::default(),
            dp: -1,
            dt: -1,
            sacrifices: true,
        },
        ShrinkCase {
            id: "phyrexian_defiler",
            mana: ManaGift::default(),
            dp: -3,
            dt: -3,
            sacrifices: true,
        },
        ShrinkCase {
            id: "staff_of_zegon",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            dp: -2,
            dt: 0,
            sacrifices: false,
        },
        ShrinkCase {
            id: "bloodtallow_candle",
            mana: ManaGift {
                c: 6,
                ..Default::default()
            },
            dp: -5,
            dt: -5,
            sacrifices: true,
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
fn q41_activated_pump_canary_simplest_pumps_a_chosen_creature() {
    let case = pump_cases()
        .into_iter()
        .find(|case| case.id == "wyluli_wolf")
        .expect("Wyluli Wolf case");
    let mut e = engine(41_060_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    activate(&mut e, source, target, &case.mana).expect("activate the pump");
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(3)),
        "the printed +1/+1 applies to the chosen creature"
    );
    assert!(e.state.objects[&source].tapped);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
    assert!(e.state.stack.is_empty());
}

#[test]
fn q41_activated_pump_canary_hardest_pays_eight_and_pumps_six() {
    let case = pump_cases()
        .into_iter()
        .find(|case| case.id == "tower_of_champions")
        .expect("Tower of Champions case");
    let mut e = engine(41_060_002, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.mana));
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 8));
    activate(&mut e, source, target, &ManaGift::default()).expect("activate the pump");
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(8), Some(8))
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact eight generic");
}

#[test]
fn q41_activated_pump_cards_apply_their_printed_values() {
    for (index, case) in pump_cases().into_iter().enumerate() {
        let mut e = engine(41_061_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate(&mut e, source, target, &case.mana).expect("activate the pump");
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some((2 + case.dp) as u32), Some((2 + case.dt) as u32)),
            "{}: printed pump",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    }
}

#[test]
fn q41_activated_pump_cards_expire_at_cleanup() {
    for (index, case) in pump_cases().into_iter().enumerate() {
        let mut e = engine(41_062_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        activate(&mut e, source, target, &case.mana).expect("activate the pump");
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
fn q41_activated_pump_cards_require_a_creature_target_and_one_activation() {
    for (index, case) in pump_cases().into_iter().enumerate() {
        let mut e = engine(41_063_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "accorders_shield");
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(artifact)).is_err(),
            "{}: a noncreature is not a legal target",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana spent", case.id);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the pump needs a target",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana spent", case.id);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        activate(&mut e, source, target, &case.mana).expect("first activation");
        give_mana(&mut e, 0, gift(&case.mana));
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(target)).is_err(),
            "{}: the tapped source cannot activate again even with mana available",
            case.id
        );
    }
}

#[test]
fn q41_activated_pump_cards_fizzle_a_target_that_left() {
    for (index, case) in pump_cases().into_iter().enumerate() {
        let mut e = engine(41_064_000 + index as u64, case.id);
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

#[test]
fn q41_activated_shrink_canary_simplest_leaves_toughness_alone() {
    let case = shrink_cases()
        .into_iter()
        .find(|case| case.id == "ghosts_of_the_damned")
        .expect("Ghosts of the Damned case");
    let mut e = engine(41_070_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    activate(&mut e, source, target, &case.mana).expect("activate the shrink");
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(1), Some(2)),
        "the printed -1/-0 does not reduce toughness"
    );
    assert!(e.state.objects[&source].tapped);
}

#[test]
fn q41_activated_shrink_canary_hardest_sacrifices_and_kills() {
    let case = shrink_cases()
        .into_iter()
        .find(|case| case.id == "phyrexian_defiler")
        .expect("Phyrexian Defiler case");
    let mut e = engine(41_070_002, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
    activate(&mut e, source, target, &case.mana).expect("activate the shrink");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn q41_activated_shrink_cards_apply_their_printed_values() {
    for (index, case) in shrink_cases().into_iter().enumerate() {
        let mut e = engine(41_071_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
        activate(&mut e, source, target, &case.mana).expect("activate the shrink");
        assert_eq!(
            e.effective_power(target),
            Some((6 + case.dp) as u32),
            "{}: printed power penalty",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some((6 + case.dt) as u32),
            "{}: printed toughness penalty",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            if case.sacrifices {
                Zone::Graveyard
            } else {
                Zone::Battlefield
            },
            "{}: the printed sacrifice cost",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    }
}

#[test]
fn q41_activated_shrink_cards_expire_at_cleanup() {
    for (index, case) in shrink_cases().into_iter().enumerate() {
        let mut e = engine(41_072_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
        activate(&mut e, source, target, &case.mana).expect("activate the shrink");
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(6), Some(6)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}

#[test]
fn q41_sacrifice_destroy_enchantment_canary_destroys_the_chosen_aura() {
    let mut e = engine(41_080_001, "druid_lyrist");
    let source = inject_permanent_on_battlefield(&mut e, 0, "druid_lyrist");
    let aura = inject_permanent_on_battlefield(&mut e, 1, "clachan_festival");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, source, 0, target_object(aura)).expect("activate the Lyrist");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Graveyard,
        "the source is sacrificed as a cost"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
}

#[test]
fn q41_sacrifice_destroy_enchantment_cards_use_their_printed_mana() {
    for (index, (card, gift, use_white)) in [
        (
            "druid_lyrist",
            ManaGift {
                g: 1,
                ..Default::default()
            },
            false,
        ),
        (
            "elvish_lyrist",
            ManaGift {
                g: 1,
                ..Default::default()
            },
            false,
        ),
        (
            "elvish_hexhunter",
            ManaGift {
                w: 1,
                ..Default::default()
            },
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(41_081_000 + index as u64, card);
        let source = inject_permanent_on_battlefield(&mut e, 0, card);
        let aura = inject_permanent_on_battlefield(&mut e, 1, "clachan_festival");
        give_mana(&mut e, 0, gift);
        apply_ability(&mut e, 0, source, 0, target_object(aura)).expect("activate");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard, "{card}");
        assert_eq!(e.state.objects[&source].zone, Zone::Graveyard, "{card}");
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{card}");
        if use_white {
            assert_eq!(
                pool(&e),
                (0, 0, 0, 0, 0, 0),
                "the hybrid cost accepts white"
            );
        }
    }
}

#[test]
fn q41_sacrifice_destroy_enchantment_canary_hardest_uses_white_hybrid() {
    let mut e = engine(41_083_001, "elvish_hexhunter");
    let source = inject_permanent_on_battlefield(&mut e, 0, "elvish_hexhunter");
    let aura = inject_permanent_on_battlefield(&mut e, 1, "clachan_festival");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, source, 0, target_object(aura)).expect("activate with white hybrid");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
}

#[test]
fn q41_sacrifice_destroy_enchantment_requires_a_legal_target() {
    for (index, (card, mana)) in [
        (
            "druid_lyrist",
            ManaGift {
                g: 1,
                ..Default::default()
            },
        ),
        (
            "elvish_lyrist",
            ManaGift {
                g: 1,
                ..Default::default()
            },
        ),
        (
            "elvish_hexhunter",
            ManaGift {
                g: 1,
                ..Default::default()
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(41_082_000 + index as u64, card);
        let source = inject_permanent_on_battlefield(&mut e, 0, card);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "accorders_shield");
        give_mana(&mut e, 0, mana);
        let before = format!("{:?}", e.state);
        assert!(
            apply_ability(
                &mut e,
                0,
                source,
                0,
                vec![TargetRef {
                    object_id: artifact,
                    kind: tricerules_proto::ruled::v1::TargetRefKind::Permanent as i32,
                    ..Default::default()
                }]
            )
            .is_err(),
            "{card}: an artifact is not an enchantment"
        );
        assert_eq!(format!("{:?}", e.state), before, "{card}");
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{card}: no cost is paid"
        );
    }
}
