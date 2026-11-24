//! Actual-card scenarios for the queue-v77 01-target-quality-removal cohort.
//!
//! Expectations are printed Oracle values. CR 601.2c makes the printed characteristic filter part
//! of target legality, CR 701.8 destroys the chosen creature, and CR 608.2b rechecks the target on
//! resolution. Every card is mono-black, so the exact printed mana cost is the only cost paid.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    legal: &'static str,
    illegal: &'static str,
    illegal_tapped: bool,
    recheck_to: &'static str,
    recheck_tap: bool,
}

fn cases() -> [Case; 10] {
    [
        Case {
            id: "rend_flesh",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "wanderlight_spirit",
            illegal_tapped: false,
            recheck_to: "wanderlight_spirit",
            recheck_tap: false,
        },
        Case {
            id: "asphyxiate",
            mana: ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "grizzly_bears",
            illegal_tapped: true,
            recheck_to: "grizzly_bears",
            recheck_tap: true,
        },
        Case {
            id: "heros_demise",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            legal: "wan_shi_tong,_librarian",
            illegal: "grizzly_bears",
            illegal_tapped: false,
            recheck_to: "grizzly_bears",
            recheck_tap: false,
        },
        Case {
            id: "walk_the_plank",
            mana: ManaGift {
                b: 2,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "scroll_thief",
            illegal_tapped: false,
            recheck_to: "scroll_thief",
            recheck_tap: false,
        },
        Case {
            id: "chill_to_the_bone",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "ohran_frostfang",
            illegal_tapped: false,
            recheck_to: "ohran_frostfang",
            recheck_tap: false,
        },
        Case {
            id: "human_frailty",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            legal: "black_knight",
            illegal: "grizzly_bears",
            illegal_tapped: false,
            recheck_to: "grizzly_bears",
            recheck_tap: false,
        },
        Case {
            id: "cast_down",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "wan_shi_tong,_librarian",
            illegal_tapped: false,
            recheck_to: "wan_shi_tong,_librarian",
            recheck_tap: false,
        },
        Case {
            id: "dark_betrayal",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            legal: "black_knight",
            illegal: "grizzly_bears",
            illegal_tapped: false,
            recheck_to: "grizzly_bears",
            recheck_tap: false,
        },
        Case {
            id: "victim_of_night",
            mana: ManaGift {
                b: 2,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "stromkirk_mentor",
            illegal_tapped: false,
            recheck_to: "stromkirk_mentor",
            recheck_tap: false,
        },
        Case {
            id: "power_word_kill",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "angel_of_light",
            illegal_tapped: false,
            recheck_to: "angel_of_light",
            recheck_tap: false,
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("swamp", &[])]);
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

fn inject_case_creature(e: &mut GameEngine, player: usize, card: &str, tapped: bool) -> u32 {
    let id = inject_creature_on_battlefield(e, player, card);
    if tapped {
        e.state.objects.get_mut(&id).expect("creature").tapped = true;
    }
    id
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
fn q77_quality_removal_canary_simplest_dark_betrayal_destroys_a_black_creature() {
    let case = &cases()[7];
    let mut e = engine(77_100_001);
    let target = inject_case_creature(&mut e, 1, case.legal, false);
    assert_eq!(
        e.state.objects[&target].card_id.as_str(),
        "black_knight",
        "the canary uses a genuinely black creature"
    );
    cast_at(&mut e, case, target).expect("cast Dark Betrayal");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed {{B}} cost");
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Battlefield,
        "the creature is destroyed only on resolution"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q77_quality_removal_canary_hardest_power_word_kill_rejects_each_excluded_type() {
    let case = &cases()[9];
    for excluded in ["angel_of_light", "soulcage_fiend", "black_dragon"] {
        let mut e = engine(77_100_010);
        let target = inject_case_creature(&mut e, 1, excluded, false);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
                .is_err(),
            "{excluded}: an excluded creature type is not a legal target"
        );
        assert_eq!(pool(&e), before, "{excluded}: rejected cast spends no mana");
        assert!(e.state.stack.is_empty(), "{excluded}");
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{excluded}"
        );
    }

    let mut e = engine(77_100_011);
    let target = inject_case_creature(&mut e, 1, case.legal, false);
    cast_at(&mut e, case, target).expect("cast at a non-excluded creature");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn q77_quality_removal_cards_cast_with_exact_costs_and_destroy_matching_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(77_101_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, case.legal, false);
        cast_at(&mut e, &case, target).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        assert_eq!(
            pool(&e),
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
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q77_quality_removal_cards_destroy_only_the_chosen_matching_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(77_102_000 + index as u64);
        let legal = inject_case_creature(&mut e, 1, case.legal, false);
        let bystander = if case.legal == "grizzly_bears" {
            inject_creature_on_battlefield(&mut e, 1, "juggernaut")
        } else {
            inject_creature_on_battlefield(&mut e, 1, "grizzly_bears")
        };
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
    }
}

#[test]
fn q77_quality_removal_cards_reject_illegal_targets_without_spending_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(77_103_000 + index as u64);
        let illegal = inject_case_creature(&mut e, 1, case.illegal, case.illegal_tapped);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(illegal)))
                .is_err(),
            "{}: illegal target rejected",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected cast spends no mana",
            case.id
        );
        assert!(
            e.state.objects[&illegal].zone == Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q77_quality_removal_cards_recheck_filters_on_resolution() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(77_104_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, case.legal, false);
        let spell = cast_at(&mut e, &case, target).expect("cast at the matching creature");
        if case.recheck_tap {
            e.state.objects.get_mut(&target).expect("creature").tapped = true;
        } else {
            e.state.objects.get_mut(&target).expect("creature").card_id = case.recheck_to.into();
        }
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}: a target that stopped matching is not destroyed",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the spell fizzled",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q77_quality_removal_cards_fizzle_on_stale_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(77_105_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, case.legal, false);
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
