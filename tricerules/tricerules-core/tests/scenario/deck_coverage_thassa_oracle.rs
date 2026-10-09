//! Exact Thassa's Oracle identity and rules behavior.

use super::helpers::*;
use tricerules_cards::ManaCost;
use tricerules_core::state::GameOutcome;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChoiceKind, SubmitResolutionChoice};

const THASSAS_ORACLE: &str = "thassas_oracle";

fn game(seed: u64, library_size: usize, extras: &[&str]) -> GameEngine {
    let mut specials = vec![THASSAS_ORACLE];
    specials.extend_from_slice(extras);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &specials),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    for &card in &specials {
        ensure_in_hand(&mut engine, 0, card);
    }
    set_library_size(&mut engine, 0, library_size);
    engine
}

fn set_library_size(engine: &mut GameEngine, player: usize, size: usize) -> Vec<u32> {
    let all: Vec<_> = engine.state.players[player]
        .library
        .iter()
        .copied()
        .collect();
    assert!(size <= all.len());
    let kept = all[..size].to_vec();
    let moved = all[size..].to_vec();
    engine.state.players[player].library = kept.iter().copied().collect();
    for object_id in moved {
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
        engine.state.players[player].graveyard.push(object_id);
    }
    kept
}

fn cast_oracle(engine: &mut GameEngine) -> u32 {
    let slot = hand_index_for_card(engine, 0, THASSAS_ORACLE);
    let object_id = engine.state.players[0].hand[slot];
    engine.state.players[0].mana_pool.blue = 2;
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Thassa's Oracle");
    object_id
}

fn pass_until_library_choice(engine: &mut GameEngine) {
    for _ in 0..5 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        pass_priority_round(engine);
    }
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Oracle trigger asks for its private library choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryLook);
}

fn choose_cards(cards: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: cards,
            ..Default::default()
        })),
    }
}

#[test]
fn thassa_oracle_has_a_complete_card_definition() {
    let card = tricerules_cards::registry::global()
        .get(THASSAS_ORACLE)
        .expect("Thassa's Oracle needs a complete definition");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost, ManaCost::parse("{U}{U}").unwrap());
    assert_eq!(face.types, ["Creature", "Merfolk", "Wizard"]);
    assert_eq!(face.triggered_abilities.len(), 1);
}

fn resolve_nonwinning_library_choice(seed: u64, choose_one: bool) -> Vec<u32> {
    let mut engine = game(seed, 5, &[]);
    // A second Oracle contributes two blue symbols, but direct fixture placement does not create
    // another enters-the-battlefield event.
    inject_permanent_on_battlefield(&mut engine, 0, THASSAS_ORACLE);
    cast_oracle(&mut engine);
    pass_until_library_choice(&mut engine);

    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.min, 0);
    assert_eq!(pending.presentation.max, 1);
    assert_eq!(pending.presentation.candidates.len(), 4);
    assert!(matches!(
        &pending.continuation,
        tricerules_core::state::ResolutionContinuation::LibraryLook {
            stage: tricerules_core::state::PendingLibraryLookStage::ThassaOracle { devotion_x: 4 },
            ..
        }
    ));
    let looked: Vec<_> = pending.presentation.candidates.clone();
    let untouched = *engine.state.players[0].library.get(4).unwrap();

    // Rejected actor, candidate, and stale-top answers must leave the pending choice intact.
    assert!(engine.apply_command(1, &choose_cards(Vec::new())).is_err());
    assert!(engine
        .apply_command(0, &choose_cards(vec![u32::MAX]))
        .is_err());
    engine.state.players[0].library.swap(0, 1);
    assert!(engine
        .apply_command(0, &choose_cards(vec![looked[0]]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    engine.state.players[0].library.swap(0, 1);

    let chosen = if choose_one {
        vec![looked[0]]
    } else {
        Vec::new()
    };
    engine
        .apply_command(0, &choose_cards(chosen.clone()))
        .expect("accept a choice of up to one card");
    assert_eq!(engine.state.outcome, None);
    assert_eq!(engine.state.players[0].library.len(), 5);
    let remaining_looked: Vec<_> = looked
        .iter()
        .copied()
        .filter(|object_id| !chosen.contains(object_id))
        .collect();
    let top_count = if choose_one { 2 } else { 1 };
    assert_eq!(
        engine.state.players[0].library[0],
        chosen.first().copied().unwrap_or(untouched)
    );
    if choose_one {
        assert_eq!(engine.state.players[0].library[1], untouched);
    }
    let bottom: Vec<_> = engine.state.players[0]
        .library
        .iter()
        .skip(top_count)
        .copied()
        .collect();
    if bottom.len() > 1 {
        assert_ne!(
            bottom, remaining_looked,
            "this seed must demonstrate a nontrivial random bottom order"
        );
    }
    let mut expected = remaining_looked;
    expected.sort_unstable();
    let mut actual = bottom;
    actual.sort_unstable();
    assert_eq!(actual, expected);
    assert!(engine.state.players[0]
        .library
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Library));
    engine.state.players[0].library.iter().copied().collect()
}

#[test]
fn trigger_uses_blue_devotion_and_randomizes_only_the_unselected_looked_cards() {
    assert_eq!(
        resolve_nonwinning_library_choice(0x0A11_CE01, true),
        resolve_nonwinning_library_choice(0x0A11_CE01, true),
        "the accepted choice and seeded bottom order replay deterministically"
    );
}

#[test]
fn choosing_no_card_keeps_untouched_cards_above_randomized_bottom_cards() {
    let result = resolve_nonwinning_library_choice(0x0A11_CE05, false);
    let repeat = resolve_nonwinning_library_choice(0x0A11_CE05, false);
    assert_eq!(result, repeat);
    assert_eq!(result.len(), 5);
}

#[test]
fn captured_devotion_at_least_library_size_wins_after_the_private_choice() {
    let mut engine = game(0x0A11_CE02, 4, &[]);
    inject_permanent_on_battlefield(&mut engine, 0, THASSAS_ORACLE);
    cast_oracle(&mut engine);
    pass_until_library_choice(&mut engine);
    let selected = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    engine
        .apply_command(0, &choose_cards(vec![selected]))
        .expect("complete Oracle's look");
    assert_eq!(engine.state.outcome, Some(GameOutcome::Winner(0)));
}

#[test]
fn library_shorter_than_devotion_looks_at_all_remaining_cards_then_wins() {
    let mut engine = game(0x0A11_CE08, 2, &[]);
    inject_permanent_on_battlefield(&mut engine, 0, THASSAS_ORACLE);
    cast_oracle(&mut engine);
    pass_until_library_choice(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates
            .len(),
        2
    );
    engine
        .apply_command(0, &choose_cards(Vec::new()))
        .expect("complete Oracle's look at the remaining library");
    assert_eq!(engine.state.players[0].library.len(), 2);
    assert_eq!(engine.state.outcome, Some(GameOutcome::Winner(0)));
}

fn remove_oracle_before_trigger_resolves(seed: u64, library_size: usize) -> Option<GameOutcome> {
    let mut engine = game(seed, library_size, &["unsummon"]);
    let oracle = cast_oracle(&mut engine);
    pass_priority_round(&mut engine); // Resolve Oracle; its ETB trigger remains on the stack.
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| { item.source_permanent_id == Some(oracle) && item.ability_index.is_some() }));

    engine.state.players[0].mana_pool.blue = 1;
    let bounce = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(bounce, target_object(oracle)))
        .expect("bounce the source while its trigger is on the stack");
    pass_priority_round(&mut engine); // Unsummon resolves; the ETB trigger still exists.
    assert_eq!(engine.state.objects[&oracle].zone, Zone::Hand);
    pass_priority_round(&mut engine); // Resolve with zero blue devotion.
    assert!(engine.state.pending_resolution.is_none());
    engine.state.outcome
}

#[test]
fn zero_devotion_skips_the_look_but_wins_if_the_library_is_empty() {
    assert_eq!(remove_oracle_before_trigger_resolves(0x0A11_CE03, 2), None);
    assert_eq!(
        remove_oracle_before_trigger_resolves(0x0A11_CE04, 0),
        Some(GameOutcome::Winner(0))
    );
}

#[test]
fn positive_devotion_and_an_empty_library_win_without_a_choice() {
    let mut engine = game(0x0A11_CE06, 0, &[]);
    cast_oracle(&mut engine);
    pass_priority_round(&mut engine); // Resolve Oracle; trigger is put on the stack.
    pass_priority_round(&mut engine); // Its positive devotion still requires no empty-library look.
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.outcome, Some(GameOutcome::Winner(0)));
}

#[test]
fn trigger_controller_keeps_its_library_after_source_changes_controller() {
    let mut engine = game(0x0A11_CE07, 5, &[]);
    inject_permanent_on_battlefield(&mut engine, 0, THASSAS_ORACLE);
    set_library_size(&mut engine, 1, 2);
    let oracle = cast_oracle(&mut engine);
    pass_priority_round(&mut engine); // Resolve Oracle; its ETB trigger is now independent.

    let trigger = engine
        .state
        .stack
        .iter()
        .find(|item| item.source_permanent_id == Some(oracle) && item.ability_index.is_some())
        .expect("Oracle ETB trigger remains on the stack");
    assert_eq!(trigger.controller, 0);
    // Model a control change after triggering without changing the ability's controller.
    let battlefield_position = engine.state.players[0]
        .battlefield
        .iter()
        .position(|object_id| *object_id == oracle)
        .expect("Oracle is still on its owner's battlefield");
    engine.state.players[0]
        .battlefield
        .remove(battlefield_position);
    engine.state.players[1].battlefield.push(oracle);
    let source = engine.state.objects.get_mut(&oracle).unwrap();
    source.base_controller = 1;
    source.controller = 1;
    pass_priority_round(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the original trigger controller chooses from their library");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.candidates.len(), 2);
    assert!(pending
        .presentation
        .candidates
        .iter()
        .all(|object_id| engine.state.players[0].library.contains(object_id)));

    engine
        .apply_command(0, &choose_cards(Vec::new()))
        .expect("original trigger controller completes the optional choice");
    assert_eq!(engine.state.outcome, None);
    assert_eq!(engine.state.players[0].library.len(), 5);
    assert_eq!(engine.state.players[1].library.len(), 2);
}
