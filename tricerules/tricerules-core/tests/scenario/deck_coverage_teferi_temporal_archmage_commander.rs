//! Teferi, Temporal Archmage command-zone declaration and ordinary mainboard admission.
use super::helpers::*;
use tricerules_cards::Color;
use tricerules_core::{EngineDeck, GameEngine, Zone};

const TEFERI: &str = "teferi,_temporal_archmage";
const P10: i32 = 10;
const P20: i32 = 20;

#[test]
fn teferi_is_a_declared_blue_commander_and_a_separate_mainboard_card() {
    let registry = tricerules_cards::registry::global();
    let commander_game = GameEngine::new_with_commander_decks(
        registry,
        26_102_733,
        &[P10, P20],
        20,
        Some(vec![
            EngineDeck {
                mainboard: deck_with("island", &[]),
                commanders: vec![TEFERI.into()],
            },
            EngineDeck {
                mainboard: deck_with("island", &[]),
                commanders: vec![],
            },
        ]),
        true,
    )
    .expect("Teferi may be declared in the Commander slot");

    let p10 = &commander_game.state.players[0];
    assert!(p10.has_declared_commander);
    assert_eq!(p10.command_zone.len(), 1);
    let commander_id = p10.command_zone[0];
    assert_eq!(p10.declared_commander_object_ids, vec![commander_id]);
    assert_eq!(p10.color_identity, vec![Color::Blue]);
    let commander = &commander_game.state.objects[&commander_id];
    assert_eq!(commander.id, commander_id);
    assert_eq!(commander.owner, P10);
    assert_eq!(commander.card_id, TEFERI);
    assert_eq!(commander.zone, Zone::Command);
    assert_eq!(
        commander_game
            .state
            .objects
            .values()
            .filter(|object| object.owner == P10 && object.card_id == TEFERI)
            .count(),
        1,
        "the declared Teferi has one physical identity and no mainboard duplicate"
    );
    assert!(!p10
        .hand
        .iter()
        .chain(p10.library.iter())
        .any(|object_id| commander_game.state.objects[object_id].card_id == TEFERI));

    let p20 = &commander_game.state.players[1];
    assert!(!p20.has_declared_commander);
    assert!(p20.command_zone.is_empty());
    assert!(p20.declared_commander_object_ids.is_empty());
    assert!(p20.color_identity.is_empty());

    // Keep the ordinary mainboard admission check independent from the Commander singleton
    // fixture: this is a separate game with no declared commanders.
    let ordinary_game = GameEngine::new(
        registry,
        26_102_734,
        &[P10, P20],
        20,
        Some(vec![
            deck_with("island", &[TEFERI]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("the full Teferi card is admitted in an ordinary mainboard");
    let ordinary_p10 = &ordinary_game.state.players[0];
    assert!(!ordinary_p10.has_declared_commander);
    assert!(ordinary_p10.command_zone.is_empty());
    let mainboard_teferi = ordinary_game
        .state
        .objects
        .values()
        .find(|object| object.owner == P10 && object.card_id == TEFERI)
        .expect("the Teferi mainboard card has its own physical object");
    assert!(matches!(mainboard_teferi.zone, Zone::Hand | Zone::Library));
    assert_eq!(
        ordinary_game
            .state
            .objects
            .values()
            .filter(|object| object.owner == P10 && object.card_id == TEFERI)
            .count(),
        1
    );
}
