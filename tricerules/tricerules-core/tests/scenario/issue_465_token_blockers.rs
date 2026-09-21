//! Reviewed scenarios for the #465 token-definition batch: Dwarven Castle Guard, Synapse
//! Necromage, and Scalestorm Summoner.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 111.1 (tokens),
//! 603.6c (dies triggers), 508.1/603.2c (attack triggers), and 608.2 (a conditional instruction
//! is evaluated as it resolves).

use super::helpers::*;
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

/// A two-player game advanced to the declare-attackers step with both pools refilled.
fn combat_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

/// Kill `source` with Murder so the committed battlefield-to-graveyard move runs through the
/// state-based-action funnel and emits the CR 603.6c dies event.
fn kill_with_murder(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    let slot = hand_index_for_card(e, 0, "murder");
    e.apply_command(0, &cast_spell(slot, target_object(source)))
        .expect("cast Murder");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("murder resolves");
}

fn remove_from_battlefield(e: &mut GameEngine, player: usize, object_id: u32) {
    e.state.players[player]
        .battlefield
        .retain(|id| *id != object_id);
    e.state.players[player].graveyard.push(object_id);
    e.state.objects.get_mut(&object_id).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object_id).or_default() += 1;
}

#[test]
fn issue_465_dwarven_castle_guard() {
    let mut e = engine(722_001);
    let source = inject_creature_with_stats(&mut e, 0, "dwarven_castle_guard", 2, 1);
    kill_with_murder(&mut e, source);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "hero_c_1_1").len(),
        1,
        "the dies trigger creates one Hero token"
    );
}

#[test]
fn issue_465_synapse_necromage() {
    let mut e = engine(722_002);
    let source = inject_creature_with_stats(&mut e, 0, "synapse_necromage", 3, 1);
    kill_with_murder(&mut e, source);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    let fungus = battlefield_token_oids(&e, 0, "fungus_b_1_1_cant_block");
    assert_eq!(
        fungus.len(),
        2,
        "the dies trigger creates exactly two Fungus tokens"
    );
}

#[test]
fn issue_465_scalestorm_summoner() {
    // Without a creature that has power 4 or greater, the attack creates no Dinosaur, but the
    // ability still triggers (its condition is not an intervening "if").
    let mut weak = combat_engine(722_003);
    let summoner = inject_creature_on_battlefield(&mut weak, 0, "scalestorm_summoner");
    weak.apply_command(0, &declare_attackers(vec![summoner]))
        .expect("declare the Summoner as an attacker");
    assert_eq!(
        weak.state.stack.len(),
        1,
        "the attack trigger is placed even without a power-4 creature"
    );
    resolve_entire_stack_two_player(&mut weak);
    assert!(
        battlefield_token_oids(&weak, 0, "dinosaur_r_3_1").is_empty(),
        "a 3/3 source alone does not satisfy the power-4 condition"
    );

    // With a 4-power creature the attack creates one Dinosaur.
    let mut strong = combat_engine(722_004);
    let summoner = inject_creature_on_battlefield(&mut strong, 0, "scalestorm_summoner");
    inject_creature_with_stats(&mut strong, 0, "grizzly_bears", 4, 4);
    strong
        .apply_command(0, &declare_attackers(vec![summoner]))
        .expect("declare the Summoner as an attacker");
    resolve_entire_stack_two_player(&mut strong);
    assert_eq!(
        battlefield_token_oids(&strong, 0, "dinosaur_r_3_1").len(),
        1,
        "the power-4 condition is met and one Dinosaur token is created"
    );

    // The condition is checked only as the ability resolves: raising a creature's power after the
    // trigger is on the stack still creates the Dinosaur (matching the printed ruling).
    let mut response = combat_engine(722_006);
    let summoner = inject_creature_on_battlefield(&mut response, 0, "scalestorm_summoner");
    response
        .apply_command(0, &declare_attackers(vec![summoner]))
        .expect("declare the Summoner as an attacker");
    assert_eq!(response.state.stack.len(), 1);
    inject_creature_with_stats(&mut response, 0, "grizzly_bears", 4, 4);
    resolve_entire_stack_two_player(&mut response);
    assert_eq!(
        battlefield_token_oids(&response, 0, "dinosaur_r_3_1").len(),
        1,
        "the condition is rechecked at resolution, so power added in response qualifies"
    );

    // The condition is rechecked as the trigger resolves: losing the only 4-power creature
    // before resolution means no Dinosaur.
    let mut intervening = combat_engine(722_005);
    let summoner = inject_creature_on_battlefield(&mut intervening, 0, "scalestorm_summoner");
    let big = inject_creature_with_stats(&mut intervening, 0, "grizzly_bears", 4, 4);
    intervening
        .apply_command(0, &declare_attackers(vec![summoner]))
        .expect("declare the Summoner as an attacker");
    remove_from_battlefield(&mut intervening, 0, big);
    resolve_entire_stack_two_player(&mut intervening);
    assert!(
        battlefield_token_oids(&intervening, 0, "dinosaur_r_3_1").is_empty(),
        "the condition is evaluated at resolution, after the 4-power creature left"
    );
}
