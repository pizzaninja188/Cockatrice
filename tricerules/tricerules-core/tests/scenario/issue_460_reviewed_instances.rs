//! Issue #460 - the fixed negative pump, ETB team pump, activated each-opponent damage, and
//! second-spell cost-reduction families through the command boundary.
//!
//! Every expectation is the reviewed printed Oracle behavior, not a copy of generator output.
//! Exact Scryfall records and `rulings_uri` responses were fetched 2026-09-20 against pinned
//! snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`. The Fleeting Distraction ruling confirms the
//! spell fizzles with no draw when its only target is illegal; the Uthros Psionicist ruling
//! confirms any spell cast this turn counts toward the reduction (including itself, one that was
//! countered, or one still on the stack) and that the reduction never changes the spell's mana
//! value. Governing CR concepts: CR 611.2a / 613.4c (until-end-of-turn P/T modifiers), CR 608.2b
//! (target revalidation), CR 608.2c (printed instruction order), CR 514.2 (end-of-turn duration),
//! CR 704.5f (lethal-toughness state-based action), CR 603.6a (entry triggers), CR 602.2b
//! (activated-ability costs), CR 118.3 (life loss), CR 601.2f (static cost reductions), and
//! CR 601.2a / 700.2 (spells cast this turn).

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

#[track_caller]
fn hand_action_reduction(engine: &mut GameEngine, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(engine, 0, card_id) as u32;
    let batch = engine.initial_response_batch();
    batch.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| panic!("missing cast action for {card_id}"))
        .generic_cost_reduction
}

#[test]
fn issue_460_fleeting_distraction_shrinks_power_and_draws_exactly_one() {
    let mut engine = semantic::main_phase(460_100);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    let source = inject_card_into_hand(&mut engine, 0, "fleeting_distraction");
    let slot = hand_index_for_card(&engine, 0, "fleeting_distraction");
    let hand_before: Vec<u32> = engine.state.players[0].hand.to_vec();
    let library_before: Vec<u32> = engine.state.players[0].library.iter().copied().collect();
    let drawn = library_before[0];
    let drawn_card = engine.state.objects[&drawn].card_id.clone();
    let drawn_generation = semantic::generation(&engine, drawn);
    let source_generation = semantic::generation(&engine, source);
    grant_pool(&mut engine, 0);

    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));
    semantic::complete(&mut engine, 24, |_| None).require_exercised();
    semantic::assert_main_priority(&engine, 0);

    assert_eq!(
        engine.effective_power(target),
        Some(1),
        "shrink power by one"
    );
    assert_eq!(
        engine.effective_toughness(target),
        Some(2),
        "toughness unchanged"
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);

    let mut expected_hand: Vec<u32> = hand_before
        .iter()
        .copied()
        .filter(|oid| *oid != source)
        .collect();
    expected_hand.push(drawn);
    assert_eq!(
        engine.state.players[0].hand, expected_hand,
        "Fleeting Distraction draws exactly the top card"
    );
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before[1..],
        "exactly one card leaves the library"
    );
    assert_eq!(engine.state.players[1].life, 20);
    semantic::assert_object(
        &engine,
        drawn,
        &drawn_card,
        0,
        0,
        Zone::Hand,
        drawn_generation + 1,
    );
    semantic::assert_object(
        &engine,
        source,
        "fleeting_distraction",
        0,
        0,
        Zone::Graveyard,
        source_generation + 2,
    );
}

#[test]
fn issue_460_fleeting_distraction_fizzles_without_drawing_on_an_illegal_target() {
    let mut engine = semantic::main_phase(460_200);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 3, 3);
    let source = inject_card_into_hand(&mut engine, 0, "fleeting_distraction");
    let slot = hand_index_for_card(&engine, 0, "fleeting_distraction");
    let generation = semantic::generation(&engine, source);
    grant_pool(&mut engine, 0);
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));

    // The response bounces the only target before Fleeting Distraction resolves.
    inject_card_into_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    semantic::accepted(&mut engine, 0, &cast_spell(unsummon, target_object(target)));
    let hand_before_resolution: Vec<u32> = engine.state.players[0].hand.to_vec();
    resolve_entire_stack_two_player(&mut engine);

    // CR 608.2b: the only target is illegal, so the spell leaves the stack without resolving and
    // the printed draw must not happen.
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(
        engine.state.players[0].hand, hand_before_resolution,
        "a fizzled Fleeting Distraction draws no card"
    );
    semantic::assert_object(
        &engine,
        source,
        "fleeting_distraction",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
}

#[test]
fn issue_460_overkill_sets_lethal_toughness() {
    let mut engine = semantic::main_phase(460_300);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    let source = inject_card_into_hand(&mut engine, 0, "overkill");
    let slot = hand_index_for_card(&engine, 0, "overkill");
    grant_pool(&mut engine, 0);
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));
    semantic::complete(&mut engine, 24, |_| None).require_exercised();

    // CR 704.5f: the -0/-9999 modifier drops toughness below zero, so the creature dies.
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.players[1].graveyard.contains(&target));
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn issue_460_malamet_pumps_only_the_controllers_team_and_expires() {
    let mut engine = semantic::main_phase(460_400);
    let own = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    let opposing = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    let source = inject_card_into_hand(&mut engine, 0, "malamet_war_scribe");
    let slot = hand_index_for_card(&engine, 0, "malamet_war_scribe");
    grant_pool(&mut engine, 0);
    semantic::accepted(&mut engine, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut engine);
    semantic::assert_main_priority(&engine, 0);

    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(own), Some(4), "team gets +2 power");
    assert_eq!(
        engine.effective_toughness(own),
        Some(3),
        "team gets +1 toughness"
    );
    assert_eq!(
        engine.effective_power(source),
        Some(6),
        "the source is on the team"
    );
    assert_eq!(engine.effective_toughness(source), Some(4));
    assert_eq!(
        engine.effective_power(opposing),
        Some(2),
        "an opponent's creature is not pumped"
    );
    assert_eq!(engine.effective_toughness(opposing), Some(2));

    // CR 514.2: the pump lasts only until end of turn.
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.effective_power(own), Some(2), "the pump expires");
    assert_eq!(engine.effective_toughness(own), Some(2));
    assert_eq!(engine.effective_power(source), Some(4));
    assert_eq!(engine.effective_toughness(source), Some(3));
    assert_eq!(engine.effective_power(opposing), Some(2));
}

#[test]
fn issue_460_panicked_altisaur_taps_for_two_to_each_opponent() {
    let mut engine = semantic::main_phase(460_500);
    let source = inject_creature_on_battlefield(&mut engine, 0, "panicked_altisaur");
    let hand_before: Vec<u32> = engine.state.players[0].hand.to_vec();

    let activation = activate_ability_for(&engine, source, 0, vec![]);
    semantic::accepted(&mut engine, 0, &activation);
    assert!(
        engine.state.objects[&source].tapped,
        "the {{T}} cost taps the source"
    );
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[1].life, 18, "each opponent loses two");
    assert_eq!(
        engine.state.players[0].life, 20,
        "the controller is unchanged"
    );
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert!(engine.state.objects[&source].tapped);

    // CR 602.2b: an already-tapped source cannot pay the tap cost again.
    let before = engine.state.command_index;
    let stale = activate_ability_for(&engine, source, 0, vec![]);
    engine
        .apply_command(0, &stale)
        .expect_err("a tapped source cannot pay the tap cost");
    assert_eq!(engine.state.command_index, before);
    assert_eq!(engine.state.players[1].life, 18);
}

#[test]
fn issue_460_uthros_reduces_only_the_second_spell_and_never_its_mana_value() {
    let mut engine = semantic::main_phase(460_600);
    inject_creature_on_battlefield(&mut engine, 0, "uthros_psionicist");
    for _ in 0..3 {
        inject_card_into_hand(&mut engine, 0, "divination");
    }

    // First spell of the turn: the condition is not met, so it costs the full {2}{U}.
    assert_eq!(hand_action_reduction(&mut engine, "divination"), 0);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let first = hand_index_for_card(&engine, 0, "divination");
    let first_command = cast_spell(first, vec![]);
    let before = engine.state.command_index;
    engine
        .apply_command(0, &first_command)
        .expect_err("the first spell is not reduced");
    assert_eq!(engine.state.command_index, before);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    semantic::accepted(&mut engine, 0, &first_command);
    let first_stack = engine
        .state
        .stack
        .last()
        .expect("first spell on the stack")
        .id;
    assert_eq!(
        engine.characteristics(first_stack).unwrap().mana_value,
        3,
        "Divination keeps mana value 3"
    );
    resolve_entire_stack_two_player(&mut engine);

    // Second spell: exactly one prior spell this turn, so Uthros reduces it by 2 to {U}.
    assert_eq!(hand_action_reduction(&mut engine, "divination"), 2);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let second = hand_index_for_card(&engine, 0, "divination");
    semantic::accepted(&mut engine, 0, &cast_spell(second, vec![]));
    let second_stack = engine
        .state
        .stack
        .last()
        .expect("second spell on the stack")
        .id;
    assert_eq!(
        engine.characteristics(second_stack).unwrap().mana_value,
        3,
        "the reduction never changes the spell's mana value"
    );
    resolve_entire_stack_two_player(&mut engine);

    // Third spell: two prior spells this turn, so the condition fails again.
    assert_eq!(hand_action_reduction(&mut engine, "divination"), 0);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let third = hand_index_for_card(&engine, 0, "divination");
    let third_command = cast_spell(third, vec![]);
    let before = engine.state.command_index;
    engine
        .apply_command(0, &third_command)
        .expect_err("the third spell is not reduced");
    assert_eq!(engine.state.command_index, before);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    semantic::accepted(&mut engine, 0, &third_command);
}
