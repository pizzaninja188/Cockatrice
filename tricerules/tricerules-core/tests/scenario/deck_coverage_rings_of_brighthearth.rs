//! Rings of Brighthearth observes committed nonmana activations and copies their abilities.
//!
//! CR 602.2b, 603.2, 605.1a, and 707.10a-c govern the activation, trigger, and copy.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::Zone;

const RINGS: &str = "rings_of_brighthearth";

const OBSERVER_FIXTURE: &str = r#"(
    id: "activation_observer_fixture",
    name: "Activation Observer Fixture",
    face_id: "activation_observer_fixture",
    mana_cost: "{3}", types: ["Artifact"],
    triggered_abilities: [(
        ability_id: "triggered_01", presentation: Fallback,
        trigger: WheneverControllerActivatesNonManaAbility,
        effect: [GainLife(amount: 1)],
    )],
)"#;

const TARGETED_MODAL_ACTIVATED_COPY_FIXTURE: &str = r#"(
    id: "targeted_modal_activated_copy", name: "Targeted Modal Activated Copy",
    face_id: "targeted_modal_activated_copy", types: ["Artifact"],
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
        costs: [Mana("{4}"), Tap], effect: [], timing: SorcerySpeed,
        modal: Some((min_modes: 1, max_modes: 1, modes: [
            (mode_id: "double_any_permanent", presentation: Fallback,
                effects: [DoubleCounters(target: (kind: AnyPermanent))],
                targeting: Some((groups: [
                    (min: 1, max: 1, prompt: "Choose a permanent", effect_indices: [0])
                ]))
            ),
            (mode_id: "draw_one", presentation: Fallback,
                effects: [Draw(who: Controller, count: 1)]),
            (mode_id: "draw_two", presentation: Fallback,
                effects: [Draw(who: Controller, count: 2)])
        ]))
    )],
)"#;

fn observer_fixture_game(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                OBSERVER_FIXTURE,
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/sol_ring.ron"),
                include_str!("../../../tricerules-cards/data/voltaic_key.ron"),
                include_str!("../../../tricerules-cards/data/slobad,_goblin_tinkerer.ron"),
            ],
            &[],
        )
        .expect("nonmana activation observer fixture compiles"),
    ));
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("create fixture game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn observer_fixture_only_committed_controller_nonmana_activations_have_exact_stack_receipts() {
    let mut engine = observer_fixture_game(261_027_110);
    inject_permanent_on_battlefield(&mut engine, 0, "activation_observer_fixture");
    let mana = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    give_colorless(&mut engine, 1);
    apply_ability(&mut engine, 0, key, 0, target_object(land)).expect_err("invalid target");
    assert!(engine.state.stack.is_empty());
    apply_ability(&mut engine, 0, mana, 0, vec![]).expect("mana ability");
    assert!(engine.state.stack.is_empty());
    apply_ability(&mut engine, 0, key, 0, target_object(mana)).expect("nonmana ability");
    assert_eq!(engine.state.stack.len(), 2);
    let original = engine.state.stack[0].id;
    let trigger = &engine.state.stack[1];
    assert!(trigger.is_triggered);
    assert_eq!(
        trigger
            .trigger_context
            .activated_stack_object
            .unwrap()
            .object_id,
        original
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].life, 21);
    pass_both_players(&mut engine);

    let opponent_key = inject_permanent_on_battlefield(&mut engine, 1, "voltaic_key");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass to opponent");
    apply_ability(&mut engine, 1, opponent_key, 0, target_object(mana))
        .expect("opponent activates");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the opponent does not trigger the observer"
    );
}

#[test]
fn observer_fixture_only_sources_still_present_after_costs_see_the_activation() {
    let mut engine = observer_fixture_game(261_027_111);
    let sacrificed = inject_permanent_on_battlefield(&mut engine, 0, "activation_observer_fixture");
    let survivor = inject_permanent_on_battlefield(&mut engine, 0, "activation_observer_fixture");
    let slobad = inject_permanent_on_battlefield(&mut engine, 0, "slobad,_goblin_tinkerer");
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let mut command = activate_ability_with_costs(
        slobad,
        0,
        target_object(artifact),
        vec![permanent_cost_selection(0, sacrificed)],
    );
    if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
        activation.expected_zone_change_generation = engine
            .state
            .zone_change_generation
            .get(&slobad)
            .copied()
            .unwrap_or(0);
    }
    engine
        .apply_scenario_command(0, &command)
        .expect("pay the sacrifice cost");
    assert_eq!(engine.state.objects[&sacrificed].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 2);
    assert_eq!(engine.state.stack[1].source_permanent_id, Some(survivor));
    assert_eq!(
        engine.state.stack[1]
            .trigger_context
            .activated_stack_object
            .unwrap()
            .object_id,
        engine.state.stack[0].id
    );
}

fn game(seed: u64) -> GameEngine {
    let rings = tricerules_cards::registry::global()
        .get(RINGS)
        .expect("Rings of Brighthearth must be registered");
    assert_eq!(rings.name, "Rings of Brighthearth");
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("create game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn targeted_modal_copy_game(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                TARGETED_MODAL_ACTIVATED_COPY_FIXTURE,
                include_str!("../../../tricerules-cards/data/rings_of_brighthearth.ron"),
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/sol_ring.ron"),
                include_str!("../../../tricerules-cards/data/mind_stone.ron"),
            ],
            &[],
        )
        .expect("targeted modal copy fixture registry compiles"),
    ));
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("create targeted modal copy fixture game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn give_colorless(engine: &mut GameEngine, amount: u32) {
    give_mana(
        engine,
        0,
        ManaGift {
            c: amount,
            ..Default::default()
        },
    );
}

fn pay_for_copy(engine: &mut GameEngine) -> RuledEventBatch {
    let batch = pass_both_players_with_batch(engine);
    let choice = find_resolution_choice(&batch).expect("Rings offers the optional branch");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    let payment = engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose to pay for the copy");
    let choice = find_resolution_choice(&payment).expect("Rings asks for payment");
    assert_eq!(choice.choice_kind(), ChoiceKind::ManaPayment);
    assert_eq!(choice.generic_mana_cost, 2);
    submit_mana_resolution_decision(engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {2} for the copy")
}

fn tap(engine: &mut GameEngine, object: u32) {
    engine.state.objects.get_mut(&object).unwrap().tapped = true;
}

#[test]
fn rings_is_cast_for_three_and_enters_as_an_artifact() {
    let mut engine = game(261_027_100);
    let rings = inject_card_into_hand(&mut engine, 0, RINGS);
    give_colorless(&mut engine, 3);
    let slot = hand_index_for_card(&engine, 0, RINGS);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Rings");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&rings].zone, Zone::Battlefield);
    assert!(engine.characteristics(rings).unwrap().is_artifact());
}

#[test]
fn rings_observes_only_a_successful_nonmana_activation() {
    let mut engine = game(261_027_101);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
    let ring = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    give_colorless(&mut engine, 1);
    apply_ability(&mut engine, 0, key, 0, target_object(land))
        .expect_err("illegal target prevents the activation");
    assert!(!engine.state.objects[&key].tapped);
    assert!(engine.state.stack.is_empty());
    apply_ability(&mut engine, 0, ring, 0, vec![]).expect("activate a mana ability");
    assert!(
        engine.state.stack.is_empty(),
        "mana abilities do not trigger Rings"
    );
    apply_ability(&mut engine, 0, key, 0, target_object(ring)).expect("activate Key");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "ability and Rings trigger are on the stack"
    );
    assert!(engine.state.stack.last().unwrap().is_triggered);
    let batch = pass_both_players_with_batch(&mut engine);
    assert_eq!(
        find_resolution_choice(&batch).unwrap().choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the Rings trigger");
    assert_eq!(engine.state.stack.len(), 1, "declining creates no copy");
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(!engine.state.objects[&ring].tapped);
}

#[test]
fn rings_sacrificed_as_an_activation_cost_does_not_observe_the_activation() {
    let mut engine = game(261_027_102);
    let rings = inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let slobad = inject_permanent_on_battlefield(&mut engine, 0, "slobad,_goblin_tinkerer");
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let mut command = activate_ability_with_costs(
        slobad,
        0,
        target_object(artifact),
        vec![permanent_cost_selection(0, rings)],
    );
    if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
        activation.expected_zone_change_generation = engine
            .state
            .zone_change_generation
            .get(&slobad)
            .copied()
            .unwrap_or(0);
    }
    engine
        .apply_scenario_command(0, &command)
        .expect("sacrifice Rings to activate Slobad");
    assert_eq!(engine.state.objects[&rings].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the departed Rings cannot trigger"
    );
    assert!(!engine.state.stack.last().unwrap().is_triggered);
}

#[test]
fn rings_copies_daretti_without_paying_loyalty_again_or_triggering_itself() {
    let mut engine = game(261_027_103);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let daretti = inject_permanent_on_battlefield(&mut engine, 0, "daretti,_scrap_savant");
    engine
        .state
        .objects
        .get_mut(&daretti)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    for object in std::mem::take(&mut engine.state.players[0].hand) {
        engine.state.objects.get_mut(&object).unwrap().zone = Zone::Graveyard;
        engine.state.players[0].graveyard.push(object);
    }
    give_colorless(&mut engine, 2);
    apply_ability(&mut engine, 0, daretti, 0, vec![]).expect("activate Daretti's +2");
    assert_eq!(
        engine.state.objects[&daretti].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(engine.state.stack.len(), 2);
    let batch = pay_for_copy(&mut engine);
    assert!(batch.events.iter().any(|event| matches!(&event.ev, Some(Ev::StackPushed(push)) if push.is_copy && !push.is_triggered && push.card_id.is_empty())));
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "copy plus original, with no recursive trigger"
    );
    assert!(engine.state.stack.last().unwrap().is_copy);
    assert_eq!(
        engine.state.objects[&daretti].counter_count(CounterKind::Loyalty),
        5
    );
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&daretti].counter_count(CounterKind::Loyalty),
        5
    );
}

#[test]
fn rings_copies_key_with_original_or_changed_target() {
    for retarget in [false, true] {
        let mut engine = game(261_027_104 + u64::from(retarget));
        inject_permanent_on_battlefield(&mut engine, 0, RINGS);
        let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&key)
            .copied()
            .unwrap_or(0);
        let original = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let other = inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
        tap(&mut engine, original);
        tap(&mut engine, other);
        give_colorless(&mut engine, 3);
        apply_ability(&mut engine, 0, key, 0, target_object(original)).expect("activate Key");
        assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
        let batch = pay_for_copy(&mut engine);
        let choice = find_resolution_choice(&batch).expect("copy target choice");
        assert_eq!(choice.deciding_player_id, 0);
        assert!(choice.candidate_object_ids.contains(&other));
        let selection = if retarget { vec![other] } else { vec![] };
        let batch = engine
            .apply_scenario_command(0, &submit_resolution_choice(selection))
            .expect("keep or replace Key's target");
        let copy = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
                _ => None,
            })
            .expect("copy is placed on the stack");
        assert_eq!(
            copy.targets[0].object_id,
            if retarget { other } else { original }
        );
        assert!(copy.card_id.is_empty() && !copy.is_triggered);
        assert_eq!(copy.ability_controller_player_id, Some(0));
        let source = copy
            .ability_source
            .as_ref()
            .expect("the current public source is published");
        assert_eq!(
            (source.object_id, source.zone_change_generation),
            (key, source_generation)
        );
        assert_eq!(engine.state.stack.len(), 2);
        pass_both_players(&mut engine);
        assert_eq!(engine.state.objects[&original].tapped, retarget);
        assert_eq!(engine.state.objects[&other].tapped, !retarget);
        pass_both_players(&mut engine);
        assert!(!engine.state.objects[&original].tapped);
        assert!(
            engine.state.objects[&key].tapped,
            "the copy never pays Key's tap cost"
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn rings_copies_targeted_activated_modal_mode_and_allows_retargeting() {
    use tricerules_proto::ruled::v1 as rv1;

    let mut engine = targeted_modal_copy_game(261_027_146);

    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "targeted_modal_activated_copy");
    let original_target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let copy_target = inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
    engine
        .state
        .objects
        .get_mut(&original_target)
        .unwrap()
        .set_counter(CounterKind::Charge, 2);
    engine
        .state
        .objects
        .get_mut(&copy_target)
        .unwrap()
        .set_counter(CounterKind::Charge, 3);

    let original_generation = engine
        .state
        .zone_change_generation
        .get(&original_target)
        .copied()
        .unwrap_or(0);
    let copy_generation = engine
        .state
        .zone_change_generation
        .get(&copy_target)
        .copied()
        .unwrap_or(0);
    let source_ref = rv1::CostObjectRef {
        object_id: source,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0),
    };
    let source_generation = source_ref.zone_change_generation;
    give_colorless(&mut engine, 6);
    let activation = rv1::ActivateAbility {
        source_object_id: source,
        ability_index: 0,
        expected_zone_change_generation: source_ref.zone_change_generation,
        selected_modes: vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: vec![rv1::TargetRef {
                object_id: original_target,
                group_index: 0,
                kind: rv1::TargetRefKind::Permanent as i32,
                expected_zone_change_generation: Some(original_generation),
                ..Default::default()
            }],
        }],
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_ref),
            mana: Some(rv1::PaymentMana {
                c: 4,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ActivateAbility(activation)),
            },
        )
        .expect("pay {4} and activate the targeted modal mode");

    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    let original_stack_id = engine.state.stack[0].id;
    let original_stack = &engine.state.stack[0];
    assert!(original_stack.targets.is_empty());
    assert_eq!(original_stack.chosen_modes.len(), 1);
    assert_eq!(
        original_stack.chosen_modes[0].mode_id,
        tricerules_card_model::ModeId::new("double_any_permanent").unwrap()
    );
    assert_eq!(
        original_stack.chosen_modes[0].targets[0].object_id,
        original_target
    );
    assert_eq!(
        original_stack.chosen_modes[0].targets[0].zone_change_generation,
        Some(original_generation)
    );

    let batch = pay_for_copy(&mut engine);
    let choice = find_resolution_choice(&batch).expect("copy target choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::TargetObjects);
    assert!(choice.candidate_object_ids.contains(&original_target));
    assert!(choice.candidate_object_ids.contains(&copy_target));
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);

    // The copy prompt and owner come from the captured activation, even after the source leaves.
    engine.state.players[0]
        .battlefield
        .retain(|&object_id| object_id != source);
    engine.state.players[0].graveyard.push(source);
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Graveyard;
    engine
        .state
        .zone_change_generation
        .insert(source, source_generation.saturating_add(1));

    // A delayed selection against the prior incarnation is reoffered without placing a copy or
    // changing the original activation. The next response uses the newly offered generation.
    let new_copy_generation = copy_generation.saturating_add(1);
    engine
        .state
        .zone_change_generation
        .insert(copy_target, new_copy_generation);
    let refreshed = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![copy_target]))
        .expect("stale target receipt is reoffered");
    let refreshed_choice = find_resolution_choice(&refreshed).expect("refreshed target choice");
    assert!(refreshed_choice.candidate_object_ids.contains(&copy_target));
    assert!(!refreshed.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackPushed(push)) if push.is_copy)));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the original remains the only stack item"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&source].tapped);
    assert!(!engine.state.players[0].battlefield.contains(&source));
    assert_eq!(
        engine.state.objects[&copy_target].counter_count(CounterKind::Charge),
        3
    );
    let original_stack = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == original_stack_id)
        .expect("original activation remains unchanged");
    assert_eq!(
        original_stack.chosen_modes[0].targets[0].object_id,
        original_target
    );
    assert_eq!(
        original_stack.chosen_modes[0].targets[0].zone_change_generation,
        Some(original_generation)
    );

    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![copy_target]))
        .expect("retarget the modal copy to another permanent");
    let copy_id = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push.object_id),
            _ => None,
        })
        .expect("Rings places the modal copy on the stack");
    let original_stack = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == original_stack_id)
        .expect("original activation remains on the stack");
    assert_eq!(
        original_stack.chosen_modes[0].mode_id,
        tricerules_card_model::ModeId::new("double_any_permanent").unwrap()
    );
    assert_eq!(
        original_stack.chosen_modes[0].targets[0].object_id, original_target,
        "retargeting the copy leaves the original target unchanged"
    );
    assert_eq!(
        original_stack.chosen_modes[0].targets[0].zone_change_generation,
        Some(original_generation)
    );
    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == copy_id)
        .expect("copy is present on the stack");
    assert_eq!(copy.chosen_modes.len(), 1);
    assert_eq!(
        copy.chosen_modes[0].mode_id,
        tricerules_card_model::ModeId::new("double_any_permanent").unwrap(),
        "retargeting preserves the captured mode"
    );
    assert_eq!(copy.chosen_modes[0].targets[0].object_id, copy_target);
    assert_eq!(
        copy.chosen_modes[0].targets[0].zone_change_generation,
        Some(new_copy_generation)
    );

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&copy_target].counter_count(CounterKind::Charge),
        6,
        "the retargeted copy doubles only its independently selected target"
    );
    assert_eq!(
        engine.state.objects[&original_target].counter_count(CounterKind::Charge),
        2,
        "resolving the copy does not alter the original target"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&original_target].counter_count(CounterKind::Charge),
        4,
        "the original activation later doubles its original target"
    );
    assert_eq!(
        engine.state.objects[&copy_target].counter_count(CounterKind::Charge),
        6
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&source].tapped);
}

#[test]
fn rings_copies_targetless_activated_modal_mode_with_captured_presentation() {
    use tricerules_proto::ruled::v1 as rv1;

    let mut engine = targeted_modal_copy_game(261_027_147);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "targeted_modal_activated_copy");
    let source_ref = rv1::CostObjectRef {
        object_id: source,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0),
    };
    give_colorless(&mut engine, 6);
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();
    let activation = rv1::ActivateAbility {
        source_object_id: source,
        ability_index: 0,
        expected_zone_change_generation: source_ref.zone_change_generation,
        selected_modes: vec![rv1::SelectedSpellMode {
            mode_index: 1,
            targets: Vec::new(),
        }],
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_ref),
            mana: Some(rv1::PaymentMana {
                c: 4,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ActivateAbility(activation)),
            },
        )
        .expect("pay {4} and activate draw_one");

    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(engine.state.stack[0].chosen_modes[0].targets.is_empty());
    let batch = pay_for_copy(&mut engine);
    assert!(find_resolution_choice(&batch).is_none());
    let copy = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
            _ => None,
        })
        .expect("targetless modal copy is placed immediately");
    assert_eq!(copy.chosen_mode_indices, vec![1]);
    assert_eq!(copy.chosen_mode_labels.len(), 1);
    let mode_presentation = copy
        .chosen_mode_presentations
        .first()
        .expect("copied mode keeps its public presentation");
    assert!(!mode_presentation.fallback_text.trim().is_empty());
    assert!(mode_presentation.path.last().is_some_and(|component| {
        component.kind == rv1::PresentationPathKind::Mode as i32 && component.id == "draw_one"
    }));
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);

    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&source].tapped);
}

#[test]
fn rings_copies_welder_with_original_or_new_paired_targets() {
    for retarget in [false, true] {
        let mut engine = game(261_027_106 + u64::from(retarget));
        inject_permanent_on_battlefield(&mut engine, 0, RINGS);
        let welder = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
        let original_out = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let other_out = inject_permanent_on_battlefield(&mut engine, 0, "short_sword");
        let original_in = inject_graveyard_card(&mut engine, 0, "chromatic_lantern");
        let other_in = inject_graveyard_card(&mut engine, 0, "mind_stone");
        give_colorless(&mut engine, 2);
        apply_ability(
            &mut engine,
            0,
            welder,
            0,
            vec![
                TargetRef {
                    kind: TargetRefKind::Permanent as i32,
                    object_id: original_out,
                    group_index: 0,
                    ..Default::default()
                },
                TargetRef {
                    kind: TargetRefKind::Graveyard as i32,
                    object_id: original_in,
                    group_index: 1,
                    ..Default::default()
                },
            ],
        )
        .expect("activate Welder with a valid pair");
        let batch = pay_for_copy(&mut engine);
        assert!(
            find_resolution_choice(&batch).is_some(),
            "first target choice"
        );
        let first = if retarget { vec![other_out] } else { vec![] };
        let batch = engine
            .apply_scenario_command(0, &submit_resolution_choice(first))
            .expect("choose first target");
        assert!(
            find_resolution_choice(&batch).is_some(),
            "second target choice"
        );
        let second = if retarget { vec![other_in] } else { vec![] };
        let batch = engine
            .apply_scenario_command(0, &submit_resolution_choice(second))
            .expect("choose second target");
        let copy = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
                _ => None,
            })
            .expect("Welder copy is placed on stack");
        assert_eq!(
            copy.targets[0].object_id,
            if retarget { other_out } else { original_out }
        );
        assert_eq!(
            copy.targets[1].object_id,
            if retarget { other_in } else { original_in }
        );
        pass_both_players(&mut engine);
        assert_eq!(
            engine.state.objects[&if retarget { other_out } else { original_out }].zone,
            Zone::Graveyard
        );
        assert_eq!(
            engine.state.objects[&if retarget { other_in } else { original_in }].zone,
            Zone::Battlefield
        );
        pass_both_players(&mut engine);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn edge_welder_copy_may_keep_both_targets_after_their_matched_player_changes() {
    let mut engine = game(261_027_128);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let welder = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let incoming = inject_graveyard_card(&mut engine, 0, "mind_stone");
    give_colorless(&mut engine, 2);
    apply_ability(
        &mut engine,
        0,
        welder,
        0,
        vec![
            TargetRef {
                kind: TargetRefKind::Permanent as i32,
                object_id: departure,
                group_index: 0,
                ..Default::default()
            },
            TargetRef {
                kind: TargetRefKind::Graveyard as i32,
                object_id: incoming,
                group_index: 1,
                ..Default::default()
            },
        ],
    )
    .expect("activate Welder with a valid same-player pair");

    // Synthetic control change isolates CR 115.7d after announcement; both objects stay in their
    // original zones and incarnations, but their current pair no longer matches a single player.
    let object = engine.state.objects.get_mut(&departure).unwrap();
    object.base_controller = 1;
    object.controller = 1;
    engine.state.players[0]
        .battlefield
        .retain(|&id| id != departure);
    engine.state.players[1].battlefield.push(departure);

    let batch = pay_for_copy(&mut engine);
    assert!(find_resolution_choice(&batch).is_some());
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the first target");
    assert!(find_resolution_choice(&batch).is_some());
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the second target");
    let copy = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
            _ => None,
        })
        .expect("both unchanged targets are retained even when no longer a valid pair");
    assert_eq!(copy.targets[0].object_id, departure);
    assert_eq!(copy.targets[1].object_id, incoming);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&departure].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the invalid copy does not exchange the artifacts"
    );
}

#[test]
fn edge_welder_copy_reopens_if_a_new_target_breaks_the_matched_player_pair() {
    let mut engine = game(261_027_129);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let welder = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let opponent_artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let incoming = inject_graveyard_card(&mut engine, 0, "mind_stone");
    inject_graveyard_card(&mut engine, 1, "chromatic_lantern");
    give_colorless(&mut engine, 2);
    apply_ability(
        &mut engine,
        0,
        welder,
        0,
        vec![
            TargetRef {
                kind: TargetRefKind::Permanent as i32,
                object_id: departure,
                group_index: 0,
                ..Default::default()
            },
            TargetRef {
                kind: TargetRefKind::Graveyard as i32,
                object_id: incoming,
                group_index: 1,
                ..Default::default()
            },
        ],
    )
    .expect("activate Welder");
    let batch = pay_for_copy(&mut engine);
    assert!(find_resolution_choice(&batch)
        .unwrap()
        .candidate_object_ids
        .contains(&opponent_artifact));
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![opponent_artifact]))
        .expect("select the new departure target");
    assert!(find_resolution_choice(&batch).is_some());
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the original incoming target");
    assert!(
        find_resolution_choice(&batch).is_some(),
        "the mismatched new pair is reoffered"
    );
    assert!(!batch.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackPushed(push)) if push.is_copy)));
}

fn copy_welder_after_its_original_departure_returns(
    matching_lki: bool,
) -> (GameEngine, RuledEventBatch, u32, u32) {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
    let mut engine = game(261_027_138 + u64::from(matching_lki));
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let welder = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    // Synthetic pre-activation control change: the source remains owned by P0 but controlled by P1.
    engine.state.players[0]
        .battlefield
        .retain(|&id| id != departure);
    engine.state.players[1].battlefield.push(departure);
    let artifact = engine.state.objects.get_mut(&departure).unwrap();
    artifact.base_controller = 1;
    artifact.controller = 1;
    let initial = inject_graveyard_card(&mut engine, 1, "mind_stone");
    let same_player = inject_graveyard_card(&mut engine, 1, "chromatic_lantern");
    let other_player = inject_graveyard_card(&mut engine, 0, "short_sword");
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&departure)
        .copied()
        .unwrap_or(0);
    give_colorless(&mut engine, 2);
    apply_ability(
        &mut engine,
        0,
        welder,
        0,
        vec![
            TargetRef {
                kind: TargetRefKind::Permanent as i32,
                object_id: departure,
                group_index: 0,
                ..Default::default()
            },
            TargetRef {
                kind: TargetRefKind::Graveyard as i32,
                object_id: initial,
                group_index: 1,
                ..Default::default()
            },
        ],
    )
    .expect("activate Welder with P1's valid pair");
    engine.enable_dev_commands();
    for (zone, indexed_player) in [(DevZone::Graveyard, 1), (DevZone::Battlefield, 0)] {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        // The dev helper searches the current zone index (controller on the battlefield,
                        // owner in the graveyard); the actual move still puts the card in its owner's zone.
                        target_player_id: indexed_player,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: "Sol Ring".into(),
                            zone: zone as i32,
                            ready: true,
                        })),
                    })),
                },
            )
            .expect("the owned artifact leaves and returns through the real zone-change path");
    }
    assert_eq!(
        engine.state.last_known_controller_by_generation[&(departure, old_generation)],
        1
    );
    assert_eq!(engine.state.objects[&departure].controller, 0);
    assert_eq!(
        engine.state.zone_change_generation[&departure],
        old_generation + 2
    );
    let batch = pay_for_copy(&mut engine);
    assert!(find_resolution_choice(&batch).is_some());
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the stale original battlefield target");
    let next = find_resolution_choice(&batch).expect("choose the graveyard target");
    let incoming = if matching_lki {
        same_player
    } else {
        other_player
    };
    assert!(next.candidate_object_ids.contains(&incoming));
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![incoming]))
        .expect("answer the graveyard target offer");
    (engine, batch, departure, incoming)
}

#[test]
fn edge_welder_new_graveyard_target_cannot_use_a_stale_departures_returned_controller() {
    let (_engine, batch, _, _) = copy_welder_after_its_original_departure_returns(false);
    assert!(
        find_resolution_choice(&batch).is_some(),
        "P0 does not match the departed target's last controller P1"
    );
    assert!(!batch.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackPushed(push)) if push.is_copy)));
}

#[test]
fn edge_welder_new_graveyard_target_can_match_a_stale_departures_last_controller() {
    let (_engine, batch, departure, incoming) =
        copy_welder_after_its_original_departure_returns(true);
    let copy = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
            _ => None,
        })
        .expect("P1 matches the old battlefield occurrence's last controller");
    assert_eq!(copy.targets[0].object_id, departure);
    assert_eq!(copy.targets[1].object_id, incoming);
}

#[test]
fn edge_welder_new_battlefield_target_does_not_inherit_stale_graveyard_owner() {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
    let mut engine = game(261_027_140);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let welder = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let other = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let incoming = inject_graveyard_card(&mut engine, 0, "mind_stone");
    give_colorless(&mut engine, 2);
    apply_ability(
        &mut engine,
        0,
        welder,
        0,
        vec![
            TargetRef {
                kind: TargetRefKind::Permanent as i32,
                object_id: departure,
                group_index: 0,
                ..Default::default()
            },
            TargetRef {
                kind: TargetRefKind::Graveyard as i32,
                object_id: incoming,
                group_index: 1,
                ..Default::default()
            },
        ],
    )
    .expect("activate Welder with a valid pair");
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Mind Stone".into(),
                        zone: DevZone::Hand as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("the original graveyard target moves to its owner's hand");
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Hand);

    let batch = pay_for_copy(&mut engine);
    assert!(find_resolution_choice(&batch)
        .unwrap()
        .candidate_object_ids
        .contains(&other));
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![other]))
        .expect("choose P1's battlefield artifact");
    assert!(find_resolution_choice(&batch).is_some());
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("retain the stale graveyard target");
    let copy = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
            _ => None,
        })
        .expect("a stale graveyard target does not constrain the new battlefield target");
    assert_eq!(copy.targets[0].object_id, other);
    assert_eq!(copy.targets[1].object_id, incoming);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&other].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Hand);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the copy resolves without an exchange"
    );
}

#[test]
fn edge_cycling_from_hand_triggers_rings_and_both_draws_resolve() {
    use tricerules_proto::ruled::v1::AbilitySourceZone;
    let mut engine = game(261_027_120);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let cave = inject_card_into_hand(&mut engine, 0, "forgotten_cave");
    let hand_before = engine.state.players[0].hand.len();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let mut command = activate_ability_for(&engine, cave, 1, vec![]);
    if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
        activation.source_zone = AbilitySourceZone::Hand as i32;
    }
    engine
        .apply_scenario_command(0, &command)
        .expect("cycle Forgotten Cave");
    assert_eq!(engine.state.objects[&cave].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 2);
    let copied = pay_for_copy(&mut engine);
    let copy = copied
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
            _ => None,
        })
        .expect("cycling copy is published");
    assert_eq!(copy.ability_controller_player_id, Some(0));
    assert!(
        copy.ability_source.is_none(),
        "the hand/graveyard source never becomes a battlefield arrow"
    );
    assert_eq!(engine.state.stack.len(), 2);
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert!(engine.state.stack.is_empty());
}

fn counter_with_tidebinder(engine: &mut GameEngine, ability_id: u32) {
    if engine.state.priority_player_id() == 0 {
        engine
            .apply_scenario_command(0, &pass())
            .expect("pass to opponent");
    }
    inject_card_into_hand(engine, 1, "tishanas_tidebinder");
    give_mana(
        engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 1, "tishanas_tidebinder");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("flash Tidebinder");
    pass_both_players(engine);
    let choice = RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                object_id: ability_id,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(1, &choice)
        .expect("target original ability");
    pass_both_players(engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
}

#[test]
fn edge_countered_original_is_copied_with_its_latest_targets_and_captured_definition() {
    let mut engine = game(261_027_121);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
    let first = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let changed = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    tap(&mut engine, first);
    tap(&mut engine, changed);
    give_colorless(&mut engine, 3);
    apply_ability(&mut engine, 0, key, 0, target_object(first)).expect("activate Key");
    let original = engine.state.stack[0].id;
    // Isolate a target change before countering: the exit receipt must freeze the latest target.
    engine.state.stack[0].targets[0].object_id = changed;
    engine.state.stack[0].targets[0].zone_change_generation = Some(
        engine
            .state
            .zone_change_generation
            .get(&changed)
            .copied()
            .unwrap_or(0),
    );
    counter_with_tidebinder(&mut engine, original);
    let batch = pay_for_copy(&mut engine);
    assert!(find_resolution_choice(&batch).is_some());
    let pushed = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the latest target");
    let copy = pushed
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
            _ => None,
        })
        .expect("copy reaches stack");
    assert_eq!(copy.targets[0].object_id, changed);
    pass_both_players(&mut engine);
    assert!(!engine.state.objects[&changed].tapped);
    assert!(engine.state.objects[&first].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn edge_fizzled_original_keeps_its_prepruning_target_in_the_copy() {
    let mut engine = game(261_027_122);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
    let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    give_colorless(&mut engine, 3);
    apply_ability(&mut engine, 0, key, 0, target_object(target)).expect("activate Key");
    // Synthetic stack ordering isolates the original resolving before its already-created observer.
    engine.state.stack.swap(0, 1);
    engine.state.players[0]
        .battlefield
        .retain(|&id| id != target);
    engine.state.players[0].graveyard.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    let batch = pay_for_copy(&mut engine);
    assert!(
        find_resolution_choice(&batch).is_some(),
        "the old target survives as an occurrence"
    );
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep an illegal original target");
    let copy = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
            _ => None,
        })
        .expect("copy reaches stack");
    assert_eq!(copy.targets[0].object_id, target);
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
}

const TAIL_FIXTURE: &str = r#"(
  id: "copy_tail_fixture", name: "Copy Tail Fixture", face_id: "copy_tail_fixture",
  mana_cost: "{3}", types: ["Artifact"],
  triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
    trigger: WheneverControllerActivatesNonManaAbility,
    program: Some((body: (steps: [
      (id: "choose", kind: Choose(choice: "copy", chooser: Controller, optional: true,
        branches: [(branch_id: "pay", presentation: Fallback, cost: Mana("{2}"),
          body: (steps: [(id: "copy", kind: Perform(CopyTriggeringActivatedAbility))]))],
        otherwise: (),
      )),
      (id: "tail", kind: Perform(GainLife(amount: 1))),
    ]))),
  )],
)"#;

#[test]
fn edge_targeted_copy_resumes_the_parent_program_tail_once_before_priority() {
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                TAIL_FIXTURE,
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/sol_ring.ron"),
                include_str!("../../../tricerules-cards/data/voltaic_key.ron"),
            ],
            &[],
        )
        .expect("tail fixture compiles"),
    ));
    let mut engine = GameEngine::new(
        registry,
        261_027_123,
        &[0, 1],
        20,
        Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "copy_tail_fixture");
    let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
    let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    give_colorless(&mut engine, 3);
    apply_ability(&mut engine, 0, key, 0, target_object(target)).unwrap();
    let batch = pay_for_copy(&mut engine);
    assert!(find_resolution_choice(&batch).is_some());
    assert_eq!(engine.state.players[0].life, 20);
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("complete copy target");
    assert_eq!(engine.state.players[0].life, 21);
    let copy_pos = batch
        .events
        .iter()
        .position(|event| {
            matches!(&event.ev,
        Some(Ev::StackPushed(push)) if push.is_copy)
        })
        .expect("copy push");
    let life_pos = batch
        .events
        .iter()
        .position(|event| {
            matches!(&event.ev,
        Some(Ev::LifeChanged(change)) if change.player_id == 0 && change.delta == 1)
        })
        .expect("tail");
    assert!(
        copy_pos < life_pos,
        "copy is placed before the parent's tail runs"
    );
    assert!(engine.state.pending_resolution.is_none());
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].life, 21);
}

const HAND_SOURCE: &str = r#"(
  id: "hand_red_source", name: "Hand Red Source", face_id: "hand_red_source",
  mana_cost: "{R}", types: ["Creature"], power: 1, toughness: 1,
  activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
    source_zone: Hand, costs: [Mana("{1}")],
    effect: [Tap(subject: Chosen((kind: Creature)))],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Target creature", effect_indices: [0])])),
  )],
)"#;
const PROTECTED_CREATURE: &str = r#"(
  id: "red_protected_fixture", name: "Red Protected Fixture", face_id: "red_protected_fixture",
  mana_cost: "{W}", types: ["Artifact", "Creature"], power: 1, toughness: 1,
  protections: [Color(Red)],
)"#;

#[test]
fn edge_hand_source_current_or_discarded_respects_protection_when_retargeting() {
    use tricerules_proto::ruled::v1::AbilitySourceZone;
    for discarded_as_cost in [false, true] {
        let hand_source = if discarded_as_cost {
            HAND_SOURCE.replace(
                "costs: [Mana(\"{1}\")]",
                "costs: [Mana(\"{1}\"), DiscardSelf]",
            )
        } else {
            HAND_SOURCE.to_string()
        };
        let registry = Box::leak(Box::new(
            tricerules_cards::CardRegistry::from_chunks_and_tokens(
                &[
                    &hand_source,
                    PROTECTED_CREATURE,
                    include_str!("../../../tricerules-cards/data/rings_of_brighthearth.ron"),
                    include_str!("../../../tricerules-cards/data/mountain.ron"),
                    include_str!("../../../tricerules-cards/data/forest.ron"),
                    include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                ],
                &[],
            )
            .expect("hand-source fixture compiles"),
        ));
        let mut engine = GameEngine::new(
            registry,
            261_027_124 + u64::from(discarded_as_cost),
            &[0, 1],
            20,
            Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        inject_permanent_on_battlefield(&mut engine, 0, RINGS);
        let source = inject_card_into_hand(&mut engine, 0, "hand_red_source");
        let legal = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
        let protected = inject_permanent_on_battlefield(&mut engine, 1, "red_protected_fixture");
        give_colorless(&mut engine, 3);
        let mut command = activate_ability_for(&engine, source, 0, target_object(legal));
        if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
            activation.source_zone = AbilitySourceZone::Hand as i32;
        }
        engine
            .apply_scenario_command(0, &command)
            .expect("activate from hand");
        let batch = pay_for_copy(&mut engine);
        let choice = find_resolution_choice(&batch).expect("retarget offer");
        assert!(
            !choice.candidate_object_ids.contains(&protected),
            "protection from red applies to the exact source, whether current or discarded"
        );
        engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .unwrap();
    }
}

#[test]
fn edge_battlefield_source_uses_current_colors_then_its_exact_departure_lki() {
    use tricerules_cards::{Color, ContinuousEffectKind, EffectDuration};
    use tricerules_core::{AffectedScope, ContinuousEffect};
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
    for leave_and_return in [false, true] {
        let registry = Box::leak(Box::new(
            tricerules_cards::CardRegistry::from_chunks_and_tokens(
                &[
                    PROTECTED_CREATURE,
                    include_str!("../../../tricerules-cards/data/rings_of_brighthearth.ron"),
                    include_str!("../../../tricerules-cards/data/mountain.ron"),
                    include_str!("../../../tricerules-cards/data/forest.ron"),
                    include_str!("../../../tricerules-cards/data/sol_ring.ron"),
                    include_str!("../../../tricerules-cards/data/voltaic_key.ron"),
                ],
                &[],
            )
            .expect("source-quality fixture compiles"),
        ));
        let mut engine = GameEngine::new(
            registry,
            261_027_126 + u64::from(leave_and_return),
            &[0, 1],
            20,
            Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        inject_permanent_on_battlefield(&mut engine, 0, RINGS);
        let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
        let original_generation = engine
            .state
            .zone_change_generation
            .get(&key)
            .copied()
            .unwrap_or(0);
        let legal = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let protected = inject_permanent_on_battlefield(&mut engine, 1, "red_protected_fixture");
        give_colorless(&mut engine, 3);
        apply_ability(&mut engine, 0, key, 0, target_object(legal)).unwrap();
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            affected: AffectedScope::Single(key),
            kind: ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: 1,
            trigger_grant_origin: None,
        });
        assert!(engine
            .characteristics(key)
            .unwrap()
            .colors
            .contains(&Color::Red));
        if leave_and_return {
            engine.enable_dev_commands();
            for zone in [DevZone::Graveyard, DevZone::Battlefield] {
                let actor = engine.state.priority_player_id();
                engine
                    .apply_scenario_command(
                        actor,
                        &RuledCommand {
                            cmd: Some(Cmd::DevCommand(DevCommand {
                                target_player_id: 0,
                                dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                                    card_name: "Voltaic Key".into(),
                                    zone: zone as i32,
                                    ready: true,
                                })),
                            })),
                        },
                    )
                    .expect("move Key through the real zone-change funnel");
            }
            assert_eq!(
                engine.state.zone_change_generation[&key],
                original_generation + 2
            );
            assert!(!engine
                .characteristics(key)
                .unwrap()
                .colors
                .contains(&Color::Red));
        }
        let batch = pay_for_copy(&mut engine);
        let choice = find_resolution_choice(&batch).expect("copy retarget offer");
        assert!(
            !choice.candidate_object_ids.contains(&protected),
            "use the current red source or the red source incarnation that left"
        );
        let pushed = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .unwrap();
        let copy = pushed
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
                _ => None,
            })
            .expect("copy is published");
        if leave_and_return {
            assert!(
                copy.ability_source.is_none(),
                "a returned incarnation is not the old source"
            );
        } else {
            let source = copy
                .ability_source
                .as_ref()
                .expect("current source remains public");
            assert_eq!(
                (source.object_id, source.zone_change_generation),
                (key, original_generation)
            );
        }
    }
}

#[test]
fn edge_sacrificed_clue_keeps_public_token_presentation_without_a_source_arrow() {
    let mut engine = game(261_027_130);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let clue = inject_permanent_on_battlefield(&mut engine, 0, "clue");
    let hand_before = engine.state.players[0].hand.len();
    give_colorless(&mut engine, 4);
    let activation =
        apply_ability(&mut engine, 0, clue, 0, vec![]).expect("sacrifice Clue as cost");
    let original = activation
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if !push.is_triggered && !push.is_copy => Some(push),
            _ => None,
        })
        .expect("Clue activation is published");
    assert!(original.ability_source.is_none());
    assert_eq!(
        original.source_token_identity.as_ref().unwrap().name,
        "Clue"
    );
    let batch = pay_for_copy(&mut engine);
    let copy = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
            _ => None,
        })
        .expect("Clue copy is published");
    assert_eq!(copy.ability_controller_player_id, Some(0));
    assert!(copy.ability_source.is_none());
    assert_eq!(copy.source_token_identity.as_ref().unwrap().name, "Clue");
    assert!(copy.card_id.is_empty());
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn edge_two_rings_copy_folios_chosen_x_without_paying_the_activated_cost_again() {
    let mut engine = game(261_027_131);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let folio = inject_permanent_on_battlefield(&mut engine, 0, "folio_of_fancies");
    let before = [
        engine.state.players[0].hand.len(),
        engine.state.players[1].hand.len(),
    ];
    give_colorless(&mut engine, 8);
    let mut activate = activate_ability_for(&engine, folio, 0, vec![]);
    if let Some(Cmd::ActivateAbility(ability)) = activate.cmd.as_mut() {
        ability.x_value = 2;
    }
    engine
        .apply_scenario_command(0, &activate)
        .expect("pay X twice and tap Folio");
    assert!(engine.state.objects[&folio].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 4);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 3);
    for _ in 0..2 {
        pay_for_copy(&mut engine);
        assert!(engine.state.stack.last().unwrap().is_copy);
        assert_eq!(engine.state.stack.last().unwrap().chosen_x, 2);
        pass_both_players(&mut engine);
    }
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), before[0] + 6);
    assert_eq!(engine.state.players[1].hand.len(), before[1] + 6);
    assert!(engine.state.objects[&folio].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn edge_unaffordable_rings_payment_cannot_copy_and_can_be_declined() {
    let mut engine = game(261_027_132);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
    let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    tap(&mut engine, target);
    give_colorless(&mut engine, 1);
    apply_ability(&mut engine, 0, key, 0, target_object(target)).unwrap();
    let offered = pass_both_players_with_batch(&mut engine);
    assert_eq!(
        find_resolution_choice(&offered).unwrap().choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("attempt to pay for the optional branch");
    assert!(
        submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana).is_err()
    );
    assert!(
        engine.state.pending_resolution.is_some(),
        "failed payment leaves the choice available"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::Decline)
        .expect("decline payment");
    assert_eq!(engine.state.stack.len(), 1, "no copy was created");
    pass_both_players(&mut engine);
    assert!(!engine.state.objects[&target].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn edge_daretti_copy_and_original_make_independent_discard_choices() {
    let mut engine = game(261_027_133);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let daretti = inject_permanent_on_battlefield(&mut engine, 0, "daretti,_scrap_savant");
    engine
        .state
        .objects
        .get_mut(&daretti)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    let first = engine.state.players[0].hand[0];
    let second = engine.state.players[0].hand[1];
    let library_before = engine.state.players[0].library.len();
    give_colorless(&mut engine, 2);
    apply_ability(&mut engine, 0, daretti, 0, vec![]).unwrap();
    pay_for_copy(&mut engine);
    let copied_choice = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&copied_choice)
        .unwrap()
        .candidate_object_ids
        .contains(&first));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![first]))
        .expect("choose one card for the copy");
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    let original_choice = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&original_choice)
        .unwrap()
        .candidate_object_ids
        .contains(&second));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![second]))
        .expect("choose a different card for the original");
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert_eq!(
        engine.state.objects[&daretti].counter_count(CounterKind::Loyalty),
        5
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn edge_a_retargeted_ability_copy_causes_ward_and_can_be_countered() {
    let mut engine = game(261_027_136);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
    let original = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let warded = inject_permanent_on_battlefield(&mut engine, 1, "dusk_rose_reliquary");
    give_colorless(&mut engine, 3);
    apply_ability(&mut engine, 0, key, 0, target_object(original)).unwrap();
    let batch = pay_for_copy(&mut engine);
    assert!(find_resolution_choice(&batch)
        .unwrap()
        .candidate_object_ids
        .contains(&warded));
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![warded]))
        .expect("retarget the copy to the opposing warded artifact");
    let copy = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push.object_id),
            _ => None,
        })
        .expect("ability copy is published");
    let ward = engine.state.stack.last().expect("Ward trigger is on top");
    assert!(ward.is_triggered);
    assert_eq!(ward.source_permanent_id, Some(warded));
    let offer = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&offer).is_some());
    let countered = engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Ward's payment");
    assert!(countered.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackObjectCountered(item)) if item.object_id == copy)));
    assert!(!engine.state.stack.iter().any(|item| item.id == copy));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the original activation remains"
    );
}

#[test]
fn edge_rings_copy_replays_accepted_payment_and_retarget_commands() {
    use prost::Message;
    use tricerules_proto::ruled::v1::{CostObjectRef, PaymentMana, PaymentSelection};

    fn fixture() -> (GameEngine, u32, u32, u32) {
        let mut engine = game(261_027_137);
        inject_permanent_on_battlefield(&mut engine, 0, RINGS);
        let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
        let original = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let retarget = inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
        tap(&mut engine, original);
        tap(&mut engine, retarget);
        give_colorless(&mut engine, 3);
        (engine, key, original, retarget)
    }
    fn record(
        engine: &mut GameEngine,
        log: &mut Vec<(i32, Vec<u8>, Vec<u8>)>,
        actor: i32,
        mut command: RuledCommand,
    ) -> RuledEventBatch {
        if let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() {
            answer.resolution_prompt_id = engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .resolution_prompt_id
                .get();
        }
        let encoded = command.encode_to_vec();
        let batch = engine
            .apply_command(actor, &command)
            .expect("accepted command");
        log.push((actor, encoded, batch.encode_to_vec()));
        batch
    }

    let (mut engine, key, original, retarget) = fixture();
    let (mut replay, _, _, _) = fixture();
    let mut log = Vec::new();
    let activate = activate_ability_for(&engine, key, 0, target_object(original));
    record(&mut engine, &mut log, 0, activate);
    record(&mut engine, &mut log, 0, pass());
    record(&mut engine, &mut log, 1, pass());
    record(
        &mut engine,
        &mut log,
        0,
        submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
    );
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let source = pending.presentation.source_object_id;
    let mut payment = submit_resolution_decision(ResolutionChoiceDecision::PayMana);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = payment.cmd.as_mut() {
        answer.payment = Some(PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(CostObjectRef {
                object_id: source,
                zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&source)
                    .copied()
                    .unwrap_or(0),
            }),
            mana: Some(PaymentMana {
                c: 2,
                ..Default::default()
            }),
            ..Default::default()
        });
    }
    record(&mut engine, &mut log, 0, payment);
    record(
        &mut engine,
        &mut log,
        0,
        submit_resolution_choice(vec![retarget]),
    );
    for _ in 0..2 {
        record(&mut engine, &mut log, 0, pass());
        record(&mut engine, &mut log, 1, pass());
    }
    for (actor, bytes, expected) in log {
        let command = RuledCommand::decode(bytes.as_slice()).expect("decode logged command");
        let actual = replay
            .apply_command(actor, &command)
            .expect("replay accepted command");
        assert_eq!(actual.encode_to_vec(), expected);
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    assert!(!engine.state.objects[&retarget].tapped);
    assert!(!engine.state.objects[&original].tapped);
}

const PAID_CARD_FIXTURE: &str = r#"(
  id: "copied_payment_fixture", name: "Copied Payment Fixture", face_id: "copied_payment_fixture",
  types: ["Artifact"],
  activated_abilities: [(ability_id: "activated_01", presentation: Fallback, costs: [Discard],
    effect: [Draw(count: Count(CardsMatchingResult(filter: (source: Payment, action: Discard, players: Controller))))],
  )],
)"#;

#[test]
fn edge_copied_ability_retains_its_original_paid_card_receipt() {
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                PAID_CARD_FIXTURE,
                include_str!("../../../tricerules-cards/data/rings_of_brighthearth.ron"),
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("paid-card receipt fixture compiles"),
    ));
    let mut engine = GameEngine::new(
        registry,
        261_027_134,
        &[0, 1],
        20,
        Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "copied_payment_fixture");
    let before = engine.state.players[0].hand.len();
    let discarded = engine.state.players[0].hand[0];
    give_colorless(&mut engine, 2);
    engine
        .apply_scenario_command(
            0,
            &activate_ability_with_costs(source, 0, vec![], vec![hand_cost_selection(0, 0)]),
        )
        .expect("discard one card to activate");
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    pay_for_copy(&mut engine);
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        before + 1,
        "both abilities draw from the same one-card payment receipt without another discard"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn edge_rings_controller_departure_removes_stacked_and_parked_work() {
    for parked in [false, true] {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            261_027_135 + u64::from(parked),
            &[0, 1, 2],
            20,
            Some(vec![
                deck_with("mountain", &[]),
                deck_with("forest", &[]),
                deck_with("island", &[]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        inject_permanent_on_battlefield(&mut engine, 0, RINGS);
        let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
        let target = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
        give_colorless(&mut engine, 3);
        apply_ability(&mut engine, 0, key, 0, target_object(target)).unwrap();
        if parked {
            pass_priority_round(&mut engine);
            assert!(engine.state.pending_resolution.is_some());
        }
        engine
            .apply_scenario_command(0, &concede())
            .expect("Rings controller concedes");
        assert!(engine.state.players[0].has_lost);
        assert!(engine.state.stack.iter().all(|item| item.controller != 0));
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_trigger_order.is_none());
        assert!(!engine.state.objects.contains_key(&key));
    }
}

const DEPARTURE_TAIL_FIXTURE: &str = r#"(
  id: "copy_departure_tail_fixture", name: "Copy Departure Tail Fixture",
  face_id: "copy_departure_tail_fixture", mana_cost: "{3}", types: ["Artifact"],
  triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
    trigger: WheneverControllerActivatesNonManaAbility,
    program: Some((body: (steps: [
      (id: "choose", kind: Choose(choice: "copy", chooser: Controller, optional: true,
        branches: [(branch_id: "pay", presentation: Fallback, cost: Mana("{2}"),
          body: (steps: [(id: "copy", kind: Perform(CopyTriggeringActivatedAbility))]))],
        otherwise: (),
      )),
      (id: "tail", kind: Perform(LoseLife(amount: Fixed(1), who: EachOpponent))),
    ]))),
  )],
)"#;

fn has_only_captured_activation_receipt(engine: &GameEngine, trigger: u32) -> bool {
    // RON supports the generation-keyed maps that are present after a real zone departure.
    let snapshot = ron::to_string(&engine.state).expect("inspect captured trigger receipt");
    let receipts = snapshot
        .split_once("captured_activated_abilities:")
        .expect("diagnostic state includes captured activation receipts")
        .1;
    receipts.starts_with(&format!("{{{trigger}:"))
}

fn departure_tail_game(owner: usize) -> (GameEngine, u32, u32) {
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                DEPARTURE_TAIL_FIXTURE,
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/sol_ring.ron"),
                include_str!("../../../tricerules-cards/data/voltaic_key.ron"),
            ],
            &[],
        )
        .expect("departure tail fixture compiles"),
    ));
    let mut engine = GameEngine::new(
        registry,
        261_027_141 + owner as u64,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("mountain", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, owner, "copy_departure_tail_fixture");
    if owner != 0 {
        // Synthetic pre-activation control change isolates the source owner departing while the
        // controller of this already-begun trigger remains in the game.
        engine.state.players[owner]
            .battlefield
            .retain(|&id| id != source);
        engine.state.players[0].battlefield.push(source);
        let object = engine.state.objects.get_mut(&source).unwrap();
        object.base_controller = 0;
        object.controller = 0;
    }
    let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
    let target = inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
    tap(&mut engine, target);
    give_colorless(&mut engine, 3);
    apply_ability(&mut engine, 0, key, 0, target_object(target)).expect("activate Key");
    let trigger = engine.state.stack.last().unwrap().id;
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose the copy branch");
    let batch = submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay for the copy");
    assert_eq!(
        find_resolution_choice(&batch).unwrap().choice_kind(),
        ChoiceKind::TargetObjects
    );
    assert_eq!(
        engine.state.players[2].life, 20,
        "the parent tail has not run yet"
    );
    assert!(has_only_captured_activation_receipt(&engine, trigger));
    (engine, trigger, target)
}

#[test]
fn edge_controller_departure_during_copy_target_choice_completes_begun_parent() {
    let (mut engine, trigger, _) = departure_tail_game(0);
    let original = engine.state.stack.first().unwrap().id;
    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("controller concedes during copy target choice");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger))
            .count(),
        1,
        "the begun parent resolves exactly once"
    );
    assert!(
        !departure.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackPushed(push)) if push.is_copy)),
        "no copy belongs to the departed controller"
    );
    let removed = departure
        .events
        .iter()
        .position(|event| {
            matches!(&event.ev,
        Some(Ev::StackObjectRemoved(removed)) if removed.object_id == original)
        })
        .expect("departed original is removed");
    let tail = departure
        .events
        .iter()
        .position(|event| {
            matches!(&event.ev,
        Some(Ev::LifeChanged(change)) if change.player_id == 2 && change.delta == -1)
        })
        .expect("surviving player's tail");
    assert!(
        removed < tail,
        "the parent tail runs after departed objects are removed"
    );
    assert!(!has_only_captured_activation_receipt(&engine, trigger));
}

#[test]
fn edge_source_owner_departure_during_copy_target_choice_keeps_live_controller() {
    let (mut engine, trigger, target) = departure_tail_game(1);
    let departure = engine
        .apply_scenario_command(1, &concede())
        .expect("the source owner concedes while its controller remains");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the live controller retains the choice");
    assert_eq!(pending.deciding_player, 0);
    let completed = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("live controller keeps the copy target");
    assert!(completed.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackPushed(push)) if push.is_copy && push.ability_controller_player_id == Some(0))));
    assert_eq!(
        engine.state.players[2].life, 19,
        "the synthetic parent tail finishes once"
    );
    assert_eq!(
        departure
            .events
            .iter()
            .chain(completed.events.iter())
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger))
            .count(),
        1
    );
    assert!(!has_only_captured_activation_receipt(&engine, trigger));
    assert!(engine.state.objects[&target].tapped);
    pass_priority_round(&mut engine);
    assert!(
        !engine.state.objects[&target].tapped,
        "the copy resolves for the live controller"
    );
}

#[test]
fn edge_rings_paid_copy_parent_exits_when_controller_departs() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        261_027_143,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("mountain", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
    let target = inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
    give_colorless(&mut engine, 3);
    apply_ability(&mut engine, 0, key, 0, target_object(target)).unwrap();
    let trigger = engine.state.stack.last().unwrap().id;
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose Rings payment");
    let batch = submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay Rings before choosing a copy target");
    assert_eq!(
        find_resolution_choice(&batch).unwrap().choice_kind(),
        ChoiceKind::TargetObjects
    );
    assert!(has_only_captured_activation_receipt(&engine, trigger));

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("controller concedes");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger))
            .count(),
        1
    );
    assert!(!departure.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackPushed(push)) if push.is_copy)));
    assert!(!has_only_captured_activation_receipt(&engine, trigger));
    assert!(engine.state.winner().is_none());
}

#[test]
fn edge_departure_reopens_invalidated_changed_copy_target_prefix() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        261_027_144,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("mountain", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, RINGS);
    let welder = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let original = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let incoming = inject_graveyard_card(&mut engine, 0, "mind_stone");
    let changed = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    give_colorless(&mut engine, 2);
    apply_ability(
        &mut engine,
        0,
        welder,
        0,
        vec![
            TargetRef {
                kind: TargetRefKind::Permanent as i32,
                object_id: original,
                group_index: 0,
                ..Default::default()
            },
            TargetRef {
                kind: TargetRefKind::Graveyard as i32,
                object_id: incoming,
                group_index: 1,
                ..Default::default()
            },
        ],
    )
    .expect("activate Welder");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .unwrap();
    let batch =
        submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana).unwrap();
    assert!(find_resolution_choice(&batch)
        .unwrap()
        .candidate_object_ids
        .contains(&changed));
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![changed]))
        .expect("choose another player's artifact as the first copy target");
    assert!(find_resolution_choice(&batch)
        .unwrap()
        .candidate_object_ids
        .contains(&incoming));

    let departure = engine
        .apply_scenario_command(1, &concede())
        .expect("owner of the changed target departs during the second target choice");
    let reopened = find_resolution_choice(&departure).expect("invalid changed prefix is reoffered");
    assert!(
        reopened.candidate_object_ids.contains(&original),
        "the first occurrence is reopened"
    );
    assert!(!reopened.candidate_object_ids.contains(&changed));
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("retain the original first target");
    assert!(find_resolution_choice(&batch)
        .unwrap()
        .candidate_object_ids
        .contains(&incoming));
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("retain the original second target");
    let copy = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
            _ => None,
        })
        .expect("the refreshed choices place the copy");
    assert_eq!(copy.targets[0].object_id, original);
    assert_eq!(copy.targets[1].object_id, incoming);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&original].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
}

fn copy_choice_with_departure_observer(watched_owner: usize) -> (GameEngine, u32, u32, u32) {
    use tricerules_core::state::{
        ActiveEventObserver, EventObserverMatcher, EventObserverPayload, TriggerObjectRef,
    };
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                DEPARTURE_TAIL_FIXTURE,
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/plains.ron"),
                include_str!("../../../tricerules-cards/data/sol_ring.ron"),
                include_str!("../../../tricerules-cards/data/voltaic_key.ron"),
                include_str!("../../../tricerules-cards/data/banishing_light.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/pacifism.ron"),
            ],
            &[],
        )
        .expect("observer suspension fixture compiles"),
    ));
    let mut engine = GameEngine::new(
        registry,
        261_027_145 + watched_owner as u64,
        &[0, 1, 2, 3],
        20,
        Some(vec![
            deck_with("mountain", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
            deck_with("plains", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let watched = inject_permanent_on_battlefield(&mut engine, watched_owner, "banishing_light");
    let recipient = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let aura = inject_card_into_hand(&mut engine, 2, "pacifism");
    // Synthetic observer/exile setup isolates the existing immediate-return suspension boundary;
    // the player departures, Aura attachment and Program completion use real engine commands.
    engine.state.players[2].hand.retain(|&id| id != aura);
    engine.state.players[2].exile.push(aura);
    engine.state.objects.get_mut(&aura).unwrap().zone = Zone::Exile;
    engine
        .state
        .active_event_observers
        .push(ActiveEventObserver {
            watched: Some(TriggerObjectRef {
                object_id: watched,
                zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&watched)
                    .copied()
                    .unwrap_or(0),
                controller_at_event: watched_owner as i32,
            }),
            matcher: EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
            payload: EventObserverPayload::ReturnExiledObject {
                exiled: TriggerObjectRef {
                    object_id: aura,
                    zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(&aura)
                        .copied()
                        .unwrap_or(0),
                    controller_at_event: 2,
                },
            },
        });
    inject_permanent_on_battlefield(&mut engine, 0, "copy_departure_tail_fixture");
    let key = inject_permanent_on_battlefield(&mut engine, 0, "voltaic_key");
    let target = inject_permanent_on_battlefield(&mut engine, 3, "sol_ring");
    give_colorless(&mut engine, 3);
    apply_ability(&mut engine, 0, key, 0, target_object(target)).unwrap();
    let trigger = engine.state.stack.last().unwrap().id;
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .unwrap();
    let batch =
        submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana).unwrap();
    assert_eq!(
        find_resolution_choice(&batch).unwrap().choice_kind(),
        ChoiceKind::TargetObjects
    );
    (engine, trigger, recipient, aura)
}

#[test]
fn edge_copy_parent_controller_departs_under_an_observer_return_choice() {
    let (mut engine, trigger, recipient, aura) = copy_choice_with_departure_observer(1);
    let interrupted = engine
        .apply_scenario_command(1, &concede())
        .expect("P1's departure starts its owed Aura return");
    let aura_choice = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        aura_choice.presentation.choice_kind,
        ChoiceKind::AuraPermanent
    );
    assert_eq!(aura_choice.deciding_player, 2);
    let departed = engine
        .apply_scenario_command(0, &concede())
        .expect("the copy controller leaves while the independent return is parked");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        2
    );
    assert_eq!(
        engine.state.players[2].life, 20,
        "the return choice precedes the parent tail"
    );
    let completed = engine
        .apply_scenario_command(2, &submit_resolution_choice(vec![recipient]))
        .expect("finish the independently owed Aura return");
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[2].life, 19);
    assert_eq!(engine.state.players[3].life, 19);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(interrupted.events.iter().chain(departed.events.iter()).chain(completed.events.iter())
        .filter(|event| matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger)).count(), 1);
    assert!(!completed.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackPushed(push)) if push.is_copy)));
    assert!(!has_only_captured_activation_receipt(&engine, trigger));
}

#[test]
fn edge_departing_copy_controller_waits_for_its_owed_observer_return() {
    let (mut engine, trigger, recipient, aura) = copy_choice_with_departure_observer(0);
    let departed = engine
        .apply_scenario_command(0, &concede())
        .expect("the controller's departure starts an independent Aura return");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::AuraPermanent
    );
    assert_eq!(
        engine.state.players[2].life, 20,
        "the parent tail waits for the return"
    );
    let completed = engine
        .apply_scenario_command(2, &submit_resolution_choice(vec![recipient]))
        .expect("finish the independently owed Aura return");
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
    assert_eq!(engine.state.players[3].life, 19);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        departed
            .events
            .iter()
            .chain(completed.events.iter())
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger))
            .count(),
        1
    );
    assert!(!has_only_captured_activation_receipt(&engine, trigger));
}
