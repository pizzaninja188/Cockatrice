//! Actual-card scenarios for the queue-v30 tap-looter cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 activates the ability;
//! CR 121 draws in the printed order and CR 701.9 applies the mandatory private
//! discard. CR 107.5 prevents a tapped source from paying {T} again.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: Option<ManaGift>,
    body: Option<(u32, u32)>,
    draw: usize,
    discard: usize,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "thought_courier",
            mana: Some(ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            }),
            body: Some((1, 1)),
            draw: 1,
            discard: 1,
        },
        Case {
            id: "merfolk_looter",
            mana: Some(ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            }),
            body: Some((1, 1)),
            draw: 1,
            discard: 1,
        },
        Case {
            id: "bazaar_of_baghdad",
            mana: None,
            body: None,
            draw: 2,
            discard: 3,
        },
        Case {
            id: "magus_of_the_bazaar",
            mana: Some(ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            }),
            body: Some((0, 1)),
            draw: 2,
            discard: 3,
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
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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

fn put_ready(e: &mut GameEngine, case: &Case) -> u32 {
    if case.mana.is_none() {
        inject_card_into_hand(e, 0, case.id);
        let slot = hand_index_for_card(e, 0, case.id);
        e.apply_scenario_command(0, &play_land(slot))
            .expect("play Bazaar");
        battlefield_object_for_card(e, 0, case.id)
    } else {
        inject_permanent_on_battlefield(e, 0, case.id)
    }
}

fn top_cards(e: &mut GameEngine, count: usize) -> Vec<u32> {
    let mut drawn = Vec::new();
    for _ in 0..count {
        let object = inject_library_card(e, 0, "forest");
        e.state.players[0].library.retain(|oid| *oid != object);
        e.state.players[0].library.push_front(object);
        drawn.push(object);
    }
    drawn
}

#[test]
fn q30_tap_looters_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(30_040_001);
    let source = put_ready(&mut e, case);
    let drawn = top_cards(&mut e, 1)[0];
    apply_ability(&mut e, 0, source, 0, vec![]).expect("loot");
    assert!(e.state.objects[&source].tapped);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
    assert!(e.state.pending_resolution.is_some());
}

#[test]
fn q30_tap_looters_canary_hardest() {
    let case = &cases()[3];
    let mut e = engine(30_040_002);
    let source = put_ready(&mut e, case);
    let drawn = top_cards(&mut e, 2);
    apply_ability(&mut e, 0, source, 0, vec![]).expect("loot two");
    resolve_entire_stack_two_player(&mut e);
    assert!(drawn
        .iter()
        .all(|oid| e.state.objects[oid].zone == Zone::Hand));
    let pending = e.state.pending_resolution.as_ref().expect("discard three");
    assert_eq!(pending.deciding_player, e.state.players[0].id);
}

#[test]
fn q30_tap_looters_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_041_000 + index as u64);
        if let Some(mana) = &case.mana {
            inject_card_into_hand(&mut e, 0, case.id);
            give_mana(&mut e, 0, gift(mana));
            let slot = hand_index_for_card(&e, 0, case.id);
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
            resolve_entire_stack_two_player(&mut e);
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
                "{}",
                case.id
            );
            let source = battlefield_object_for_card(&e, 0, case.id);
            if let Some(body) = case.body {
                assert_eq!(
                    (e.effective_power(source), e.effective_toughness(source)),
                    (Some(body.0), Some(body.1)),
                    "{}",
                    case.id
                );
            }
        } else {
            let source = put_ready(&mut e, &case);
            assert_eq!(
                e.state.objects[&source].zone,
                Zone::Battlefield,
                "{}",
                case.id
            );
        }
    }
}

#[test]
fn q30_tap_looters_cards_draw_then_discard() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_042_000 + index as u64);
        let source = put_ready(&mut e, &case);
        let drawn = top_cards(&mut e, case.draw);
        let hand_before = e.state.players[0].hand.len();
        apply_ability(&mut e, 0, source, 0, vec![]).expect("activate");
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        resolve_entire_stack_two_player(&mut e);
        for object in &drawn {
            assert_eq!(e.state.objects[object].zone, Zone::Hand, "{}", case.id);
        }
        let pending = e
            .state
            .pending_resolution
            .as_ref()
            .unwrap_or_else(|| panic!("{}: discard pending", case.id));
        assert_eq!(
            pending.deciding_player, e.state.players[0].id,
            "{}",
            case.id
        );
        let candidates = pending.presentation.candidates.clone();
        assert!(
            e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
                .is_err(),
            "{}: an empty discard is rejected",
            case.id
        );
        assert!(e.state.pending_resolution.is_some(), "{}", case.id);
        let chosen = candidates[..case.discard].to_vec();
        assert!(
            e.apply_scenario_command(1, &submit_resolution_choice(chosen.clone()))
                .is_err(),
            "{}: the opponent cannot answer the discard",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(chosen.clone()))
            .expect("discard the printed count");
        for object in &chosen {
            assert_eq!(
                e.state.objects[object].zone,
                Zone::Graveyard,
                "{}: the chosen discard goes to its owner's graveyard",
                case.id
            );
        }
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + case.draw - case.discard,
            "{}: exact draw/discard counts",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn q30_tap_looters_cards_reject_a_tapped_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_043_000 + index as u64);
        let source = put_ready(&mut e, &case);
        let hand_before = e.state.players[0].hand.len();
        e.state.objects.get_mut(&source).expect("source").tapped = true;
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: a tapped source cannot pay {{T}}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
    }
}
