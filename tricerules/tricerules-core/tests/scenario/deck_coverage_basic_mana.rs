//! Exact scoped basic lands: printed characteristics and immediate colored mana.
use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_core::GameEngine;

#[test]
fn scoped_basic_lands_tap_for_only_their_printed_color_and_reject_invalid_actors_or_reuse() {
    for (card_id, name, subtype, expected) in [
        ("island", "Island", "Island", (0, 1, 0, 0, 0, 0)),
        ("forest", "Forest", "Forest", (0, 0, 0, 0, 1, 0)),
        ("plains", "Plains", "Plains", (1, 0, 0, 0, 0, 0)),
        ("swamp", "Swamp", "Swamp", (0, 0, 1, 0, 0, 0)),
    ] {
        let card = CardRegistry::global()
            .get(card_id)
            .expect("exact basic land registered");
        assert_eq!(card.name, name);
        assert_eq!(card.face_count(), 1);
        let face = card.primary_face();
        assert_eq!(face.mana_cost.to_string(), "");
        assert_eq!(face.types, ["Land", subtype]);
        assert_eq!(face.supertypes, ["Basic"]);
        assert_eq!(face.activated_abilities.len(), 1);
        let decks = Some(vec![vec![card_id.to_owned(); 12], forest_only_deck()]);
        let mut engine = GameEngine::new(2026093006, &[0, 1], 20, decks, true).unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let hand_index = hand_index_for_card(&engine, 0, card_id);
        engine.apply_command(0, &play_land(hand_index)).unwrap();
        let land = battlefield_object_for_card(&engine, 0, card_id);
        let activation = activate_ability_for(&engine, land, 0, vec![]);
        let command_index = engine.state.command_index;
        engine
            .apply_command(1, &activation)
            .expect_err("another player cannot tap this land");
        assert_eq!(engine.state.command_index, command_index);
        assert!(!engine.state.objects[&land].tapped);
        assert_eq!(engine.state.players[0].mana_pool, Default::default());
        let priority = engine.state.priority_player_id();
        let batch = engine.apply_command(0, &activation).unwrap();
        let pool = &engine.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            expected
        );
        assert!(engine.state.objects[&land].tapped);
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.priority_player_id(), priority);
        assert_eq!(engine.state.players[1].mana_pool, Default::default());
        let event = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::ManaPoolUpdated(pool)) if pool.player_id == 0 => Some(pool),
                _ => None,
            })
            .expect("authoritative pool update");
        assert_eq!(
            (event.w, event.u, event.b, event.r, event.g, event.c),
            expected
        );
        let command_index = engine.state.command_index;
        engine
            .apply_command(0, &activate_ability_for(&engine, land, 0, vec![]))
            .expect_err("tapped land cannot pay tap again");
        assert_eq!(engine.state.command_index, command_index);
        let pool = &engine.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            expected
        );
    }
}
