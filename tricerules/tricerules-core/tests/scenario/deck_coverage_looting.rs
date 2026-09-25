//! Exact deck-corpus coverage for Careful Study and Faithless Looting.
//!
//! Oracle text and rulings checked 2026-09-25. CR 121.2 governs each individual draw, CR 701.9
//! governs discarding, and CR 702.34 governs casting Faithless Looting from the graveyard and
//! exiling that physical card after it leaves the stack.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, CastMethod, CastSpell, ChoiceKind};

const CAREFUL_STUDY: &str = "careful_study";
const FAITHLESS_LOOTING: &str = "faithless_looting";

fn looting_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let objects: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    engine.state.players[player]
        .library
        .retain(|object_id| !objects.contains(object_id));
    for object_id in objects.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    objects
}

fn resolve_top_stack(engine: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_command(first, &pass())
        .expect("first player passes");
    engine
        .apply_command(second, &pass())
        .expect("second player passes and the spell resolves")
}

fn resolve_two_card_discard(engine: &mut GameEngine, known_discard: u32, drawn: &[u32]) {
    let batch = resolve_top_stack(engine);
    let choice = find_resolution_choice(&batch).expect("draw-then-discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (2, 2));
    assert!(drawn
        .iter()
        .all(|object_id| choice.candidate_object_ids.contains(object_id)));
    assert!(choice.candidate_object_ids.contains(&known_discard));
    assert!(
        engine.apply_command(1, &pass()).is_err(),
        "no player can act between the draw and discard during resolution"
    );
    engine
        .apply_command(
            choice.deciding_player_id,
            &submit_resolution_choice(vec![known_discard, drawn[0]]),
        )
        .expect("discard the known card and one newly drawn card");
    assert_eq!(engine.state.objects[&known_discard].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&drawn[0]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&drawn[1]].zone, Zone::Hand);
}

#[test]
fn careful_study_draws_two_then_discards_two_in_one_resolution() {
    let mut engine = looting_engine(20_260_940);
    let drawn = seat_on_top(&mut engine, 0, &["grizzly_bears", "storm_crow"]);
    let spell = inject_card_into_hand(&mut engine, 0, CAREFUL_STUDY);
    let known_discard = inject_card_into_hand(&mut engine, 0, "hill_giant");
    let library_before = engine.state.players[0].library.len();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, CAREFUL_STUDY);
    semantic::accepted(&mut engine, 0, &cast_spell_face(slot, vec![], 0));

    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    resolve_two_card_discard(&mut engine, known_discard, &drawn);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn faithless_looting_uses_its_flashback_cost_and_exiles_after_resolution() {
    let mut engine = looting_engine(20_260_941);
    let normal_draws = seat_on_top(&mut engine, 0, &["grizzly_bears", "storm_crow"]);
    let spell = inject_card_into_hand(&mut engine, 0, FAITHLESS_LOOTING);
    let first_discard = inject_card_into_hand(&mut engine, 0, "hill_giant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, FAITHLESS_LOOTING);
    semantic::accepted(&mut engine, 0, &cast_spell_face(slot, vec![], 0));
    resolve_two_card_discard(&mut engine, first_discard, &normal_draws);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);

    let flashback_draws = seat_on_top(&mut engine, 0, &["serra_angel", "island"]);
    let second_discard = inject_card_into_hand(&mut engine, 0, "mountain");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 3,
            ..Default::default()
        },
    );
    let generation = engine.state.zone_change_generation[&spell];
    let flashback = tricerules_proto::ruled::v1::RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            ..Default::default()
        })),
    };
    semantic::accepted(&mut engine, 0, &flashback);
    resolve_two_card_discard(&mut engine, second_discard, &flashback_draws);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Exile);
    assert!(
        engine.apply_command(0, &flashback).is_err(),
        "the exiled physical spell cannot be flashback-cast a second time"
    );
}
