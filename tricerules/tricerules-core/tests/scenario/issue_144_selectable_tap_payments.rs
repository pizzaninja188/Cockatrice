use super::helpers::*;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CostChoiceKind, CostObjectRef, CostObjectRefs,
    CostSelection, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn tap_selection(cost_index: u32, objects: &[(u32, u64)]) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: objects
                .iter()
                .map(|&(object_id, zone_change_generation)| CostObjectRef {
                    object_id,
                    zone_change_generation,
                })
                .collect(),
        })),
    }
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

#[test]
fn gene_pollinator_publishes_and_atomically_pays_another_untapped_permanent() {
    let decks = Some(vec![
        deck_with("forest", &["gene_pollinator", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(144_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let gene = relocate_to_battlefield(&mut engine, 0, "gene_pollinator", false);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.objects.get_mut(&gene).unwrap().summoning_sick = false;
    engine.state.objects.get_mut(&bear).unwrap().summoning_sick = true;
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);

    let legal = engine.initial_response_batch();
    let key = u64::from(gene) << 32;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&key];
    assert!(choices.non_mana_costs_payable);
    assert_eq!(choices.choices.len(), 1);
    assert_eq!(choices.choices[0].kind(), CostChoiceKind::Tap);
    assert_eq!(choices.choices[0].candidate_ids, [bear]);
    assert_eq!(choices.choices[0].candidate_objects[0].object_id, bear);
    assert_eq!(
        choices.choices[0].candidate_objects[0].zone_change_generation,
        generation
    );

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                gene,
                0,
                vec![],
                vec![tap_selection(1, &[(bear, generation)])],
            ),
        )
        .expect("tap Gene and a summoning-sick permanent as the separate payment");
    assert!(engine.state.objects[&gene].tapped);
    assert!(engine.state.objects[&bear].tapped);
    let pool = engine.state.players[0].mana_pool;
    assert_eq!(
        pool.white + pool.blue + pool.black + pool.red + pool.green + pool.colorless,
        1
    );
}

#[test]
fn stale_or_duplicate_tap_selection_rejects_without_partial_taps() {
    let decks = Some(vec![
        deck_with("forest", &["gene_pollinator", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(144_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let gene = relocate_to_battlefield(&mut engine, 0, "gene_pollinator", false);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.objects.get_mut(&gene).unwrap().summoning_sick = false;
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);

    let stale = activate_ability_with_costs(
        gene,
        0,
        vec![],
        vec![tap_selection(1, &[(bear, generation + 1)])],
    );
    engine
        .apply_command(0, &stale)
        .expect_err("stale generation must fail");
    assert!(!engine.state.objects[&gene].tapped);
    assert!(!engine.state.objects[&bear].tapped);

    let duplicate = activate_ability_with_costs(
        gene,
        0,
        vec![],
        vec![tap_selection(1, &[(bear, generation), (bear, generation)])],
    );
    engine
        .apply_command(0, &duplicate)
        .expect_err("duplicate object must fail");
    assert!(!engine.state.objects[&gene].tapped);
    assert!(!engine.state.objects[&bear].tapped);
}

#[test]
fn gravelgill_scoundrel_uses_a_private_generation_bound_resolution_payment() {
    let decks = Some(vec![
        deck_with("island", &["gravelgill_scoundrel", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(144_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let scoundrel = relocate_to_battlefield(&mut engine, 0, "gravelgill_scoundrel", false);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&scoundrel)
        .unwrap()
        .summoning_sick = false;
    engine.state.objects.get_mut(&bear).unwrap().summoning_sick = true;
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine.apply_command(0, &pass()).expect("attacker passes");
    engine.apply_command(1, &pass()).expect("defender passes");
    engine
        .apply_command(0, &declare_attackers(vec![scoundrel]))
        .expect("attack and stage trigger");
    engine.apply_command(0, &pass()).expect("attacker passes");
    let branch_batch = engine.apply_command(1, &pass()).expect("trigger resolves");
    let branch = find_resolution_choice(&branch_batch).expect("resolution branch");
    assert_eq!(
        branch.choice_kind(),
        tricerules_proto::ruled::v1::ChoiceKind::ResolutionBranch
    );

    let payment_batch = engine
        .apply_command(0, &select_branch(0))
        .expect("select tap-payment branch");
    let payment = find_resolution_choice(&payment_batch).expect("tap payment");
    assert_eq!(
        payment.choice_kind(),
        tricerules_proto::ruled::v1::ChoiceKind::CostObjects
    );
    assert_eq!(payment.candidate_object_ids, [bear]);
    assert_eq!(payment.min, 0);
    assert_eq!(payment.max, 1);
    assert!(payment.prompt_text.contains("or decline"));

    engine
        .apply_command(0, &submit_resolution_choice(vec![bear]))
        .expect("tap the other creature and resume");
    assert!(engine.state.objects[&bear].tapped);
    assert!(
        !engine.state.objects[&scoundrel].tapped,
        "vigilance keeps attacker untapped"
    );
}

#[test]
fn command_bridge_taps_a_physical_permanent_or_sacrifices_itself() {
    let decks = Some(vec![
        deck_with("forest", &["command_bridge", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(144_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.objects.get_mut(&bear).unwrap().summoning_sick = true;
    ensure_in_hand(&mut engine, 0, "command_bridge");
    let slot = hand_index_for_card(&engine, 0, "command_bridge");
    let bridge = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &play_land(slot))
        .expect("play Command Bridge");
    assert!(engine.state.objects[&bridge].tapped);
    engine.apply_command(0, &pass()).expect("controller passes");
    let branch_batch = engine.apply_command(1, &pass()).expect("ETB resolves");
    let branch = find_resolution_choice(&branch_batch).expect("tap or sacrifice branch");
    assert_eq!(branch.resolution_branches.len(), 2);
    let payment_batch = engine
        .apply_command(0, &select_branch(0))
        .expect("choose tap payment");
    let payment = find_resolution_choice(&payment_batch).expect("tap candidate prompt");
    assert_eq!(payment.candidate_object_ids, [bear]);
    assert_eq!(payment.min, 1);
    assert!(!payment.prompt_text.contains("decline"));
    engine
        .apply_command(0, &submit_resolution_choice(vec![bear]))
        .expect("tap creature");
    assert!(engine.state.objects[&bear].tapped);
    assert_eq!(
        engine.state.objects[&bridge].zone,
        tricerules_core::Zone::Battlefield
    );

    let decks = Some(vec![
        deck_with("forest", &["command_bridge"]),
        deck_with("forest", &[]),
    ]);
    let mut fallback = GameEngine::new(144_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut fallback);
    ensure_in_hand(&mut fallback, 0, "command_bridge");
    let slot = hand_index_for_card(&fallback, 0, "command_bridge");
    let bridge = fallback.state.players[0].hand[slot];
    fallback
        .apply_command(0, &play_land(slot))
        .expect("play Command Bridge");
    fallback
        .apply_command(0, &pass())
        .expect("controller passes");
    fallback
        .apply_command(1, &pass())
        .expect("fallback sacrifices Bridge");
    assert_eq!(
        fallback.state.objects[&bridge].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn untapped_command_bridge_can_pay_for_itself_or_remain_untapped() {
    for choose_bridge in [true, false] {
        let decks = Some(vec![
            deck_with("forest", &["command_bridge", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(144_006, &[0, 1], 20, decks, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        ensure_in_hand(&mut engine, 0, "command_bridge");
        let slot = hand_index_for_card(&engine, 0, "command_bridge");
        let bridge = engine.state.players[0].hand[slot];
        engine
            .apply_command(0, &play_land(slot))
            .expect("play Bridge");
        // Model an untap before the entry trigger resolves. Its text does not say "another".
        engine.state.objects.get_mut(&bridge).unwrap().tapped = false;
        engine.apply_command(0, &pass()).expect("controller passes");
        engine
            .apply_command(1, &pass())
            .expect("entry trigger resolves");
        let payment_batch = engine
            .apply_command(0, &select_branch(0))
            .expect("tap branch");
        let payment = find_resolution_choice(&payment_batch).expect("payment choice");
        assert!(payment.candidate_object_ids.contains(&bridge));
        assert!(payment.candidate_object_ids.contains(&bear));
        let chosen = if choose_bridge { bridge } else { bear };
        engine
            .apply_command(0, &submit_resolution_choice(vec![chosen]))
            .expect("pay only the chosen tap cost");
        assert_eq!(
            engine.state.objects[&bridge].zone,
            tricerules_core::Zone::Battlefield
        );
        assert_eq!(engine.state.objects[&bridge].tapped, choose_bridge);
        assert_eq!(engine.state.objects[&bear].tapped, !choose_bridge);
    }
}
