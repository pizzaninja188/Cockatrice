use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_proto::ruled::v1::CastCostGroupSelection;

fn engine_with(cards: &[&str]) -> GameEngine {
    let mut engine = GameEngine::new(
        181_001,
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
fn tackle_artist_uses_actual_mana_spent_for_its_opus_upgrade() {
    let mut engine = engine_with(&["tackle_artist", "unexpected_assistance"]);
    let artist = relocate_to_battlefield(&mut engine, 0, "tackle_artist", false);
    ensure_in_hand(&mut engine, 0, "unexpected_assistance");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "unexpected_assistance");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast a five-mana instant");
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .spell_casts
            .last()
            .unwrap()
            .mana_spent,
        5,
        "the completed cast fact retains its own payment receipt"
    );
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&artist].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "Opus must use the five mana actually spent on the triggering spell"
    );
}

#[test]
fn tackle_artist_uses_the_total_kicked_cost_not_printed_mana_value() {
    let mut engine = engine_with(&["tackle_artist", "grow_from_the_ashes"]);
    let artist = relocate_to_battlefield(&mut engine, 0, "tackle_artist", false);
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
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&artist].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "the two-mana kicker raises actual spending from three to five"
    );
}

#[test]
fn tackle_artist_uses_one_counter_below_five_and_ignores_creature_spells() {
    let mut engine = engine_with(&["tackle_artist", "chandras_outrage", "hill_giant"]);
    let artist = relocate_to_battlefield(&mut engine, 0, "tackle_artist", false);
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
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
    let slot = hand_index_for_card(&engine, 0, "chandras_outrage");
    engine
        .apply_command(0, &cast_spell(slot, target_object(opposing)))
        .expect("cast a four-mana instant");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&artist].counter_count(CounterKind::PlusOnePlusOne),
        1
    );

    ensure_in_hand(&mut engine, 0, "hill_giant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "hill_giant");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast a four-mana creature");
    assert_eq!(
        engine.state.objects[&artist].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "Opus filters to instant and sorcery spells"
    );
}

#[test]
fn increment_compares_spending_to_either_current_stat() {
    let mut engine = engine_with(&["hungry_graffalon", "chandras_outrage", "divination"]);
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
    let slot = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast for three");
    assert_eq!(engine.state.stack.len(), 1, "three exceeds neither 3 nor 4");
    resolve_entire_stack_two_player(&mut engine);

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
    let slot = hand_index_for_card(&engine, 0, "chandras_outrage");
    engine
        .apply_command(0, &cast_spell(slot, target_object(opposing)))
        .expect("cast for four");
    assert_eq!(engine.state.stack.len(), 2, "four exceeds power three");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&graffalon].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn increment_checks_current_power_and_toughness_again_on_resolution() {
    let mut engine = engine_with(&["hungry_graffalon", "unexpected_assistance", "giant_growth"]);
    let graffalon = relocate_to_battlefield(&mut engine, 0, "hungry_graffalon", false);
    ensure_in_hand(&mut engine, 0, "unexpected_assistance");
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

    let spell = hand_index_for_card(&engine, 0, "unexpected_assistance");
    engine
        .apply_command(0, &cast_spell(spell, vec![]))
        .expect("five mana creates the Increment trigger");
    let growth = hand_index_for_card(&engine, 0, "giant_growth");
    engine
        .apply_command(0, &cast_spell(growth, target_object(graffalon)))
        .expect("respond to Increment");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&graffalon].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "after Giant Growth, five exceeds neither 6 nor 7"
    );
}

#[test]
fn direct_spell_copies_do_not_create_spell_cast_spend_context() {
    let mut engine = engine_with(&["tackle_artist", "divination", "twincast"]);
    let artist = relocate_to_battlefield(&mut engine, 0, "tackle_artist", false);
    ensure_in_hand(&mut engine, 0, "divination");
    ensure_in_hand(&mut engine, 0, "twincast");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            c: 2,
            ..Default::default()
        },
    );

    let divination = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(divination, vec![]))
        .expect("cast Divination");
    let original = engine
        .state
        .stack
        .iter()
        .find(|item| item.card_id == "divination")
        .expect("original spell on stack")
        .id;
    let twincast = hand_index_for_card(&engine, 0, "twincast");
    engine
        .apply_command(0, &cast_spell(twincast, target_object(original)))
        .expect("cast Twincast");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.turn_history.current.spell_casts.len(), 2);
    assert_eq!(
        engine.state.objects[&artist].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "Divination and Twincast trigger Opus; the direct Divination copy does not"
    );
}

#[test]
fn increment_trigger_does_not_follow_a_new_source_generation() {
    let mut engine = engine_with(&["hungry_graffalon", "unexpected_assistance"]);
    let graffalon = relocate_to_battlefield(&mut engine, 0, "hungry_graffalon", false);
    ensure_in_hand(&mut engine, 0, "unexpected_assistance");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "unexpected_assistance");
    engine
        .apply_command(0, &cast_spell(spell, vec![]))
        .expect("five mana creates the Increment trigger");
    *engine
        .state
        .zone_change_generation
        .entry(graffalon)
        .or_default() += 2;
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&graffalon].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "a leave-and-returned permanent is not the trigger's original source"
    );
}
