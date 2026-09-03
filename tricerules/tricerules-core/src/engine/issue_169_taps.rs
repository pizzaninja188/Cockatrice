//! Event-boundary regressions, including the official Tangle Wire actor counterexample.
use super::*;

fn permanent(engine: &mut GameEngine, player: usize, card: &str) -> ObjectId {
    let oid = engine.state.players[player].hand[0];
    engine.state.objects.get_mut(&oid).unwrap().card_id = card.into();
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        oid,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    oid
}

fn grant(
    engine: &mut GameEngine,
    source: ObjectId,
    cardinality: TapTriggerCardinality,
    condition: Option<GameCondition>,
) {
    let mut ability = engine
        .registry
        .get("ajanis_pridemate")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::WheneverPlayerTapsCreature {
        player: CastTriggerPlayer::Controller,
        controllers: RelativePlayerSet::Opponents,
        cardinality,
    };
    ability.intervening_if = condition;
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

fn fixture(cardinality: TapTriggerCardinality) -> (GameEngine, ObjectId, ObjectId, ObjectId) {
    let mut engine = GameEngine::new(169010, &[7, 19], 20, None, true).unwrap();
    engine.state.players.push(PlayerState::new(42, 20));
    let source = permanent(&mut engine, 0, "grizzly_bears");
    let first = permanent(&mut engine, 1, "grizzly_bears");
    let second = permanent(&mut engine, 1, "grizzly_bears");
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != second);
    engine.state.players[2].battlefield.push(second);
    let object = engine.state.objects.get_mut(&second).unwrap();
    object.owner = 42;
    object.base_controller = 42;
    object.controller = 42;
    grant(&mut engine, source, cardinality, None);
    (engine, source, first, second)
}

#[test]
fn actor_is_neither_object_owner_nor_controller_of_originating_spell() {
    for actor in [7, 19, 42] {
        let (mut engine, _, first, second) = fixture(TapTriggerCardinality::EachObject);
        // A creature stolen from the watcher still belongs to an opponent at the event.
        engine.state.objects.get_mut(&first).unwrap().owner = 7;
        let events = engine.tap_permanents(actor, &[first, second]);
        let triggers = engine.collect_event_triggers(&events);
        assert_eq!(triggers.len(), if actor == 7 { 2 } else { 0 });
        assert!(triggers
            .iter()
            .all(|t| t.trigger_context.affected_player == Some(7)));
    }
}

#[test]
fn distinct_instructions_in_one_collected_batch_are_not_coalesced() {
    let (mut engine, _, first, second) = fixture(TapTriggerCardinality::OneOrMorePerAction);
    let mut events = engine.tap_permanents(7, &[first]);
    events.extend(engine.tap_permanents(7, &[second]));
    assert_eq!(engine.collect_event_triggers(&events).len(), 2);
    assert_eq!(engine.state.next_tap_action_id, 2);
    assert!(engine
        .tap_permanents(7, &[first, first, u32::MAX])
        .is_empty());
    assert_eq!(
        engine.state.next_tap_action_id, 2,
        "no-op does not allocate an action"
    );
}

#[test]
fn snapshots_survive_source_departure_and_observed_object_blink() {
    let (mut engine, source, first, _) = fixture(TapTriggerCardinality::EachObject);
    let generation = engine.state.zone_change_generation[&first];
    let events = engine.tap_permanents(7, &[first]);
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        source,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        first,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        first,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    let triggers = engine.collect_event_triggers(&events);
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].controller, 7);
    assert_eq!(
        triggers[0]
            .trigger_context
            .observed_object
            .unwrap()
            .zone_change_generation,
        generation
    );
    assert_ne!(engine.state.zone_change_generation[&first], generation);
}

#[test]
fn later_status_changes_do_not_change_the_trigger_time_intervening_if() {
    let (mut engine, source, first, _) = fixture(TapTriggerCardinality::EachObject);
    engine.state.continuous_effects.clear();
    grant(
        &mut engine,
        source,
        TapTriggerCardinality::EachObject,
        Some(GameCondition::ObjectTapped {
            object: ConditionObjectRef::Source,
            tapped: false,
        }),
    );
    let events = engine.tap_permanents(7, &[first]);
    set_tapped(&mut engine.state, source, true);
    assert_eq!(
        engine.collect_event_triggers(&events).len(),
        1,
        "trigger-time condition is captured; resolution will check it again"
    );
}

#[test]
fn regeneration_is_performed_by_the_regenerated_permanents_controller() {
    let (mut engine, _, first, _) = fixture(TapTriggerCardinality::EachObject);
    engine
        .state
        .objects
        .get_mut(&first)
        .unwrap()
        .regeneration_shields = 1;
    let (regenerated, event) =
        resolution::consume_regen_shield(&mut engine, first, &mut Vec::new());
    assert!(regenerated);
    let events = event.into_iter().collect::<Vec<_>>();
    assert!(engine.collect_event_triggers(&events).is_empty());
    assert!(matches!(&events[0], GameEvent::BecameTapped { action, .. } if action.actor == 19));
}

#[test]
fn animated_nontokens_and_tokens_use_derived_event_time_types() {
    for token in [false, true] {
        let (mut engine, _, first, _) = fixture(TapTriggerCardinality::EachObject);
        engine.state.objects.get_mut(&first).unwrap().card_id = "forest".into();
        if token {
            let origin = engine.copiable_values_for(first).unwrap();
            engine.state.objects.get_mut(&first).unwrap().token_origin = Some(origin);
        }
        let ordinary = engine.tap_permanents(7, &[first]);
        assert!(engine.collect_event_triggers(&ordinary).is_empty());
        set_tapped(&mut engine.state, first, false);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(first),
            kind: ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Creature],
                creature_types: vec![],
            }),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });
        let animated = engine.tap_permanents(7, &[first]);
        engine.state.continuous_effects.pop();
        assert_eq!(engine.collect_event_triggers(&animated).len(), 1);
    }
}

#[test]
fn sharae_grouping_cap_and_control_changes_have_independent_identities() {
    let (mut engine, source, first, second) = fixture(TapTriggerCardinality::EachObject);
    engine.state.continuous_effects.clear();
    engine.state.objects.get_mut(&source).unwrap().card_id = "sharae_of_numbing_depths".into();
    let taps = engine.tap_permanents(7, &[first, second]);
    engine.fire_triggers(&taps);
    assert_eq!(engine.state.staged_trigger_groups.len(), 1);
    assert_eq!(engine.state.staged_trigger_groups[0].triggers.len(), 1);
    engine.state.staged_trigger_groups.clear(); // countering/removing the trigger does not refund
    engine.state.players[0]
        .battlefield
        .retain(|oid| *oid != source);
    engine.state.players[1].battlefield.push(source);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .base_controller = 19;
    engine.state.objects.get_mut(&source).unwrap().controller = 19;
    set_tapped(&mut engine.state, second, false);
    let taps = engine.tap_permanents(19, &[second]);
    engine.fire_triggers(&taps);
    assert!(
        engine.state.staged_trigger_groups.is_empty(),
        "control does not refresh a cap"
    );
    engine.state.turn_instance += 1; // same active player: an extra turn is still a new instance
    set_tapped(&mut engine.state, second, false);
    let taps = engine.tap_permanents(19, &[second]);
    engine.fire_triggers(&taps);
    assert_eq!(engine.state.staged_trigger_groups.len(), 1);
    assert_eq!(
        engine.state.staged_trigger_groups[0].triggers[0].controller,
        19
    );
}

#[test]
fn false_event_conditions_do_not_revive_or_spend_allowances_later() {
    let (mut engine, source, first, _) = fixture(TapTriggerCardinality::EachObject);
    engine.state.continuous_effects.clear();
    grant(
        &mut engine,
        source,
        TapTriggerCardinality::EachObject,
        Some(GameCondition::ObjectTapped {
            object: ConditionObjectRef::Source,
            tapped: true,
        }),
    );
    let events = engine.tap_permanents(7, &[first]);
    set_tapped(&mut engine.state, source, true);
    assert!(engine.collect_event_triggers(&events).is_empty());
    assert!(engine.state.trigger_uses_this_turn.is_empty());
}
