use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::CounterKind;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChooseTriggerTarget, RuledCommand, TargetRef,
};

const INNKEEPERS_TALENT: &str = "innkeepers_talent";
const P0: i32 = 10;
const P1: i32 = 20;

fn setup(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    registry
        .get(INNKEEPERS_TALENT)
        .expect("Innkeeper's Talent must be registered before scenario setup");

    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[P0, P1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("two-player Innkeeper's Talent scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_talent(engine: &mut GameEngine) -> u32 {
    // The hand card and mana are synthetic setup; the card is cast through the engine.
    let source = inject_card_into_hand(engine, 0, INNKEEPERS_TALENT);
    cast_talent_from_hand(engine, source)
}

fn cast_talent_from_hand(engine: &mut GameEngine, source: u32) -> u32 {
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    give_mana(
        engine,
        P0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, INNKEEPERS_TALENT);
    assert_eq!(engine.state.players[0].hand[slot], source);
    engine
        .apply_scenario_command(P0, &cast_spell(slot, vec![]))
        .expect("pay {1}{G} to cast the actual Innkeeper's Talent definition");
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.class_level(source), 1);
    source
}

fn give_priority_to(engine: &mut GameEngine, player: i32) {
    if engine.state.priority_player_id() != player {
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("yield priority to the requested caster");
    }
    assert_eq!(engine.state.priority_player_id(), player);
}

fn cast_disenchant(engine: &mut GameEngine, caster: i32, target: u32) -> u32 {
    let caster_index = engine.state.player_idx(caster).expect("known caster");
    let spell = inject_card_into_hand(engine, caster_index, "disenchant");
    // Disenchant costs {1}{W}; two colorless mana leaves exactly one to pay Ward {1}.
    give_mana(
        engine,
        caster,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    give_priority_to(engine, caster);
    let slot = hand_index_for_card(engine, caster_index, "disenchant");
    engine
        .apply_scenario_command(caster, &cast_spell(slot, vec![target_ref(target)]))
        .expect("cast registered Disenchant at the selected permanent");
    spell
}

fn cast_boomerang(engine: &mut GameEngine, caster: i32, target: u32) -> u32 {
    let caster_index = engine.state.player_idx(caster).expect("known caster");
    let spell = inject_card_into_hand(engine, caster_index, "boomerang");
    give_mana(
        engine,
        caster,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    give_priority_to(engine, caster);
    let slot = hand_index_for_card(engine, caster_index, "boomerang");
    engine
        .apply_scenario_command(caster, &cast_spell(slot, vec![target_ref(target)]))
        .expect("cast registered Boomerang at the selected permanent");
    spell
}

fn target_ref(object_id: u32) -> TargetRef {
    TargetRef {
        expected_zone_change_generation: None,
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: 0,
    }
}

fn add_synthetic_charge_counter(engine: &mut GameEngine, object: u32) {
    // Synthetic setup only: a Charge counter makes the permanent eligible for Talent's grant.
    let timestamp = engine.state.command_index;
    engine
        .state
        .objects
        .get_mut(&object)
        .expect("countered permanent")
        .add_counters(CounterKind::Charge, 1, timestamp);
    engine.initial_response_batch();
}

fn remove_synthetic_charge_counter(engine: &mut GameEngine, object: u32) {
    // Synthetic setup only: remove the eligibility counter after Ward has already triggered.
    engine
        .state
        .objects
        .get_mut(&object)
        .expect("countered permanent")
        .set_counter(CounterKind::Charge, 0);
    engine.initial_response_batch();
}

fn countered_artifact_fixture(seed: u64) -> (GameEngine, u32, u32) {
    let mut engine = setup(seed);
    let talent = cast_talent(&mut engine);
    level_up(
        &mut engine,
        talent,
        ManaGift {
            g: 1,
            ..Default::default()
        },
        2,
    );
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    add_synthetic_charge_counter(&mut engine, artifact);
    (engine, talent, artifact)
}

fn change_control_synthetically(engine: &mut GameEngine, object: u32, from: usize, to: usize) {
    // Synthetic control change after target selection, used to test resolution-time legality.
    engine.state.players[from]
        .battlefield
        .retain(|object_id| *object_id != object);
    engine.state.players[to].battlefield.push(object);
    let new_controller = engine.state.players[to].id;
    let changed = engine
        .state
        .objects
        .get_mut(&object)
        .expect("controlled permanent");
    changed.base_controller = new_controller;
    changed.controller = new_controller;
}

fn class_level_ability_index(engine: &mut GameEngine, class: u32) -> u32 {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .and_then(|view| view.per_player.first())
        .into_iter()
        .flat_map(|player| player.battlefield_objects.iter())
        .find(|object| object.object_id == class)
        .and_then(|object| object.activated_abilities.first())
        .map(|ability| ability.ability_index)
        .expect("the current Class level ability is offered in the zone view")
}

fn level_up(engine: &mut GameEngine, class: u32, mana: ManaGift, expected_level: u32) {
    give_mana(engine, P0, mana);
    assert_eq!(zone_view_ability_flags(engine, 0, class), [true]);
    let ability_index = class_level_ability_index(engine, class);
    apply_ability(engine, P0, class, ability_index, vec![])
        .expect("pay for the currently offered Class level ability");
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    assert_eq!(engine.state.class_level(class), expected_level);
}

fn beginning_combat_target(engine: &mut GameEngine, creature: u32) {
    let batch = engine
        .apply_scenario_command(P0, &primitive_yield())
        .expect("enter beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::BeginCombat);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(batch
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::TriggerNeedsTarget(_)))));
    engine
        .apply_scenario_command(P0, &trigger_target(creature))
        .expect("choose the controlled creature for the combat trigger");
    resolve_entire_stack_two_player(engine);
}

fn trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                expected_zone_change_generation: None,
                object_id,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
            ..Default::default()
        })),
    }
}

#[test]
fn innkeepers_talent_paid_level_one_cast_puts_a_counter_at_its_combat() {
    let mut engine = setup(26_102_711);
    let talent = cast_talent(&mut engine);
    // The recipient is a synthetic battlefield fixture; target choice and resolution use the engine.
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    beginning_combat_target(&mut engine, creature);

    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(engine.state.class_level(talent), 1);
}

#[test]
fn innkeepers_talent_pays_offered_level_two_and_three_then_doubles_combat_counter() {
    let mut engine = setup(26_102_712);
    let talent = cast_talent(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    level_up(
        &mut engine,
        talent,
        ManaGift {
            g: 1,
            ..Default::default()
        },
        2,
    );
    level_up(
        &mut engine,
        talent,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
        3,
    );
    beginning_combat_target(&mut engine, creature);

    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "the level-III static doubles the counter put by the real combat trigger"
    );
    assert_eq!(engine.state.class_level(talent), 3);
}

#[test]
fn innkeepers_talent_level_two_ward_covers_an_artifact_and_requires_exactly_one_mana() {
    for (seed, pay_ward) in [(26_102_721, false), (26_102_722, true)] {
        let (mut engine, talent, artifact) = countered_artifact_fixture(seed);
        let spell = cast_disenchant(&mut engine, P1, artifact);
        assert_eq!(engine.state.objects[&artifact].card_id, "sol_ring");
        assert_eq!(
            engine.state.objects[&artifact].counter_count(CounterKind::Charge),
            1
        );
        assert_eq!(engine.state.class_level(talent), 2);
        assert_eq!(engine.state.stack.len(), 2, "Ward is above Disenchant");
        assert!(
            engine
                .state
                .stack
                .last()
                .expect("Ward trigger")
                .is_triggered
        );

        pass_both_players(&mut engine);
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Talent Ward asks the spell's controller to pay");
        assert_eq!(pending.deciding_player, P1);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::ManaPayment);
        let payment = pending.continuation.mana_payment().expect("Ward payment");
        assert_eq!(payment.generic_mana_cost, 1);
        assert!(payment.mana_cost.is_empty());

        if pay_ward {
            let payer_index = engine.state.player_idx(P1).expect("spell controller");
            assert_eq!(engine.state.players[payer_index].mana_pool.colorless, 1);
            submit_mana_resolution_decision(
                &mut engine,
                P1,
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::PayMana,
            )
            .expect("pay exactly one remaining mana for Ward");
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.iter().any(|item| item.id == spell));
            let payer_index = engine.state.player_idx(P1).expect("spell controller");
            assert_eq!(engine.state.players[payer_index].mana_pool.colorless, 0);

            pass_both_players(&mut engine);
            assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
            assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        } else {
            let batch = engine
                .apply_scenario_command(
                    P1,
                    &submit_resolution_decision(
                        tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
                    ),
                )
                .expect("decline Talent Ward");
            assert!(batch.events.iter().any(|event| matches!(
                &event.ev,
                Some(Ev::StackObjectCountered(countered)) if countered.object_id == spell
            )));
            assert!(!engine.state.stack.iter().any(|item| item.id == spell));
            assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
            assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
            assert!(engine.state.pending_resolution.is_none());
        }
    }
}

#[test]
fn innkeepers_talent_ward_tracks_level_physical_counters_and_control() {
    #[derive(Clone, Copy)]
    enum CounterSetup {
        None,
        BeforeLevel,
        AfterLevelTwo,
        RemoveAfterGrant,
    }

    let cases = [
        ("level I", 1, 0, CounterSetup::BeforeLevel, P1, false),
        ("counterless level II", 2, 0, CounterSetup::None, P1, false),
        (
            "counter added after level II",
            2,
            0,
            CounterSetup::AfterLevelTwo,
            P1,
            true,
        ),
        (
            "last counter removed",
            2,
            0,
            CounterSetup::RemoveAfterGrant,
            P1,
            false,
        ),
        (
            "opponent countered permanent excluded",
            2,
            1,
            CounterSetup::BeforeLevel,
            P0,
            false,
        ),
        (
            "Talent controller targets own countered permanent",
            2,
            0,
            CounterSetup::BeforeLevel,
            P0,
            false,
        ),
    ];

    for (offset, (label, level, target_player, counter_setup, caster, expect_ward)) in
        cases.into_iter().enumerate()
    {
        let mut engine = setup(26_102_730 + offset as u64);
        let talent = cast_talent(&mut engine);
        let artifact = inject_permanent_on_battlefield(&mut engine, target_player, "sol_ring");
        if matches!(
            counter_setup,
            CounterSetup::BeforeLevel | CounterSetup::RemoveAfterGrant
        ) {
            add_synthetic_charge_counter(&mut engine, artifact);
        }
        if level >= 2 {
            level_up(
                &mut engine,
                talent,
                ManaGift {
                    g: 1,
                    ..Default::default()
                },
                2,
            );
        }
        if matches!(counter_setup, CounterSetup::AfterLevelTwo) {
            add_synthetic_charge_counter(&mut engine, artifact);
        }
        if matches!(counter_setup, CounterSetup::RemoveAfterGrant) {
            remove_synthetic_charge_counter(&mut engine, artifact);
        }

        cast_disenchant(&mut engine, caster, artifact);
        let ward_count = engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_triggered)
            .count();
        let expected_ward_count = if expect_ward { 1 } else { 0 };
        assert_eq!(ward_count, expected_ward_count, "{label}");
        assert_eq!(
            engine.state.stack.len(),
            1 + expected_ward_count,
            "{label}: exactly the targeted spell and optional Ward are on the stack"
        );
    }

    for (offset, level) in [2, 3].into_iter().enumerate() {
        let mut engine = setup(26_102_740 + offset as u64);
        let talent = cast_talent(&mut engine);
        level_up(
            &mut engine,
            talent,
            ManaGift {
                g: 1,
                ..Default::default()
            },
            2,
        );
        if level == 3 {
            level_up(
                &mut engine,
                talent,
                ManaGift {
                    g: 1,
                    c: 3,
                    ..Default::default()
                },
                3,
            );
        }
        assert!(engine.state.objects[&talent].counters.is_empty());
        cast_disenchant(&mut engine, P1, talent);
        assert_eq!(
            engine.state.stack.len(),
            1,
            "counterless Talent at level {level} does not grant itself Ward"
        );
    }
}

#[test]
fn innkeepers_talent_ward_trigger_survives_counter_loss_or_talent_bounce() {
    for (offset, bounce_talent) in [(0, false), (1, true)] {
        let (mut engine, talent, artifact) = countered_artifact_fixture(26_102_750 + offset);
        let spell = cast_disenchant(&mut engine, P1, artifact);
        assert_eq!(engine.state.stack.len(), 2);

        if bounce_talent {
            cast_boomerang(&mut engine, P0, talent);
            pass_both_players(&mut engine);
            assert_eq!(engine.state.objects[&talent].zone, Zone::Hand);
            assert_eq!(engine.state.class_level(talent), 1);
        } else {
            remove_synthetic_charge_counter(&mut engine, artifact);
            assert_eq!(
                engine.state.objects[&artifact].counter_count(CounterKind::Charge),
                0
            );
        }

        pass_both_players(&mut engine);
        assert!(engine.state.pending_resolution.is_some());
        let batch = engine
            .apply_scenario_command(
                P1,
                &submit_resolution_decision(
                    tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
                ),
            )
            .expect("decline the already-triggered Ward after its grant is lost");
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(Ev::StackObjectCountered(countered)) if countered.object_id == spell
                ))
                .count(),
            1,
            "the original targeted spell is countered exactly once"
        );
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
        assert!(engine.state.pending_resolution.is_none());
        assert!(!engine.state.stack.iter().any(|item| item.id == spell));
    }
}

#[test]
fn innkeepers_talent_class_actions_and_combat_target_legality_are_bounded() {
    let mut engine = setup(26_102_760);
    let talent = cast_talent(&mut engine);
    give_mana(
        &mut engine,
        P0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let level_two = class_level_ability_index(&mut engine, talent);
    let before_wrong_actor = format!("{:?}", engine.state);
    let error = apply_ability(&mut engine, P1, talent, level_two, vec![])
        .expect_err("only the Class controller may activate its level ability");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(format!("{:?}", engine.state), before_wrong_actor);

    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    beginning_combat_target(&mut engine, creature);
    assert_eq!(engine.state.turn_step, TurnStep::BeginCombat);
    let before_wrong_timing = format!("{:?}", engine.state);
    let error = apply_ability(&mut engine, P0, talent, level_two, vec![])
        .expect_err("the Class level ability is sorcery-speed");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(format!("{:?}", engine.state), before_wrong_timing);

    let mut engine = setup(26_102_761);
    let _talent = cast_talent(&mut engine);
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .apply_scenario_command(P0, &primitive_yield())
        .expect("enter the Talent controller's beginning of combat");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let before_illegal_target = format!("{:?}", engine.state);
    let error = engine
        .apply_scenario_command(P0, &trigger_target(opponent_creature))
        .expect_err(
            "the combat trigger cannot target a creature not controlled by Talent's controller",
        );
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(format!("{:?}", engine.state), before_illegal_target);

    engine
        .apply_scenario_command(P0, &trigger_target(own_creature))
        .expect("choose the controlled creature before it changes control");
    change_control_synthetically(&mut engine, own_creature, 0, 1);
    engine.initial_response_batch();
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&own_creature].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "the chosen creature is no longer controlled by Talent's controller when the trigger resolves"
    );

    let mut opponent_combat = setup(26_102_762);
    let _talent = cast_talent(&mut opponent_combat);
    // Synthetic turn positioning isolates the opponent's beginning-of-combat transition; the
    // transition itself is still generated by the engine through its ordinary phase command.
    opponent_combat.state.active_player_idx = 1;
    opponent_combat.state.priority_idx = 1;
    opponent_combat.state.turn_step = TurnStep::Main1;
    opponent_combat.state.passes_since_stack_change = 0;
    opponent_combat.initial_response_batch();
    opponent_combat
        .apply_scenario_command(P1, &primitive_yield())
        .expect("enter the opponent's beginning of combat");
    assert_eq!(opponent_combat.state.turn_step, TurnStep::BeginCombat);
    assert!(opponent_combat.state.pending_triggers.is_empty());
    assert!(opponent_combat.state.stack.is_empty());
}

#[test]
fn innkeepers_talent_bounce_and_recast_resets_level_three_counter_doubling() {
    let mut engine = setup(26_102_770);
    let talent = cast_talent(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    level_up(
        &mut engine,
        talent,
        ManaGift {
            g: 1,
            ..Default::default()
        },
        2,
    );
    level_up(
        &mut engine,
        talent,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
        3,
    );
    let prior_generation = engine
        .state
        .zone_change_generation
        .get(&talent)
        .copied()
        .unwrap_or(0);

    cast_boomerang(&mut engine, P0, talent);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&talent].zone, Zone::Hand);
    assert_eq!(engine.state.class_level(talent), 1);
    let bounced_generation = engine
        .state
        .zone_change_generation
        .get(&talent)
        .copied()
        .unwrap_or(0);
    assert!(bounced_generation > prior_generation);

    cast_talent_from_hand(&mut engine, talent);
    assert_eq!(engine.state.objects[&talent].zone, Zone::Battlefield);
    assert_eq!(engine.state.class_level(talent), 1);
    assert!(
        engine
            .state
            .zone_change_generation
            .get(&talent)
            .copied()
            .unwrap_or(0)
            > bounced_generation
    );

    beginning_combat_target(&mut engine, creature);
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "recasting the physical Class starts at level I, so level-III doubling is off"
    );
}

#[test]
fn innkeepers_talent_ward_choice_replays_from_its_exact_wire_prompt() {
    use prost::Message;
    use tricerules_proto::ruled::v1::{PaymentMana, PreviewPayment};

    let (mut original, talent, artifact) = countered_artifact_fixture(26_102_780);
    let spell = cast_disenchant(&mut original, P1, artifact);
    pass_both_players(&mut original);
    let prompt_id = original
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward payment prompt")
        .resolution_prompt_id
        .get();
    assert_ne!(prompt_id, 0, "the accepted Ward offer has a real prompt ID");

    let (mut replay, replay_talent, replay_artifact) = countered_artifact_fixture(26_102_780);
    assert_eq!((talent, artifact), (replay_talent, replay_artifact));
    let replay_spell = cast_disenchant(&mut replay, P1, replay_artifact);
    pass_both_players(&mut replay);
    let replay_prompt_id = replay
        .state
        .pending_resolution
        .as_ref()
        .expect("replay Ward payment prompt")
        .resolution_prompt_id
        .get();
    assert_eq!(prompt_id, replay_prompt_id);
    assert_eq!(spell, replay_spell);

    let mut accepted =
        submit_resolution_decision(tricerules_proto::ruled::v1::ResolutionChoiceDecision::PayMana);
    let Some(Cmd::SubmitResolutionChoice(answer)) = accepted.cmd.as_mut() else {
        unreachable!()
    };
    // Record the engine-offered identity before encoding; do not let a scenario adapter fill it.
    answer.resolution_prompt_id = prompt_id;

    let offer_request = PreviewPayment {
        transaction_id: 1,
        revision: 1,
        resolution_choice: Some(answer.clone()),
        ..Default::default()
    };
    let original_offer = original.preview_payment(P1, &offer_request);
    let replay_offer = replay.preview_payment(P1, &offer_request);
    assert!(original_offer.valid, "{}", original_offer.error);
    assert!(replay_offer.valid, "{}", replay_offer.error);
    assert_eq!(original_offer.selection, replay_offer.selection);
    let mut selection = original_offer
        .selection
        .expect("raw Ward payment preview returns a selection");
    let payer_index = original.state.player_idx(P1).expect("spell controller");
    let replay_payer_index = replay
        .state
        .player_idx(P1)
        .expect("replay spell controller");
    let original_pool_before = original.state.players[payer_index].mana_pool;
    let replay_pool_before = replay.state.players[replay_payer_index].mana_pool;
    assert_eq!(
        original_pool_before,
        tricerules_core::state::ManaPool {
            colorless: 1,
            ..Default::default()
        }
    );
    assert_eq!(replay_pool_before, original_pool_before);
    selection.mana = Some(PaymentMana {
        c: 1,
        ..Default::default()
    });
    answer.payment = Some(selection);

    let paid_request = PreviewPayment {
        transaction_id: 1,
        revision: 2,
        resolution_choice: Some(answer.clone()),
        ..Default::default()
    };
    let original_paid = original.preview_payment(P1, &paid_request);
    let replay_paid = replay.preview_payment(P1, &paid_request);
    assert!(original_paid.valid, "{}", original_paid.error);
    assert!(original_paid.complete, "{}", original_paid.error);
    assert!(replay_paid.valid, "{}", replay_paid.error);
    assert!(replay_paid.complete, "{}", replay_paid.error);
    assert_eq!(original_paid.selection, replay_paid.selection);

    let decoded = RuledCommand::decode(accepted.encode_to_vec().as_slice())
        .expect("accepted Ward command decodes from protobuf");
    assert_eq!(decoded, accepted);

    let original_batch = original
        .apply_command(P1, &accepted)
        .expect("apply the correlated Ward payment directly");
    let replay_batch = replay
        .apply_command(P1, &decoded)
        .expect("replay the same correlated Ward payment directly");
    assert_eq!(original_batch, replay_batch);
    assert_eq!(
        original.diagnostic_snapshot().expect("original snapshot"),
        replay.diagnostic_snapshot().expect("replay snapshot")
    );
    assert!(original.state.pending_resolution.is_none());
    assert!(original.state.stack.iter().any(|item| item.id == spell));
    assert_eq!(original.state.class_level(talent), 2);
    let original_payer = original.state.player_idx(P1).expect("spell controller");
    let replay_payer = replay
        .state
        .player_idx(P1)
        .expect("replay spell controller");
    assert_eq!(
        original.state.players[original_payer].mana_pool,
        Default::default(),
        "the exact one colorless mana left from Disenchant was spent on Ward"
    );
    assert_eq!(
        replay.state.players[replay_payer].mana_pool,
        Default::default()
    );

    resolve_entire_stack_two_player(&mut original);
    resolve_entire_stack_two_player(&mut replay);
    assert_eq!(
        original
            .diagnostic_snapshot()
            .expect("resolved original snapshot"),
        replay
            .diagnostic_snapshot()
            .expect("resolved replay snapshot")
    );
    assert_eq!(original.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(original.state.objects[&spell].zone, Zone::Graveyard);
}
