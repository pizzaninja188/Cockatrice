//! Actual-card coverage for Khalni Ambush // Khalni Territory.
//!
//! Scryfall Oracle and rulings checked 2026-09-26. CR 701.14a-b covers fighting and its
//! both-target legality; CR 608.2b covers target rechecks; CR 712.11b-c and 712.12 cover choosing
//! the modal double-faced card's spell or land face; CR 614.1c and 605.1a/605.3b cover the back
//! face entering tapped and its immediate mana ability.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

const KHALNI: &str = "khalni_ambush_khalni_territory";

fn khalni_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn creature_targets(you_control: u32, not_you_control: u32) -> Vec<TargetRef> {
    vec![
        TargetRef {
            object_id: you_control,
            group_index: 0,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        },
        TargetRef {
            object_id: not_you_control,
            group_index: 1,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        },
    ]
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
fn khalni_ambush_fights_with_current_power_and_rejects_wrong_controller_targets() {
    let mut engine = khalni_engine(20_260_926);
    let friendly = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_with_stats(&mut engine, 1, "colossal_dreadmaw", 6, 6);
    inject_card_into_hand(&mut engine, 0, KHALNI);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, KHALNI);

    let batch = engine.initial_response_batch();
    let legal = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    assert_eq!(legal.groups.len(), 2);
    assert_eq!(legal.groups[0].valid_permanent_ids, [friendly]);
    assert_eq!(legal.groups[1].valid_permanent_ids, [opposing]);
    assert_eq!(legal.groups[1].distinct_from_group_indices, [0]);

    let mana_before = engine.state.players[0].mana_pool;
    let hand_before = engine.state.players[0].hand.clone();
    engine
        .apply_command(
            0,
            &cast_spell_face(slot, creature_targets(opposing, friendly), 0),
        )
        .expect_err("each target group enforces its controller restriction");
    assert_eq!(engine.state.players[0].mana_pool, mana_before);
    assert_eq!(engine.state.players[0].hand, hand_before);

    semantic::accepted(
        &mut engine,
        0,
        &cast_spell_face(slot, creature_targets(friendly, opposing), 0),
    );
    engine
        .state
        .objects
        .get_mut(&friendly)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&friendly].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opposing].damage, 4);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Battlefield);
}

#[test]
fn khalni_ambush_deals_no_damage_if_either_target_leaves_before_resolution() {
    for (illegal_group, seed) in [(0, 20_260_927), (1, 20_260_928)] {
        let mut engine = khalni_engine(seed);
        let friendly = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let opposing = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
        inject_card_into_hand(&mut engine, 0, KHALNI);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                g: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, KHALNI);
        semantic::accepted(
            &mut engine,
            0,
            &cast_spell_face(slot, creature_targets(friendly, opposing), 0),
        );

        if illegal_group == 0 {
            leave_battlefield_to_hand(&mut engine, 0, friendly);
        } else {
            leave_battlefield_to_hand(&mut engine, 1, opposing);
        }
        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(engine.state.objects[&friendly].damage, 0);
        assert_eq!(
            engine.state.objects[&friendly].zone,
            if illegal_group == 0 {
                Zone::Hand
            } else {
                Zone::Battlefield
            }
        );
        assert_eq!(engine.state.objects[&opposing].damage, 0);
        assert_eq!(
            engine.state.objects[&opposing].zone,
            if illegal_group == 1 {
                Zone::Hand
            } else {
                Zone::Battlefield
            }
        );
    }
}

#[test]
fn khalni_territory_enters_tapped_then_produces_green_mana_immediately() {
    let mut engine = khalni_engine(20_260_929);
    let land = inject_card_into_hand(&mut engine, 0, KHALNI);
    let slot = hand_index_for_card(&engine, 0, KHALNI);
    semantic::accepted(&mut engine, 0, &play_land_face(slot, 1));

    assert_eq!(engine.state.objects[&land].face_up_index, 1);
    assert!(engine.state.objects[&land].tapped);
    engine
        .apply_command(0, &activate_ability_for(&engine, land, 0, vec![]))
        .expect_err("Khalni Territory enters tapped");

    end_active_turn(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 1);
    end_active_turn(&mut engine, 1);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 0);
    assert!(!engine.state.objects[&land].tapped);

    engine
        .apply_command(0, &activate_ability_for(&engine, land, 0, vec![]))
        .expect("activate Khalni Territory's green mana ability");
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.stack.is_empty(), "mana resolves immediately");
    assert!(engine.state.objects[&land].tapped);
}
