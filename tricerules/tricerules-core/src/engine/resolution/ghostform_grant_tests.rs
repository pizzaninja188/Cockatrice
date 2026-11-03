//! Prepared synthetic Ghostform +1 consumer. No Kaya registration or legal-start claim.
use super::*;

fn permanent(engine: &mut GameEngine, owner: PlayerId, card: &str) -> ObjectId {
    let id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let object = new_object_from_card(
        id,
        owner,
        card,
        Zone::Battlefield,
        engine.registry.get(card).unwrap().primary_face(),
    );
    engine.state.objects.insert(id, object);
    engine.state.zone_change_generation.insert(id, 0);
    let index = engine.state.player_idx(owner).unwrap();
    engine.state.players[index].battlefield.push(id);
    engine.emit_static_abilities_on_enter(id);
    id
}

fn fixture() -> (GameEngine, ObjectId, ObjectId) {
    fixture_with_target("grizzly_bears")
}
fn fixture_with_target(card: &str) -> (GameEngine, ObjectId, ObjectId) {
    fixture_with_players(card, &[0, 1])
}
fn fixture_with_players(card: &str, players: &[PlayerId]) -> (GameEngine, ObjectId, ObjectId) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        13_040_101,
        players,
        20,
        None,
        true,
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = permanent(&mut engine, 0, "mind_stone");
    let target = permanent(&mut engine, 0, card);
    let mut face = engine
        .registry
        .get("mind_stone")
        .unwrap()
        .primary_face()
        .clone();
    let ability:ActivatedAbilityDef=ron::from_str(r#"(ability_id:"activated_01",presentation:Fallback,costs:[],
        effect:[PutCounters(counter:Ghostform,count:1,subject:Chosen((kind:Creature,token:Some(false)))),
            GrantTriggeredAbility(duration:Indefinite,subject:Chosen((kind:Creature,token:Some(false))),
                ability:(ability_id:"granted_01",presentation:Fallback,
                    trigger:WheneverPermanentLeavesBattlefield(controller:Controller,filter:(source_only:true),destination:OneOf([Graveyard,Exile])),
                    effect:[ReturnTriggeredCard(reference:TriggerObject,from:[Graveyard,Exile],destination:Hand),
                        CreateTokens(token:"spirit_w_1_1_flying",count:1)]))],
        targeting:Some((groups:[(min:0,max:1,prompt:"Choose up to one nontoken creature",effect_indices:[0,1])]))
    )"#).unwrap();
    for effect in &ability.effect {
        effect
            .validate(tricerules_card_model::primitives::EffectContext::Ability)
            .unwrap();
    }
    face.activated_abilities = vec![ability];
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "mind_stone".into(),
        source_face_index: 0,
        display_name: "Ghostform Source Fixture".into(),
        face,
        room_faces: None,
    });
    engine.reconcile_activated_ability_slots();
    (engine, source, target)
}

fn command(
    engine: &mut GameEngine,
    actor: PlayerId,
    cmd: rv1::ruled_command::Cmd,
) -> RuledEventBatch {
    engine
        .apply_command(actor, &RuledCommand { cmd: Some(cmd) })
        .unwrap()
}
fn pass(engine: &mut GameEngine) {
    let actor = engine.state.priority_player_id();
    command(
        engine,
        actor,
        rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {}),
    );
}
fn grant(engine: &mut GameEngine, source: ObjectId, target: ObjectId) {
    begin_grant(engine, source, target);
    resolve_all(engine);
}
fn begin_grant(engine: &mut GameEngine, source: ObjectId, target: ObjectId) {
    command(
        engine,
        0,
        rv1::ruled_command::Cmd::ActivateAbility(rv1::ActivateAbility {
            source_object_id: source,
            expected_zone_change_generation: engine.state.zone_change_generation[&source],
            ability_index: 0,
            targets: vec![rv1::TargetRef {
                object_id: target,
                expected_zone_change_generation: Some(engine.state.zone_change_generation[&target]),
                kind: rv1::TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
            ..Default::default()
        }),
    );
}

#[test]
fn ghostform_runtime_keeps_the_authored_indefinite_grant_separate_from_its_counter() {
    let (mut engine, source, target) = fixture();
    grant(&mut engine, source, target);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Ghostform),
        1
    );
    assert!(
        engine
            .state
            .continuous_effects
            .iter()
            .any(
                |effect| matches!(effect.kind, ContinuousEffectKind::GrantTriggeredAbility(_))
                    && effect.affected == AffectedScope::Single(target)
                    && effect.duration == EffectDuration::Indefinite
            ),
        "indefinite grant must not be hardcoded to end of turn"
    );
}

fn resolve_all(engine: &mut GameEngine) {
    for _ in 0..64 {
        if let Some(order) = engine.state.pending_trigger_order.as_ref() {
            let actor = order.deciding_player;
            let id = order.candidates[0].object_id;
            command(
                engine,
                actor,
                rv1::ruled_command::Cmd::SubmitTriggerOrder(rv1::SubmitTriggerOrder {
                    trigger_object_id: id,
                }),
            );
        } else if engine.state.stack.is_empty() {
            assert!(engine.state.pending_resolution.is_none());
            return;
        } else {
            pass(engine);
        }
    }
    panic!("prepared Ghostform stack did not settle");
}
fn kill(engine: &mut GameEngine, target: ObjectId) {
    engine.state.objects.get_mut(&target).unwrap().damage = 100;
    pass(engine);
}
fn spirits(engine: &GameEngine, player: PlayerId) -> usize {
    let index = engine.state.player_idx(player).unwrap();
    engine.state.players[index]
        .battlefield
        .iter()
        .filter(|id| engine.state.objects[id].card_id == "spirit_w_1_1_flying")
        .count()
}

#[test]
fn ghostform_prevented_counter_still_grants_one_independent_ability() {
    let (mut engine, source, target) = fixture_with_target("tatterkite");
    grant(&mut engine, source, target);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Ghostform),
        0
    );
    kill(&mut engine, target);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_all(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(spirits(&engine, 0), 1);
}

#[test]
fn ghostform_removed_counter_cleanup_and_source_departure_do_not_end_the_grant() {
    let (mut engine, source, target) = fixture();
    grant(&mut engine, source, target);
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::Ghostform, 0);
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        source,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    // Prepared end-step position; use the real cleanup/turn rollover command path.
    let turn = engine.state.turn_instance;
    engine.state.turn_step = TurnStep::EndStep;
    pass(&mut engine);
    pass(&mut engine);
    assert!(engine.state.turn_instance > turn);
    kill(&mut engine, target);
    resolve_all(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(spirits(&engine, 0), 1);
}

#[test]
fn ghostform_two_grants_create_two_spirits_and_return_only_once() {
    let (mut engine, source, target) = fixture();
    grant(&mut engine, source, target);
    grant(&mut engine, source, target);
    let generation = engine.state.zone_change_generation[&target];
    kill(&mut engine, target);
    resolve_all(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(engine.state.zone_change_generation[&target], generation + 2);
    assert_eq!(spirits(&engine, 0), 2);
}

#[test]
fn ghostform_death_replaced_by_exile_triggers_once_and_returns_that_exact_incarnation() {
    let (mut engine, source, target) = fixture();
    grant(&mut engine, source, target);
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::Finality, 1);
    kill(&mut engine, target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_all(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(spirits(&engine, 0), 1);
}

#[test]
fn ghostform_departing_controller_gets_spirit_while_physical_owner_gets_hand_card() {
    let (mut engine, source, target) = fixture();
    grant(&mut engine, source, target);
    engine.state.players[0]
        .battlefield
        .retain(|id| *id != target);
    engine.state.players[1].battlefield.push(target);
    let object = engine.state.objects.get_mut(&target).unwrap();
    object.controller = 1;
    object.base_controller = 1;
    kill(&mut engine, target);
    assert_eq!(engine.state.stack.last().unwrap().controller, 1);
    resolve_all(&mut engine);
    assert!(engine.state.players[0].hand.contains(&target));
    assert!(!engine.state.players[1].hand.contains(&target));
    assert_eq!(spirits(&engine, 0), 0);
    assert_eq!(spirits(&engine, 1), 1);
}

#[test]
fn ghostform_intervening_zone_change_prevents_return_but_not_spirit_creation() {
    let (mut engine, source, target) = fixture();
    grant(&mut engine, source, target);
    kill(&mut engine, target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    for zone in [Zone::Exile, Zone::Graveyard] {
        move_object_to_zone(&mut engine.state, engine.registry, target, zone, None).unwrap();
    }
    resolve_all(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(spirits(&engine, 0), 1);
}

#[test]
fn ghostform_counter_replacement_pause_resumes_one_grant_even_when_placement_becomes_zero() {
    use crate::engine::replacement::{
        CounterPlacementReplacement, CounterReplacementKind, PendingReplacementEvent,
    };
    let (mut engine, source, target) = fixture();
    permanent(&mut engine, 0, "doubling_season");
    permanent(&mut engine, 1, "vorinclex,_monstrous_raider");
    begin_grant(&mut engine, source, target);
    pass(&mut engine);
    pass(&mut engine);
    let Some(PendingReplacementEvent::Counters(batch)) = &engine.state.pending_replacement_event
    else {
        panic!("counter replacement order must park")
    };
    let halving=batch.applications.iter().find(|application|matches!(&application.replacement,
        CounterPlacementReplacement::Battlefield {instance,..} if instance.kind==CounterReplacementKind::HalveCountersOpponentsWouldPut)).unwrap().choice_id;
    let prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    assert!(!engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| matches!(effect.kind, ContinuousEffectKind::GrantTriggeredAbility(_))));
    let answer = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: prompt,
                chosen_object_ids: vec![halving],
                ..Default::default()
            },
        )),
    };
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(1, &answer).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine.apply_command(0, &answer).unwrap();
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Ghostform),
        0
    );
    assert_eq!(
        engine
            .state
            .continuous_effects
            .iter()
            .filter(|effect| effect.affected == AffectedScope::Single(target)
                && matches!(effect.kind, ContinuousEffectKind::GrantTriggeredAbility(_)))
            .count(),
        1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ghostform_zero_targets_does_not_add_a_counter_or_grant() {
    let (mut engine, source, target) = fixture();
    command(
        &mut engine,
        0,
        rv1::ruled_command::Cmd::ActivateAbility(rv1::ActivateAbility {
            source_object_id: source,
            expected_zone_change_generation: 0,
            ability_index: 0,
            ..Default::default()
        }),
    );
    resolve_all(&mut engine);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Ghostform),
        0
    );
    assert!(!engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| matches!(effect.kind, ContinuousEffectKind::GrantTriggeredAbility(_))));
}

#[test]
fn ghostform_external_grant_is_suppressed_by_later_ability_loss() {
    let (mut engine, source, target) = fixture();
    grant(&mut engine, source, target);
    let aura = permanent(&mut engine, 0, "kenriths_transformation");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(target));
    kill(&mut engine, target);
    resolve_all(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(spirits(&engine, 0), 0);
}

#[test]
fn ghostform_copied_creature_does_not_copy_the_external_grant_or_counter() {
    let (mut engine, source, target) = fixture();
    grant(&mut engine, source, target);
    let values =
        crate::engine::copying::copiable_values_from(&engine.state, engine.registry, target)
            .unwrap();
    let copy = permanent(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&copy).unwrap().copiable_values = Some(values);
    assert_eq!(
        engine.state.objects[&copy].counter_count(CounterKind::Ghostform),
        0
    );
    kill(&mut engine, copy);
    resolve_all(&mut engine);
    assert_eq!(engine.state.objects[&copy].zone, Zone::Graveyard);
    assert_eq!(spirits(&engine, 0), 0);
    kill(&mut engine, target);
    resolve_all(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(spirits(&engine, 0), 1);
}

#[test]
fn ghostform_source_owner_leaving_does_not_remove_a_surviving_creatures_grant() {
    let (mut engine, source, target) = fixture_with_players("grizzly_bears", &[0, 1, 2]);
    engine.state.players[0]
        .battlefield
        .retain(|id| *id != target);
    engine.state.players[1].battlefield.push(target);
    let object = engine.state.objects.get_mut(&target).unwrap();
    object.owner = 1;
    object.controller = 1;
    object.base_controller = 1;
    grant(&mut engine, source, target);
    command(
        &mut engine,
        0,
        rv1::ruled_command::Cmd::Concede(rv1::Concede {}),
    );
    assert!(!engine.state.is_terminal());
    kill(&mut engine, target);
    resolve_all(&mut engine);
    assert!(engine.state.players[1].hand.contains(&target));
    assert_eq!(spirits(&engine, 1), 1);
}

#[test]
fn ghostform_returned_creature_is_a_new_object_without_the_old_grant() {
    let (mut engine, source, target) = fixture();
    grant(&mut engine, source, target);
    kill(&mut engine, target);
    resolve_all(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(spirits(&engine, 0), 1);
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        target,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    kill(&mut engine, target);
    resolve_all(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(
        spirits(&engine, 0),
        1,
        "old incarnation's grant must not return or create again"
    );
}
