//! Tainted Treats: CR 608.2b/h target legality and battlefield mana value LKI.

use crate::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::state::CopiableValues;
use tricerules_core::Zone;

fn prepared(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &["tainted_treats"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "tainted_treats");
    engine
}

fn cast_and_resolve(engine: &mut GameEngine, target: u32) {
    give_mana(
        engine,
        0,
        ManaGift {
            b: 1,
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "tainted_treats");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
}

#[test]
fn tainted_treats_destroys_low_value_creature_and_creates_food() {
    let mut engine = prepared(480_101);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_and_resolve(&mut engine, target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);
}

#[test]
fn tainted_treats_destroys_high_value_creature_without_food() {
    let mut engine = prepared(480_102);
    let target = inject_creature_on_battlefield(&mut engine, 1, "serra_angel");
    cast_and_resolve(&mut engine, target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(battlefield_token_oids(&engine, 0, "food").is_empty());
}

#[test]
fn tainted_treats_creates_food_when_low_value_target_is_indestructible() {
    let mut engine = prepared(480_103);
    let target = inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    cast_and_resolve(&mut engine, target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);
}

#[test]
fn tainted_treats_illegal_target_prevents_food() {
    let mut engine = prepared(480_104);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "tainted_treats");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
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
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(battlefield_token_oids(&engine, 0, "food").is_empty());
}

#[test]
fn tainted_treats_uses_copied_battlefield_mana_value_after_destroy() {
    let mut engine = prepared(480_105);
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
    cast_and_resolve(&mut engine, target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(battlefield_token_oids(&engine, 0, "food").is_empty());
}

#[test]
fn tainted_treats_treats_x_as_zero_on_the_battlefield() {
    let mut engine = prepared(480_108);
    let target = inject_creature_on_battlefield(&mut engine, 1, "endless_one");
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 5);
    cast_and_resolve(&mut engine, target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);
}

#[test]
fn tainted_treats_can_destroy_artifact_and_rejects_enchantment() {
    let mut engine = prepared(480_106);
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "carrot_cake");
    cast_and_resolve(&mut engine, artifact);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);

    let mut engine = prepared(480_107);
    let enchantment = inject_permanent_on_battlefield(&mut engine, 1, "bad_moon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "tainted_treats");
    assert!(engine
        .apply_command(0, &cast_spell(slot, target_object(enchantment)))
        .is_err());
    assert!(battlefield_token_oids(&engine, 0, "food").is_empty());
}
