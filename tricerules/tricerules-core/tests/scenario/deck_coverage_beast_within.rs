//! Actual-card semantics for Beast Within, checked against the pinned Oracle and current rulings.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["beast_within"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_beast_within(engine: &mut GameEngine, target: u32) {
    ensure_in_hand(engine, 0, "beast_within");
    give_mana(
        engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "beast_within");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Beast Within");
    resolve_entire_stack_two_player(engine);
}

fn assert_beast_is_controlled_by(engine: &GameEngine, player: usize) {
    let beasts = battlefield_token_oids(engine, player, "beast_g_3_3");
    assert_eq!(beasts.len(), 1);
    let beast = beasts[0];
    assert_eq!(engine.state.objects[&beast].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(beast), Some(3));
    assert_eq!(engine.effective_toughness(beast), Some(3));
}

#[test]
fn beast_within_destroys_a_land_and_gives_the_beast_to_its_controller() {
    let mut engine = engine(202_609_260);
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");

    resolve_beast_within(&mut engine, land);

    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_beast_is_controlled_by(&engine, 1);
    assert!(battlefield_token_oids(&engine, 0, "beast_g_3_3").is_empty());
}

#[test]
fn beast_within_gives_the_beast_to_a_foreign_controller_after_destroying_the_owner_card() {
    let mut engine = engine(202_609_263);
    // Player 0 owns the target; player 1 controls it. Destroy sends it to its owner's graveyard,
    // while the token recipient is determined from the target's last-known controller.
    let target = inject_creature_under_foreign_control(&mut engine, 0, 1, "grizzly_bears");

    resolve_beast_within(&mut engine, target);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&target));
    assert!(!engine.state.players[1].graveyard.contains(&target));
    assert_beast_is_controlled_by(&engine, 1);
    assert!(battlefield_token_oids(&engine, 0, "beast_g_3_3").is_empty());
}

#[test]
fn beast_within_still_creates_a_beast_if_its_legal_target_is_indestructible() {
    let mut engine = engine(202_609_261);
    let myr = inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    assert!(engine.effective_has_keyword(myr, Keyword::Indestructible));

    resolve_beast_within(&mut engine, myr);

    assert_eq!(engine.state.objects[&myr].zone, Zone::Battlefield);
    assert_beast_is_controlled_by(&engine, 1);
    assert!(battlefield_token_oids(&engine, 0, "beast_g_3_3").is_empty());
}

#[test]
fn beast_within_creates_no_beast_if_its_target_is_illegal_on_resolution() {
    let mut engine = engine(202_609_262);
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    ensure_in_hand(&mut engine, 0, "beast_within");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "beast_within");
    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect("cast Beast Within");
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != land);
    engine.state.players[1].graveyard.push(land);
    engine.state.objects.get_mut(&land).unwrap().zone = Zone::Graveyard;
    *engine.state.zone_change_generation.entry(land).or_default() += 1;

    resolve_entire_stack_two_player(&mut engine);

    assert!(battlefield_token_oids(&engine, 0, "beast_g_3_3").is_empty());
    assert!(battlefield_token_oids(&engine, 1, "beast_g_3_3").is_empty());
}
