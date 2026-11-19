//! Actual-card scenarios for the queue-v37 cycling fetch-land cohort.
//!
//! Expectations are printed Oracle values. CR 305.6/605.1a make the {T} mana ability
//! immediate; CR 602.1 pays all costs on activation; CR 701.23b/e covers the hidden-zone
//! search; CR 701.24a requires the shuffle; CR 614.1c taps the found basic land; and
//! CR 702.29a defines Cycling as a hand ability paid with the printed mana and discard.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, ChoiceKind, RuledCommand,
};

struct Case {
    id: &'static str,
    cycling: ManaGift,
    cost: &'static str,
    allowed: [&'static str; 3],
    disallowed: &'static str,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "bountiful_landscape",
            cycling: ManaGift {
                g: 1,
                u: 1,
                r: 1,
                ..Default::default()
            },
            cost: "{G}{U}{R}",
            allowed: ["forest", "island", "mountain"],
            disallowed: "swamp",
        },
        Case {
            id: "contaminated_landscape",
            cycling: ManaGift {
                w: 1,
                u: 1,
                b: 1,
                ..Default::default()
            },
            cost: "{W}{U}{B}",
            allowed: ["plains", "island", "swamp"],
            disallowed: "mountain",
        },
        Case {
            id: "deceptive_landscape",
            cycling: ManaGift {
                w: 1,
                b: 1,
                g: 1,
                ..Default::default()
            },
            cost: "{W}{B}{G}",
            allowed: ["plains", "swamp", "forest"],
            disallowed: "island",
        },
        Case {
            id: "foreboding_landscape",
            cycling: ManaGift {
                b: 1,
                g: 1,
                u: 1,
                ..Default::default()
            },
            cost: "{B}{G}{U}",
            allowed: ["swamp", "forest", "island"],
            disallowed: "plains",
        },
        Case {
            id: "perilous_landscape",
            cycling: ManaGift {
                u: 1,
                r: 1,
                w: 1,
                ..Default::default()
            },
            cost: "{U}{R}{W}",
            allowed: ["island", "mountain", "plains"],
            disallowed: "forest",
        },
        Case {
            id: "seething_landscape",
            cycling: ManaGift {
                u: 1,
                b: 1,
                r: 1,
                ..Default::default()
            },
            cost: "{U}{B}{R}",
            allowed: ["island", "swamp", "mountain"],
            disallowed: "plains",
        },
        Case {
            id: "shattered_landscape",
            cycling: ManaGift {
                r: 1,
                w: 1,
                b: 1,
                ..Default::default()
            },
            cost: "{R}{W}{B}",
            allowed: ["mountain", "plains", "swamp"],
            disallowed: "forest",
        },
        Case {
            id: "sheltering_landscape",
            cycling: ManaGift {
                r: 1,
                g: 1,
                w: 1,
                ..Default::default()
            },
            cost: "{R}{G}{W}",
            allowed: ["mountain", "forest", "plains"],
            disallowed: "island",
        },
        Case {
            id: "tranquil_landscape",
            cycling: ManaGift {
                g: 1,
                w: 1,
                u: 1,
                ..Default::default()
            },
            cost: "{G}{W}{U}",
            allowed: ["forest", "plains", "island"],
            disallowed: "swamp",
        },
        Case {
            id: "twisted_landscape",
            cycling: ManaGift {
                b: 1,
                r: 1,
                g: 1,
                ..Default::default()
            },
            cost: "{B}{R}{G}",
            allowed: ["swamp", "mountain", "forest"],
            disallowed: "plains",
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn hand_ability(e: &GameEngine, source: u32, index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: index,
            ..Default::default()
        })),
    }
}

fn resolve_stack_for_choice(e: &mut GameEngine) -> RuledEventBatch {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    e.apply_scenario_command(first, &pass())
        .expect("first pass");
    e.apply_scenario_command(second, &pass())
        .expect("second pass resolves")
}

fn activate_fetch(e: &mut GameEngine, land: u32) -> Vec<u32> {
    e.apply_scenario_command(0, &activate_ability_for(e, land, 1, vec![]))
        .expect("activate the fetch ability");
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    let batch = resolve_stack_for_choice(e);
    let choice = find_resolution_choice(&batch).expect("basic-land search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    choice.candidate_object_ids.clone()
}

#[test]
fn q37_cycling_fetch_lands_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(37_020_001, case.id);
    ensure_in_hand(&mut e, 0, case.id);
    let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
    let drawn = inject_library_card(&mut e, 0, "grizzly_bears");
    e.state.players[0].library.retain(|o| *o != drawn);
    e.state.players[0].library.push_front(drawn);
    give_mana(&mut e, 0, gift(&case.cycling));
    e.apply_scenario_command(0, &hand_ability(&e, card, 2))
        .expect("activate Cycling");
    assert_eq!(e.state.objects[&card].zone, Zone::Graveyard);
    assert_eq!(
        pool(&e, 0),
        (0, 0, 0, 0, 0, 0),
        "exact printed cycling cost"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&drawn].zone,
        Zone::Hand,
        "cycling draws one"
    );
}

#[test]
fn q37_cycling_fetch_lands_canary_hardest() {
    let case = &cases()[9];
    let mut e = engine(37_020_002, case.id);
    let land = move_ready_to_battlefield(&mut e, 0, case.id);
    let forest = inject_library_card(&mut e, 0, "forest");
    let plains = inject_library_card(&mut e, 0, case.disallowed);
    let candidates = activate_fetch(&mut e, land);
    assert!(candidates.contains(&forest));
    assert!(!candidates.contains(&plains));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![forest]))
        .expect("find the forest");
    assert_eq!(e.state.objects[&forest].zone, Zone::Battlefield);
    assert!(e.state.objects[&forest].tapped);
    assert_eq!(e.state.objects[&plains].zone, Zone::Library);
}

#[test]
fn q37_cycling_fetch_lands_cards_tap_for_colorless_and_fetch_only_printed_types() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_021_000 + index as u64, case.id);
        let land = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
            .expect("tap for colorless");
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 1), "{}", case.id);
        assert!(e.state.stack.is_empty());
        e.apply_scenario_command(0, &undo_mana_ability())
            .expect("undo");
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(!e.state.objects[&land].tapped);

        let mut good_ids = Vec::new();
        for good in case.allowed {
            good_ids.push(inject_library_card(&mut e, 0, good));
        }
        let bad = inject_library_card(&mut e, 0, case.disallowed);
        let candidates = activate_fetch(&mut e, land);
        for good in good_ids {
            assert!(
                candidates.contains(&good),
                "{}: allowed basic offered",
                case.id
            );
        }
        assert!(
            !candidates.contains(&bad),
            "{}: off-color excluded",
            case.id
        );
    }
}

#[test]
fn q37_cycling_fetch_lands_cards_cycle_for_the_printed_cost_and_draw_one() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_022_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
        let drawn = inject_library_card(&mut e, 0, "grizzly_bears");
        e.state.players[0].library.retain(|o| *o != drawn);
        e.state.players[0].library.push_front(drawn);
        give_mana(&mut e, 0, gift(&case.cycling));
        e.apply_scenario_command(0, &hand_ability(&e, card, 2))
            .expect("activate Cycling");
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        assert_eq!(e.state.objects[&card].zone, Zone::Graveyard);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{}", case.id);
        assert_eq!(case.cost.matches('{').count(), 3, "{}", case.id);
    }
}

#[test]
fn q37_cycling_fetch_lands_cards_reject_short_mana_and_tapped_fetch() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_023_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &hand_ability(&e, card, 2))
            .expect_err("cycling needs the printed three colored mana");
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{}", case.id);

        let mut tapped = engine(37_024_000 + index as u64, case.id);
        let land = move_ready_to_battlefield(&mut tapped, 0, case.id);
        tapped.state.objects.get_mut(&land).expect("land").tapped = true;
        tapped
            .apply_scenario_command(0, &activate_ability_for(&tapped, land, 1, vec![]))
            .expect_err("a tapped land cannot pay {T}");
        assert_eq!(tapped.state.objects[&land].zone, Zone::Battlefield);
    }
}
