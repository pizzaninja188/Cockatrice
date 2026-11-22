//! Actual-card scenarios for the queue-v39 counter-with-rider cohort.
//!
//! Expectations are printed Oracle values. CR 701.6 counters the targeted spell; CR 608.2c keeps
//! the printed rider order; CR 119.3 gains life; CR 701.18 performs the private scry; and
//! CR 608.2b fizzles a stale counterspell target.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    life: Option<i32>,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "absorb",
            mana: ManaGift {
                w: 1,
                u: 2,
                ..Default::default()
            },
            life: Some(3),
        },
        Case {
            id: "fall_of_the_gavel",
            mana: ManaGift {
                w: 1,
                u: 1,
                c: 3,
                ..Default::default()
            },
            life: Some(5),
        },
        Case {
            id: "memory_drain",
            mana: ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
            life: None,
        },
    ]
}

fn gift(case: &Case) -> ManaGift {
    ManaGift {
        w: case.mana.w,
        u: case.mana.u,
        b: case.mana.b,
        r: case.mana.r,
        g: case.mana.g,
        c: case.mana.c,
    }
}

fn engine(seed: u64, counter: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears", "savannah_lions"]),
        deck_with("island", &[counter]),
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

fn cast_bears(e: &mut GameEngine) -> u32 {
    ensure_in_hand(e, 0, "grizzly_bears");
    give_mana(
        e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Grizzly Bears");
    let spell = e.state.stack.last().expect("bear spell").id;
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    spell
}

fn counter_spell(e: &mut GameEngine, case: &Case, target: u32) -> Result<(), String> {
    ensure_in_hand(e, 1, case.id);
    give_mana(e, 1, gift(case));
    let slot = hand_index_for_card(e, 1, case.id);
    e.apply_scenario_command(1, &cast_spell(slot, target_object(target)))
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

fn pass_until_scry(e: &mut GameEngine) {
    for _ in 0..20 {
        if e.state.pending_resolution.is_some() || e.state.stack.is_empty() {
            return;
        }
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("pass priority");
    }
    panic!("resolution never reached the scry choice");
}

#[test]
fn q39_counter_rider_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(39_070_001, case.id);
    let target = cast_bears(&mut e);
    counter_spell(&mut e, case, target).expect("counter the bear");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 23);
    assert!(e.state.players[1]
        .graveyard
        .iter()
        .any(|oid| e.state.objects[oid].card_id == case.id));
}

#[test]
fn q39_counter_rider_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(39_070_002, case.id);
    let target = cast_bears(&mut e);
    let top = inject_library_card(&mut e, 1, "forest");
    e.state.players[1].library.retain(|oid| *oid != top);
    e.state.players[1].library.push_front(top);
    let second = inject_library_card(&mut e, 1, "island");
    e.state.players[1].library.retain(|oid| *oid != second);
    e.state.players[1].library.push_front(second);
    counter_spell(&mut e, case, target).expect("counter the bear");
    pass_until_scry(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    let pending = e.state.pending_resolution.as_ref().expect("scry 2 choice");
    assert!(pending.presentation.candidates.contains(&top));
    assert!(pending.presentation.candidates.contains(&second));
    e.apply_scenario_command(1, &submit_resolution_choice(vec![top]))
        .expect("bottom one scried card");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        *e.state.players[1].library.back().expect("bottom card"),
        top
    );
}

#[test]
fn q39_counter_rider_cards_counter_the_target_and_apply_their_printed_rider() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_071_000 + index as u64, case.id);
        let target = cast_bears(&mut e);
        if case.id == "memory_drain" {
            let top = inject_library_card(&mut e, 1, "forest");
            e.state.players[1].library.retain(|oid| *oid != top);
            e.state.players[1].library.push_front(top);
            let second = inject_library_card(&mut e, 1, "island");
            e.state.players[1].library.retain(|oid| *oid != second);
            e.state.players[1].library.push_front(second);
            counter_spell(&mut e, case, target).expect("counter");
            pass_until_scry(&mut e);
            let pending = e.state.pending_resolution.as_ref().expect("scry choice");
            assert!(pending.presentation.candidates.contains(&top));
            assert!(pending.presentation.candidates.contains(&second));
            e.apply_scenario_command(1, &submit_resolution_choice(vec![top]))
                .expect("scry choice");
        } else {
            counter_spell(&mut e, case, target).expect("counter");
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(
                e.state.players[1].life,
                20 + case.life.unwrap(),
                "{}",
                case.id
            );
        }
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
fn q39_counter_rider_cards_reject_a_non_spell_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_072_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 1, case.id);
        give_mana(&mut e, 1, gift(case));
        let slot = hand_index_for_card(&e, 1, case.id);
        let before = e.state.players[1].hand.len();
        assert!(
            e.apply_scenario_command(1, &cast_spell(slot, target_player(0)))
                .is_err(),
            "{}: a player is not a spell target",
            case.id
        );
        assert_eq!(e.state.players[1].hand.len(), before);
        assert!(e.state.stack.is_empty());
    }
}

#[test]
fn q39_counter_rider_cards_fizzle_when_the_target_spell_leaves_the_stack() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_073_000 + index as u64, case.id);
        let target = cast_bears(&mut e);
        counter_spell(&mut e, case, target).expect("counter");
        e.state.stack.retain(|entry| entry.id != target);
        e.state.objects.get_mut(&target).expect("spell").zone = Zone::Graveyard;
        e.state.players[0].graveyard.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: no life rider applies",
            case.id
        );
        assert!(
            e.state.pending_resolution.is_none(),
            "{}: no scry rider",
            case.id
        );
    }
}
