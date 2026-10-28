use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::CounterKind;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ActivateAbility, CostObjectRef, PaymentMana, PaymentSelection,
    RuledCommand, SelectedSpellMode, TargetRef, TargetRefKind,
};

const AETHERIC_AMPLIFIER: &str = "aetheric_amplifier";
const P0: i32 = 10;
const P1: i32 = 20;

fn setup(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    registry
        .get(AETHERIC_AMPLIFIER)
        .expect("Aetheric Amplifier must be registered before scenario setup");
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[P0, P1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("two-player Aetheric Amplifier scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_aetheric_amplifier(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, AETHERIC_AMPLIFIER);
    give_mana(
        engine,
        P0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, AETHERIC_AMPLIFIER);
    assert_eq!(engine.state.players[0].hand[slot], source);
    engine
        .apply_scenario_command(P0, &cast_spell(slot, vec![]))
        .expect("pay {3} to cast the actual Aetheric Amplifier definition");
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    source
}

fn source_generation(engine: &GameEngine, object: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object)
        .copied()
        .unwrap_or(0)
}

fn permanent_target(engine: &GameEngine, object: u32) -> TargetRef {
    TargetRef {
        object_id: object,
        group_index: 0,
        kind: TargetRefKind::Permanent as i32,
        expected_zone_change_generation: Some(source_generation(engine, object)),
        ..Default::default()
    }
}

fn modal_activation(
    engine: &GameEngine,
    source: u32,
    mode_index: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    let generation = source_generation(engine, source);
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            ability_index: 1,
            expected_zone_change_generation: generation,
            selected_modes: vec![SelectedSpellMode {
                mode_index,
                targets,
            }],
            payment: Some(PaymentSelection {
                expected_state_revision: engine.state.command_index,
                source: Some(CostObjectRef {
                    object_id: source,
                    zone_change_generation: generation,
                }),
                mana: Some(PaymentMana {
                    c: 4,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        })),
    }
}

fn activate_mana_option(engine: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!("mana option command activates the artifact");
    };
    ability.mana_option_index = option;
    command
}

fn add_permanent_counter(engine: &mut GameEngine, object: u32, kind: CounterKind, count: u32) {
    let timestamp = engine.state.command_index;
    engine
        .state
        .objects
        .get_mut(&object)
        .expect("target permanent")
        .add_counters(kind, count, timestamp);
    engine.initial_response_batch();
}

#[test]
fn aetheric_amplifier_tap_mana_ability_produces_each_color_without_a_stack_item() {
    for option in 0..5 {
        let mut engine = setup(26_102_801 + option as u64);
        let source = cast_aetheric_amplifier(&mut engine);
        engine
            .apply_scenario_command(P0, &activate_mana_option(&engine, source, option))
            .expect("choose one of the actual ability's five colored mana options");

        assert!(engine.state.objects[&source].tapped);
        assert!(engine.state.stack.is_empty());
        let pool = &engine.state.players[0].mana_pool;
        assert_eq!(
            [pool.white, pool.blue, pool.black, pool.red, pool.green],
            std::array::from_fn::<_, 5, _>(|index| u32::from(index == option as usize))
        );
        assert_eq!(pool.colorless, 0);
    }
}

#[test]
fn aetheric_amplifier_permanent_mode_doubles_every_counter_and_accepts_counterless_target() {
    let mut engine = setup(26_102_806);
    let source = cast_aetheric_amplifier(&mut engine);
    let countered = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    add_permanent_counter(&mut engine, countered, CounterKind::Charge, 2);
    add_permanent_counter(&mut engine, countered, CounterKind::PlusOnePlusOne, 1);
    give_mana(
        &mut engine,
        P0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let target_generation = source_generation(&engine, countered);
    engine
        .apply_command(
            P0,
            &modal_activation(
                &engine,
                source,
                0,
                vec![permanent_target(&engine, countered)],
            ),
        )
        .expect("pay {4}, tap, and choose exactly one permanent target");
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.stack.len(), 1);
    let selected = &engine.state.stack[0].chosen_modes[0];
    assert_eq!(selected.mode_id.as_str(), "double_any_permanent");
    assert!(engine.state.stack[0].targets.is_empty());
    assert_eq!(selected.targets.len(), 1);
    assert_eq!(selected.targets[0].object_id, countered);
    assert_eq!(
        selected.targets[0].zone_change_generation,
        Some(target_generation)
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&countered].counter_count(CounterKind::Charge),
        4
    );
    assert_eq!(
        engine.state.objects[&countered].counter_count(CounterKind::PlusOnePlusOne),
        2
    );

    // The synthetic untap is setup only: it lets this one-card scenario exercise the legal
    // zero-counter edge without a second three-mana card cast.
    engine.state.objects.get_mut(&source).unwrap().tapped = false;
    engine.initial_response_batch();
    let counterless = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    assert!(engine.state.objects[&counterless].counters.is_empty());
    give_mana(
        &mut engine,
        P0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            P0,
            &modal_activation(
                &engine,
                source,
                0,
                vec![permanent_target(&engine, counterless)],
            ),
        )
        .expect("a permanent with no counters remains a legal target");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&counterless].counters.is_empty());
}

#[test]
fn aetheric_amplifier_player_mode_doubles_only_its_ability_controller() {
    let mut engine = setup(26_102_807);
    let source = cast_aetheric_amplifier(&mut engine);
    engine.state.players[0]
        .counters
        .insert(CounterKind::Poison, 3);
    engine.state.players[1]
        .counters
        .insert(CounterKind::Poison, 5);
    engine.initial_response_batch();
    give_mana(
        &mut engine,
        P0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );

    engine
        .apply_command(P0, &modal_activation(&engine, source, 1, vec![]))
        .expect("pay {4}, tap, and choose the nontargeted player-counter mode");
    assert!(engine.state.stack[0].chosen_modes[0].targets.is_empty());
    assert_eq!(
        engine.state.stack[0].chosen_modes[0].mode_id.as_str(),
        "double_your_counters"
    );
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Poison),
        Some(&6)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&5)
    );
}

#[test]
fn aetheric_amplifier_rejects_bad_modes_targets_generations_actor_and_revision_before_payment() {
    let mut engine = setup(26_102_808);
    let source = cast_aetheric_amplifier(&mut engine);
    let first = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let second = inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
    give_mana(
        &mut engine,
        P0,
        ManaGift {
            c: 20,
            ..Default::default()
        },
    );

    let valid = modal_activation(&engine, source, 0, vec![permanent_target(&engine, first)]);
    let mut cases: Vec<(&str, i32, RuledCommand)> = Vec::new();

    let mut no_mode = valid.clone();
    if let Some(Cmd::ActivateAbility(ability)) = no_mode.cmd.as_mut() {
        ability.selected_modes.clear();
    }
    cases.push(("no selected mode", P0, no_mode));

    let mut extra_mode = valid.clone();
    if let Some(Cmd::ActivateAbility(ability)) = extra_mode.cmd.as_mut() {
        ability.selected_modes.push(SelectedSpellMode {
            mode_index: 1,
            targets: vec![],
        });
    }
    cases.push(("more than one selected mode", P0, extra_mode));

    let mut invalid_mode_index = valid.clone();
    if let Some(Cmd::ActivateAbility(ability)) = invalid_mode_index.cmd.as_mut() {
        ability.selected_modes[0].mode_index = 2;
    }
    cases.push((
        "mode index outside the actual modal ability",
        P0,
        invalid_mode_index,
    ));

    let mut invalid_ability_index = valid.clone();
    if let Some(Cmd::ActivateAbility(ability)) = invalid_ability_index.cmd.as_mut() {
        ability.ability_index = 9;
    }
    cases.push((
        "activated ability index outside the actual card",
        P0,
        invalid_ability_index,
    ));

    let mut missing_target = valid.clone();
    if let Some(Cmd::ActivateAbility(ability)) = missing_target.cmd.as_mut() {
        ability.selected_modes[0].targets.clear();
    }
    cases.push(("missing required permanent target", P0, missing_target));

    let mut extra_target = valid.clone();
    if let Some(Cmd::ActivateAbility(ability)) = extra_target.cmd.as_mut() {
        ability.selected_modes[0]
            .targets
            .push(permanent_target(&engine, second));
    }
    cases.push(("more than one permanent target", P0, extra_target));

    let mut stale_source = valid.clone();
    if let Some(Cmd::ActivateAbility(ability)) = stale_source.cmd.as_mut() {
        let stale_generation = source_generation(&engine, source).saturating_add(1);
        ability.expected_zone_change_generation = stale_generation;
        ability
            .payment
            .as_mut()
            .and_then(|payment| payment.source.as_mut())
            .expect("modal activation has a physical source receipt")
            .zone_change_generation = stale_generation;
    }
    cases.push(("stale source generation", P0, stale_source));

    let mut stale_target = valid.clone();
    if let Some(Cmd::ActivateAbility(ability)) = stale_target.cmd.as_mut() {
        ability.selected_modes[0].targets[0].expected_zone_change_generation =
            Some(source_generation(&engine, first).saturating_add(1));
    }
    cases.push(("stale target generation", P0, stale_target));

    let mut stale_revision = valid.clone();
    if let Some(Cmd::ActivateAbility(ability)) = stale_revision.cmd.as_mut() {
        ability
            .payment
            .as_mut()
            .expect("modal activation has a payment receipt")
            .expected_state_revision = engine.state.command_index.saturating_sub(1);
    }
    cases.push(("stale payment revision", P0, stale_revision));
    cases.push(("wrong activating player", P1, valid.clone()));

    for (reason, actor, command) in cases {
        let before = format!("{:?}", engine.state);
        assert!(
            engine.apply_scenario_command(actor, &command).is_err(),
            "{reason} must be rejected"
        );
        assert_eq!(
            format!("{:?}", engine.state),
            before,
            "{reason} must leave costs, counters, stack, and revision unpaid"
        );
    }
}

#[test]
fn aetheric_amplifier_sorcery_mode_rejects_a_nonmain_phase_without_paying() {
    let mut engine = setup(26_102_809);
    let source = cast_aetheric_amplifier(&mut engine);
    let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    give_mana(
        &mut engine,
        P0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let command = modal_activation(&engine, source, 0, vec![permanent_target(&engine, target)]);
    engine.state.turn_step = TurnStep::EndStep;

    let before = format!("{:?}", engine.state);
    assert!(engine.apply_scenario_command(P0, &command).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}

#[test]
fn aetheric_amplifier_uses_current_counter_bags_captured_controller_and_replacement() {
    let mut engine = setup(26_102_810);
    let source = cast_aetheric_amplifier(&mut engine);
    let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let _season = inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    add_permanent_counter(&mut engine, target, CounterKind::Quest, 2);
    add_permanent_counter(&mut engine, target, CounterKind::Charge, 1);
    give_mana(
        &mut engine,
        P0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );

    engine
        .apply_command(
            P0,
            &modal_activation(&engine, source, 0, vec![permanent_target(&engine, target)]),
        )
        .expect("activate the printed target-permanent mode");
    let changed = engine
        .state
        .objects
        .get_mut(&target)
        .expect("target permanent");
    changed.counters.insert(CounterKind::Quest, 3);
    changed.counters.insert(CounterKind::Charge, 4);
    changed.counters.insert(CounterKind::PlusOnePlusOne, 2);
    engine.initial_response_batch();
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Quest),
        9
    );
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Charge),
        12
    );
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        6
    );

    let mut player_mode_engine = setup(26_102_811);
    let player_mode_source = cast_aetheric_amplifier(&mut player_mode_engine);
    player_mode_engine.state.players[0]
        .counters
        .insert(CounterKind::Poison, 3);
    player_mode_engine.state.players[1]
        .counters
        .insert(CounterKind::Poison, 5);
    player_mode_engine.initial_response_batch();
    give_mana(
        &mut player_mode_engine,
        P0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    player_mode_engine
        .apply_command(
            P0,
            &modal_activation(&player_mode_engine, player_mode_source, 1, vec![]),
        )
        .expect("activate the printed player-counter mode");

    player_mode_engine.state.players[0]
        .counters
        .insert(CounterKind::Poison, 4);
    player_mode_engine.state.players[1]
        .counters
        .insert(CounterKind::Poison, 7);
    player_mode_engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != player_mode_source);
    player_mode_engine.state.players[1]
        .battlefield
        .push(player_mode_source);
    let changed_source = player_mode_engine
        .state
        .objects
        .get_mut(&player_mode_source)
        .expect("the activated source remains on the battlefield");
    changed_source.base_controller = P1;
    changed_source.controller = P1;
    player_mode_engine.initial_response_batch();
    resolve_entire_stack_two_player(&mut player_mode_engine);

    assert_eq!(
        player_mode_engine.state.players[0]
            .counters
            .get(&CounterKind::Poison),
        Some(&8),
        "the ability keeps its activation controller and reads that player's current counters"
    );
    assert_eq!(
        player_mode_engine.state.players[1]
            .counters
            .get(&CounterKind::Poison),
        Some(&7),
        "the source's new controller does not become the ability's controller"
    );
}

fn cast_rings_of_brighthearth(engine: &mut GameEngine) -> u32 {
    let rings = inject_card_into_hand(engine, 0, "rings_of_brighthearth");
    give_mana(
        engine,
        P0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "rings_of_brighthearth");
    engine
        .apply_scenario_command(P0, &cast_spell(slot, vec![]))
        .expect("cast the actual Rings of Brighthearth definition");
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&rings].zone, Zone::Battlefield);
    rings
}

fn pay_for_rings_copy(engine: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let batch = pass_both_players_with_batch(engine);
    let branch = find_resolution_choice(&batch).expect("Rings offers the copy option");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    let payment = engine
        .apply_scenario_command(
            P0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose to pay for the actual ability copy");
    let offer = find_resolution_choice(&payment).expect("Rings requests its {2} payment");
    assert_eq!(offer.choice_kind(), ChoiceKind::ManaPayment);
    assert_eq!(offer.generic_mana_cost, 2);
    submit_mana_resolution_decision(engine, P0, ResolutionChoiceDecision::PayMana)
        .expect("pay {2} for Rings")
}

#[test]
fn aetheric_amplifier_actual_targeted_mode_is_copied_by_rings_with_a_new_target() {
    let mut engine = setup(26_102_812);
    let source = cast_aetheric_amplifier(&mut engine);
    let _rings = cast_rings_of_brighthearth(&mut engine);
    let original_target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let copy_target = inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
    add_permanent_counter(&mut engine, original_target, CounterKind::Charge, 2);
    add_permanent_counter(&mut engine, copy_target, CounterKind::Charge, 3);
    give_mana(
        &mut engine,
        P0,
        ManaGift {
            c: 6,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(
            P0,
            &modal_activation(
                &engine,
                source,
                0,
                vec![permanent_target(&engine, original_target)],
            ),
        )
        .expect("pay {4} and activate Aetheric Amplifier's targeted mode");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "Rings triggers on the actual activation"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);

    let copy_offer = pay_for_rings_copy(&mut engine);
    let target_offer = find_resolution_choice(&copy_offer).expect("choose the copy's target");
    assert_eq!(target_offer.choice_kind(), ChoiceKind::TargetObjects);
    assert!(target_offer.candidate_object_ids.contains(&copy_target));
    let copy_batch = engine
        .apply_scenario_command(P0, &submit_resolution_choice(vec![copy_target]))
        .expect("retarget the copied Aetheric Amplifier mode");
    let copy_id = copy_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackPushed(push))
                if push.is_copy =>
            {
                Some(push.object_id)
            }
            _ => None,
        })
        .expect("Rings publishes a copy of the actual activated mode");
    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == copy_id)
        .expect("the actual ability copy is on the stack");
    assert_eq!(copy.chosen_modes.len(), 1);
    assert_eq!(
        copy.chosen_modes[0].mode_id.as_str(),
        "double_any_permanent"
    );
    assert_eq!(copy.chosen_modes[0].targets[0].object_id, copy_target);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&copy_target].counter_count(CounterKind::Charge),
        6
    );
    assert_eq!(
        engine.state.objects[&original_target].counter_count(CounterKind::Charge),
        4
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn aetheric_amplifier_activation_and_resolution_replay_from_protobuf_wire() {
    use prost::Message;

    fn fresh() -> (GameEngine, u32, u32) {
        let mut engine = setup(26_102_813);
        let source = cast_aetheric_amplifier(&mut engine);
        let target = inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
        add_permanent_counter(&mut engine, target, CounterKind::Charge, 3);
        give_mana(
            &mut engine,
            P0,
            ManaGift {
                c: 4,
                ..Default::default()
            },
        );
        (engine, source, target)
    }

    let (mut engine, source, target) = fresh();
    let commands = vec![
        (
            P0,
            modal_activation(&engine, source, 0, vec![permanent_target(&engine, target)]),
        ),
        (P0, pass()),
        (P1, pass()),
    ];
    let mut expected_batches = Vec::new();
    for (actor, command) in &commands {
        expected_batches.push(
            engine
                .apply_scenario_command(*actor, command)
                .expect("record accepted Aetheric activation command"),
        );
    }
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Charge),
        6
    );

    let (mut replay, _, _) = fresh();
    for ((actor, command), expected) in commands.into_iter().zip(expected_batches) {
        let encoded = command.encode_to_vec();
        let decoded = RuledCommand::decode(encoded.as_slice()).expect("decode raw wire command");
        assert_eq!(
            replay
                .apply_scenario_command(actor, &decoded)
                .expect("replay accepted raw wire command"),
            expected
        );
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}
