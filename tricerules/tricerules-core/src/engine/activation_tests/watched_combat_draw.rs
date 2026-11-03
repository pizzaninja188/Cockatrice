use super::super::damage::{DamageClassification, DamageEvent, DamageRecipient};
use super::*;

// Exact Tamiyo +1 clause, cost and entry loyalty only; no complete Tamiyo admission.
const SOURCE: &str = r#"(id:"synthetic_tamiyo_observer",name:"Synthetic Tamiyo Observer",face_id:"synthetic_tamiyo_observer",
    types:["Planeswalker"],loyalty:4,
    activated_abilities:[(ability_id:"plus_01",presentation:Fallback,costs:[Loyalty(1)],
        effect:[CreateWatchedCombatDamageDrawUntilNextTurn(target:(kind:Creature),ability:(
            ability_id:"delayed_01",presentation:Fallback,trigger:WheneverWatchedObjectDealsCombatDamage,
            effect:[Draw(who:Controller,count:1)]))],
        targeting:Some((groups:[(min:0,max:2,prompt:"Choose up to two creatures",effect_indices:[0])]))
    )])"#;
const CREATURE: &str = r#"(id:"observer_creature",name:"Observer Creature",face_id:"observer_creature",
    types:["Creature","Elf"],power:4,toughness:8)"#;
const TRAMPLER: &str = r#"(id:"observer_trampler",name:"Observer Trampler",face_id:"observer_trampler",
    types:["Creature"],power:4,toughness:8,keywords:[Trample])"#;
const DOUBLE_STRIKER: &str = r#"(id:"observer_double_striker",name:"Observer Double Striker",face_id:"observer_double_striker",
    types:["Creature"],power:4,toughness:8,keywords:[DoubleStrike])"#;
const BLOCKER: &str = r#"(id:"observer_blocker",name:"Observer Blocker",face_id:"observer_blocker",
    types:["Creature"],power:1,toughness:2)"#;

fn fixture() -> (GameEngine, ObjectId, [ObjectId; 2]) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/gratuitous_violence.ron"),
            include_str!("../../../../tricerules-cards/data/rings_of_brighthearth.ron"),
            SOURCE,
            CREATURE,
            TRAMPLER,
            DOUBLE_STRIKER,
            BLOCKER,
        ],
        &[],
    )
    .unwrap();
    let mut engine = GameEngine::new_with_registry(
        602_455,
        &[17, 29, 43],
        20,
        Some(vec![
            EngineDeck {
                mainboard: vec!["forest".into(); 40],
                commanders: vec![]
            };
            3
        ]),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 17, "Synthetic Tamiyo Observer");
    let creatures = [
        place_ready(&mut engine, 29, "Observer Creature"),
        place_ready(&mut engine, 43, "Observer Creature"),
    ];
    (engine, source, creatures)
}

fn begin(engine: &mut GameEngine, source: ObjectId, targets: &[ObjectId]) {
    activate(engine, source, targets);
    resolve_ability(engine);
}

fn activate(engine: &mut GameEngine, source: ObjectId, targets: &[ObjectId]) {
    let generation = engine.payment_object_ref(source).zone_change_generation;
    let targets = targets
        .iter()
        .map(|&id| rv1::TargetRef {
            object_id: id,
            expected_zone_change_generation: Some(
                engine.payment_object_ref(id).zone_change_generation,
            ),
            ..Default::default()
        })
        .collect();
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: source,
                        ability_index: 0,
                        expected_zone_change_generation: generation,
                        targets,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
}

fn resolve_ability(engine: &mut GameEngine) {
    for _ in 0..3 {
        engine
            .apply_command(
                engine.state.priority_player_id(),
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("target-bound observer resolves through normal ability priority");
    }
}

#[test]
fn watched_combat_draw_zero_two_and_partially_illegal_target_incarnations() {
    for (count, invalid) in [(0, 0), (1, 0), (2, 0), (2, 1), (2, 2)] {
        let (mut engine, source, creatures) = fixture();
        activate(&mut engine, source, &creatures[..count]);
        for &id in creatures.iter().take(invalid) {
            super::super::resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                id,
                Zone::Graveyard,
                None,
            )
            .unwrap();
        }
        resolve_ability(&mut engine);
        assert_eq!(engine.state.active_event_observers.len(), count - invalid);
        let watched = engine
            .state
            .active_event_observers
            .iter()
            .map(|observer| observer.watched.unwrap().object_id)
            .collect::<Vec<_>>();
        assert_eq!(watched, creatures[invalid..count]);
    }
}

#[test]
fn watched_combat_draw_invalid_announcements_are_atomic() {
    for mutation in 0..6 {
        let (mut engine, source, creatures) = fixture();
        let ids = match mutation {
            0 => vec![creatures[0], creatures[0]],
            1 => vec![creatures[0], creatures[1], source],
            2 => vec![source],
            _ => vec![creatures[0]],
        };
        let mut command = rv1::ActivateAbility {
            source_object_id: source,
            ability_index: 0,
            expected_zone_change_generation: engine
                .payment_object_ref(source)
                .zone_change_generation,
            targets: ids
                .iter()
                .map(|&id| rv1::TargetRef {
                    object_id: id,
                    expected_zone_change_generation: Some(
                        engine.payment_object_ref(id).zone_change_generation,
                    ),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        if mutation == 3 {
            command.targets[0].expected_zone_change_generation = Some(900);
        }
        if mutation == 4 {
            command.expected_zone_change_generation += 1;
        }
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(
                if mutation == 5 { 29 } else { 17 },
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(command))
                }
            )
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        assert!(engine.state.active_event_observers.is_empty());
    }
}

#[test]
fn watched_combat_draw_counts_sources_once_per_slice_and_repeats_later_damage() {
    let (mut engine, source, creatures) = fixture();
    begin(&mut engine, source, &creatures);
    let slice = [
        damage(
            &engine,
            creatures[0],
            DamageRecipient::Permanent(creatures[1]),
            1,
        ),
        damage(&engine, creatures[0], DamageRecipient::Player(43), 3),
        damage(&engine, creatures[1], DamageRecipient::Player(17), 4),
    ];
    let mut events = Vec::new();
    let first = engine.fire_triggers_collecting(&slice, &mut events);
    assert_eq!(first.len(), 2, "two watched sources, not three recipients");
    assert!(first
        .iter()
        .all(|trigger| trigger.controller == 17 && trigger.source_id == source));
    let later = engine.fire_triggers_collecting(&slice[..2], &mut events);
    assert_eq!(
        later.len(),
        1,
        "a later combat-damage step repeats without recipient multiplication"
    );
    assert_eq!(engine.state.active_event_observers.len(), 2);
}

#[test]
fn watched_combat_draw_excludes_noncombat_zero_unknown_generation_and_unwatched_sources() {
    let (mut engine, source, creatures) = fixture();
    begin(&mut engine, source, &[creatures[0]]);
    let qualifying = damage(&engine, creatures[0], DamageRecipient::Player(17), 1);
    for mutation in 0..5 {
        let mut event = qualifying.clone();
        let GameEvent::DamageDealt { event: damage } = &mut event else {
            unreachable!();
        };
        match mutation {
            0 => damage.classification = DamageClassification::Noncombat,
            1 => damage.amount = 0,
            2 => damage.source.zone_change_generation = None,
            3 => {
                damage.source.zone_change_generation = damage
                    .source
                    .zone_change_generation
                    .map(|generation| generation + 1)
            }
            _ => damage.source.object_id = creatures[1],
        }
        let mut events = Vec::new();
        assert!(engine
            .fire_triggers_collecting(&[event], &mut events)
            .is_empty());
    }
}

#[test]
fn watched_combat_draw_keeps_captured_controller_and_source_after_type_control_or_source_loss() {
    let (mut engine, source, creatures) = fixture();
    begin(&mut engine, source, &[creatures[0]]);
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        source,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    let mut event = damage(&engine, creatures[0], DamageRecipient::Player(17), 1);
    let GameEvent::DamageDealt { event: damage } = &mut event else {
        unreachable!();
    };
    damage.source.controller = 43;
    damage.source.types = vec!["Artifact".into()];
    let mut events = Vec::new();
    let collected = engine.fire_triggers_collecting(&[event], &mut events);
    assert_eq!(
        collected.len(),
        1,
        "the watched object need not remain a creature or keep its old controller"
    );
    assert_eq!(collected[0].controller, 17);
    assert_eq!(collected[0].source_id, source);
    let before = engine.state.players[0].hand.len();
    engine.flush_staged_triggers(&mut events);
    engine.resolve_top_of_stack(&mut events).unwrap();
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
}

#[test]
fn watched_combat_draw_reentry_does_not_rebind_and_old_damage_fact_survives_source_death() {
    let (mut engine, source, creatures) = fixture();
    begin(&mut engine, source, &[creatures[0]]);
    let old_fact = damage(&engine, creatures[0], DamageRecipient::Player(17), 1);
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        creatures[0],
        Zone::Graveyard,
        None,
    )
    .unwrap();
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        creatures[0],
        Zone::Battlefield,
        None,
    )
    .unwrap();
    let new_fact = damage(&engine, creatures[0], DamageRecipient::Player(17), 1);
    let mut events = Vec::new();
    assert!(engine
        .fire_triggers_collecting(&[new_fact], &mut events)
        .is_empty());
    assert_eq!(
        engine
            .fire_triggers_collecting(&[old_fact], &mut events)
            .len(),
        1,
        "already committed old-incarnation damage still triggers if that source died in its batch"
    );
}

#[test]
fn watched_combat_draw_uses_resolving_controller_in_prepared_copy_control_shape() {
    let (mut engine, source, creatures) = fixture();
    activate(&mut engine, source, &[creatures[0]]);
    // Prepared resolving-controller shape, not an actual copy/control factory integration.
    engine.state.stack.last_mut().unwrap().controller = 29;
    resolve_ability(&mut engine);
    let event = damage(&engine, creatures[0], DamageRecipient::Player(43), 1);
    let mut events = Vec::new();
    let collected = engine.fire_triggers_collecting(&[event], &mut events);
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].controller, 29);
    let before = engine.state.players[1].hand.len();
    engine.flush_staged_triggers(&mut events);
    engine.resolve_top_of_stack(&mut events).unwrap();
    assert_eq!(engine.state.players[1].hand.len(), before + 1);
}

#[test]
fn watched_combat_draw_survives_cleanup_and_opponent_turns_but_expires_before_next_actual_turn() {
    for extra in [false, true] {
        let (mut engine, source, creatures) = fixture();
        begin(&mut engine, source, &creatures);
        if extra {
            engine.state.extra_turn_queue.push(17);
        }
        for next in if extra { vec![17] } else { vec![29, 43, 17] } {
            engine.state.turn_step = TurnStep::Cleanup;
            engine.finish_cleanup_roll_new_turn(Vec::new()).unwrap();
            assert_eq!(engine.state.active_player_id(), next);
            assert_eq!(
                engine.state.active_event_observers.len(),
                if next == 17 { 0 } else { 2 }
            );
        }
    }
}

#[test]
fn watched_combat_draw_departed_controller_cannot_receive_new_delayed_triggers() {
    let (mut engine, source, creatures) = fixture();
    begin(&mut engine, source, &creatures);
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    let event = damage(&engine, creatures[0], DamageRecipient::Player(43), 1);
    let mut events = Vec::new();
    assert!(engine
        .fire_triggers_collecting(&[event], &mut events)
        .is_empty());
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn watched_combat_draw_source_owner_departure_preserves_surviving_controllers_watch() {
    let (mut engine, source, creatures) = fixture();
    engine.state.objects.get_mut(&source).unwrap().owner = 43;
    begin(&mut engine, source, &[creatures[0]]);
    engine
        .apply_command(
            43,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    assert!(!engine.state.objects.contains_key(&source));
    let event = damage(&engine, creatures[0], DamageRecipient::Player(17), 1);
    let mut events = Vec::new();
    let collected = engine.fire_triggers_collecting(&[event], &mut events);
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].controller, 17);
    assert_eq!(collected[0].source_id, source);
    let before = engine.state.players[0].hand.len();
    engine.flush_staged_triggers(&mut events);
    engine.resolve_top_of_stack(&mut events).unwrap();
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
}

fn answer(engine: &mut GameEngine, mut answer: rv1::SubmitResolutionChoice) {
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let actor = pending.deciding_player;
    answer.resolution_prompt_id = pending.resolution_prompt_id.get();
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)),
            },
        )
        .unwrap();
}

#[test]
fn watched_combat_draw_real_rings_copy_creates_independent_same_source_watchers() {
    let (mut engine, source, creatures) = fixture();
    place_ready(&mut engine, 17, "Rings of Brighthearth");
    engine.state.players[0].mana_pool.colorless = 2; // Prepared actual-copy integration, not normal play.
    activate(&mut engine, source, &[creatures[0]]);
    assert_eq!(engine.state.stack.len(), 2);
    resolve_ability(&mut engine); // actual Rings trigger offers optional two-mana payment.
    answer(
        &mut engine,
        rv1::SubmitResolutionChoice {
            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
            ..Default::default()
        },
    );
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let payment = rv1::PaymentSelection {
        expected_state_revision: engine.state.command_index,
        source: Some(engine.payment_object_ref(pending.presentation.source_object_id)),
        mana: Some(rv1::PaymentMana {
            c: 2,
            ..Default::default()
        }),
        ..Default::default()
    };
    answer(
        &mut engine,
        rv1::SubmitResolutionChoice {
            decision: rv1::ResolutionChoiceDecision::PayMana as i32,
            payment: Some(payment),
            ..Default::default()
        },
    );
    answer(&mut engine, rv1::SubmitResolutionChoice::default()); // retain original target.
    assert_eq!(engine.state.stack.len(), 2);
    assert!(engine
        .state
        .stack
        .iter()
        .all(|item| item.targets.len() == 1 && item.targets[0].object_id == creatures[0]));
    resolve_ability(&mut engine);
    resolve_ability(&mut engine);
    assert_eq!(engine.state.active_event_observers.len(), 2);
    let event = damage(&engine, creatures[0], DamageRecipient::Player(17), 1);
    let mut events = Vec::new();
    assert_eq!(
        engine.fire_triggers_collecting(&[event], &mut events).len(),
        2
    );
    engine.flush_staged_triggers(&mut events);
    let order = engine.state.pending_trigger_order.as_ref().unwrap();
    let selected = order.candidates[0].object_id;
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitTriggerOrder(
                    rv1::SubmitTriggerOrder {
                        trigger_object_id: selected,
                    },
                )),
            },
        )
        .unwrap();
    assert_eq!(engine.state.stack.len(), 2);
    let before = engine.state.players[0].hand.len();
    engine.resolve_top_of_stack(&mut events).unwrap();
    engine.resolve_top_of_stack(&mut events).unwrap();
    assert_eq!(engine.state.players[0].hand.len(), before + 2);
}

fn combat(
    engine: &mut GameEngine,
    attacker: ObjectId,
    blocker: Option<ObjectId>,
) -> crate::state::CombatState {
    use crate::state::{CombatAttackAssignment, CombatDefenderTarget, CombatState};
    let attacker_ref = TriggerObjectRef {
        object_id: attacker,
        zone_change_generation: engine.payment_object_ref(attacker).zone_change_generation,
        controller_at_event: 29,
    };
    let state = CombatState {
        attacking: vec![attacker],
        attack_assignments: std::collections::HashMap::from([(
            attacker,
            CombatAttackAssignment {
                attacker: attacker_ref,
                defender: CombatDefenderTarget::Player(43),
                defending_player: 43,
            },
        )]),
        blockers: blocker
            .map(|id| std::collections::HashMap::from([(attacker, vec![id])]))
            .unwrap_or_default(),
        damage_assignments: std::collections::HashMap::new(),
        trample_player_damage: std::collections::HashMap::new(),
        damage_assignment_needed: false,
        attackers_declared: true,
        blockers_declared_by: vec![43],
        blockers_declared: true,
        assign_combat_damage_phase: false,
        first_strike_attackers: vec![],
        first_strike_blockers: std::collections::HashMap::new(),
        first_strike_damage_done: false,
    };
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = TurnStep::CombatDamage;
    engine.state.combat = Some(state.clone());
    state
}

fn staged_count(engine: &GameEngine) -> usize {
    engine
        .state
        .staged_trigger_groups
        .iter()
        .map(|group| group.triggers.len())
        .sum::<usize>()
        + engine
            .state
            .pending_trigger_order
            .as_ref()
            .map_or(0, |order| order.candidates.len())
        + engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_triggered)
            .count()
}

#[test]
fn watched_combat_draw_real_combat_counts_attacker_and_blocker_as_two_sources() {
    let (mut engine, source, creatures) = fixture();
    begin(&mut engine, source, &creatures);
    let combat = combat(&mut engine, creatures[0], Some(creatures[1]));
    let mut events = Vec::new();
    engine
        .resolve_combat_damage(
            &combat,
            super::super::combat::DamagePass::Normal,
            &mut events,
        )
        .unwrap();
    assert_eq!(engine.state.objects[&creatures[0]].damage, 4);
    assert_eq!(engine.state.objects[&creatures[1]].damage, 4);
    assert_eq!(staged_count(&engine), 2);
}

#[test]
fn watched_combat_draw_real_trample_split_is_one_and_double_strike_steps_are_two() {
    for double in [false, true] {
        let (mut engine, source, _) = fixture();
        let attacker = place_ready(
            &mut engine,
            29,
            if double {
                "Observer Double Striker"
            } else {
                "Observer Trampler"
            },
        );
        let blocker = (!double).then(|| place_ready(&mut engine, 43, "Observer Blocker"));
        begin(&mut engine, source, &[attacker]);
        let mut combat = combat(&mut engine, attacker, blocker);
        if let Some(blocker) = blocker {
            combat
                .damage_assignments
                .insert(attacker, vec![(blocker, 2)]);
            combat.trample_player_damage.insert(attacker, 2);
        } else {
            combat.first_strike_attackers = vec![attacker];
        }
        engine.state.combat = Some(combat.clone());
        let mut events = Vec::new();
        engine
            .resolve_combat_damage(
                &combat,
                if double {
                    super::super::combat::DamagePass::FirstStrike
                } else {
                    super::super::combat::DamagePass::Normal
                },
                &mut events,
            )
            .unwrap();
        assert_eq!(staged_count(&engine), 1);
        assert_eq!(engine.state.players[2].life, if double { 16 } else { 18 });
        if double {
            engine
                .resolve_combat_damage(
                    &combat,
                    super::super::combat::DamagePass::Normal,
                    &mut events,
                )
                .unwrap();
            assert_eq!(staged_count(&engine), 2);
            assert_eq!(engine.state.players[2].life, 12);
        } else {
            assert_eq!(engine.state.objects[&blocker.unwrap()].damage, 2);
        }
    }
}

#[test]
fn watched_combat_draw_real_fully_prevented_combat_has_no_occurrence() {
    let (mut engine, source, creatures) = fixture();
    begin(&mut engine, source, &[creatures[0]]);
    engine.add_damage_prevention(
        None,
        "prepared full prevention",
        DamagePreventionScope::Recipient(43),
        DamagePreventionAmount::FixedPerEvent(10),
    );
    let combat = combat(&mut engine, creatures[0], None);
    engine
        .resolve_combat_damage(
            &combat,
            super::super::combat::DamagePass::Normal,
            &mut Vec::new(),
        )
        .unwrap();
    assert_eq!(engine.state.players[2].life, 20);
    assert_eq!(staged_count(&engine), 0);
}

#[test]
fn watched_combat_draw_real_damage_replacement_commits_and_triggers_once_after_choice() {
    use super::super::damage::DamageReplacementApplication;
    let (mut engine, source, creatures) = fixture();
    place_ready(&mut engine, 29, "Gratuitous Violence");
    begin(&mut engine, source, &[creatures[0]]);
    engine.add_damage_prevention(
        None,
        "prepared one damage prevention",
        DamagePreventionScope::Recipient(43),
        DamagePreventionAmount::FixedPerEvent(1),
    );
    let combat = combat(&mut engine, creatures[0], None);
    let mut events = Vec::new();
    engine
        .resolve_combat_damage(
            &combat,
            super::super::combat::DamagePass::Normal,
            &mut events,
        )
        .unwrap();
    engine.publish_resolution_prompt_ids(&mut events).unwrap();
    assert_eq!(engine.state.players[2].life, 20);
    assert_eq!(
        staged_count(&engine),
        0,
        "no damage occurrence before replacement choice commits"
    );
    let Some(super::super::replacement::PendingReplacementEvent::Damage(batch)) =
        &engine.state.pending_replacement_event
    else {
        panic!("actual damage order parks");
    };
    let application = batch
        .applications
        .iter()
        .find(|choice| {
            matches!(
                choice.application,
                DamageReplacementApplication::DoubleEffect(_)
            )
        })
        .unwrap()
        .choice_id;
    let prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: prompt,
                chosen_object_ids: vec![application],
                ..Default::default()
            },
        )),
    };
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(17, &command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine.apply_command(43, &command).unwrap();
    assert_eq!(
        engine.state.players[2].life, 13,
        "double four, then prevent one"
    );
    assert_eq!(staged_count(&engine), 1);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(43, &command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    let before = engine.state.players[0].hand.len();
    engine.resolve_top_of_stack(&mut events).unwrap();
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
    assert_eq!(staged_count(&engine), 0);
}

fn damage(
    engine: &GameEngine,
    source: ObjectId,
    recipient: DamageRecipient,
    amount: u32,
) -> GameEvent {
    let mut event = DamageEvent::combat(
        source,
        engine.controller_of(source).unwrap(),
        "Observer Creature",
        recipient,
        amount,
    );
    engine.capture_combat_damage_source(&mut event);
    GameEvent::DamageDealt { event }
}

#[test]
fn watched_combat_draw_opponent_creature_damaging_a_blocker_draws_for_captured_controller() {
    let (mut engine, source, creatures) = fixture();
    begin(&mut engine, source, &[creatures[0]]);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    let before = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.len())
        .collect::<Vec<_>>();
    // Prepared committed-event seam: real combat and replacement-resume integration are separate.
    let event = damage(
        &engine,
        creatures[0],
        DamageRecipient::Permanent(creatures[1]),
        1,
    );
    let mut events = Vec::new();
    let collected = engine.fire_triggers_collecting(&[event], &mut events);
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].controller, 17);
    assert_eq!(collected[0].source_id, source);
    engine.flush_staged_triggers(&mut events);
    engine.resolve_top_of_stack(&mut events).unwrap();
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.hand.len())
            .collect::<Vec<_>>(),
        [before[0] + 1, before[1], before[2]]
    );
}
