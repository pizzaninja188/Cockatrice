//! Actual-card scenarios for the queue-v30 filtered creature destruction cohort.
//!
//! Expectations are printed Oracle values. CR 701.8 destroys the chosen creature;
//! the printed power, toughness, flying, or color restriction is part of target
//! legality (CR 115.1/601.2c) and is rechecked on resolution (CR 608.2b).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    legal: &'static str,
    illegal: &'static str,
}

fn cases() -> [Case; 10] {
    [
        Case {
            id: "wing_snare",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            legal: "windreaper_falcon",
            illegal: "grizzly_bears",
        },
        Case {
            id: "plummet",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            legal: "windreaper_falcon",
            illegal: "grizzly_bears",
        },
        Case {
            id: "vanquish_the_weak",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "juggernaut",
        },
        Case {
            id: "reave_soul",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "juggernaut",
        },
        Case {
            id: "sungold_barrage",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            legal: "gavony_unhallowed",
            illegal: "grizzly_bears",
        },
        Case {
            id: "collar_the_culprit",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            legal: "gavony_unhallowed",
            illegal: "grizzly_bears",
        },
        Case {
            id: "ghostly_visit",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "black_knight",
        },
        Case {
            id: "hand_of_death",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "black_knight",
        },
        Case {
            id: "flesh_to_dust",
            mana: ManaGift {
                b: 2,
                c: 3,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "bonesplitter",
        },
        Case {
            id: "terminate",
            mana: ManaGift {
                b: 1,
                r: 1,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "bonesplitter",
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("forest", &[])]);
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

fn inject_case_creature(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    match card {
        "juggernaut" => inject_creature_with_stats(e, player, card, 5, 3),
        "gavony_unhallowed" => inject_creature_with_stats(e, player, card, 2, 4),
        _ => inject_creature_on_battlefield(e, player, card),
    }
}

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) -> Result<u32, String> {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .map(|_| e.state.stack.last().expect("spell").id)
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn q30_destroy_creature_filtered_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(30_020_001);
    let target = inject_case_creature(&mut e, 1, case.legal);
    cast_at(&mut e, case, target).expect("cast Wing Snare");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn q30_destroy_creature_filtered_canary_hardest() {
    let case = &cases()[9];
    let mut e = engine(30_020_002);
    let target = inject_case_creature(&mut e, 1, case.legal);
    cast_at(&mut e, case, target).expect("cast Terminate");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn q30_destroy_creature_filtered_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_021_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, case.legal);
        cast_at(&mut e, &case, target).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        let pool = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed cost",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
    }
}

#[test]
fn q30_destroy_creature_filtered_cards_destroy_only_matching_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_022_000 + index as u64);
        let legal = inject_case_creature(&mut e, 1, case.legal);
        let bystander = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_at(&mut e, &case, legal).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&legal].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(
            e.state.objects[&bystander].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q30_destroy_creature_filtered_cards_reject_illegal_targets_without_spending_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_023_000 + index as u64);
        let illegal = if case.illegal == "bonesplitter" {
            inject_permanent_on_battlefield(&mut e, 1, case.illegal)
        } else {
            inject_case_creature(&mut e, 1, case.illegal)
        };
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = {
            let pool = &e.state.players[0].mana_pool;
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless,
            )
        };
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(illegal)))
                .is_err(),
            "{}: illegal target rejected",
            case.id
        );
        let pool = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            before,
            "{}: rejected cast spends no mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&illegal].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q30_destroy_creature_filtered_cards_fizzle_on_stale_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_024_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, case.legal);
        let spell = cast_at(&mut e, &case, target).expect("cast at the creature");
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("creature").zone = Zone::Exile;
        e.state.players[1].exile.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{}", case.id);
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        assert!(e.state.players[0].graveyard.contains(&spell), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q30_destroy_creature_filtered_cards_recheck_filters_on_resolution() {
    let mut cases_to_check = Vec::new();
    for case in cases() {
        if matches!(
            case.id,
            "vanquish_the_weak" | "sungold_barrage" | "wing_snare" | "ghostly_visit"
        ) {
            cases_to_check.push(case);
        }
    }
    for (index, case) in cases_to_check.into_iter().enumerate() {
        let mut e = engine(30_026_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, case.legal);
        let spell = cast_at(&mut e, &case, target).expect("cast at the matching creature");
        match case.id {
            "vanquish_the_weak" => {
                e.state.objects.get_mut(&target).expect("creature").power = Some(4);
            }
            "sungold_barrage" => {
                e.state
                    .objects
                    .get_mut(&target)
                    .expect("creature")
                    .toughness = Some(3);
            }
            "wing_snare" => {
                e.state.objects.get_mut(&target).expect("creature").card_id =
                    "grizzly_bears".into();
            }
            "ghostly_visit" => {
                e.state.objects.get_mut(&target).expect("creature").card_id = "black_knight".into();
            }
            _ => unreachable!(),
        }
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}: a target that stopped matching the filter is not destroyed",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the spell fizzled",
            case.id
        );
    }
}

#[test]
fn q30_destroy_creature_filtered_cards_prevent_regeneration_where_printed() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_025_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, case.legal);
        e.state
            .objects
            .get_mut(&target)
            .expect("creature")
            .regeneration_shields = 1;
        cast_at(&mut e, &case, target).expect("cast the removal");
        resolve_entire_stack_two_player(&mut e);
        if matches!(case.id, "flesh_to_dust" | "terminate") {
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Graveyard,
                "{}: regeneration cannot replace this destruction",
                case.id
            );
        } else {
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Battlefield,
                "{}: a regeneration shield replaces ordinary destruction",
                case.id
            );
        }
    }
}
