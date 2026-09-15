//! Issue #284 — Campus Guide and Spider-Bot use an optional typed ETB branch to search the
//! controller's library for exactly one basic land, reveal it, shuffle, and put it on top.
//!
//! CR 603.6a governs the ETB trigger; CR 701.20 governs revealing, CR 701.23 and 701.23b govern
//! searching and fail-to-find, CR 701.24 and 701.24b govern shuffling and excluding found cards
//! from that shuffle, and CR 401 governs the library and top-of-library placement. The scenarios
//! also exercise the engine's private candidate publication and current-generation validation.

use super::helpers::*;
use tricerules_core::{EngineError, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn issue_284_engine(seed: u64, card_id: &str, basic: &str) -> GameEngine {
    let decks = Some(vec![deck_with(basic, &[card_id]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("issue #284 engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn decline_branch() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::Decline as i32,
            ..Default::default()
        })),
    }
}

fn open_branch(engine: &mut GameEngine, card_id: &str) -> (u32, ResolutionChoiceRequired) {
    let source = move_ready_to_battlefield(engine, 0, card_id);
    let batch = {
        engine
            .apply_command(0, &pass())
            .expect("controller passes ETB");
        engine
            .apply_command(1, &pass())
            .expect("opponent passes ETB")
    };
    let choice = find_resolution_choice(&batch).expect("optional ETB branch");
    (source, choice)
}

fn move_library_to_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .library
        .retain(|id| *id != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn move_graveyard_to_library(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|id| *id != object_id);
    engine.state.players[player].library.push_back(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Library;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn has_log(batch: &RuledEventBatch, text: &str) -> bool {
    batch
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == text))
}

#[test]
fn issue_284_declining_the_etb_branch_does_not_search_or_shuffle() {
    let mut engine = issue_284_engine(284_001, "campus_guide", "forest");
    let source = move_ready_to_battlefield(&mut engine, 0, "campus_guide");
    let library_before: Vec<_> = engine.state.players[0].library.iter().copied().collect();
    engine
        .apply_command(0, &pass())
        .expect("controller passes ETB");
    let branch_batch = engine
        .apply_command(1, &pass())
        .expect("opponent passes ETB");
    let branch = find_resolution_choice(&branch_batch).expect("optional ETB branch");
    assert_eq!(branch.deciding_player_id, 0);
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (0, 1));
    assert!(branch.candidate_object_ids.is_empty());

    assert!(
        engine.apply_command(1, &decline_branch()).is_err(),
        "only the controller may decline the branch"
    );
    let completion = engine
        .apply_command(0, &decline_branch())
        .expect("decline the optional branch");

    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("shuffles"))));
}

#[test]
fn issue_284_search_is_private_controller_only_and_basic_land_only() {
    let mut engine = issue_284_engine(284_002, "spider-bot", "forest");
    let own_basic = inject_library_card(&mut engine, 0, "forest");
    let own_nonbasic = inject_library_card(&mut engine, 0, "taiga");
    let opposing_basic = inject_library_card(&mut engine, 1, "forest");
    let (_source, branch) = open_branch(&mut engine, "spider-bot");
    assert_eq!(branch.deciding_player_id, 0);
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert!(branch.public_reveal.is_none());

    assert!(
        engine.apply_command(1, &select_branch(0)).is_err(),
        "only the controller may select the ETB branch"
    );
    let search_batch = engine
        .apply_command(0, &select_branch(0))
        .expect("select the search branch");
    let search = find_resolution_choice(&search_batch).expect("private library search");
    assert_eq!(search.deciding_player_id, 0);
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((search.min, search.max), (0, 1));
    assert!(search.public_reveal.is_none());
    assert!(search.candidate_object_ids.contains(&own_basic));
    assert!(!search.candidate_object_ids.contains(&own_nonbasic));
    assert!(!search.candidate_object_ids.contains(&opposing_basic));
    assert!(search
        .candidate_object_ids
        .iter()
        .all(|object_id| engine.state.objects[object_id].card_id == "forest"));

    assert!(matches!(
        engine.apply_command(1, &submit_resolution_choice(vec![own_basic])),
        Err(EngineError::Illegal(_))
    ));
    assert!(engine.state.pending_resolution.is_some());
}

#[test]
fn issue_284_accepting_search_reveals_shuffles_and_puts_the_basic_land_on_top() {
    let mut engine = issue_284_engine(284_003, "campus_guide", "forest");
    let basic = inject_library_card(&mut engine, 0, "forest");
    let nonbasic = inject_library_card(&mut engine, 0, "taiga");
    let (_source, _) = open_branch(&mut engine, "campus_guide");
    let search_batch = engine
        .apply_command(0, &select_branch(0))
        .expect("select search branch");
    let search = find_resolution_choice(&search_batch).expect("search choice");
    assert!(search.candidate_object_ids.contains(&basic));
    assert!(!search.candidate_object_ids.contains(&nonbasic));

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![basic]))
        .expect("choose the basic land");
    assert_eq!(engine.state.objects[&basic].zone, Zone::Library);
    assert_eq!(engine.state.players[0].library.front(), Some(&basic));
    assert!(engine.state.players[0].library.contains(&nonbasic));
    assert!(completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(reveal)) if reveal.cards.iter().any(|card| card.object_id == basic))));
    assert!(has_log(&completion, "P0 shuffles their library."));
    assert!(has_log(&completion, "P0 reveals Forest."));
    assert!(has_log(
        &completion,
        "P0 puts Forest on top of their library."
    ));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_284_rejects_stale_noncandidate_and_unauthorized_library_choices() {
    let mut engine = issue_284_engine(284_004, "spider-bot", "forest");
    let stale = inject_library_card(&mut engine, 0, "forest");
    let noncandidate = inject_library_card(&mut engine, 0, "taiga");
    let unauthorized = inject_library_card(&mut engine, 1, "forest");
    let (_source, _) = open_branch(&mut engine, "spider-bot");
    engine
        .apply_command(0, &select_branch(0))
        .expect("select search branch");
    let search = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("search continuation")
        .presentation
        .clone();
    assert!(search.candidates.contains(&stale));
    assert!(!search.candidates.contains(&noncandidate));
    assert!(!search.candidates.contains(&unauthorized));

    move_library_to_graveyard(&mut engine, 0, stale);
    move_graveyard_to_library(&mut engine, 0, stale);
    for (player, chosen, label) in [
        (0, noncandidate, "noncandidate"),
        (0, stale, "stale"),
        (1, unauthorized, "unauthorized"),
    ] {
        assert!(
            matches!(
                engine.apply_command(player, &submit_resolution_choice(vec![chosen])),
                Err(EngineError::Illegal(_))
            ),
            "{label} choice must be rejected"
        );
        assert!(engine.state.pending_resolution.is_some());
    }
    engine
        .apply_command(0, &submit_resolution_choice(Vec::new()))
        .expect("fail to find after rejected stale choices");
    assert!(engine.state.pending_resolution.is_none());
}
