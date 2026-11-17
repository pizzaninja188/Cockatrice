//! Actual-card scenarios for the queue-v27 look-three cohort.
//!
//! Expectations are printed Oracle values. The resolving controller privately looks
//! at the top three cards (CR 401.2), puts exactly one into hand, and puts the rest
//! into the graveyard in the chosen order. Strategic Planning's pinned ruling covers
//! a library with fewer than three cards.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "glimpse_the_future",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "ransack_the_lab",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "strategic_planning",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
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
    let decks = Some(vec![deck_with("island", &[card]), deck_with("forest", &[])]);
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

fn cast_case(e: &mut GameEngine, case: &Case) {
    ensure_in_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
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
        "{}: exact cast cost",
        case.id
    );
}

fn resolve_top(e: &mut GameEngine) {
    let live = e.state.players.iter().filter(|p| !p.has_lost).count();
    for _ in 0..live {
        let actor = e.state.priority_player_id();
        e.apply_scenario_command(actor, &pass())
            .expect("pass priority");
    }
}

fn answer(e: &mut GameEngine, actor: i32, chosen: Vec<u32>) {
    let pending = e.state.pending_resolution.as_ref().expect("look prompt");
    assert_eq!(pending.deciding_player, actor);
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: pending.resolution_prompt_id.get(),
                chosen_object_ids: chosen,
                ..Default::default()
            },
        )),
    };
    e.apply_scenario_command(actor, &command)
        .expect("answer look prompt");
}

#[test]
fn look_three_batch_cards_partition_exactly_the_top_three() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_090_000 + index as u64, case.id);
        cast_case(&mut e, &case);
        let looked = e.state.players[0]
            .library
            .iter()
            .take(3)
            .copied()
            .collect::<Vec<_>>();
        resolve_top(&mut e);
        let pending = e.state.pending_resolution.as_ref().expect("look prompt");
        assert_eq!(pending.deciding_player, 0, "{}", case.id);
        assert_eq!(
            pending.presentation.candidates.to_vec(),
            looked,
            "{}",
            case.id
        );
        answer(&mut e, 0, vec![looked[1]]);
        answer(&mut e, 0, vec![looked[2], looked[0]]);
        assert_eq!(e.state.objects[&looked[1]].zone, Zone::Hand, "{}", case.id);
        assert!(e.state.players[0].hand.contains(&looked[1]), "{}", case.id);
        let graveyard = &e.state.players[0].graveyard;
        let tail = &graveyard[graveyard.len().saturating_sub(3)..];
        assert!(
            tail.contains(&looked[0]) && tail.contains(&looked[2]),
            "{}: both remainders are in the graveyard; looked={looked:?} grave={graveyard:?}",
            case.id
        );
        assert!(
            !graveyard.contains(&looked[1]),
            "{}: the selected card is not also in the graveyard",
            case.id
        );
        assert_eq!(count_card_id_in_graveyard(&e, 0, case.id), 1, "{}", case.id);
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn look_three_batch_cards_keep_the_look_private_to_the_resolving_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_091_000 + index as u64, case.id);
        cast_case(&mut e, &case);
        resolve_top(&mut e);
        let pending = e.state.pending_resolution.as_ref().expect("look prompt");
        let prompt = pending.resolution_prompt_id.get();
        let looked = e.state.players[0]
            .library
            .iter()
            .take(3)
            .copied()
            .collect::<Vec<_>>();
        let rejected = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt,
                    chosen_object_ids: vec![looked[0]],
                    ..Default::default()
                },
            )),
        };
        let before = e.diagnostic_snapshot().unwrap();
        assert!(
            e.apply_scenario_command(1, &rejected).is_err(),
            "{}: only the resolving controller answers",
            case.id
        );
        assert_eq!(e.diagnostic_snapshot().unwrap(), before, "{}", case.id);
        answer(&mut e, 0, vec![looked[0]]);
        answer(&mut e, 0, vec![looked[1], looked[2]]);
    }
}

#[test]
fn look_three_batch_cards_handle_a_library_with_fewer_than_three_cards() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_092_000 + index as u64, case.id);
        cast_case(&mut e, &case);
        e.state.players[0].library.truncate(2);
        let looked = e.state.players[0]
            .library
            .iter()
            .take(2)
            .copied()
            .collect::<Vec<_>>();
        resolve_entire_stack_two_player(&mut e);
        let Some(pending) = e.state.pending_resolution.as_ref() else {
            panic!(
                "{}: no look prompt; looked={looked:?} hand={:?} grave={:?} stack={}",
                case.id,
                e.state.players[0].hand,
                e.state.players[0].graveyard,
                e.state.stack.len()
            );
        };
        assert_eq!(
            pending.presentation.candidates.to_vec(),
            looked,
            "{}",
            case.id
        );
        answer(&mut e, 0, vec![looked[0]]);
        assert_eq!(e.state.objects[&looked[0]].zone, Zone::Hand, "{}", case.id);
        if e.state.pending_resolution.is_some() {
            answer(&mut e, 0, vec![looked[1]]);
        }
        assert_eq!(
            e.state.objects[&looked[1]].zone,
            Zone::Graveyard,
            "{}: the remaining card is binned",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}
