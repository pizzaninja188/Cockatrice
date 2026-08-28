use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, CostObjectRef, CostObjectRefs, CostSelection,
};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn choose_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_153_chaos_spewer_requires_blight_after_declining_mana() {
    let mut engine = GameEngine::new(
        153_103,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["chaos_spewer"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "chaos_spewer");
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let slot = hand_index_for_card(&engine, 0, "chaos_spewer");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    // Already-floating mana must remain available for explicit pip selection in the client.
    engine.state.players[0].mana_pool.colorless = 2;
    let payment_batch = engine.apply_command(0, &choose_branch(0)).unwrap();
    let payment = payment_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) if choice.choice_kind == ChoiceKind::ManaPayment as i32 => Some(choice),
            _ => None,
        })
        .expect("choosing Pay must open a staged mana payment");
    assert_eq!(payment.generic_mana_cost, 2);
    assert!(
        payment.mana_cost.is_empty(),
        "generic costs use the existing pip picker"
    );
    assert!(payment.payment_currently_legal);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("cancelled mana cannot skip the required choice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    engine.apply_command(0, &choose_branch(1)).unwrap();
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("declining mana must perform mandatory Blight");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CostObjects);
    assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
    let creature = pending.presentation.candidates[0];
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .is_err());
    engine
        .apply_command(0, &submit_resolution_choice(vec![creature]))
        .unwrap();
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::MinusOneMinusOne),
        2
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn issue_153_resolution_blight_parks_before_discard_and_defers_lethal_sba() {
    let mut engine = GameEngine::new(
        153_102,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["dream_seizer", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "dream_seizer");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&bear)
        .unwrap()
        .add_counters(CounterKind::MinusOneMinusOne, 1, 0);
    engine.state.players[0].mana_pool.black = 1;
    engine.state.players[0].mana_pool.colorless = 3;
    let slot = hand_index_for_card(&engine, 0, "dream_seizer");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    engine.apply_command(0, &choose_branch(0)).unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::CostObjects,
        "Blight must be paid before the opponent chooses a discard"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![bear]))
        .unwrap();
    assert_eq!(engine.state.objects[&bear].zone, Zone::Battlefield);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!(
        pending
            .continuation
            .stack()
            .unwrap()
            .item
            .blight_receipts
            .len(),
        1
    );
    let discard = pending.presentation.candidates[0];
    engine
        .apply_command(1, &submit_resolution_choice(vec![discard]))
        .unwrap();
    assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

fn blight_selection(cost_index: u32, object_id: u32, zone_change_generation: u64) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id,
                zone_change_generation,
            }],
        })),
    }
}

#[test]
fn issue_153_rejected_blight_is_atomic_and_receipts_bind_the_incarnation() {
    let mut engine = GameEngine::new(
        153_104,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["gristle_glutton"]),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .unwrap();
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    // Use a three-seat fixture directly in main phase so controller, not owner, governs eligibility.
    engine.state.turn_step = tricerules_core::TurnStep::Main1;
    let source = relocate_to_battlefield(&mut engine, 0, "gristle_glutton", false);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = false;
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", true);
    engine.state.objects.get_mut(&bear).unwrap().controller = 0;
    engine.state.objects.get_mut(&bear).unwrap().base_controller = 0;
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != bear);
    engine.state.players[0].battlefield.push(bear);
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);
    let before = engine.state.command_index;
    for selected in [
        blight_selection(1, bear, generation + 1),
        blight_selection(1, source, u64::MAX),
    ] {
        assert!(engine
            .apply_command(
                0,
                &activate_ability_with_costs(source, 0, vec![], vec![selected])
            )
            .is_err());
        assert_eq!(engine.state.command_index, before);
        assert!(!engine.state.objects[&source].tapped);
        assert_eq!(
            engine.state.objects[&bear].counter_count(CounterKind::MinusOneMinusOne),
            0
        );
        assert!(engine.state.stack.is_empty());
    }
    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                source,
                0,
                vec![],
                vec![blight_selection(1, bear, generation)],
            ),
        )
        .unwrap();
    let receipt = engine.state.stack.last().unwrap().blight_receipts[0];
    assert_eq!(receipt.player, 0);
    assert_eq!(receipt.count, 1);
    assert_eq!(receipt.creature.unwrap().object_id, bear);
    assert_eq!(receipt.creature.unwrap().zone_change_generation, generation);
    assert_eq!(engine.state.objects[&bear].owner, 1);
}

#[test]
fn issue_153_blackthorn_etb_pays_before_drawing_and_losing_life() {
    let mut engine = GameEngine::new(
        153_105,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["blighted_blackthorn"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "blighted_blackthorn");
    engine.state.players[0].mana_pool.black = 1;
    engine.state.players[0].mana_pool.colorless = 4;
    let slot = hand_index_for_card(&engine, 0, "blighted_blackthorn");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    let before = engine.state.players[0].hand.len();
    engine.apply_command(0, &choose_branch(0)).unwrap();
    assert_eq!(engine.state.players[0].hand.len(), before);
    let oid = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![oid]))
        .unwrap();
    assert_eq!(
        engine.state.objects[&oid].counter_count(CounterKind::MinusOneMinusOne),
        2
    );
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
    assert_eq!(engine.state.players[0].life, 19);
}

fn cast_blight(
    object_id: u32,
    generation: u64,
) -> tricerules_proto::ruled::v1::CastCostGroupSelection {
    tricerules_proto::ruled::v1::CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        selected_object: Some(
            tricerules_proto::ruled::v1::cast_cost_group_selection::SelectedObject::PermanentId(
                object_id,
            ),
        ),
        expected_zone_change_generation: generation,
    }
}

#[test]
fn issue_153_copied_cinder_inherits_payment_without_blighting_again() {
    let mut engine = GameEngine::new(
        153_106,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["cinder_strike", "grizzly_bears"]),
            deck_with("island", &["twincast", "colossal_dreadmaw"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "cinder_strike");
    ensure_card_in_hand(&mut engine, 1, "twincast");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[1].mana_pool.blue = 2;
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);
    let slot = hand_index_for_card(&engine, 0, "cinder_strike");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![cast_blight(bear, generation)],
            ),
        )
        .unwrap();
    let original = engine.state.stack.last().unwrap().clone();
    let slot = hand_index_for_card(&engine, 1, "twincast");
    engine
        .apply_command(1, &cast_spell(slot, target_object(original.id)))
        .unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(1, &submit_resolution_choice(vec![target]))
        .unwrap();
    let copy = engine.state.stack.iter().find(|item| item.is_copy).unwrap();
    assert_eq!(copy.blight_receipts, original.blight_receipts);
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::MinusOneMinusOne),
        1
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&target].damage, 4);
}

#[test]
fn issue_153_wild_unraveling_requires_exactly_one_additional_payment() {
    for blight in [true, false] {
        let mut engine = GameEngine::new(
            153_107,
            &[0, 1],
            20,
            Some(vec![
                deck_with("mountain", &["lightning_bolt"]),
                deck_with("island", &["wild_unraveling", "grizzly_bears"]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        ensure_card_in_hand(&mut engine, 0, "lightning_bolt");
        ensure_card_in_hand(&mut engine, 1, "wild_unraveling");
        let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
        engine.state.players[0].mana_pool.red = 1;
        engine.state.players[1].mana_pool.blue = 2;
        engine.state.players[1].mana_pool.colorless = 1;
        let slot = hand_index_for_card(&engine, 0, "lightning_bolt");
        engine
            .apply_command(0, &cast_spell(slot, target_player(1)))
            .unwrap();
        let target = engine.state.stack.last().unwrap().id;
        let slot = hand_index_for_card(&engine, 1, "wild_unraveling");
        assert!(engine
            .apply_command(1, &cast_spell(slot, target_object(target)))
            .is_err());
        assert_eq!(engine.state.players[1].mana_pool.blue, 2);
        let generation = engine
            .state
            .zone_change_generation
            .get(&bear)
            .copied()
            .unwrap_or(0);
        let selection = if blight {
            cast_blight(bear, generation)
        } else {
            tricerules_proto::ruled::v1::CastCostGroupSelection {
                group_index: 0,
                option_index: 1,
                ..Default::default()
            }
        };
        engine
            .apply_command(
                1,
                &cast_spell_with_cast_cost_groups(slot, target_object(target), vec![selection]),
            )
            .unwrap();
        assert_eq!(
            engine.state.players[1].mana_pool.colorless,
            u32::from(blight)
        );
        assert_eq!(
            engine.state.objects[&bear].zone,
            if blight {
                Zone::Graveyard
            } else {
                Zone::Battlefield
            }
        );
        pass_both_players(&mut engine);
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.players[1].life, 20);
    }
}

#[test]
fn issue_153_cinder_strike_uses_paid_receipt_for_one_damage_instruction() {
    for paid in [false, true] {
        let mut engine = GameEngine::new(
            153_101,
            &[0, 1],
            20,
            Some(vec![
                deck_with("mountain", &["cinder_strike", "grizzly_bears"]),
                deck_with("forest", &["colossal_dreadmaw"]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        ensure_card_in_hand(&mut engine, 0, "cinder_strike");
        let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
        engine.state.players[0].mana_pool.red = 1;
        let generation = engine
            .state
            .zone_change_generation
            .get(&bear)
            .copied()
            .unwrap_or(0);
        let selections = if paid {
            vec![cast_blight(bear, generation)]
        } else {
            vec![]
        };
        let slot = hand_index_for_card(&engine, 0, "cinder_strike");
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(slot, target_object(target), selections),
            )
            .unwrap();
        assert_eq!(
            engine.state.stack.last().unwrap().blight_receipts.len(),
            usize::from(paid)
        );
        pass_both_players(&mut engine);
        assert_eq!(
            engine.state.objects[&target].damage,
            if paid { 4 } else { 2 }
        );
    }
}

#[test]
fn issue_153_activation_publishes_one_creature_and_blights_without_targeting() {
    let mut engine = GameEngine::new(
        153_100,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["gristle_glutton", "tatterkite"]),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "gristle_glutton", false);
    let prohibited = relocate_to_battlefield(&mut engine, 0, "tatterkite", false);
    let opponent = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = false;
    let generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let legal = engine.initial_response_batch();
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&(u64::from(source) << 32)];
    assert_eq!(
        choices.choices.len(),
        1,
        "Blight must publish one nontargeted cost selection"
    );
    assert_eq!(choices.choices[0].candidate_ids, [source]);
    assert!(!choices.choices[0].candidate_ids.contains(&prohibited));
    assert!(!choices.choices[0].candidate_ids.contains(&opponent));
    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                source,
                0,
                vec![],
                vec![blight_selection(1, source, generation)],
            ),
        )
        .unwrap();
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::MinusOneMinusOne),
        1
    );
    assert!(engine.state.stack.last().unwrap().targets.is_empty());
}
