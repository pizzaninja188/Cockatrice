//! Actual-card scenarios for the queue-v36 pump-and-scry cohort.
//!
//! Expectations are printed Oracle values. CR 115.1a/601.2c choose the target as the
//! spell is cast; CR 613.4c applies the pump; CR 701.22a-b govern the printed scry;
//! CR 514.2 ends the pump; CR 608.2b fizzles the spell with no scry when the only
//! target is illegal at resolution.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: i32,
    toughness: i32,
    scry: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "battlewise_valor",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            scry: 1,
        },
        Case {
            id: "ferocious_charge",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 4,
            toughness: 4,
            scry: 2,
        },
        Case {
            id: "inordinate_rage",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 2,
            scry: 1,
        },
        Case {
            id: "stand_firm",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            scry: 2,
        },
        Case {
            id: "titans_strength",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 1,
            scry: 1,
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
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
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

fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect::<Vec<_>>();
    e.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        e.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn resolve_top_stack(e: &mut GameEngine) -> RuledEventBatch {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    e.apply_scenario_command(first, &pass())
        .expect("first pass");
    e.apply_scenario_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn cast_at(e: &mut GameEngine, case: &Case, targets: Vec<TargetRef>) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    let card = e.state.players[0].hand[slot];
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    card
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

#[test]
fn q36_pump_scry_canary_simplest() {
    let case = &cases()[3];
    let mut e = engine(36_010_001);
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant"]);
    let spell = cast_at(&mut e, case, target_object(bear));
    let parked = resolve_top_stack(&mut e);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(3), Some(3)),
        "Stand Firm pumps the target"
    );
    let choice = find_resolution_choice(&parked).expect("Stand Firm parks its scry");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids, top);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("bottom the first card");
    assert_eq!(
        *e.state.players[0].library.back().expect("library"),
        top[0],
        "the chosen card goes to the bottom"
    );
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(top[1]),
        "the other card stays on top"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q36_pump_scry_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(36_010_002);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant"]);
    let spell = cast_at(&mut e, case, target_object(bear));
    let parked = resolve_top_stack(&mut e);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(6), Some(6)),
        "Ferocious Charge pumps an opponent's creature"
    );
    let choice = find_resolution_choice(&parked).expect("Ferocious Charge parks its scry");
    assert_eq!((choice.min, choice.max), (0, 2));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[1]]))
        .expect("bottom one of the two cards");
    assert_eq!(
        *e.state.players[0].library.back().expect("library"),
        top[1],
        "the chosen card goes to the bottom"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn q36_pump_scry_cards_apply_the_printed_pump_and_scry_count() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_011_000 + index as u64);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
        let top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant", "forest"]);
        cast_at(&mut e, case, target_object(bear));
        let parked = resolve_top_stack(&mut e);
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (
                Some((2 + case.power) as u32),
                Some((2 + case.toughness) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: an untargeted creature is unaffected",
            case.id
        );
        let choice = find_resolution_choice(&parked).expect("scry parks a choice");
        assert_eq!(
            (choice.min, choice.max),
            (0, case.scry),
            "{}: printed scry count",
            case.id
        );
        assert_eq!(
            choice.candidate_object_ids,
            top[..case.scry as usize].to_vec(),
            "{}: the offered cards are the top of the library",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
            .expect("bottom the first scried card");
        assert_eq!(
            *e.state.players[0].library.back().expect("library"),
            top[0],
            "{}: the chosen card goes to the bottom",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(2), Some(2)),
            "{}: the pump ends with the turn",
            case.id
        );
    }
}

#[test]
fn q36_pump_scry_cards_require_a_creature_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_012_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: a creature target is required",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
                .is_err(),
            "{}: a player is not a legal target",
            case.id
        );
        assert_eq!(
            pool(&e),
            (
                case.mana.w,
                case.mana.u,
                case.mana.b,
                case.mana.r,
                case.mana.g,
                case.mana.c
            ),
            "{}: rejected casts do not pay mana",
            case.id
        );
    }
}

#[test]
fn q36_pump_scry_cards_fizzle_without_scrying_when_the_target_is_gone() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_013_000 + index as u64);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant"]);
        let spell = cast_at(&mut e, case, target_object(bear));
        move_to_graveyard(&mut e, 0, bear);
        let parked = resolve_top_stack(&mut e);
        assert!(
            find_resolution_choice(&parked).is_none(),
            "{}: an illegal only target fizzles with no scry",
            case.id
        );
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
        assert_eq!(
            e.state.players[0].library.front().copied(),
            Some(top[0]),
            "{}: the library is unchanged",
            case.id
        );
    }
}
