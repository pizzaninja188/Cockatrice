use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, ruled_event::Ev, CostChoiceKind, CostObjectRef,
    CostSelection, CounterRemovalSelection, PreviewPayment,
};

fn counter_selection(
    cost_index: u32,
    object_id: u32,
    zone_change_generation: u64,
) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(Selection::CounterRemoval(CounterRemovalSelection {
            source: Some(CostObjectRef {
                object_id,
                zone_change_generation,
            }),
            option_id: 1,
        })),
    }
}

#[test]
fn issue_193_ray_fillet_publishes_and_pays_from_a_selected_controlled_creature() {
    let decks = Some(vec![
        deck_with("island", &["ray_fillet,_man_ray", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(193_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let ray = relocate_to_battlefield(&mut engine, 0, "ray_fillet,_man_ray", false);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&bear)
        .expect("bear")
        .add_counters(CounterKind::PlusOnePlusOne, 1, 0);
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    let legal = engine.initial_response_batch();
    let key = u64::from(ray) << 32;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&key];
    assert!(choices.non_mana_costs_payable);
    assert_eq!(choices.choices.len(), 1);
    let choice = &choices.choices[0];
    assert_eq!(choice.kind(), CostChoiceKind::RemoveCounters);
    assert_eq!(choice.candidate_ids, [bear]);
    assert_eq!(choice.counter_removal.as_ref().unwrap().source, None);
    let candidate = choice.candidate_objects[0]
        .object
        .as_ref()
        .expect("generation-bound counter source");
    assert_eq!(candidate.object_id, bear);
    assert_eq!(candidate.zone_change_generation, generation);
    assert_eq!(choice.candidate_objects[0].contribution, 1);

    let before = engine.state.players[0].hand.len();
    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                ray,
                0,
                vec![],
                vec![counter_selection(1, bear, generation)],
            ),
        )
        .expect("pay Ray Fillet's counter cost from the selected creature");
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
}

#[test]
fn issue_193_illegal_or_stale_counter_sources_reject_without_partial_payment() {
    let decks = Some(vec![
        deck_with(
            "island",
            &["ray_fillet,_man_ray", "grizzly_bears", "grizzly_bears"],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(193_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let ray = relocate_to_battlefield(&mut engine, 0, "ray_fillet,_man_ray", false);
    let funded = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let empty = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let opponent = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    for oid in [funded, opponent] {
        engine
            .state
            .objects
            .get_mut(&oid)
            .expect("creature")
            .add_counters(CounterKind::PlusOnePlusOne, 1, 0);
    }
    let generation = |engine: &GameEngine, oid| {
        engine
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0)
    };
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    let legal = engine.initial_response_batch();
    let key = u64::from(ray) << 32;
    let choice = &legal.legal_by_player[&0].cost_choices_by_ability[&key].choices[0];
    assert_eq!(choice.candidate_ids, [funded]);

    let mana_before = engine.state.players[0].mana_pool.colorless;
    let command_index_before = engine.state.command_index;
    for selected in [
        counter_selection(1, funded, generation(&engine, funded) + 1),
        counter_selection(1, empty, generation(&engine, empty)),
        counter_selection(1, opponent, generation(&engine, opponent)),
    ] {
        engine
            .apply_command(
                0,
                &activate_ability_with_costs(ray, 0, vec![], vec![selected]),
            )
            .expect_err("illegal selected counter source must reject");
        assert_eq!(engine.state.players[0].mana_pool.colorless, mana_before);
        assert_eq!(engine.state.command_index, command_index_before);
        assert_eq!(
            engine.state.objects[&funded].counter_count(CounterKind::PlusOnePlusOne),
            1
        );
        assert_eq!(
            engine.state.objects[&opponent].counter_count(CounterKind::PlusOnePlusOne),
            1
        );
        assert!(engine.state.stack.is_empty());
    }

    engine.state.players[0]
        .battlefield
        .retain(|oid| *oid != funded);
    engine.state.players[1].battlefield.push(funded);
    engine.state.objects.get_mut(&funded).unwrap().controller = 1;
    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                ray,
                0,
                vec![],
                vec![counter_selection(1, funded, generation(&engine, funded))],
            ),
        )
        .expect_err("current derived control must be revalidated");
    assert_eq!(engine.state.players[0].mana_pool.colorless, mana_before);
    assert_eq!(
        engine.state.objects[&funded].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_193_sage_of_fables_supplies_a_counter_then_spends_it_from_the_other_wizard() {
    let decks = Some(vec![
        deck_with("island", &["sage_of_fables", "fugitive_wizard"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(193_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let sage = relocate_to_battlefield(&mut engine, 0, "sage_of_fables", false);
    let initial = engine.initial_response_batch();
    let presentation = initial
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .and_then(|view| view.per_player.iter().find(|view| view.player_id == 0))
        .and_then(|view| {
            view.battlefield_objects
                .iter()
                .find(|object| object.object_id == sage)
        })
        .and_then(|object| object.activated_abilities.first())
        .and_then(|ability| ability.presentation.as_ref())
        .expect("Sage activation presentation");
    assert_eq!(presentation.external_card_name, "Sage of Fables");
    assert_eq!(presentation.external_face_name, "Sage of Fables");
    assert_eq!(presentation.oracle_line_indices, [2]);
    assert_eq!(presentation.oracle_text_sha256.len(), 64);
    ensure_card_in_hand(&mut engine, 0, "fugitive_wizard");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "fugitive_wizard");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast the other Wizard");
    pass_both_players(&mut engine);
    let wizard = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == "fugitive_wizard")
        .expect("Wizard entered");
    assert_eq!(
        engine.state.objects[&wizard].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.objects[&sage].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "Sage excludes itself from its entry replacement"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let generation = engine.state.zone_change_generation[&wizard];
    let before = engine.state.players[0].hand.len();
    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                sage,
                0,
                vec![],
                vec![counter_selection(1, wizard, generation)],
            ),
        )
        .expect("spend the other Wizard's counter");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&wizard].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
}

#[test]
fn issue_193_ability_payment_preview_does_not_offer_spell_only_convoke() {
    let decks = Some(vec![
        deck_with("island", &["sage_of_fables", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(193_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let sage = relocate_to_battlefield(&mut engine, 0, "sage_of_fables", false);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&bear)
        .expect("bear")
        .add_counters(CounterKind::PlusOnePlusOne, 1, 0);
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);
    let command = activate_ability_with_costs(
        sage,
        0,
        vec![],
        vec![counter_selection(1, bear, generation)],
    );
    let Some(Cmd::ActivateAbility(mut activation)) = command.cmd else {
        unreachable!();
    };

    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            transaction_id: 193,
            revision: 1,
            activate_ability: Some(activation.clone()),
            ..Default::default()
        },
    );

    assert!(preview.valid, "{}", preview.error);
    assert!(!preview.complete);
    assert_eq!(preview.remaining_cost, "{2}");
    assert!(
        preview.candidates.is_empty(),
        "activated abilities must not inherit spell-only Convoke candidates"
    );

    activation.payment = preview.selection;
    activation
        .payment
        .as_mut()
        .expect("preview selection")
        .convoke
        .push(tricerules_proto::ruled::v1::ObjectPaymentContribution {
            object: Some(CostObjectRef {
                object_id: bear,
                zone_change_generation: generation,
            }),
            kind: tricerules_proto::ruled::v1::ObjectPaymentKind::Generic as i32,
        });
    let stale_selection_preview = engine.preview_payment(
        0,
        &PreviewPayment {
            transaction_id: 193,
            revision: 2,
            activate_ability: Some(activation),
            ..Default::default()
        },
    );
    assert!(
        stale_selection_preview.valid,
        "{}",
        stale_selection_preview.error
    );
    assert!(stale_selection_preview.selection_changed);
    assert!(stale_selection_preview
        .selection
        .as_ref()
        .expect("normalized selection")
        .convoke
        .is_empty());
    assert!(!stale_selection_preview.complete);
    assert!(stale_selection_preview.candidates.is_empty());
}
