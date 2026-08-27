//! Issue #125: turn-scoped replacements that exile one exact permanent if it would die.

use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_event::Ev;

fn cast_damage_spell(engine: &mut GameEngine, player: i32, card_id: &str, target: u32) {
    ensure_in_hand(engine, player as usize, card_id);
    give_mana(
        engine,
        player,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, player as usize, card_id);
    engine
        .apply_command(player, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
    pass_both_players(engine);
}

#[test]
fn lava_coil_exiles_its_lethally_damaged_target() {
    let decks = Some(vec![
        deck_with("mountain", &["lava_coil"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(12_501, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);

    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    cast_damage_spell(&mut engine, 0, "lava_coil", target);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.turn_history.current.creatures_died, 0);
}

#[test]
fn prevented_damage_still_exiles_a_later_sacrifice_without_a_dies_event() {
    let decks = Some(vec![
        deck_with("mountain", &["lava_coil", "village_rites", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(12_502, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.add_damage_prevention_shield(target, 4);

    cast_damage_spell(&mut engine, 0, "lava_coil", target);
    assert_eq!(engine.state.objects[&target].damage, 0);

    ensure_in_hand(&mut engine, 0, "village_rites");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "village_rites");
    let batch = engine
        .apply_command(
            0,
            &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, target)]),
        )
        .expect("sacrifice target to Village Rites");

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.turn_history.current.creatures_died, 0);
    assert!(batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::PermanentMoved(moved))
                if moved.object_id == target
                    && moved.destination
                        == tricerules_proto::ruled::v1::permanent_moved::Destination::Exile as i32
        )
    }));
}

#[test]
fn regeneration_replaces_the_first_destruction_but_not_a_later_death() {
    let decks = Some(vec![
        deck_with("mountain", &["scorching_dragonfire"]),
        deck_with("forest", &["cudgel_troll"]),
    ]);
    let mut engine = GameEngine::new(12_503, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "cudgel_troll", false);
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .regeneration_shields = 1;

    cast_damage_spell(&mut engine, 0, "scorching_dragonfire", target);
    let object = &engine.state.objects[&target];
    assert_eq!(object.zone, Zone::Battlefield);
    assert_eq!(object.damage, 0);
    assert_eq!(object.regeneration_shields, 0);

    engine.state.objects.get_mut(&target).unwrap().damage = 3;
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.turn_history.current.creatures_died, 0);
}

#[test]
fn toughness_zero_and_simultaneous_unmarked_death_use_the_actual_destinations() {
    let decks = Some(vec![
        deck_with("mountain", &["scorching_dragonfire", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(12_504, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let marked = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let ordinary = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.add_damage_prevention_shield(marked, 3);
    cast_damage_spell(&mut engine, 0, "scorching_dragonfire", marked);

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(marked),
        kind: ContinuousEffectKind::PtModify {
            delta_power: 0,
            delta_toughness: -2,
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    engine.state.objects.get_mut(&ordinary).unwrap().damage = 2;
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&marked].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&ordinary].zone, Zone::Graveyard);
    assert_eq!(engine.state.turn_history.current.creatures_died, 1);
}

#[test]
fn zone_change_generation_and_cleanup_each_end_the_replacement() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["scorching_dragonfire", "scorching_dragonfire", "unsummon"],
        ),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(12_505, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);

    let bounced = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine.state.add_damage_prevention_shield(bounced, 3);
    cast_damage_spell(&mut engine, 0, "scorching_dragonfire", bounced);
    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(bounced)))
        .expect("cast Unsummon");
    pass_both_players(&mut engine);
    engine.state.players[1].hand.retain(|oid| *oid != bounced);
    engine.state.players[1].battlefield.push(bounced);
    let returned = bounced;
    let returned_object = engine
        .state
        .objects
        .get_mut(&returned)
        .expect("returned bear");
    returned_object.zone = Zone::Battlefield;
    returned_object.summoning_sick = false;
    engine.state.objects.get_mut(&returned).unwrap().damage = 2;
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&returned].zone, Zone::Graveyard);

    let expired = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine.state.add_damage_prevention_shield(expired, 3);
    cast_damage_spell(&mut engine, 0, "scorching_dragonfire", expired);
    let expiring_turn = engine.state.active_player_id();
    for _ in 0..8 {
        if engine.state.active_player_id() != expiring_turn {
            break;
        }
        engine
            .apply_command(expiring_turn, &primitive_yield())
            .expect("advance marked target through cleanup");
        resolve_cleanup_discards_if_any(&mut engine);
    }
    assert_ne!(engine.state.active_player_id(), expiring_turn);
    engine.state.objects.get_mut(&expired).unwrap().damage = 2;
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&expired].zone, Zone::Graveyard);
}
