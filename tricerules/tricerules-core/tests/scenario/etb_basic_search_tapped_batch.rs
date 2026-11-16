//! Actual-card scenarios for the queue-v22 optional basic-land entry search
//! cohort (battlefield tapped).
//!
//! Expectations are printed Oracle values. CR 603.6a stages the entry trigger;
//! CR 701.23 makes the search optional, private and basic-land-only; the found
//! land enters tapped; CR 701.24 shuffles.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{
    ResolutionChoiceDecision, RuledCommand, RuledEventBatch, SubmitResolutionChoice,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "farhaven_elf",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "ondu_giant",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "wild_wanderer",
            mana: ManaGift {
                g: 1,
                c: 3,
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

fn shuffle_log_count(batch: &RuledEventBatch, player: i32) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == format!("P{player} shuffles their library.")
            )
        })
        .count()
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
fn etb_basic_search_tapped_batch_canary_cards_find_a_basic_land_tapped() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "farhaven_elf" | "ondu_giant"))
    {
        let mut e = engine(11_500_000, &case);
        let nonbasic = inject_library_card(&mut e, 0, "grizzly_bears");
        let opponent_land = inject_library_card(&mut e, 1, "forest");
        cast_and_reach_branch(&mut e, &case);
        let branch = e
            .state
            .pending_resolution
            .as_ref()
            .expect("optional search branch");
        assert!(
            branch.presentation.candidates.is_empty(),
            "{}: the branch prompt leaks no library card",
            case.id
        );
        assert_eq!(
            branch.deciding_player, 0,
            "{}: the controller chooses",
            case.id
        );
        e.apply_scenario_command(0, &select_branch(0))
            .expect("choose to search");
        let choice = e
            .state
            .pending_resolution
            .as_ref()
            .expect("private library choice");
        assert_eq!(choice.deciding_player, 0, "{}", case.id);
        assert!(
            choice
                .presentation
                .candidates
                .iter()
                .all(|oid| e.state.objects[oid].card_id == "forest"),
            "{}: only basic lands are offered",
            case.id
        );
        assert!(
            !choice.presentation.candidates.contains(&nonbasic),
            "{}: the nonbasic is not offered",
            case.id
        );
        assert!(
            !choice.presentation.candidates.contains(&opponent_land),
            "{}: another player's library is not searched",
            case.id
        );
        let land = choice.presentation.candidates[0];
        let completion = e
            .apply_scenario_command(0, &submit_resolution_choice(vec![land]))
            .expect("choose the basic land");
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}: the land enters the battlefield",
            case.id
        );
        assert_eq!(
            e.state.objects[&land].controller, 0,
            "{}: the searching player controls the land",
            case.id
        );
        assert_eq!(
            shuffle_log_count(&completion, 0),
            1,
            "{}: one shuffle after the search",
            case.id
        );
        assert!(
            e.state.objects[&land].tapped,
            "{}: the searched land enters tapped",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn etb_basic_search_tapped_batch_cards_search_every_printed_card() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_510_000 + index as u64, &case);
        cast_and_reach_branch(&mut e, &case);
        e.apply_scenario_command(0, &select_branch(0))
            .expect("choose to search");
        let land = e
            .state
            .pending_resolution
            .as_ref()
            .expect("private library choice")
            .presentation
            .candidates[0];
        e.apply_scenario_command(0, &submit_resolution_choice(vec![land]))
            .expect("choose the basic land");
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.objects[&land].tapped, "{}", case.id);
    }
}

#[test]
fn etb_basic_search_tapped_batch_cards_may_decline() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_520_000 + index as u64, &case);
        cast_and_reach_branch(&mut e, &case);
        let library_before = e.state.players[0].library.len();
        let batch = e
            .apply_scenario_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::Decline),
            )
            .expect("decline the search");
        assert_eq!(
            shuffle_log_count(&batch, 0),
            0,
            "{}: declining does not shuffle",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}: declining moves no card",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn etb_basic_search_tapped_batch_cards_shuffle_after_searching() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_530_000 + index as u64, &case);
        cast_and_reach_branch(&mut e, &case);
        e.apply_scenario_command(0, &select_branch(0))
            .expect("choose to search");
        let land = e
            .state
            .pending_resolution
            .as_ref()
            .expect("private library choice")
            .presentation
            .candidates[0];
        let library_before = e.state.players[0].library.len();
        let completion = e
            .apply_scenario_command(0, &submit_resolution_choice(vec![land]))
            .expect("choose the basic land");
        assert_eq!(shuffle_log_count(&completion, 0), 1, "{}", case.id);
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: the found land left the library",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}
