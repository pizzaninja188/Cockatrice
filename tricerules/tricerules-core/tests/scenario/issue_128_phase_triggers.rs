use crate::helpers::*;
use tricerules_cards::primitives::{
    ConditionObjectRef, ContinuousEffectKind, EffectDuration, GameCondition,
};
use tricerules_cards::{Amount, SpellEffectKind, TriggeredAbilityDef};
use tricerules_cards::{CardRegistry, CastTriggerPlayer, CounterKind, Keyword, TriggerCondition};
use tricerules_core::state::PlayerState;
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, PhaseId, RuledCommand, TargetRef,
};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        })),
    }
}

fn engine_at_main1(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn advance_from_main1_to_main2(engine: &mut GameEngine, active: i32) -> RuledEventBatch {
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 1 to beginning of combat");
    engine
        .apply_command(active, &primitive_yield())
        .expect("beginning of combat advance");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_command(active, &primitive_yield())
            .expect("declare no attackers");
    }
    engine
        .apply_command(active, &primitive_yield())
        .expect("end combat to second main")
}

fn finish_turn_from_begin_combat(engine: &mut GameEngine, active: i32) {
    engine
        .apply_command(active, &primitive_yield())
        .expect("beginning of combat advance");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_command(active, &primitive_yield())
            .expect("declare no attackers");
    }
    engine
        .apply_command(active, &primitive_yield())
        .expect("end combat to second main");
    engine
        .apply_command(active, &primitive_yield())
        .expect("second main to end step");
    engine
        .apply_command(active, &primitive_yield())
        .expect("end step to cleanup");
    resolve_cleanup_discards_if_any(engine);
}

fn refire_second_main(engine: &mut GameEngine, active: usize) -> RuledEventBatch {
    engine.state.turn_step = TurnStep::EndCombat;
    engine.state.active_player_idx = active;
    engine.state.priority_idx = active;
    engine.state.passes_since_stack_change = 0;
    engine
        .apply_command(engine.state.players[active].id, &primitive_yield())
        .expect("enter another modeled second main phase")
}

#[test]
fn riling_dawnbreaker_targets_before_priority_and_pumps_only_another_controlled_creature() {
    let mut engine = engine_at_main1(128_001);
    let riling =
        inject_creature_on_battlefield(&mut engine, 0, "riling_dawnbreaker_signaling_roar");
    let friendly = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    let batch = engine
        .apply_command(0, &primitive_yield())
        .expect("enter beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::BeginCombat);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(engine.state.stack.is_empty());
    assert!(
        priority_changes_in(&batch).is_empty(),
        "target choice withholds priority"
    );

    let prompt = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::TriggerNeedsTarget(prompt)) => Some(prompt),
            _ => None,
        })
        .expect("Riling target prompt");
    assert_eq!(
        prompt.targets.as_ref().expect("target schema").groups.len(),
        1
    );
    assert_eq!(
        prompt.targets.as_ref().unwrap().groups[0].valid_permanent_ids,
        [friendly],
        "only another creature controlled by the ability controller is legal"
    );

    for illegal in [riling, opposing] {
        assert!(engine
            .apply_command(0, &choose_trigger_target(illegal))
            .is_err());
        assert_eq!(engine.state.pending_triggers.len(), 1);
    }
    engine
        .apply_command(0, &choose_trigger_target(friendly))
        .expect("choose friendly creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(friendly), Some(3));
    assert_eq!(engine.effective_power(opposing), Some(2));

    finish_turn_from_begin_combat(&mut engine, 0);
    assert_eq!(
        engine.effective_power(friendly),
        Some(2),
        "pump expires at cleanup"
    );
}

#[test]
fn riling_dawnbreaker_is_controller_turn_only_and_does_not_retroactively_observe_combat() {
    let mut opponent_turn = engine_at_main1(128_002);
    inject_creature_on_battlefield(&mut opponent_turn, 0, "riling_dawnbreaker_signaling_roar");
    inject_creature_on_battlefield(&mut opponent_turn, 0, "grizzly_bears");
    opponent_turn.state.active_player_idx = 1;
    opponent_turn.state.priority_idx = 1;
    opponent_turn
        .apply_command(1, &primitive_yield())
        .expect("opponent enters combat");
    assert!(opponent_turn.state.pending_triggers.is_empty());
    assert!(opponent_turn.state.stack.is_empty());

    let mut late_source = engine_at_main1(128_003);
    late_source
        .apply_command(0, &primitive_yield())
        .expect("enter combat without Riling");
    inject_creature_on_battlefield(&mut late_source, 0, "riling_dawnbreaker_signaling_roar");
    inject_creature_on_battlefield(&mut late_source, 0, "grizzly_bears");
    assert!(late_source.state.pending_triggers.is_empty());
    assert!(late_source.state.stack.is_empty());
}

#[test]
fn riling_dawnbreaker_skips_no_target_events_and_revalidates_target_generation() {
    let mut no_target = engine_at_main1(128_004);
    inject_creature_on_battlefield(&mut no_target, 0, "riling_dawnbreaker_signaling_roar");
    let batch = no_target
        .apply_command(0, &primitive_yield())
        .expect("enter combat without another controlled creature");
    assert!(no_target.state.pending_triggers.is_empty());
    assert!(no_target.state.stack.is_empty());
    assert_eq!(priority_changes_in(&batch), [0]);

    let mut stale = engine_at_main1(128_005);
    inject_creature_on_battlefield(&mut stale, 0, "riling_dawnbreaker_signaling_roar");
    let target = inject_creature_on_battlefield(&mut stale, 0, "grizzly_bears");
    stale
        .apply_command(0, &primitive_yield())
        .expect("enter combat with a target");
    stale
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose the target generation");
    *stale
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(
        stale.effective_power(target),
        Some(2),
        "the pump must not follow the object into a new generation"
    );
}

#[test]
fn acrobatic_cheerleader_requires_tapped_and_rechecks_before_resolution() {
    let mut untapped = engine_at_main1(128_010);
    inject_creature_on_battlefield(&mut untapped, 0, "acrobatic_cheerleader");
    advance_from_main1_to_main2(&mut untapped, 0);
    assert!(untapped.state.stack.is_empty());
    assert!(untapped.state.triggered_once.is_empty());

    let mut changes = engine_at_main1(128_011);
    let cheerleader = inject_creature_on_battlefield(&mut changes, 0, "acrobatic_cheerleader");
    changes.state.objects.get_mut(&cheerleader).unwrap().tapped = true;
    let batch = advance_from_main1_to_main2(&mut changes, 0);
    assert_eq!(changes.state.stack.len(), 1);
    assert_eq!(priority_changes_in(&batch), [0]);
    changes.state.objects.get_mut(&cheerleader).unwrap().tapped = false;
    resolve_entire_stack_two_player(&mut changes);
    assert_eq!(
        changes.state.objects[&cheerleader].counter_count(CounterKind::Keyword(Keyword::Flying)),
        0,
        "the intervening-if condition is checked again at resolution"
    );
    assert_eq!(changes.state.triggered_once.len(), 1);
}

#[test]
fn acrobatic_cheerleader_spends_its_allowance_before_resolution() {
    let mut engine = engine_at_main1(128_012);
    let cheerleader = inject_creature_on_battlefield(&mut engine, 0, "acrobatic_cheerleader");
    engine.state.objects.get_mut(&cheerleader).unwrap().tapped = true;
    advance_from_main1_to_main2(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1);
    engine
        .state
        .stack
        .pop()
        .expect("simulate the trigger being countered");
    engine.state.objects.get_mut(&cheerleader).unwrap().tapped = true;

    let repeated = refire_second_main(&mut engine, 0);
    assert!(engine.state.stack.is_empty());
    assert!(repeated
        .events
        .iter()
        .all(|event| !matches!(event.ev, Some(Ev::StackPushed(_)))));
    assert_eq!(engine.state.triggered_once.len(), 1);
}

#[test]
fn acrobatic_cheerleader_remains_exhausted_across_turns_counter_removal_and_control_change() {
    let mut engine = engine_at_main1(128_014);
    let cheerleader = inject_creature_on_battlefield(&mut engine, 0, "acrobatic_cheerleader");
    engine.state.objects.get_mut(&cheerleader).unwrap().tapped = true;
    advance_from_main1_to_main2(&mut engine, 0);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&cheerleader].counter_count(CounterKind::Keyword(Keyword::Flying)),
        1
    );

    engine
        .state
        .objects
        .get_mut(&cheerleader)
        .unwrap()
        .set_counter(CounterKind::Keyword(Keyword::Flying), 0);
    engine.state.objects.get_mut(&cheerleader).unwrap().tapped = true;
    assert!(refire_second_main(&mut engine, 0)
        .events
        .iter()
        .all(|event| { !matches!(event.ev, Some(Ev::StackPushed(_))) }));

    let object = engine.state.objects.get_mut(&cheerleader).unwrap();
    object.controller = 1;
    object.tapped = true;
    assert!(refire_second_main(&mut engine, 1)
        .events
        .iter()
        .all(|event| { !matches!(event.ev, Some(Ev::StackPushed(_))) }));
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.triggered_once.len(), 1);
}

#[test]
fn acrobatic_cheerleader_uses_tap_lki_and_a_returned_object_is_fresh() {
    let mut engine = engine_at_main1(128_013);
    let cheerleader = inject_creature_on_battlefield(&mut engine, 0, "acrobatic_cheerleader");
    let unsummon = inject_card_into_hand(&mut engine, 1, "unsummon");
    engine.state.objects.get_mut(&cheerleader).unwrap().tapped = true;
    advance_from_main1_to_main2(&mut engine, 0);

    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    grant_pool(&mut engine, 1);
    let unsummon_slot = engine.state.players[1]
        .hand
        .iter()
        .position(|candidate| *candidate == unsummon)
        .expect("Unsummon in hand");
    engine
        .apply_command(
            1,
            &cast_spell(unsummon_slot, targets_with_damage(vec![(cheerleader, 0)])),
        )
        .expect("cast Unsummon in response");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&cheerleader].zone, Zone::Hand);
    assert_eq!(
        engine.state.objects[&cheerleader].counter_count(CounterKind::Keyword(Keyword::Flying)),
        0,
        "the old trigger observes tapped LKI but cannot affect a departed object"
    );

    grant_pool(&mut engine, 0);
    let cheerleader_slot = engine.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == cheerleader)
        .expect("Cheerleader returned to hand");
    engine
        .apply_command(0, &cast_spell(cheerleader_slot, Vec::new()))
        .expect("recast Cheerleader");
    resolve_entire_stack_two_player(&mut engine);
    engine.state.objects.get_mut(&cheerleader).unwrap().tapped = true;
    refire_second_main(&mut engine, 0);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the new object generation may trigger"
    );
    assert_eq!(engine.state.triggered_once.len(), 2);
}

#[test]
fn simultaneous_phase_triggers_use_apnap_order_before_priority() {
    let mut engine = GameEngine::new(128_020, &[0, 1], 20, None, true).expect("new engine");
    // Product session creation is intentionally still two-player. Add one synthetic seat only in
    // this scenario so the player-set-generic APNAP implementation proves the nonactive seats do
    // not collapse into one boolean rank.
    engine.state.players.push(PlayerState::new(2_000_000, 20));
    engine.state.turn_step = TurnStep::Main1;
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;

    let mut ability = CardRegistry::global()
        .get("acrobatic_cheerleader")
        .expect("Acrobatic Cheerleader")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::AtBeginningOfCombat {
        player: CastTriggerPlayer::AnyPlayer,
    };
    ability.intervening_if = None;
    ability.triggers_only_once = false;
    ability.max_triggers_per_turn = Some(1);

    for player in 0..3 {
        let source = inject_creature_on_battlefield(&mut engine, player, "grizzly_bears");
        engine.state.add_triggered_ability_grant(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability.clone())),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
    }

    let batch = engine
        .apply_command(1, &primitive_yield())
        .expect("enter combat with three observers");
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .map(|item| item.controller)
            .collect::<Vec<_>>(),
        [1, 2_000_000, 0]
    );

    let last_push = batch
        .events
        .iter()
        .rposition(|event| matches!(event.ev, Some(Ev::StackPushed(_))))
        .expect("stack pushes");
    let priority = batch
        .events
        .iter()
        .position(|event| matches!(event.ev, Some(Ev::PriorityChanged(_))))
        .expect("priority event");
    assert!(
        last_push < priority,
        "all APNAP triggers reach the stack before priority"
    );
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PhaseChanged(change)) if change.phase_id == PhaseId::BeginCombat as i32
    )));
    assert_eq!(engine.state.trigger_uses_this_turn.len(), 3);
}

fn capped_phase_source(engine: &mut GameEngine, mut ability: TriggeredAbilityDef) -> u32 {
    let source = inject_creature_on_battlefield(engine, 0, "grizzly_bears");
    ability.trigger = TriggerCondition::AtBeginningOfCombat {
        player: CastTriggerPlayer::AnyPlayer,
    };
    ability.triggers_only_once = false;
    ability.max_triggers_per_turn = Some(1);
    let mut face = CardRegistry::global()
        .get("grizzly_bears")
        .unwrap()
        .primary_face()
        .clone();
    face.triggered_abilities = vec![ability];
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        source_card_id: "grizzly_bears".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    source
}

fn refire_begin_combat(engine: &mut GameEngine) -> RuledEventBatch {
    // An extra combat's beginning is enough to retest a printed cap without implementing
    // additional-combat effects. The phase transition and trigger are ordinary commands.
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = engine.state.active_player_idx;
    engine.state.passes_since_stack_change = 0;
    engine
        .apply_command(engine.state.active_player_id(), &primitive_yield())
        .unwrap()
}

#[test]
fn issue_164_intervening_if_and_real_turn_rollover_preserve_trigger_caps() {
    let mut engine = engine_at_main1(164_101);
    let mut ability = CardRegistry::global()
        .get("acrobatic_cheerleader")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.effect = vec![SpellEffectKind::GainLife {
        amount: Amount::Fixed(1),
    }];
    ability.intervening_if = Some(GameCondition::ObjectTapped {
        object: ConditionObjectRef::Source,
        tapped: true,
    });
    let source = capped_phase_source(&mut engine, ability);
    refire_begin_combat(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(
        engine.state.trigger_uses_this_turn.is_empty(),
        "false intervening-if never triggers"
    );
    engine.state.objects.get_mut(&source).unwrap().tapped = true;
    refire_begin_combat(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    engine.state.objects.get_mut(&source).unwrap().tapped = false;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 20,
        "the resolution-time check fails"
    );
    engine.state.objects.get_mut(&source).unwrap().tapped = true;
    refire_begin_combat(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "failed resolution does not refund the cap"
    );
    let turn = engine.state.turn_instance;
    finish_turn_from_begin_combat(&mut engine, 0);
    assert_eq!(engine.state.turn_instance, turn + 1);
    assert!(engine.state.trigger_uses_this_turn.is_empty());
    advance_to_main1_from_game_start(&mut engine);
    refire_begin_combat(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "another player's turn restores the allowance"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 21);
}

#[test]
fn issue_164_decline_rejected_target_and_refresh_do_not_refund_a_trigger() {
    let mut engine = engine_at_main1(164_102);
    let ability = CardRegistry::global()
        .get("gravedigger")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    let source = capped_phase_source(&mut engine, ability);
    let graveyard_card = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    refire_begin_combat(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let usage = engine.state.trigger_uses_this_turn.clone();
    let command_index = engine.state.command_index;
    assert!(engine
        .apply_command(0, &choose_trigger_target(source))
        .is_err());
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine.initial_response_batch();
    assert_eq!(engine.state.trigger_uses_this_turn, usage);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: true,
                    selected_modes: vec![],
                    targets: vec![],
                })),
            },
        )
        .unwrap();
    assert!(engine.state.pending_triggers.is_empty());
    assert_eq!(engine.state.objects[&graveyard_card].zone, Zone::Graveyard);
    refire_begin_combat(&mut engine);
    assert!(engine.state.pending_triggers.is_empty());
    assert_eq!(engine.state.trigger_uses_this_turn, usage);
}

#[test]
fn issue_164_no_legal_targets_still_spends_the_trigger_allowance() {
    let mut engine = engine_at_main1(164_103);
    let mut ability = CardRegistry::global()
        .get("gravedigger")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.may = false;
    capped_phase_source(&mut engine, ability);
    refire_begin_combat(&mut engine);
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine
            .state
            .trigger_uses_this_turn
            .values()
            .copied()
            .sum::<u32>(),
        1
    );
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    refire_begin_combat(&mut engine);
    assert!(
        engine.state.pending_triggers.is_empty(),
        "later legal targets cannot refund the earlier trigger"
    );
}

#[test]
fn issue_164_target_leaving_before_resolution_does_not_refund_the_cap() {
    let mut engine = engine_at_main1(164_104);
    let ability = CardRegistry::global()
        .get("riling_dawnbreaker_signaling_roar")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    capped_phase_source(&mut engine, ability);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "unsummon");
    refire_begin_combat(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(target))
        .unwrap();
    grant_pool(&mut engine, 0);
    let command = cast_spell(
        hand_index_for_card(&engine, 0, "unsummon"),
        target_object(target),
    );
    engine.apply_command(0, &command).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert!(engine.state.stack.is_empty());
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    refire_begin_combat(&mut engine);
    assert!(engine.state.pending_triggers.is_empty());
    assert_eq!(
        engine
            .state
            .trigger_uses_this_turn
            .values()
            .copied()
            .sum::<u32>(),
        1
    );
}
