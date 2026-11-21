//! Draft scenarios for queue-v53 batch C (pending review/freeze).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct RegenCase {
    id: &'static str,
    mana: ManaGift,
}

fn regen_cases() -> [RegenCase; 6] {
    [
        RegenCase {
            id: "drudge_reavers",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
        RegenCase {
            id: "ghost_ship",
            mana: ManaGift {
                u: 3,
                ..Default::default()
            },
        },
        RegenCase {
            id: "malach_of_the_dawn",
            mana: ManaGift {
                w: 3,
                ..Default::default()
            },
        },
        RegenCase {
            id: "ranger_en-vec",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
        },
        RegenCase {
            id: "troll_ascetic",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        },
        RegenCase {
            id: "cinderbones",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
        },
    ]
}

struct PumpCase {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn pump_cases() -> [PumpCase; 6] {
    [
        PumpCase {
            id: "whispering_shade",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        PumpCase {
            id: "dirtwater_wraith",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
        },
        PumpCase {
            id: "colos_yearling",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
        },
        PumpCase {
            id: "sandstone_warrior",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
        },
        PumpCase {
            id: "loxodon_stalwart",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 1,
        },
        PumpCase {
            id: "steam_spitter",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
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
        deck_with("island", &[card]),
        deck_with("forest", &["grizzly_bears", "sol_ring"]),
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

#[test]
fn q53_c_canary_simplest_drudge_reavers_regenerates() {
    let case = &regen_cases()[0];
    let mut e = engine(53_080_001, case.id);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    apply_ability(&mut e, 0, source, 0, vec![])
        .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].regeneration_shields, 1);
    inject_card_into_hand(&mut e, 0, "murder");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(source)))
        .expect("destroy the regenerating creature");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(e.state.objects[&source].tapped);
    assert_eq!(e.state.objects[&source].regeneration_shields, 0);
}

#[test]
fn q53_c_canary_hardest_troll_ascetic_regenerates() {
    let case = &regen_cases()[4];
    let mut e = engine(53_080_002, case.id);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    apply_ability(&mut e, 0, source, 0, vec![])
        .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].regeneration_shields, 1);
    inject_card_into_hand(&mut e, 0, "murder");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(source)))
        .expect("destroy the hexproof regenerating creature");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
}

#[test]
fn q53_c_regenerate_cards_apply_their_printed_cost_and_shield() {
    for (index, case) in regen_cases().iter().enumerate() {
        let mut e = engine(53_081_000 + index as u64, case.id);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        assert_eq!(
            e.state.objects[&source].regeneration_shields, 1,
            "{}: shield",
            case.id
        );
    }
}

#[test]
fn q53_c_self_pump_cards_apply_their_printed_values() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = engine(53_082_000 + index as u64, case.id);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        let before = (
            e.effective_power(source).unwrap_or(0),
            e.effective_toughness(source).unwrap_or(0),
        );
        give_mana(&mut e, 0, gift(&case.mana));
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        assert_eq!(
            (
                e.effective_power(source).unwrap_or(0),
                e.effective_toughness(source).unwrap_or(0)
            ),
            (before.0 + case.power, before.1 + case.toughness),
            "{}: printed pump",
            case.id
        );
    }
}

#[test]
fn q53_c_any_color_mana_cards_produce_each_color() {
    for (index, id) in [
        "deathbloom_gardener",
        "sylvan_caryatid",
        "leyline_prowler",
        "darksteel_ingot",
    ]
    .iter()
    .enumerate()
    {
        for option in 0..5 {
            let mut e = engine(53_083_000 + (index as u64 * 10) + option as u64, id);
            let source = if *id == "darksteel_ingot" {
                inject_permanent_on_battlefield(&mut e, 0, id)
            } else {
                inject_creature_on_battlefield(&mut e, 0, id)
            };
            let mut command = activate_ability_for(&e, source, 0, vec![]);
            let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
                unreachable!()
            };
            activation.mana_option_index = option;
            e.apply_scenario_command(0, &command)
                .unwrap_or_else(|error| panic!("activate {id} option {option}: {error:?}"));
            let mut expected = (0, 0, 0, 0, 0, 0);
            match option {
                0 => expected.0 = 1,
                1 => expected.1 = 1,
                2 => expected.2 = 1,
                3 => expected.3 = 1,
                4 => expected.4 = 1,
                _ => unreachable!(),
            }
            assert_eq!(pool(&e), expected, "{id} option {option}");
        }
    }
}

#[test]
fn q53_c_insufficient_or_wrong_color_mana_rejects_activation() {
    let mut e = engine(53_084_001, "drudge_reavers");
    let source = inject_creature_on_battlefield(&mut e, 0, "drudge_reavers");
    assert!(apply_ability(&mut e, 0, source, 0, vec![]).is_err());
    assert!(!e.state.objects[&source].tapped);
    assert_eq!(e.state.objects[&source].regeneration_shields, 0);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let before = pool(&e);
    assert!(apply_ability(&mut e, 0, source, 0, vec![]).is_err());
    assert_eq!(pool(&e), before);
    assert!(!e.state.objects[&source].tapped);
    assert_eq!(e.state.objects[&source].regeneration_shields, 0);

    let mut e = engine(53_084_002, "whispering_shade");
    let source = inject_creature_on_battlefield(&mut e, 0, "whispering_shade");
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    assert!(apply_ability(&mut e, 0, source, 0, target_object(target)).is_err());
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(2))
    );

    let mut e = engine(53_084_003, "steam_spitter");
    let source = inject_creature_on_battlefield(&mut e, 0, "steam_spitter");
    let artifact = inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let before = pool(&e);
    assert!(apply_ability(&mut e, 0, source, 0, target_object(artifact)).is_err());
    assert_eq!(pool(&e), before);
    assert!(!e.state.objects[&source].tapped);
}

#[test]
fn q53_c_mana_ability_taps_and_rejects_second_or_sick_activation() {
    let mut e = engine(53_085_001, "deathbloom_gardener");
    let source = inject_creature_on_battlefield(&mut e, 0, "deathbloom_gardener");
    let mut command = activate_ability_for(&e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 1;
    e.apply_scenario_command(0, &command)
        .expect("activate the mana ability");
    assert!(e.state.objects[&source].tapped);
    assert_eq!(pool(&e), (0, 1, 0, 0, 0, 0));
    assert!(
        e.state.stack.is_empty(),
        "mana abilities do not use the stack"
    );
    let before = pool(&e);
    let mut command = activate_ability_for(&e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 2;
    assert!(e.apply_scenario_command(0, &command).is_err());
    assert_eq!(pool(&e), before);

    let mut e = engine(53_085_002, "sylvan_caryatid");
    let source = inject_creature_on_battlefield(&mut e, 0, "sylvan_caryatid");
    e.state.objects.get_mut(&source).unwrap().summoning_sick = true;
    assert!(apply_ability(&mut e, 0, source, 0, vec![]).is_err());
    assert!(!e.state.objects[&source].tapped);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
}

#[test]
fn q53_c_self_pump_cards_expire_at_cleanup() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = engine(53_086_000 + index as u64, case.id);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        let before = (
            e.effective_power(source).unwrap_or(0),
            e.effective_toughness(source).unwrap_or(0),
        );
        give_mana(&mut e, 0, gift(&case.mana));
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_ne!(
            (
                e.effective_power(source).unwrap_or(0),
                e.effective_toughness(source).unwrap_or(0)
            ),
            before,
            "{}: the pump applied",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (
                e.effective_power(source).unwrap_or(0),
                e.effective_toughness(source).unwrap_or(0)
            ),
            before,
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}
