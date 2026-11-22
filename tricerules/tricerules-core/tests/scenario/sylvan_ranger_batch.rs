//! Actual-card scenarios for the reviewed optional basic-land search family.
//!
//! Expectations are printed Oracle values. CR 603.1 stages the entry trigger; CR 701.23
//! makes the search optional, private, basic-land-only, revealed, put into hand, then shuffled.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice};

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "borderland_ranger",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "civic_wayfinder",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "skittering_surveyor",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "sylvan_ranger",
            mana: ManaGift {
                g: 1,
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

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn engine(seed: u64, case: &Case) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[case.id]),
        deck_with("forest", &[]),
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

fn cast_and_reach_branch(e: &mut GameEngine, case: &Case) {
    ensure_in_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the creature");
    pass_both_players(e);
    pass_both_players(e);
    assert!(
        e.state.pending_resolution.is_some(),
        "{}: the optional search branch is offered",
        case.id
    );
}

#[test]
fn sylvan_ranger_batch_cards_search_privately_for_a_basic_land() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(900_000 + index as u64, &case);
        inject_library_card(&mut e, 0, "grizzly_bears");
        cast_and_reach_branch(&mut e, &case);
        let branch = e
            .state
            .pending_resolution
            .as_ref()
            .expect("optional search branch");
        assert!(branch.presentation.candidates.is_empty(), "{}", case.id);
        e.apply_scenario_command(0, &select_branch(0))
            .expect("choose to search");
        let choice = e
            .state
            .pending_resolution
            .as_ref()
            .expect("private library choice");
        assert!(!choice.presentation.candidates.is_empty(), "{}", case.id);
        assert!(
            choice
                .presentation
                .candidates
                .iter()
                .all(|oid| e.state.objects[oid].card_id != "grizzly_bears"),
            "{}: only basic lands are offered",
            case.id
        );
        let forest = choice.presentation.candidates[0];
        e.apply_scenario_command(0, &submit_resolution_choice(vec![forest]))
            .expect("choose the basic land");
        assert_eq!(e.state.objects[&forest].zone, Zone::Hand, "{}", case.id);
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn sylvan_ranger_batch_cards_may_decline_the_search() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(901_000 + index as u64, &case);
        cast_and_reach_branch(&mut e, &case);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        e.apply_scenario_command(
            0,
            &submit_resolution_decision(
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
            ),
        )
        .expect("decline the search");
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}: declining moves no card",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}
