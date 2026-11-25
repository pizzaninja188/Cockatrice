//! Actual-card scenarios for the queue-v84 01-etb-scry-riders cohort.
//!
//! Every card has the enters-the-battlefield trigger "scry N" plus one printed rider. Expectations
//! are printed Oracle values. CR 603.6a stages the entry trigger and CR 701.22 defines Scry. The
//! pinned 2023-02-04 Myr Custodian ruling orders the controller's scry before each opponent's
//! optional Scry 1; an accepting opponent's look is published only to that opponent.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChoiceKind, ResolutionChoiceDecision, RuledCommand,
    RuledEventBatch, SubmitResolutionChoice,
};

struct ScryCase {
    id: &'static str,
    count: u32,
}

fn scry_cases() -> [ScryCase; 4] {
    [
        ScryCase {
            id: "grey_havens_navigator",
            count: 1,
        },
        ScryCase {
            id: "faithful_pikemaster",
            count: 2,
        },
        ScryCase {
            id: "myr_custodian",
            count: 2,
        },
        ScryCase {
            id: "inquisitive_puppet",
            count: 1,
        },
    ]
}

fn engine(seed: u64, card: &str, opponent_card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("island", &[opponent_card]),
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

fn enter_and_park_scry(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    _top: &[u32],
) -> ResolutionChoiceRequired {
    let creature = move_ready_to_battlefield(e, player, card);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Battlefield,
        "{card}: enters the battlefield"
    );
    let resolution = resolve_top_stack(e);
    let choice =
        find_resolution_choice(&resolution).expect("the enters trigger parks a scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop, "{card}");
    assert_eq!(choice.min, 0, "{card}");
    assert!(choice.max >= 1, "{card}");
    assert!(!choice.candidate_object_ids.is_empty(), "{card}");
    choice
}

fn private_look_viewers(batch: &RuledEventBatch) -> Vec<Option<i32>> {
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::Log(log)) if log.private_card_look.is_some() => Some(log.visible_to_player_id),
            _ => None,
        })
        .collect()
}

fn answer_scry_branch(engine: &mut GameEngine, actor: i32, branch_index: u32) -> RuledEventBatch {
    let command = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch_index,
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(actor, &command)
        .expect("answer the optional opponent scry branch")
}

#[test]
fn q84_scry_canary_grey_havens_navigator_scries_one_with_flash() {
    let mut e = engine(84_010_001, "grey_havens_navigator", "forest");
    let top = seat_on_top(&mut e, 0, &["storm_crow", "grizzly_bears", "hill_giant"]);
    let choice = enter_and_park_scry(&mut e, 0, "grey_havens_navigator", &top);
    assert_eq!(choice.max, 1, "scry 1 offers one card");
    assert_eq!(choice.candidate_object_ids, vec![top[0]]);
    assert_eq!(
        choice.deciding_player_id, e.state.players[0].id,
        "the controller scries"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("put the top card on the bottom");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.players[0].library.back(), Some(&top[0]));

    let creature = battlefield_object_for_card(&e, 0, "grey_havens_navigator");
    assert!(e.effective_has_keyword(creature, Keyword::Flash));
    assert_eq!(e.effective_power(creature), Some(3));
    assert_eq!(e.effective_toughness(creature), Some(2));
}

#[test]
fn q84_scry_canary_myr_custodian_orders_controller_then_opponent_scry() {
    let mut e = engine(84_011_001, "myr_custodian", "forest");
    let controller_top = seat_on_top(&mut e, 0, &["storm_crow", "grizzly_bears", "hill_giant"]);
    let opponent_top = seat_on_top(&mut e, 1, &["forest", "plains", "mountain"]);
    let choice = enter_and_park_scry(&mut e, 0, "myr_custodian", &controller_top);
    assert_eq!(choice.max, 2, "controller scries two");
    assert_eq!(choice.deciding_player_id, e.state.players[0].id);
    e.apply_scenario_command(0, &submit_resolution_choice(controller_top[..2].to_vec()))
        .expect("complete the controller's scry 2");

    let branch = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the opponent receives the optional Scry 1")
        .presentation
        .clone();
    assert_eq!(branch.choice_kind, ChoiceKind::ResolutionBranch);
    assert_eq!(
        e.state.pending_resolution.as_ref().unwrap().deciding_player,
        e.state.players[1].id
    );

    let opponent_library_before = e.state.players[1].library.clone();
    let opponent = e.state.players[1].id;
    let opt_in_batch = answer_scry_branch(&mut e, opponent, 0);
    assert_eq!(
        private_look_viewers(&opt_in_batch),
        [Some(opponent)],
        "the accepting opponent's look is published only to that opponent"
    );
    let placement = e
        .state
        .pending_resolution
        .as_ref()
        .expect("accepted Scry 1 opens a placement choice");
    assert_eq!(placement.deciding_player, e.state.players[1].id);
    assert_eq!(placement.presentation.choice_kind, ChoiceKind::LibraryTop);
    assert_eq!(
        placement.presentation.candidates,
        vec![opponent_top[0]],
        "only the opponent's top card is offered"
    );
    e.apply_scenario_command(opponent, &submit_resolution_choice(vec![opponent_top[0]]))
        .expect("the opponent puts the looked-at card on the bottom");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        e.state.players[1].library.len(),
        opponent_library_before.len()
    );
    assert_eq!(e.state.players[1].library.back(), Some(&opponent_top[0]));
}

#[test]
fn q84_myr_custodian_opponent_may_decline_scry() {
    let mut e = engine(84_012_001, "myr_custodian", "forest");
    let controller_top = seat_on_top(&mut e, 0, &["storm_crow", "grizzly_bears", "hill_giant"]);
    let opponent_top = seat_on_top(&mut e, 1, &["forest", "plains", "mountain"]);
    let _ = enter_and_park_scry(&mut e, 0, "myr_custodian", &controller_top);
    e.apply_scenario_command(0, &submit_resolution_choice(controller_top[..2].to_vec()))
        .expect("complete the controller's scry 2");
    let opponent_library_before = e.state.players[1].library.clone();
    let opponent = e.state.players[1].id;
    let _ = answer_scry_branch(&mut e, opponent, 1);
    assert!(
        e.state.pending_resolution.is_none(),
        "declining ends the ability"
    );
    assert_eq!(
        e.state.players[1].library, opponent_library_before,
        "a declining opponent's library is unchanged"
    );
    assert_eq!(e.state.players[1].library.front(), Some(&opponent_top[0]));
}

#[test]
fn q84_scry_cards_offer_their_exact_printed_counts() {
    for (index, case) in scry_cases().into_iter().enumerate() {
        let mut e = engine(84_013_000 + index as u64, case.id, "forest");
        let top = seat_on_top(
            &mut e,
            0,
            &[
                "storm_crow",
                "grizzly_bears",
                "hill_giant",
                "forest",
                "plains",
            ],
        );
        let choice = enter_and_park_scry(&mut e, 0, case.id, &top);
        assert_eq!(choice.max, case.count, "{}: printed scry count", case.id);
        assert_eq!(
            choice.candidate_object_ids,
            top[..case.count as usize],
            "{}: only the printed top cards are offered",
            case.id
        );
        assert!(
            e.apply_scenario_command(
                0,
                &submit_resolution_choice(top[..case.count as usize].to_vec())
            )
            .is_ok(),
            "{}: exact scry choice accepted",
            case.id
        );
        if case.id == "myr_custodian" {
            let opponent = e.state.players[1].id;
            let _ = answer_scry_branch(&mut e, opponent, 1);
        }
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn q84_faithful_pikemaster_first_strike_is_turn_conditional() {
    let mut e = engine(84_014_001, "faithful_pikemaster", "forest");
    let top = seat_on_top(&mut e, 0, &["storm_crow", "grizzly_bears", "hill_giant"]);
    let _ = enter_and_park_scry(&mut e, 0, "faithful_pikemaster", &top);
    e.apply_scenario_command(0, &submit_resolution_choice(top[..2].to_vec()))
        .expect("complete scry 2");
    let creature = battlefield_object_for_card(&e, 0, "faithful_pikemaster");
    assert_eq!(e.effective_power(creature), Some(3));
    assert_eq!(e.effective_toughness(creature), Some(4));
    assert!(
        e.effective_has_keyword(creature, Keyword::FirstStrike),
        "first strike applies during its controller's turn"
    );
    e.state.active_player_idx = 1;
    assert!(
        !e.effective_has_keyword(creature, Keyword::FirstStrike),
        "first strike is absent during an opponent's turn"
    );
    e.state.active_player_idx = 0;
    assert!(e.effective_has_keyword(creature, Keyword::FirstStrike));
}

#[test]
fn q84_inquisitive_puppet_exiles_itself_for_a_human_token() {
    let mut e = engine(84_015_001, "inquisitive_puppet", "forest");
    let top = seat_on_top(&mut e, 0, &["storm_crow", "grizzly_bears", "hill_giant"]);
    let _ = enter_and_park_scry(&mut e, 0, "inquisitive_puppet", &top);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("complete scry 1");
    let puppet = battlefield_object_for_card(&e, 0, "inquisitive_puppet");
    assert!(battlefield_token_oids(&e, 0, "human_w_1_1").is_empty());
    e.apply_scenario_command(0, &activate_ability_for(&e, puppet, 0, vec![]))
        .expect("exile the Puppet as a cost");
    assert_eq!(
        e.state.objects[&puppet].zone,
        Zone::Exile,
        "the cost exiles rather than sacrifices"
    );
    resolve_entire_stack_two_player(&mut e);
    let tokens = battlefield_token_oids(&e, 0, "human_w_1_1");
    assert_eq!(tokens.len(), 1, "one 1/1 white Human token");
    assert_eq!(e.effective_power(tokens[0]), Some(1));
    assert_eq!(e.effective_toughness(tokens[0]), Some(1));
}
