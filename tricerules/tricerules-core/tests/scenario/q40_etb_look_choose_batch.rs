//! Actual-card scenarios for the queue-v40 enters-the-battlefield top-library cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the entry trigger on the stack;
//! CR 121.5 makes the look not a draw; CR 401.4 keeps the looked cards private; and CR 609.3
//! clamps the bounded selection to the available matches.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    count: usize,
    library: [&'static str; 4],
    selectable: [bool; 4],
    random: bool,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "courageous_outrider",
            count: 4,
            library: ["monk_realist", "grizzly_bears", "forest", "lightning_bolt"],
            selectable: [true, false, false, false],
            random: false,
        },
        Case {
            id: "eclipsed_flamekin",
            count: 4,
            library: ["plasma_elemental", "grizzly_bears", "island", "mountain"],
            selectable: [true, false, true, true],
            random: true,
        },
        Case {
            id: "sumala_woodshaper",
            count: 4,
            library: ["grizzly_bears", "pacifism", "forest", "lightning_bolt"],
            selectable: [true, true, false, false],
            random: true,
        },
        Case {
            id: "augur_of_bolas",
            count: 3,
            library: ["lightning_bolt", "grizzly_bears", "divination", "forest"],
            selectable: [true, false, true, false],
            random: false,
        },
        Case {
            id: "eclipsed_kithkin",
            count: 4,
            library: ["amrou_kithkin", "grizzly_bears", "plains", "forest"],
            selectable: [true, false, true, true],
            random: true,
        },
    ]
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

fn seat_on_top(e: &mut GameEngine, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, 0, card_id))
        .collect();
    e.state.players[0].library.retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        e.state.players[0].library.push_front(oid);
    }
    oids
}

fn keep_only(e: &mut GameEngine, top: &[u32]) {
    let surplus: Vec<u32> = e.state.players[0]
        .library
        .iter()
        .copied()
        .filter(|oid| !top.contains(oid))
        .collect();
    for oid in surplus {
        e.state.objects.remove(&oid);
    }
    e.state.players[0].library.retain(|oid| top.contains(oid));
}

fn resolve_entry(e: &mut GameEngine, card: &str) -> tricerules_proto::ruled::v1::RuledEventBatch {
    move_ready_to_battlefield(e, 0, card);
    answer_trigger_order_in_engine_order(e);
    assert!(
        e.state.pending_trigger_order.is_none(),
        "{card}: trigger order answered"
    );
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    e.apply_scenario_command(1, &pass())
        .expect("opponent passes and the entry trigger resolves")
}

#[test]
fn q40_etb_look_choose_canary_simplest() {
    let case = &cases()[3];
    let mut e = engine(40_020_001, case.id);
    let top = seat_on_top(&mut e, &case.library[..3]);
    let hand_before = e.state.players[0].hand.len();
    let batch = resolve_entry(&mut e, case.id);
    let choice = find_resolution_choice(&batch).expect("look choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, top);
    assert_eq!(choice.candidate_selectable, [true, false, true]);
    assert_eq!((choice.min, choice.max), (0, 1));
    let after = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("choose the instant");
    assert_eq!(e.state.objects[&top[0]].zone, Zone::Hand);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
    let order = find_resolution_choice(&after).expect("bottom ordering choice");
    assert_eq!(order.candidate_object_ids, vec![top[1], top[2]]);
    assert!(order.ordered);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[1], top[2]]))
        .expect("order the remainder");
    assert_eq!(e.state.objects[&top[1]].zone, Zone::Library);
    assert_eq!(e.state.objects[&top[2]].zone, Zone::Library);
}

#[test]
fn q40_etb_look_choose_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(40_020_002, case.id);
    let top = seat_on_top(&mut e, &case.library);
    let batch = resolve_entry(&mut e, case.id);
    let choice = find_resolution_choice(&batch).expect("look choice");
    let hand_pending = e.state.players[0].hand.len();
    assert_eq!(choice.candidate_object_ids, top);
    assert_eq!(choice.candidate_selectable, [true, true, false, false]);
    let after = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[1]]))
        .expect("choose the enchantment");
    assert_eq!(e.state.objects[&top[1]].zone, Zone::Hand);
    assert_eq!(e.state.players[0].hand.len(), hand_pending + 1);
    assert!(
        find_resolution_choice(&after).is_none(),
        "random bottom order needs no player ordering choice"
    );
    for oid in &top {
        if *oid != top[1] {
            assert_eq!(
                e.state.objects[oid].zone,
                Zone::Library,
                "unselected card stays"
            );
        }
    }
}

#[test]
fn q40_etb_look_choose_cards_apply_their_printed_filter_and_order_policy() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(40_021_000 + index as u64, case.id);
        let top = seat_on_top(&mut e, &case.library);
        let batch = resolve_entry(&mut e, case.id);
        let choice = find_resolution_choice(&batch).expect("look choice");
        let hand_pending = e.state.players[0].hand.len();
        assert_eq!(
            choice.candidate_object_ids,
            top[..case.count],
            "{}",
            case.id
        );
        assert_eq!(
            choice.candidate_selectable,
            case.selectable[..case.count],
            "{}: printed filter",
            case.id
        );
        let selected: Vec<u32> = top[..case.count]
            .iter()
            .enumerate()
            .filter(|(i, _)| case.selectable[*i])
            .map(|(_, oid)| *oid)
            .take(1)
            .collect();
        let after = e
            .apply_scenario_command(0, &submit_resolution_choice(selected.clone()))
            .expect("select at most one matching card");
        for oid in &selected {
            assert_eq!(e.state.objects[oid].zone, Zone::Hand, "{}", case.id);
        }
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_pending + selected.len(),
            "{}: the look is not a draw",
            case.id
        );
        if case.random {
            assert!(
                find_resolution_choice(&after).is_none(),
                "{}: random bottom order is not a player choice",
                case.id
            );
        } else {
            let remainder: Vec<u32> = top[..case.count]
                .iter()
                .copied()
                .filter(|oid| !selected.contains(oid))
                .collect();
            let order = find_resolution_choice(&after).expect("chosen bottom order");
            assert_eq!(order.candidate_object_ids, remainder, "{}", case.id);
            assert!(order.ordered, "{}", case.id);
            e.apply_scenario_command(0, &submit_resolution_choice(remainder))
                .expect("order the remainder");
        }
    }
}

#[test]
fn q40_etb_look_choose_cards_choose_nothing_and_keep_all_cards() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(40_022_000 + index as u64, case.id);
        let top = seat_on_top(&mut e, &case.library);
        let batch = resolve_entry(&mut e, case.id);
        find_resolution_choice(&batch).expect("look choice");
        let hand_pending = e.state.players[0].hand.len();
        let after = e
            .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
            .expect("choose no card");
        assert_eq!(e.state.players[0].hand.len(), hand_pending, "{}", case.id);
        if case.random {
            assert!(find_resolution_choice(&after).is_none(), "{}", case.id);
        } else {
            let order = find_resolution_choice(&after).expect("bottom order");
            assert_eq!(order.candidate_object_ids, top[..case.count], "{}", case.id);
            e.apply_scenario_command(
                0,
                &submit_resolution_choice(order.candidate_object_ids.clone()),
            )
            .expect("order the remainder");
        }
        for oid in &top[..case.count] {
            assert_eq!(e.state.objects[oid].zone, Zone::Library, "{}", case.id);
        }
    }
}

#[test]
fn q40_etb_look_choose_short_library_looks_at_the_remaining_cards() {
    let case = &cases()[3];
    let mut e = engine(40_023_001, case.id);
    ensure_in_hand(&mut e, 0, case.id);
    let top = seat_on_top(&mut e, &["lightning_bolt", "divination"]);
    keep_only(&mut e, &top);
    let batch = resolve_entry(&mut e, case.id);
    let choice = find_resolution_choice(&batch).expect("look choice");
    assert_eq!(choice.candidate_object_ids, top);
    assert_eq!(choice.candidate_selectable, [true, true]);
    assert_eq!((choice.min, choice.max), (0, 1));
    let drawn = e.state.turn_history.current.player(0).cards_drawn;
    let after = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[1]]))
        .expect("choose the sorcery");
    if let Some(order) = find_resolution_choice(&after) {
        e.apply_scenario_command(0, &submit_resolution_choice(order.candidate_object_ids))
            .expect("order the remainder");
    }
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        drawn,
        "no draw from a short look"
    );
}

#[test]
fn q40_etb_look_choose_no_matching_card_keeps_every_card() {
    let case = &cases()[3];
    let mut e = engine(40_024_001, case.id);
    ensure_in_hand(&mut e, 0, case.id);
    let top = seat_on_top(&mut e, &["forest", "island", "mountain"]);
    keep_only(&mut e, &top);
    let batch = resolve_entry(&mut e, case.id);
    let choice = find_resolution_choice(&batch).expect("look choice");
    let hand_pending = e.state.players[0].hand.len();
    assert_eq!(choice.candidate_object_ids, top);
    assert_eq!(choice.candidate_selectable, [false, false, false]);
    assert_eq!((choice.min, choice.max), (0, 0));
    let after = e
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("no instant or sorcery is eligible");
    assert_eq!(e.state.players[0].hand.len(), hand_pending);
    let order = find_resolution_choice(&after).expect("bottom order for the lands");
    assert_eq!(order.candidate_object_ids, top);
    e.apply_scenario_command(0, &submit_resolution_choice(top.to_vec()))
        .expect("order the remainder");
    for oid in top {
        assert_eq!(e.state.objects[&oid].zone, Zone::Library);
    }
}
