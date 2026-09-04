use crate::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cast_cost_group_selection::SelectedObject, ruled_event::Ev, CastCostGroupSelection,
    CastCostOptionKind, HandActionKind,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &["bitter_triumph", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "bitter_triumph");
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    engine
}

fn discard_option(hand_index: usize) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        selected_object: Some(SelectedObject::HandIndex(hand_index as u32)),
        expected_zone_change_generation: 0,
        battlefield_objects: None,
    }
}

fn pay_life_option() -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index: 1,
        selected_object: None,
        expected_zone_change_generation: 0,
        battlefield_objects: None,
    }
}

fn give_cast_mana(engine: &mut GameEngine) {
    engine.state.players[0].mana_pool.black = 1;
    engine.state.players[0].mana_pool.colorless = 1;
}

#[test]
fn publishes_one_required_choice_with_private_discard_candidates() {
    let mut engine = engine(199_001);
    let source_slot = hand_index_for_card(&engine, 0, "bitter_triumph");
    let fodder_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    let swamp = relocate_to_battlefield(&mut engine, 0, "swamp", false);
    let batch = engine
        .apply_command(0, &activate_ability(swamp, 0, vec![]))
        .expect("mana ability republishes cast legality");
    let action = batch.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| {
            action.kind == HandActionKind::HandActionCastSpell as i32
                && action.card_name == "Bitter Triumph"
        })
        .expect("Bitter Triumph legal action");
    let choices = action.cost_choices.as_ref().expect("cost choices");
    assert!(choices.non_mana_costs_payable);
    let group = &choices.cast_cost_groups[0];
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.options.len(), 2);
    assert_eq!(
        group.options[0].kind,
        CastCostOptionKind::DiscardCard as i32
    );
    assert_eq!(group.options[0].label, "Discard a card");
    assert!(group.options[0].selectable);
    assert!(group.options[0]
        .valid_hand_indices
        .contains(&(fodder_slot as u32)));
    assert!(!group.options[0]
        .valid_hand_indices
        .contains(&(source_slot as u32)));
    assert_eq!(group.options[1].kind, CastCostOptionKind::PayLife as i32);
    assert_eq!(group.options[1].label, "Pay 3 life");
    assert!(group.options[1].selectable);
}

#[test]
fn discard_option_is_atomic_and_destroys_the_target() {
    let mut engine = engine(199_002);
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    give_cast_mana(&mut engine);
    let source_slot = hand_index_for_card(&engine, 0, "bitter_triumph");
    let fodder_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    let fodder = engine.state.players[0].hand[fodder_slot];
    let batch = engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                source_slot,
                target_object(target),
                vec![discard_option(fodder_slot)],
            ),
        )
        .expect("discarding another card pays the additional cost");

    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&fodder));
    let receipt = &engine.state.stack.last().unwrap().cast_cost_receipts[0];
    assert_eq!(receipt.option_id.as_ref().unwrap().as_str(), "discard_card");
    assert!(
        receipt.objects.is_empty(),
        "discard is not an active reveal"
    );
    assert!(!batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::ActivePublicRevealSnapshot(snapshot)) if !snapshot.reveals.is_empty()
    )));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn life_option_emits_one_life_change_and_destroys_the_target() {
    let mut engine = engine(199_003);
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    give_cast_mana(&mut engine);
    let source_slot = hand_index_for_card(&engine, 0, "bitter_triumph");
    let hand_before = engine.state.players[0].hand.len();
    let batch = engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                source_slot,
                target_object(target),
                vec![pay_life_option()],
            ),
        )
        .expect("paying life pays the additional cost");

    assert_eq!(engine.state.players[0].life, 17);
    assert_eq!(engine.state.players[0].hand.len(), hand_before - 1);
    let life = life_changes_in(&batch);
    assert_eq!(life.len(), 1);
    assert_eq!((life[0].player_id, life[0].delta), (0, -3));
    let receipt = &engine.state.stack.last().unwrap().cast_cost_receipts[0];
    assert_eq!(receipt.option_id.as_ref().unwrap().as_str(), "pay_3_life");
    assert!(receipt.objects.is_empty());
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn life_option_is_legal_at_three_and_unselectable_below_three() {
    let mut exact = engine(199_004);
    exact.state.players[0].life = 3;
    give_cast_mana(&mut exact);
    let target = relocate_to_battlefield(&mut exact, 1, "grizzly_bears", false);
    let slot = hand_index_for_card(&exact, 0, "bitter_triumph");
    exact
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, target_object(target), vec![pay_life_option()]),
        )
        .expect("a player may pay all three remaining life");
    assert_eq!(exact.state.players[0].life, 0);

    let mut low = engine(199_005);
    low.state.players[0].life = 2;
    let swamp = relocate_to_battlefield(&mut low, 0, "swamp", false);
    let batch = low
        .apply_command(0, &activate_ability(swamp, 0, vec![]))
        .expect("mana ability republishes cast legality");
    let action = batch.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.card_name == "Bitter Triumph")
        .expect("discard remains payable");
    let group = &action.cost_choices.as_ref().unwrap().cast_cost_groups[0];
    assert!(group.options[0].selectable);
    assert!(!group.options[1].selectable);
}

#[test]
fn spell_is_not_offered_when_neither_additional_cost_can_be_paid() {
    let mut engine = engine(199_006);
    engine.state.players[0].life = 2;
    let swamp = relocate_to_battlefield(&mut engine, 0, "swamp", false);
    let source = engine.state.players[0].hand[hand_index_for_card(&engine, 0, "bitter_triumph")];
    let removed = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .filter(|oid| *oid != source)
        .collect::<Vec<_>>();
    engine.state.players[0].hand.retain(|oid| *oid == source);
    for oid in removed {
        engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Library;
        engine.state.players[0].library.push_back(oid);
    }
    let batch = engine
        .apply_command(0, &activate_ability(swamp, 0, vec![]))
        .expect("mana ability republishes cast legality");
    assert!(!batch.legal_by_player[&0]
        .hand_actions
        .iter()
        .any(|action| action.card_name == "Bitter Triumph"));
}

#[test]
fn rejects_invalid_cost_announcements_without_partial_payment() {
    for (seed, life, mana, selections) in [
        (199_007, 20, true, vec![]),
        (
            199_008,
            20,
            true,
            vec![discard_option(0), pay_life_option()],
        ),
        (199_009, 2, true, vec![pay_life_option()]),
        (199_012, 20, false, vec![pay_life_option()]),
    ] {
        let mut engine = engine(seed);
        engine.state.players[0].life = life;
        if mana {
            give_cast_mana(&mut engine);
        }
        let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
        let source_slot = hand_index_for_card(&engine, 0, "bitter_triumph");
        let selections = selections
            .into_iter()
            .map(|mut selection| {
                if selection.option_index == 0 {
                    selection.selected_object = Some(SelectedObject::HandIndex(source_slot as u32));
                }
                selection
            })
            .collect();
        let before_hand = engine.state.players[0].hand.clone();
        let before_graveyard = engine.state.players[0].graveyard.clone();
        let before_mana = engine.state.players[0].mana_pool;
        let before_life = engine.state.players[0].life;
        let before_stack = engine.state.stack.len();
        let before_command_index = engine.state.command_index;
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(source_slot, target_object(target), selections),
            )
            .expect_err("invalid cost announcement must fail");
        assert_eq!(engine.state.players[0].hand, before_hand);
        assert_eq!(engine.state.players[0].graveyard, before_graveyard);
        assert_eq!(engine.state.players[0].mana_pool, before_mana);
        assert_eq!(engine.state.players[0].life, before_life);
        assert_eq!(engine.state.stack.len(), before_stack);
        assert_eq!(engine.state.command_index, before_command_index);
    }
}
