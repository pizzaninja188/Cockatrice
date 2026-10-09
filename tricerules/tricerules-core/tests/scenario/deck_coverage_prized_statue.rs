//! Actual-card coverage for Prized Statue's entry and battlefield-to-graveyard triggers.
//!
//! The exact Oracle text and empty Scryfall ruling response were checked 2026-09-26. CR 603.2
//! governs each matching event; CR 603.10a covers the zone-change trigger's prior-state lookup.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

const PRIZED_STATUE: &str = "prized_statue";
const TREASURE_TOKEN: &str = "treasure";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

#[test]
fn prized_statue_creates_a_treasure_on_entry_and_when_put_into_a_graveyard() {
    let mut engine = engine(202_609_300);
    let statue = inject_card_into_hand(&mut engine, 0, PRIZED_STATUE);
    let slot = hand_index_for_card(&engine, 0, PRIZED_STATUE);
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Prized Statue");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&statue].zone, Zone::Battlefield);
    assert_eq!(battlefield_token_oids(&engine, 0, TREASURE_TOKEN).len(), 1);

    inject_card_into_hand(&mut engine, 0, "disenchant");
    let disenchant_slot = hand_index_for_card(&engine, 0, "disenchant");
    engine
        .apply_command(0, &cast_spell(disenchant_slot, target_object(statue)))
        .expect("target Prized Statue with Disenchant");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&statue].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&engine, 0, TREASURE_TOKEN).len(), 2);
    assert!(battlefield_token_oids(&engine, 1, TREASURE_TOKEN).is_empty());
}
