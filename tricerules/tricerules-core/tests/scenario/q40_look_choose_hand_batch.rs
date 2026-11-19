//! Actual-card scenarios for the queue-v40 top-library look-and-choose cohort.
//!
//! Expectations are printed Oracle values. CR 121.5 makes the look not a draw; CR 401.4 keeps the
//! looked cards private; CR 609.3 clamps a bounded selection to the available matches; and
//! CR 608.2c preserves the printed instruction order.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    mana: (u32, u32, u32, u32, u32, u32),
    count: usize,
    selectable: [bool; 6],
    random: bool,
    gain: i32,
}

fn cases() -> [Case; 10] {
    [
        Case {
            id: "seek_the_wilds",
            mana: (0, 0, 0, 0, 1, 1),
            count: 4,
            selectable: [true, true, false, false, false, false],
            random: false,
            gain: 0,
        },
        Case {
            id: "adventurous_impulse",
            mana: (0, 0, 0, 0, 1, 0),
            count: 3,
            selectable: [true, true, false, false, false, false],
            random: false,
            gain: 0,
        },
        Case {
            id: "commune_with_dinosaurs",
            mana: (0, 0, 0, 0, 1, 0),
            count: 5,
            selectable: [true, true, false, false, false, false],
            random: false,
            gain: 0,
        },
        Case {
            id: "peer_through_depths",
            mana: (0, 1, 0, 0, 0, 1),
            count: 5,
            selectable: [false, false, true, true, false, false],
            random: false,
            gain: 0,
        },
        Case {
            id: "ancient_stirrings",
            mana: (0, 0, 0, 0, 1, 0),
            count: 5,
            selectable: [false, true, false, false, false, true],
            random: false,
            gain: 0,
        },
        Case {
            id: "commune_with_spirits",
            mana: (0, 0, 0, 0, 1, 0),
            count: 4,
            selectable: [false, true, false, true, false, false],
            random: true,
            gain: 0,
        },
        Case {
            id: "commune_with_beavers",
            mana: (0, 0, 0, 0, 1, 0),
            count: 3,
            selectable: [true, true, true, false, false, false],
            random: false,
            gain: 0,
        },
        Case {
            id: "bond_of_flourishing",
            mana: (0, 0, 0, 0, 1, 1),
            count: 3,
            selectable: [true, true, false, true, true, false],
            random: false,
            gain: 3,
        },
        Case {
            id: "arcane_infusion",
            mana: (0, 1, 0, 1, 0, 0),
            count: 4,
            selectable: [false, false, true, true, false, false],
            random: true,
            gain: 0,
        },
        Case {
            id: "basic_conjuration",
            mana: (0, 0, 0, 0, 2, 1),
            count: 6,
            selectable: [true, false, false, false, false, false],
            random: true,
            gain: 3,
        },
    ]
}

fn library_for(id: &str) -> [&'static str; 6] {
    match id {
        "commune_with_dinosaurs" => [
            "ornery_dilophosaur",
            "forest",
            "grizzly_bears",
            "lightning_bolt",
            "pacifism",
            "bone_saw",
        ],
        "commune_with_spirits" => [
            "grizzly_bears",
            "forest",
            "lightning_bolt",
            "pacifism",
            "divination",
            "bone_saw",
        ],
        "commune_with_beavers" => [
            "grizzly_bears",
            "forest",
            "bone_saw",
            "lightning_bolt",
            "divination",
            "pacifism",
        ],
        "bond_of_flourishing" => [
            "bone_saw",
            "pacifism",
            "lightning_bolt",
            "grizzly_bears",
            "forest",
            "divination",
        ],
        _ => [
            "grizzly_bears",
            "forest",
            "lightning_bolt",
            "divination",
            "pacifism",
            "bone_saw",
        ],
    }
}

fn engine(seed: u64, spell: &str, basic: &str) -> GameEngine {
    let decks = Some(vec![deck_with(basic, &[spell]), vec!["forest".into(); 20]]);
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

fn cast_case(e: &mut GameEngine, case: &Case) -> tricerules_proto::ruled::v1::RuledEventBatch {
    ensure_in_hand(e, 0, case.id);
    let (w, u, b, r, g, c) = case.mana;
    give_mana(e, 0, ManaGift { w, u, b, r, g, c });
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    e.apply_scenario_command(0, &pass()).expect("caster passes");
    e.apply_scenario_command(1, &pass())
        .expect("opponent passes and the spell resolves")
}

#[test]
fn q40_look_choose_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(40_010_001, case.id, "forest");
    let top = seat_on_top(&mut e, &library_for(case.id));
    ensure_in_hand(&mut e, 0, case.id);
    let hand_before = e.state.players[0].hand.len();
    let chosen = top[0];
    let batch = cast_case(&mut e, case);
    let choice = find_resolution_choice(&batch).expect("look choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, top[..3]);
    assert_eq!(choice.candidate_selectable, [true, true, false]);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(!choice.ordered);

    let order_batch = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![chosen]))
        .expect("choose the creature card");
    assert_eq!(e.state.objects[&chosen].zone, Zone::Hand);
    let order = find_resolution_choice(&order_batch).expect("bottom ordering choice");
    assert_eq!(order.candidate_object_ids, top[1..3]);
    assert_eq!((order.min, order.max), (2, 2));
    assert!(order.ordered);
    e.apply_scenario_command(0, &submit_resolution_choice(top[1..3].to_vec()))
        .expect("order the remainder");
    assert_eq!(e.state.players[0].library.back(), Some(&top[2]));
    assert_eq!(
        e.state.players[0].library.iter().rev().nth(1),
        Some(&top[1])
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1 + 1,
        "no draw happened"
    );
}

#[test]
fn q40_look_choose_canary_hardest() {
    let case = &cases()[9];
    let mut e = engine(40_010_002, case.id, "forest");
    let top = seat_on_top(&mut e, &library_for(case.id));
    ensure_in_hand(&mut e, 0, case.id);
    let life_before = e.state.players[0].life;
    let library_before = e.state.players[0].library.len();
    let batch = cast_case(&mut e, case);
    let choice = find_resolution_choice(&batch).expect("look choice");
    assert_eq!(choice.candidate_object_ids, top);
    assert_eq!(
        choice.candidate_selectable,
        [true, false, false, false, false, false]
    );
    let after = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("choose the only creature");
    assert_eq!(e.state.objects[&top[0]].zone, Zone::Hand);
    assert!(
        find_resolution_choice(&after).is_none(),
        "random bottom order needs no player ordering choice"
    );
    assert_eq!(e.state.players[0].life, life_before + 3);
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 1,
        "the rest go to the bottom"
    );
}

#[test]
fn q40_look_choose_cards_apply_their_printed_filter_and_order_policy() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(40_011_000 + index as u64, case.id, "forest");
        let top = seat_on_top(&mut e, &library_for(case.id));
        ensure_in_hand(&mut e, 0, case.id);
        let life_before = e.state.players[0].life;
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let batch = cast_case(&mut e, &case);
        let choice = find_resolution_choice(&batch).expect("look choice");
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
            hand_before - 1 + selected.len(),
            "{}: look is not a draw",
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
        assert_eq!(
            e.state.players[0].life,
            life_before + case.gain,
            "{}: printed life gain",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - selected.len(),
            "{}: only the chosen card leaves the library",
            case.id
        );
        for oid in &top[..case.count] {
            if selected.contains(oid) {
                assert_ne!(e.state.objects[oid].zone, Zone::Library, "{}", case.id);
            }
        }
    }
}

#[test]
fn q40_look_choose_requires_the_printed_mana() {
    let case = &cases()[2];
    let mut e = engine(40_012_001, case.id, "forest");
    seat_on_top(&mut e, &library_for(case.id));
    ensure_in_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .is_err(),
        "casting without {{G}} must fail"
    );
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.players[0].life, 20);
}

#[test]
fn q40_look_choose_short_library_does_as_much_as_possible() {
    let case = &cases()[1];
    let mut e = engine(40_013_001, case.id, "forest");
    ensure_in_hand(&mut e, 0, case.id);
    let hand_before = e.state.players[0].hand.len();
    let top = seat_on_top(&mut e, &["grizzly_bears", "forest"]);
    keep_only(&mut e, &top);
    let batch = cast_case(&mut e, case);
    let choice = find_resolution_choice(&batch).expect("look choice");
    assert_eq!(choice.candidate_object_ids, top);
    assert_eq!(choice.candidate_selectable, [true, true]);
    assert_eq!((choice.min, choice.max), (0, 1));
    let after = e
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("choose nothing");
    if let Some(order) = find_resolution_choice(&after) {
        assert_eq!(order.candidate_object_ids, top);
        assert_eq!((order.min, order.max), (2, 2));
        e.apply_scenario_command(0, &submit_resolution_choice(top.to_vec()))
            .expect("order the two remaining cards");
    }
    assert_eq!(e.state.players[0].hand.len(), hand_before - 1);
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        0,
        "no draw from the empty/short look"
    );
}

#[test]
fn q40_look_choose_no_matching_card_still_gains_life() {
    let case = &cases()[7];
    let mut e = engine(40_014_001, case.id, "forest");
    ensure_in_hand(&mut e, 0, case.id);
    let top = seat_on_top(&mut e, &["lightning_bolt", "divination", "dispel"]);
    keep_only(&mut e, &top);
    let hand_before = e.state.players[0].hand.len();
    let life_before = e.state.players[0].life;
    let batch = cast_case(&mut e, case);
    let choice = find_resolution_choice(&batch).expect("look choice");
    assert_eq!(choice.candidate_object_ids, top);
    assert_eq!(choice.candidate_selectable, [false, false, false]);
    assert_eq!((choice.min, choice.max), (0, 0));
    let after = e
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("no permanent is eligible");
    let order = find_resolution_choice(&after).expect("bottom order for the nonmatching cards");
    assert_eq!(order.candidate_object_ids, top);
    e.apply_scenario_command(0, &submit_resolution_choice(top.to_vec()))
        .expect("order the remainder");
    assert_eq!(e.state.players[0].life, life_before + 3);
    assert_eq!(e.state.players[0].hand.len(), hand_before - 1);
}
