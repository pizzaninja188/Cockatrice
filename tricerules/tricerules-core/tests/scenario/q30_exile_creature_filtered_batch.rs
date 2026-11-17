//! Actual-card scenarios for the queue-v30 filtered creature exile cohort.
//!
//! Expectations are printed Oracle values. CR 701.13 exiles the chosen creature;
//! the printed power or tapped restriction is target legality (CR 115.1/601.2c)
//! and is rechecked on resolution (CR 608.2b).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    legal: &'static str,
    legal_tapped: bool,
    illegal: &'static str,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "reaver_ambush",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            legal: "grizzly_bears",
            legal_tapped: false,
            illegal: "juggernaut",
        },
        Case {
            id: "grotesque_demise",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            legal: "grizzly_bears",
            legal_tapped: false,
            illegal: "juggernaut",
        },
        Case {
            id: "bring_to_trial",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            legal: "juggernaut",
            legal_tapped: false,
            illegal: "grizzly_bears",
        },
        Case {
            id: "blade_banish",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            legal: "juggernaut",
            legal_tapped: false,
            illegal: "grizzly_bears",
        },
        Case {
            id: "expel",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            legal: "grizzly_bears",
            legal_tapped: true,
            illegal: "grizzly_bears",
        },
        Case {
            id: "excoriate",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            legal: "grizzly_bears",
            legal_tapped: true,
            illegal: "grizzly_bears",
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
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
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
fn q30_exile_creature_filtered_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(30_090_001);
    let target = inject_case_creature(&mut e, 1, case.legal);
    cast_at(&mut e, case, target).expect("cast Reaver Ambush");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Exile);
}

#[test]
fn q30_exile_creature_filtered_canary_hardest() {
    let case = &cases()[5];
    let mut e = engine(30_090_002);
    let target = inject_case_creature(&mut e, 1, case.legal);
    e.state.objects.get_mut(&target).expect("creature").tapped = true;
    cast_at(&mut e, case, target).expect("cast Excoriate");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Exile);
}

#[test]
fn q30_exile_creature_filtered_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_091_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, case.legal);
        if case.legal_tapped {
            e.state.objects.get_mut(&target).expect("creature").tapped = true;
        }
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
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{}", case.id);
    }
}

#[test]
fn q30_exile_creature_filtered_cards_exile_only_matching_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_092_000 + index as u64);
        let legal = inject_case_creature(&mut e, 1, case.legal);
        if case.legal_tapped {
            e.state.objects.get_mut(&legal).expect("creature").tapped = true;
        }
        let bystander = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_at(&mut e, &case, legal).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&legal].zone, Zone::Exile, "{}", case.id);
        assert!(e.state.players[1].exile.contains(&legal), "{}", case.id);
        assert_eq!(
            e.state.objects[&bystander].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn q30_exile_creature_filtered_cards_reject_illegal_targets_without_spending_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_093_000 + index as u64);
        let illegal = inject_case_creature(&mut e, 1, case.illegal);
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
fn q30_exile_creature_filtered_cards_recheck_filters_on_resolution() {
    for (index, id) in ["reaver_ambush", "bring_to_trial", "expel"]
        .into_iter()
        .enumerate()
    {
        let case = cases()
            .into_iter()
            .find(|case| case.id == id)
            .expect("filtered case");
        let mut e = engine(30_095_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, case.legal);
        if case.legal_tapped {
            e.state.objects.get_mut(&target).expect("creature").tapped = true;
        }
        let spell = cast_at(&mut e, &case, target).expect("cast at the matching creature");
        match id {
            "reaver_ambush" => {
                e.state.objects.get_mut(&target).expect("creature").power = Some(4);
            }
            "bring_to_trial" => {
                e.state.objects.get_mut(&target).expect("creature").power = Some(3);
            }
            "expel" => {
                e.state.objects.get_mut(&target).expect("creature").tapped = false;
            }
            _ => unreachable!(),
        }
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}: a target that stopped matching the filter is not exiled",
            id
        );
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", id);
    }
}

#[test]
fn q30_exile_creature_filtered_cards_fizzle_on_stale_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_094_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, case.legal);
        if case.legal_tapped {
            e.state.objects.get_mut(&target).expect("creature").tapped = true;
        }
        let spell = cast_at(&mut e, &case, target).expect("cast at the creature");
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("creature").zone = Zone::Graveyard;
        e.state.players[1].graveyard.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        assert!(e.state.players[0].graveyard.contains(&spell), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
