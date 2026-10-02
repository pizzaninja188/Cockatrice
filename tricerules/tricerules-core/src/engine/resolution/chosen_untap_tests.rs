//! Generation-bound chosen cohort and simultaneous untap replacement/prohibition boundary.
use super::*;

fn permanent(engine: &mut GameEngine, player: PlayerId, card: &str) -> ObjectId {
    let oid = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let mut object = new_object_from_card(
        oid,
        player,
        card,
        Zone::Battlefield,
        engine.registry.get(card).unwrap().primary_face(),
    );
    object.tapped = true;
    engine.state.objects.insert(oid, object);
    engine.state.zone_change_generation.insert(oid, 0);
    let index = engine.state.player_idx(player).unwrap();
    engine.state.players[index].battlefield.push(oid);
    engine.emit_static_abilities_on_enter(oid);
    oid
}

fn receipt(engine: &GameEngine, object_id: ObjectId) -> TriggerObjectRef {
    TriggerObjectRef {
        object_id,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
        controller_at_event: engine.state.objects[&object_id].controller,
    }
}

fn consume(engine: &mut GameEngine, chosen: Vec<TriggerObjectRef>) {
    let top = StackItem {
        mana_colors_spent_to_cast: Default::default(),
        id: u32::MAX,
        controller: 0,
        card_id: "frantic_search".into(),
        targets: vec![],
        ability_text: None,
        source_permanent_id: None,
        source_owner: None,
        source_zone_change: 0,
        source_face_change: 0,
        ability_index: None,
        activated_ability: None,
        triggered_ability: None,
        is_triggered: false,
        is_copy: false,
        face_index: 0,
        cast_method: SpellCastMethod::Normal,
        returned_attacker_assignment: None,
        chosen_x: 0,
        chosen_modes: vec![],
        cast_condition_results: vec![],
        cast_occurrence: None,
        cast_by: None,
        cast_cost_receipts: vec![],
        payment_result: CardResultCohort::default(),
        search_results: Default::default(),
        resolution_branch_choices: Default::default(),
        blight_receipts: vec![],
        trigger_context: TriggerContext::default(),
    };
    let previous = EffectResult {
        produced_objects: chosen,
        ..Default::default()
    };
    let mut result = EffectResult::default();
    let mut events = Vec::new();
    let mut cx = EffectCx {
        engine,
        events: &mut events,
        targets: &[],
        targets_by_role: &[],
        target_damage: &[],
        target_group_indices: &[],
        top: &top,
        controller: 0,
        affected_player: 0,
        spell_label: "Frantic Search",
        previous_effect_result: &previous,
        effect_result: &mut result,
        effect_index: 2,
    };
    assert_eq!(
        mass::untap_chosen_permanents(&mut cx).unwrap(),
        EffectOutcome::Continue
    );
}

fn prohibition_is_active(engine: &GameEngine, oid: ObjectId) -> bool {
    let snapshot = engine.characteristics(oid).unwrap();
    engine.state.continuous_effects.iter().any(|effect| {
        effect.kind == ContinuousEffectKind::ProhibitUntap
            && super::super::characteristics::effect_affects(
                &engine.state,
                engine.registry,
                effect,
                oid,
                &snapshot,
            )
    })
}

#[test]
fn chosen_untaps_prepare_the_entire_cohort_before_conditional_prohibitions_change() {
    let mut engine = GameEngine::new(26_100_704, &[0, 1], 20, None, true).unwrap();
    let first = permanent(&mut engine, 0, "forest");
    let second = permanent(&mut engine, 1, "forest");
    let aura = permanent(&mut engine, 0, "indestructibility");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(second));
    let mut copied = engine.copiable_values_for(aura).unwrap();
    let StaticAbilityDef::AttachedModifier {
        keywords,
        cant_untap,
        ..
    } = &mut copied.face.static_abilities[0].definition
    else {
        panic!("actual Aura modifier")
    };
    keywords.clear();
    *cant_untap = true;
    engine.state.objects.get_mut(&aura).unwrap().copiable_values = Some(copied);
    engine.refresh_source_static_abilities(aura);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(first),
        affected: AffectedScope::Single(aura),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: Some(GameCondition::ObjectTapped {
            object: ConditionObjectRef::Source,
            tapped: true,
        }),
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    assert!(!prohibition_is_active(&engine, second));
    let selected = vec![receipt(&engine, first), receipt(&engine, second)];
    consume(&mut engine, selected);
    assert!(!engine.state.objects[&first].tapped);
    assert!(!engine.state.objects[&second].tapped);
    assert_eq!(engine.state.untapped_this_command, [first, second]);
    assert!(prohibition_is_active(&engine, second));
}

#[test]
fn chosen_untap_skips_stale_incarnations_and_off_battlefield_receipts() {
    let mut engine = GameEngine::new(26_100_705, &[0, 1], 20, None, true).unwrap();
    let returned = permanent(&mut engine, 0, "forest");
    let departed = permanent(&mut engine, 1, "forest");
    let old = receipt(&engine, returned);
    for (oid, zone) in [
        (returned, Zone::Graveyard),
        (returned, Zone::Battlefield),
        (departed, Zone::Graveyard),
    ] {
        move_object_to_zone(&mut engine.state, engine.registry, oid, zone, None).unwrap();
    }
    engine.state.objects.get_mut(&returned).unwrap().tapped = true;
    engine
        .state
        .objects
        .get_mut(&returned)
        .unwrap()
        .set_counter(CounterKind::Stun, 2);
    let outside = receipt(&engine, departed);
    consume(&mut engine, vec![old, outside]);
    assert!(engine.state.objects[&returned].tapped);
    assert_eq!(
        engine.state.objects[&returned].counter_count(CounterKind::Stun),
        2
    );
    assert!(engine.state.untapped_this_command.is_empty());
    let fresh = receipt(&engine, returned);
    consume(&mut engine, vec![fresh]);
    assert!(engine.state.objects[&returned].tapped);
    assert_eq!(
        engine.state.objects[&returned].counter_count(CounterKind::Stun),
        1
    );
}

#[test]
fn chosen_untap_empty_and_missing_objects_preserve_unselected_lands() {
    let mut engine = GameEngine::new(26_100_706, &[0, 1], 20, None, true).unwrap();
    let selected = permanent(&mut engine, 1, "forest");
    let unselected = permanent(&mut engine, 0, "forest");
    consume(&mut engine, vec![]);
    assert!(engine.state.objects[&selected].tapped);
    assert!(engine.state.objects[&unselected].tapped);
    assert!(engine.state.untapped_this_command.is_empty());
    let valid = receipt(&engine, selected);
    let missing = TriggerObjectRef {
        object_id: engine.state.next_object_id,
        zone_change_generation: 0,
        controller_at_event: 0,
    };
    consume(&mut engine, vec![missing, valid]);
    assert!(!engine.state.objects[&selected].tapped);
    assert!(engine.state.objects[&unselected].tapped);
    assert_eq!(engine.state.untapped_this_command, [selected]);
}
