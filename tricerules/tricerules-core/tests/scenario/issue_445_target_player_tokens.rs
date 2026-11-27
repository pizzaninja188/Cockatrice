//! #445 "Target player creates ..." token modes on Ashling's Command and Brigid's Command. The
//! token recipient is the mode's own player target (CR 115.1, 111.2), never the caster by default.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

fn game(seed: u64, players: &[i32]) -> GameEngine {
    let decks = Some(
        players
            .iter()
            .map(|_| deck_with("mountain", &["grizzly_bears", "coral_merfolk"]))
            .collect(),
    );
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        players,
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn prepare(engine: &mut GameEngine, id: &str) -> usize {
    inject_card_into_hand(engine, 0, id);
    grant_pool(engine, 0);
    hand_index_for_card(engine, 0, id)
}

fn resolve_all(engine: &mut GameEngine) {
    loop {
        if engine.state.stack.is_empty() {
            return;
        }
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("pass priority");
    }
}

#[test]
fn issue_445_ashling_targeted_opponent_creates_two_treasures_and_caster_none() {
    let mut engine = game(445_001, &[0, 1]);
    let slot = prepare(&mut engine, "ashlings_command");
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_scenario_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_player(0)), (3, target_player(1))]),
        )
        .expect("draw for self, Treasures for the opponent");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 1, "treasure").len(), 2);
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before + 1,
        "cast one, draw two"
    );
    assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
}

#[test]
fn issue_445_ashling_can_target_its_caster_for_treasures() {
    let mut engine = game(445_002, &[0, 1]);
    let theirs = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let slot = prepare(&mut engine, "ashlings_command");
    engine
        .apply_scenario_command(
            0,
            &cast_modal_spell(slot, vec![(2, target_player(1)), (3, target_player(0))]),
        )
        .expect("damage the opponent's team and make our own Treasures");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 2);
    assert!(battlefield_token_oids(&engine, 1, "treasure").is_empty());
    assert_eq!(
        engine.state.objects[&theirs].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn issue_445_brigid_multiplayer_gives_only_the_target_a_green_white_kithkin() {
    let mut engine = game(445_003, &[0, 1, 2]);
    let ours = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let slot = prepare(&mut engine, "brigids_command");
    engine
        .apply_scenario_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_player(2)), (2, target_object(ours))]),
        )
        .expect("Kithkin for player two and pump our creature");
    resolve_all(&mut engine);
    let kithkin = battlefield_token_oids(&engine, 2, "kithkin_gw_1_1");
    assert_eq!(kithkin.len(), 1);
    assert!(battlefield_token_oids(&engine, 0, "kithkin_gw_1_1").is_empty());
    assert!(battlefield_token_oids(&engine, 1, "kithkin_gw_1_1").is_empty());
    assert_eq!(
        (
            engine.effective_power(kithkin[0]),
            engine.effective_toughness(kithkin[0])
        ),
        (Some(1), Some(1))
    );
    assert_eq!(
        (
            engine.effective_power(ours),
            engine.effective_toughness(ours)
        ),
        (Some(5), Some(5))
    );
}

#[test]
fn issue_445_token_mode_creates_nothing_when_its_target_player_has_left() {
    let mut engine = game(445_004, &[0, 1, 2]);
    let slot = prepare(&mut engine, "ashlings_command");
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_scenario_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_player(0)), (3, target_player(2))]),
        )
        .expect("draw for self, Treasures for player two");
    engine.state.players[2].has_lost = true;
    resolve_all(&mut engine);
    assert!((0..3).all(|player| battlefield_token_oids(&engine, player, "treasure").is_empty()));
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before + 1,
        "the legal draw mode still resolves"
    );
}

#[test]
fn issue_445_token_mode_rejects_an_object_or_absent_player_target() {
    let mut engine = game(445_005, &[0, 1]);
    let bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let slot = prepare(&mut engine, "brigids_command");
    assert!(engine
        .apply_scenario_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(1, target_object(bears)), (2, target_object(bears))]
            )
        )
        .is_err());
    assert!(engine
        .apply_scenario_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(1, target_player(999)), (2, target_object(bears))]
            )
        )
        .is_err());
}
