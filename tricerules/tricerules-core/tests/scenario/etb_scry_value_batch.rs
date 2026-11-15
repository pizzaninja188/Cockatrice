//! Actual-card scenarios for the reviewed enters-scry value cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a makes the enters ability
//! trigger; CR 701.22a defines Scry N. Each card scries exactly its printed
//! count and the choice belongs only to the entering creature's controller.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    count: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "chrome_cat",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            body: (3, 2),
            count: 1,
        },
        Case {
            id: "galadhrim_guide",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            body: (3, 4),
            count: 2,
        },
        Case {
            id: "rumbling_sentry",
            mana: ManaGift {
                w: 2,
                c: 3,
                ..Default::default()
            },
            body: (3, 6),
            count: 1,
        },
        Case {
            id: "sages_row_savant",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 1),
            count: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("island", &[card])]);
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

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first pass");
    engine
        .apply_scenario_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn cast_and_park_scry(
    e: &mut GameEngine,
    player: usize,
    case: &Case,
    top: &[u32],
) -> ResolutionChoiceRequired {
    give_mana(e, player as i32, gift(&case.mana));
    let slot = inject_card_into_hand(e, player, case.id);
    let hand_slot = e.state.players[player]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(player as i32, &cast_spell(hand_slot, vec![]))
        .expect("cast the creature");
    let pool = &e.state.players[player].mana_pool;
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
        "{}: exact cast cost",
        case.id
    );
    let _spell_resolution = resolve_top_stack(e);
    let creature = battlefield_object_for_card(e, player, case.id);
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (Some(case.body.0), Some(case.body.1)),
        "{}: printed body",
        case.id
    );
    let resolution = resolve_top_stack(e);
    let choice =
        find_resolution_choice(&resolution).expect("the enters trigger parks a scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop, "{}", case.id);
    assert_eq!(
        choice.deciding_player_id, e.state.players[player].id,
        "{}: the entering controller scries",
        case.id
    );
    assert_eq!(choice.min, 0, "{}", case.id);
    assert_eq!(choice.max, case.count, "{}: printed scry count", case.id);
    assert_eq!(
        choice.candidate_object_ids,
        top[..case.count as usize],
        "{}: only the printed top cards are offered",
        case.id
    );
    choice
}

#[test]
fn etb_scry_value_batch_cards_cast_and_park_the_exact_scry_choice() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_640_000 + index as u64, case.id);
        let top = seat_on_top(
            &mut e,
            0,
            &["storm_crow", "grizzly_bears", "hill_giant", "forest"],
        );
        let _ = cast_and_park_scry(&mut e, 0, &case, &top);
        let library_before = e.state.players[0].library.clone();
        assert!(
            e.apply_scenario_command(
                0,
                &submit_resolution_choice(top[..case.count as usize + 1].to_vec()),
            )
            .is_err(),
            "{}: more than the printed scry count is rejected",
            case.id
        );
        assert_eq!(
            e.state.players[0].library, library_before,
            "{}: rejected choice leaves the library unchanged",
            case.id
        );
        assert!(e.state.pending_resolution.is_some(), "{}", case.id);
        e.apply_scenario_command(
            0,
            &submit_resolution_choice(top[..case.count as usize].to_vec()),
        )
        .expect("complete the scry choice");
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        assert_eq!(
            e.state.players[0].library.len(),
            library_before.len(),
            "{}: scry moves no card",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.back(),
            Some(&top[case.count as usize - 1]),
            "{}: the chosen cards move to the bottom in order",
            case.id
        );
    }
}

#[test]
fn etb_scry_value_batch_cards_scope_the_choice_to_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_641_000 + index as u64, case.id);
        let top = seat_on_top(
            &mut e,
            1,
            &["storm_crow", "grizzly_bears", "hill_giant", "forest"],
        );
        move_ready_to_battlefield(&mut e, 1, case.id);
        let resolution = resolve_top_stack(&mut e);
        let choice =
            find_resolution_choice(&resolution).expect("the enters trigger parks a scry choice");
        assert_eq!(
            choice.choice_kind(),
            tricerules_proto::ruled::v1::ChoiceKind::LibraryTop,
            "{}",
            case.id
        );
        assert_eq!(
            choice.deciding_player_id, e.state.players[1].id,
            "{}: the entering controller scries",
            case.id
        );
        assert_eq!(
            choice.candidate_object_ids,
            top[..case.count as usize],
            "{}: the opponent's top cards are the candidates",
            case.id
        );
        e.apply_scenario_command(
            1,
            &submit_resolution_choice(top[..case.count as usize].to_vec()),
        )
        .expect("the controller completes the choice");
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn etb_scry_value_batch_cards_leave_no_scry_choice_on_the_opponent_batch() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_642_000 + index as u64, case.id);
        let top = seat_on_top(
            &mut e,
            0,
            &["storm_crow", "grizzly_bears", "hill_giant", "forest"],
        );
        let _ = cast_and_park_scry(&mut e, 0, &case, &top);
        e.apply_scenario_command(
            0,
            &submit_resolution_choice(top[..case.count as usize].to_vec()),
        )
        .expect("complete the scry choice");
        // With the resolution finished, the opponent's own priority batch carries no
        // library choice and no scried card identities.
        assert_eq!(
            e.state.priority_player_id(),
            e.state.players[0].id,
            "{}",
            case.id
        );
        e.apply_scenario_command(0, &pass()).expect("active passes");
        let opponent_batch = e.initial_response_batch();
        assert!(
            find_resolution_choice(&opponent_batch).is_none(),
            "{}: the opponent receives no scry choice",
            case.id
        );
    }
}
