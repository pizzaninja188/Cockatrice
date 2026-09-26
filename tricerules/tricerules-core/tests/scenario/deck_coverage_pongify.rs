//! Actual-card semantics for Pongify, checked against its pinned Oracle and ruling.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{GameEngine, Zone};

fn pongify_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["pongify"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_pongify(engine: &mut GameEngine, target: u32) {
    ensure_in_hand(engine, 0, "pongify");
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "pongify");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Pongify");
    resolve_entire_stack_two_player(engine);
}

fn assert_ape_is_controlled_by(engine: &GameEngine, player: usize) {
    let apes = battlefield_token_oids(engine, player, "ape_g_3_3");
    assert_eq!(apes.len(), 1);
    let ape = apes[0];
    assert_eq!(engine.state.objects[&ape].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(ape), Some(3));
    assert_eq!(engine.effective_toughness(ape), Some(3));
    assert_eq!(
        engine.characteristics(ape).unwrap().colors,
        vec![tricerules_cards::Color::Green]
    );
}

#[test]
fn pongify_destroys_through_regeneration_shield_and_gives_target_controller_ape() {
    let mut engine = pongify_engine(202_609_261);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .regeneration_shields = 1;

    resolve_pongify(&mut engine, target);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_ape_is_controlled_by(&engine, 1);
    assert!(battlefield_token_oids(&engine, 0, "ape_g_3_3").is_empty());
}

#[test]
fn pongify_still_gives_ape_when_target_is_indestructible() {
    let mut engine = pongify_engine(202_609_262);
    let target = inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    assert!(engine.effective_has_keyword(target, Keyword::Indestructible));

    resolve_pongify(&mut engine, target);

    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_ape_is_controlled_by(&engine, 1);
    assert!(battlefield_token_oids(&engine, 0, "ape_g_3_3").is_empty());
}

#[test]
fn pongify_creates_no_ape_when_target_is_illegal_on_resolution() {
    let mut engine = pongify_engine(202_609_263);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "pongify");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "pongify");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Pongify");
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    engine.state.players[1].graveyard.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;

    resolve_entire_stack_two_player(&mut engine);

    assert!(battlefield_token_oids(&engine, 0, "ape_g_3_3").is_empty());
    assert!(battlefield_token_oids(&engine, 1, "ape_g_3_3").is_empty());
}
