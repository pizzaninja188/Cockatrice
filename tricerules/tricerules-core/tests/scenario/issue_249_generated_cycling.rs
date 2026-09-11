//! Issue #249 — generated Cycling and basic-land-typecycling abilities reuse the engine-owned
//! hidden-zone activation and private library-search contracts delivered by issue #101.
//!
//! Oracle and rulings checked 2026-09-11. CR 702.29a defines Cycling as a hand-only activated
//! ability with mana and discard-self costs followed by a draw. CR 702.29e-f define typecycling
//! as the corresponding revealed search-to-hand followed by a shuffle.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, AbilitySourceZone, ActivateAbility, RuledCommand,
};

fn hand_ability(engine: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

#[test]
fn generated_lightshield_parry_cycles_atomically_from_hand_and_draws() {
    let decks = Some(vec![
        deck_with("plains", &["lightshield_parry"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(249_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lightshield_parry");
    let source = engine.state.players[0].hand[hand_index_for_card(&engine, 0, "lightshield_parry")];
    let drawn = inject_library_card(&mut engine, 0, "grizzly_bears");
    engine.state.players[0].library.retain(|oid| *oid != drawn);
    engine.state.players[0].library.push_front(drawn);

    let batch = engine.initial_response_batch();
    assert!(batch.legal_by_player[&0]
        .zone_ability_actions
        .iter()
        .any(
            |action| action.object_id == source && action.source_zone() == AbilitySourceZone::Hand
        ));
    assert!(batch.legal_by_player[&1].zone_ability_actions.is_empty());

    let command = hand_ability(&engine, source);
    engine
        .apply_command(0, &command)
        .expect_err("insufficient mana rejects the complete cost");
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine.apply_command(0, &command).expect("activate Cycling");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);
    engine
        .apply_command(0, &command)
        .expect_err("the discarded source generation cannot be replayed");
}

#[test]
fn generated_bedhead_beastie_searches_exact_mountains_reveals_and_shuffles() {
    let decks = Some(vec![
        deck_with("forest", &["bedhead_beastie"]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(249_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "bedhead_beastie");
    let source = engine.state.players[0].hand[hand_index_for_card(&engine, 0, "bedhead_beastie")];
    let mountain = inject_library_card(&mut engine, 0, "mountain");
    let forest = inject_library_card(&mut engine, 0, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &hand_ability(&engine, source))
        .expect("activate Mountaincycling");
    engine.apply_command(0, &pass()).expect("controller passes");
    let search = engine
        .apply_command(1, &pass())
        .expect("typecycling resolves to a private search");
    let choice = find_resolution_choice(&search).expect("private library search");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, [mountain]);
    assert!(!choice.candidate_object_ids.contains(&forest));

    let completed = engine
        .apply_command(0, &submit_resolution_choice(vec![mountain]))
        .expect("choose Mountain");
    assert_eq!(engine.state.objects[&mountain].zone, Zone::Hand);
    assert!(completed.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::CardsRevealed(reveal))
            if reveal.cards.len() == 1 && reveal.cards[0].object_id == mountain
    )));
    assert!(completed.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
    )));
}
