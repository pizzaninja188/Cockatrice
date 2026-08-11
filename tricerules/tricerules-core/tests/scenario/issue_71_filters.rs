use tricerules_cards::CounterKind;

use super::helpers::*;

#[test]
fn legions_judgment_publishes_derived_power_targets_and_rejects_a_forged_cast() {
    let decks = Some(vec![
        deck_with("plains", &["legions_judgment"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(71_101, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let boosted_bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&boosted_bear)
        .expect("bear")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 2);
    let ordinary_bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "legions_judgment");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "legions_judgment");
    let batch = engine.initial_response_batch();
    let targets = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    assert_eq!(targets.valid_permanent_ids, vec![boosted_bear]);

    let hand_before = engine.state.players[0].hand.clone();
    let mana_before = engine.state.players[0].mana_pool;
    assert!(engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![TargetRef {
                    object_id: ordinary_bear,
                    damage_amount: 0,
                }],
            ),
        )
        .is_err());
    assert_eq!(engine.state.players[0].hand, hand_before);
    let mana_after = engine.state.players[0].mana_pool;
    assert_eq!(
        (
            mana_after.white,
            mana_after.blue,
            mana_after.black,
            mana_after.red,
            mana_after.green,
            mana_after.colorless,
        ),
        (
            mana_before.white,
            mana_before.blue,
            mana_before.black,
            mana_before.red,
            mana_before.green,
            mana_before.colorless,
        )
    );

    engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![TargetRef {
                    object_id: boosted_bear,
                    damage_amount: 0,
                }],
            ),
        )
        .expect("derived power-four creature is legal");
}

#[test]
fn reckless_air_strike_modes_publish_disjoint_authoritative_targets() {
    let decks = Some(vec![
        deck_with("mountain", &["reckless_air_strike"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(71_102, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let flyer = inject_creature_on_battlefield(&mut engine, 1, "wind_drake");
    let artifact = inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    let ground = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "reckless_air_strike");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "reckless_air_strike");
    let batch = engine.initial_response_batch();
    let action = batch.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == slot as u32)
        .expect("cast action");
    assert_eq!(
        action.modes[0]
            .targets
            .as_ref()
            .unwrap()
            .valid_permanent_ids,
        vec![flyer]
    );
    assert_eq!(
        action.modes[1]
            .targets
            .as_ref()
            .unwrap()
            .valid_permanent_ids,
        vec![artifact]
    );
    assert_ne!(ground, flyer);
}

#[test]
fn run_afoul_targets_only_an_opponent_and_offers_only_their_flyers_to_sacrifice() {
    let decks = Some(vec![
        deck_with("forest", &["run_afoul"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(71_103, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let flyer = inject_creature_on_battlefield(&mut engine, 1, "wind_drake");
    inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 0, "wind_drake");
    ensure_in_hand(&mut engine, 0, "run_afoul");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "run_afoul");
    let legal = engine.initial_response_batch();
    let targets = &legal.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    assert!(!targets.can_target_self);
    assert!(targets.can_target_opponent);

    engine
        .apply_command(0, &cast_spell(slot, target_player(1)))
        .expect("target opponent");
    engine.apply_command(0, &pass()).expect("caster pass");
    let resolution = engine.apply_command(1, &pass()).expect("opponent pass");
    let choice = find_resolution_choice(&resolution).expect("sacrifice choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.candidate_object_ids, vec![flyer]);
}
