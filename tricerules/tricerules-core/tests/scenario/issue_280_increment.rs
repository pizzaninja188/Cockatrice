//! Issue #280 — Increment compares the mana actually spent on a cast against the source's
//! current power or toughness, then rechecks that intervening-if condition on resolution.
//! CR 601.2i records a completed cast, CR 603.4/608.2a governs intervening-if checks, and
//! CR 122.1/122.1a governs the +1/+1 counter result.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_proto::ruled::v1::CastCostGroupSelection;

fn engine_with(cards: &[&str]) -> GameEngine {
    let mut engine = GameEngine::new(
        280_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", cards),
            deck_with("island", &["grizzly_bears"]),
        ]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn increment_requires_more_than_either_current_stat() {
    let mut engine = engine_with(&["hungry_graffalon", "divination", "chandras_outrage"]);
    let graffalon = relocate_to_battlefield(&mut engine, 0, "hungry_graffalon", false);
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    ensure_in_hand(&mut engine, 0, "divination");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let divination = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(divination, vec![]))
        .expect("cast three-mana Divination");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "three equals power and is below toughness, so no trigger is created"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&graffalon].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    ensure_in_hand(&mut engine, 0, "chandras_outrage");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let outrage = hand_index_for_card(&engine, 0, "chandras_outrage");
    engine
        .apply_command(0, &cast_spell(outrage, target_object(opposing)))
        .expect("cast four-mana Chandra's Outrage");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "four exceeds Hungry Graffalon's power three"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&graffalon].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn increment_uses_actual_kicked_mana_spent() {
    let mut engine = engine_with(&["hungry_graffalon", "grow_from_the_ashes"]);
    let graffalon = relocate_to_battlefield(&mut engine, 0, "hungry_graffalon", false);
    ensure_in_hand(&mut engine, 0, "grow_from_the_ashes");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "grow_from_the_ashes");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![CastCostGroupSelection {
                    group_index: 0,
                    option_index: 0,
                    ..Default::default()
                }],
            ),
        )
        .expect("cast Grow from the Ashes kicked");
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .spell_casts
            .last()
            .expect("completed cast")
            .mana_spent,
        5,
        "Increment uses the completed payment receipt, not the printed mana value"
    );
    assert_eq!(engine.state.stack.len(), 2, "spell plus Increment trigger");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&graffalon].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "five mana exceeds Hungry Graffalon's power three"
    );
}

#[test]
fn increment_rechecks_current_stats_on_resolution() {
    let mut engine = engine_with(&["cuboid_colony", "divination", "giant_growth"]);
    let cuboid = relocate_to_battlefield(&mut engine, 0, "cuboid_colony", false);
    ensure_in_hand(&mut engine, 0, "divination");
    ensure_in_hand(&mut engine, 0, "giant_growth");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 3,
            g: 1,
            ..Default::default()
        },
    );

    let divination = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(divination, vec![]))
        .expect("cast three-mana Divination");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "Divination plus Increment trigger"
    );

    let growth = hand_index_for_card(&engine, 0, "giant_growth");
    engine
        .apply_command(0, &cast_spell(growth, target_object(cuboid)))
        .expect("respond with Giant Growth");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&cuboid].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "after Giant Growth, three exceeds neither current power nor toughness four"
    );
}
