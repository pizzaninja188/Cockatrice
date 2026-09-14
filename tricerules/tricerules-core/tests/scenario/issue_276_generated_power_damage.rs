//! Issue #276 — Hard-Hitting Question and Bite Down reuse the creature power-damage effect
//! with an exact creature-or-planeswalker target union. CR 115, 119.2, 120.1, 120.2b, 120.3c,
//! and 608.2b govern target publication, resolution-time power, and loyalty damage.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{TargetRef, TargetRefKind};

fn permanent_target(object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

fn leave_battlefield_to_hand(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].hand.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

#[test]
fn generated_power_damage_publishes_and_resolves_against_a_planeswalker() {
    let mut engine = GameEngine::new(276_001, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let source = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 4, 4);
    let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&walker)
        .expect("Jace object")
        .counters
        .insert(CounterKind::Loyalty, 6);
    inject_card_into_hand(&mut engine, 0, "hard-hitting_question");
    let slot = hand_index_for_card(&engine, 0, "hard-hitting_question");

    let legal = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    assert_eq!(legal.groups[0].valid_permanent_ids, [source]);
    assert_eq!(legal.groups[1].valid_permanent_ids, [walker]);

    engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![permanent_target(source, 0), permanent_target(walker, 1)],
            ),
        )
        .expect("cast Hard-Hitting Question at Jace");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        2,
        "Jace loses loyalty equal to the source creature's current power"
    );
    assert_eq!(engine.state.objects[&source].damage, 0);
}

#[test]
fn generated_instant_power_damage_publishes_and_resolves_against_a_planeswalker() {
    let mut engine = GameEngine::new(276_002, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let source = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 3, 3);
    let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&walker)
        .expect("Jace object")
        .counters
        .insert(CounterKind::Loyalty, 5);
    inject_card_into_hand(&mut engine, 0, "bite_down");
    let slot = hand_index_for_card(&engine, 0, "bite_down");
    let legal = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    assert_eq!(legal.groups[0].valid_permanent_ids, [source]);
    assert_eq!(legal.groups[1].valid_permanent_ids, [walker]);

    engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![permanent_target(source, 0), permanent_target(walker, 1)],
            ),
        )
        .expect("cast Bite Down at Jace");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        2
    );
}

#[test]
fn generated_power_damage_rejects_forged_roles_and_fizzles_on_stale_target() {
    let mut forged = GameEngine::new(276_003, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut forged);
    give_mana(
        &mut forged,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let source = inject_creature_on_battlefield(&mut forged, 0, "grizzly_bears");
    let walker = inject_permanent_on_battlefield(&mut forged, 1, "jace_beleren");
    forged
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .counters
        .insert(CounterKind::Loyalty, 4);
    inject_card_into_hand(&mut forged, 0, "bite_down");
    let slot = hand_index_for_card(&forged, 0, "bite_down");
    let hand_before = forged.state.players[0].hand.clone();
    let mana_before = forged.state.players[0].mana_pool;
    let command_index = forged.state.command_index;
    assert!(forged
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![permanent_target(walker, 0), permanent_target(source, 1)],
            ),
        )
        .is_err());
    assert_eq!(forged.state.command_index, command_index);
    assert_eq!(forged.state.players[0].hand, hand_before);
    assert_eq!(forged.state.players[0].mana_pool, mana_before);

    forged
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![permanent_target(source, 0), permanent_target(walker, 1)],
            ),
        )
        .expect("cast legal Bite Down before target departure");
    leave_battlefield_to_hand(&mut forged, 1, walker);
    resolve_entire_stack_two_player(&mut forged);
    assert_eq!(
        forged.state.objects[&walker].counter_count(CounterKind::Loyalty),
        4,
        "a departed target's new zone-change generation must fizzle the spell"
    );
    assert_eq!(forged.state.objects[&walker].zone, Zone::Hand);
}
