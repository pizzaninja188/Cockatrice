//! Feed the Swarm: one legal target, ordered destroy/life loss, and CR 608.2b failure.

use crate::helpers::*;
use tricerules_core::state::CopiableValues;
use tricerules_core::Zone;

fn prepared(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &["feed_the_swarm"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "feed_the_swarm");
    engine
}

fn cast(engine: &mut GameEngine, target: u32) {
    give_mana(
        engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "feed_the_swarm");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
}

fn resolve(engine: &mut GameEngine) {
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
}

#[test]
fn feed_the_swarm_destroys_an_opponents_creature_and_loses_its_mana_value() {
    let mut engine = prepared(483_101);
    let target = inject_creature_on_battlefield(&mut engine, 1, "serra_angel");
    cast(&mut engine, target);
    resolve(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, 15);
}

#[test]
fn feed_the_swarm_loses_life_when_legal_indestructible_target_survives() {
    let mut engine = prepared(483_102);
    let target = inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    cast(&mut engine, target);
    resolve(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].life, 17);
}

#[test]
fn feed_the_swarm_illegal_target_prevents_both_instructions() {
    let mut engine = prepared(483_103);
    let target = inject_creature_on_battlefield(&mut engine, 1, "serra_angel");
    cast(&mut engine, target);
    engine.state.players[1]
        .battlefield
        .retain(|&id| id != target);
    engine.state.players[1].exile.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Exile;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].life, 20);
}

#[test]
fn feed_the_swarm_destroys_an_opponents_enchantment() {
    let mut engine = prepared(483_104);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "bad_moon");
    cast(&mut engine, target);
    resolve(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, 18);
}

#[test]
fn feed_the_swarm_rejects_a_permanent_the_caster_controls() {
    let mut engine = prepared(483_105);
    let own = inject_creature_on_battlefield(&mut engine, 0, "serra_angel");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "feed_the_swarm");
    assert!(engine
        .apply_command(0, &cast_spell(slot, target_object(own)))
        .is_err());
    assert_eq!(engine.state.objects[&own].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].life, 20);
}

#[test]
fn feed_the_swarm_uses_copied_battlefield_mana_value_after_destroy() {
    let mut engine = prepared(483_106);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let copied = tricerules_cards::CardRegistry::global()
        .get("serra_angel")
        .unwrap()
        .primary_face()
        .clone();
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        source_card_id: "serra_angel".into(),
        source_face_index: 0,
        display_name: copied.name.clone(),
        face: copied,
        room_faces: None,
    });
    cast(&mut engine, target);
    resolve(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, 15);
}
