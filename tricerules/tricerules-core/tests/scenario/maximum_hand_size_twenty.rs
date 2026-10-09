//! Engine regression for Twenty-Toed Toad's fixed maximum hand size (CR 613.11, 514.1).

use super::helpers::*;
use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_core::{state::CopiableValues, GameEngine, TurnStep};

#[test]
fn fixed_twenty_hand_limit_does_not_request_cleanup_discard_at_twenty() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_260_929,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    // Give an existing permanent the test-only Toad-style static ability. The exact card RON
    // is deliberately absent until the primitive's red/green engine behavior is established.
    let source = inject_permanent_on_battlefield(&mut engine, 0, "spellbook");
    let mut face = tricerules_cards::registry::global()
        .get("spellbook")
        .expect("Spellbook is registered")
        .primary_face()
        .clone();
    face.static_abilities[0].definition = StaticAbilityDef::MaximumHandSizeTwenty;
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        source_card_id: "spellbook".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });

    while engine.state.players[0].hand.len() < 20 {
        inject_card_into_hand(&mut engine, 0, "forest");
    }
    engine.state.turn_step = TurnStep::EndStep;
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine.apply_command(1, &pass()).expect("opponent passes");

    assert_eq!(engine.state.cleanup_discard_player, None);
    assert_eq!(engine.state.players[0].hand.len(), 20);
    assert_eq!(engine.state.active_player_id(), 1);
}
