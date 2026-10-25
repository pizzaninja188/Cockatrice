//! Authored selection filters and the atomic typed count result for chosen permanent taps.
use super::*;
use tricerules_card_model::Cardinality;

fn permanent(engine: &mut GameEngine, player: PlayerId, card: &str) -> ObjectId {
    let oid = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let object = new_object_from_card(
        oid,
        player,
        card,
        Zone::Battlefield,
        engine.registry.get(card).unwrap().primary_face(),
    );
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

fn land_filter() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller: TargetController::You,
        permanent_types: vec![PermanentTypeFilter::Land],
        tapped: Some(false),
        ..TargetFilter::default()
    }
}

fn test_stack_item() -> StackItem {
    StackItem {
        prototyped: false,
        mana_colors_spent_to_cast: Default::default(),
        id: u32::MAX,
        controller: 0,
        card_id: "frantic_search".into(),
        targets: Vec::new(),
        ability_text: None,
        source_permanent_id: None,
        source_owner: None,
        source_zone_change: 0,
        trigger_use_key: None,
        source_face_change: 0,
        ability_index: None,
        activated_ability: None,
        activated_ability_occurrence: None,
        triggered_ability: None,
        is_triggered: false,
        is_copy: false,
        face_index: 0,
        cast_method: SpellCastMethod::Normal,
        returned_attacker_assignment: None,
        chosen_x: 0,
        chosen_modes: Vec::new(),
        cast_condition_results: Vec::new(),
        cast_occurrence: None,
        cast_by: None,
        cast_cost_receipts: Vec::new(),
        payment_result: CardResultCohort::default(),
        exiled_cohorts: Default::default(),
        chaos_warp_owner_instructions: Default::default(),
        resolution_branch_choices: Default::default(),
        blight_receipts: Vec::new(),
        trigger_context: TriggerContext::default(),
    }
}

fn resolve_tap(
    engine: &mut GameEngine,
    selected: Vec<TriggerObjectRef>,
    filter: TargetFilter,
) -> Result<(EffectResult, Vec<rv1::RuledEvent>), EngineError> {
    resolve_tap_with_cardinality(
        engine,
        selected,
        filter,
        Cardinality {
            min: 0,
            max: Some(u32::MAX),
        },
    )
}

fn resolve_tap_with_cardinality(
    engine: &mut GameEngine,
    selected: Vec<TriggerObjectRef>,
    filter: TargetFilter,
    cardinality: Cardinality,
) -> Result<(EffectResult, Vec<rv1::RuledEvent>), EngineError> {
    let top = test_stack_item();
    let previous = EffectResult {
        input_result_type: Some(ResultType::PermanentSelection {
            chooser: PlayerRecipient::Controller,
            filter: Box::new(filter),
            cardinality,
        }),
        produced_objects: selected,
        ..EffectResult::default()
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
        effect_index: 1,
        resume_previous_result: None,
        program_state: None,
    };
    mass::tap_chosen_permanents(&mut cx)?;
    drop(cx);
    Ok((result, events))
}

fn choose_permanents(
    engine: &mut GameEngine,
    min: u32,
    max: u32,
) -> Result<(EffectResult, Vec<rv1::RuledEvent>, EffectOutcome), EngineError> {
    let top = test_stack_item();
    let previous = EffectResult::default();
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
        effect_index: 0,
        resume_previous_result: None,
        program_state: None,
    };
    let outcome = choices::choose_permanents(
        &mut cx,
        SpellEffectKind::ChoosePermanents {
            chooser: PlayerRecipient::Controller,
            filter: land_filter(),
            min,
            max,
            constraints: Vec::new(),
        },
    )?;
    drop(cx);
    Ok((result, events, outcome))
}

#[test]
fn chosen_tap_checks_the_authored_filter_before_mutating_any_selection() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_801,
        &[0, 1],
        20,
        None,
        true,
    )
    .unwrap();
    let land = permanent(&mut engine, 0, "forest");
    let nonland = permanent(&mut engine, 0, "myr_battlesphere");
    let selected = vec![receipt(&engine, land), receipt(&engine, nonland)];

    assert!(matches!(
        resolve_tap(&mut engine, selected, land_filter()),
        Err(EngineError::Illegal(_))
    ));
    assert!(!engine.state.objects[&land].tapped);
    assert!(!engine.state.objects[&nonland].tapped);
    assert!(engine.state.untapped_this_command.is_empty());
}

#[test]
fn chosen_tap_rejects_a_stale_generation_before_tapping_the_valid_prefix() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_802,
        &[0, 1],
        20,
        None,
        true,
    )
    .unwrap();
    let first = permanent(&mut engine, 0, "forest");
    let returned = permanent(&mut engine, 0, "forest");
    let selected = vec![receipt(&engine, first), receipt(&engine, returned)];
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        returned,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        returned,
        Zone::Battlefield,
        None,
    )
    .unwrap();

    assert!(matches!(
        resolve_tap(&mut engine, selected, land_filter()),
        Err(EngineError::Illegal(_))
    ));
    assert!(!engine.state.objects[&first].tapped);
    assert!(!engine.state.objects[&returned].tapped);
    assert!(engine.state.untapped_this_command.is_empty());
}

#[test]
fn chosen_tap_rejects_same_generation_permanent_that_left_the_battlefield_atomically() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_804,
        &[0, 1],
        20,
        None,
        true,
    )
    .unwrap();
    let first = permanent(&mut engine, 0, "forest");
    let off_battlefield = permanent(&mut engine, 0, "forest");
    let selected = vec![receipt(&engine, first), receipt(&engine, off_battlefield)];
    let generation = engine.state.zone_change_generation[&off_battlefield];
    let player_index = engine.state.player_idx(0).unwrap();
    engine.state.players[player_index]
        .battlefield
        .retain(|object_id| *object_id != off_battlefield);
    engine.state.objects.get_mut(&off_battlefield).unwrap().zone = Zone::Graveyard;

    assert_eq!(
        engine.state.zone_change_generation[&off_battlefield], generation,
        "this fixture isolates the current-zone check from generation staleness"
    );
    assert!(matches!(
        resolve_tap(&mut engine, selected, land_filter()),
        Err(EngineError::Illegal(_))
    ));
    assert!(!engine.state.objects[&first].tapped);
    assert!(!engine.state.objects[&off_battlefield].tapped);
    assert!(engine.state.untapped_this_command.is_empty());
}

#[test]
fn chosen_tap_rejects_a_selection_above_its_typed_max_before_any_tap() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_805,
        &[0, 1],
        20,
        None,
        true,
    )
    .unwrap();
    let first = permanent(&mut engine, 0, "forest");
    let second = permanent(&mut engine, 0, "forest");
    let selected = vec![receipt(&engine, first), receipt(&engine, second)];

    assert!(matches!(
        resolve_tap_with_cardinality(
            &mut engine,
            selected,
            land_filter(),
            Cardinality {
                min: 0,
                max: Some(1),
            },
        ),
        Err(EngineError::Illegal(_))
    ));
    assert!(!engine.state.objects[&first].tapped);
    assert!(!engine.state.objects[&second].tapped);
    assert!(engine.state.untapped_this_command.is_empty());
}

#[test]
fn chosen_tap_publishes_a_zero_count_without_a_zero_effect_event() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_803,
        &[0, 1],
        20,
        None,
        true,
    )
    .unwrap();
    let (result, events) = resolve_tap(&mut engine, Vec::new(), land_filter()).unwrap();
    assert!(matches!(result.program_value, Some(ProgramValue::Count(0))));
    assert!(events.is_empty());
}

#[test]
fn mandatory_selection_prompts_for_one_and_candidate_shortage_feeds_zero_count() {
    let mut enough = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_806,
        &[0, 1],
        20,
        None,
        true,
    )
    .unwrap();
    let first = permanent(&mut enough, 0, "forest");
    let second = permanent(&mut enough, 0, "forest");
    let (_result, prompt_events, outcome) = choose_permanents(&mut enough, 1, 2).unwrap();
    assert_eq!(outcome, EffectOutcome::Suspended);
    let prompt = prompt_events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(prompt)) => Some(prompt),
            _ => None,
        })
        .expect("two eligible lands produce the mandatory selection prompt");
    assert_eq!((prompt.min, prompt.max), (1, 2));
    assert_eq!(prompt.candidate_object_ids, vec![first, second]);

    let selected = receipt(&enough, first);
    let (tapped_result, _) = resolve_tap_with_cardinality(
        &mut enough,
        vec![selected],
        land_filter(),
        Cardinality {
            min: 0,
            max: Some(2),
        },
    )
    .unwrap();
    assert!(matches!(
        tapped_result.program_value,
        Some(ProgramValue::Count(1))
    ));
    assert!(enough.state.objects[&first].tapped);
    assert!(!enough.state.objects[&second].tapped);

    let mut shortage = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_807,
        &[0, 1],
        20,
        None,
        true,
    )
    .unwrap();
    let (empty_result, shortage_events, outcome) = choose_permanents(&mut shortage, 1, 2).unwrap();
    assert_eq!(outcome, EffectOutcome::Continue);
    assert!(shortage.state.pending_resolution.is_none());
    assert!(empty_result.produced_objects.is_empty());
    assert!(!shortage_events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
    )));

    let (empty_count, tap_events) = resolve_tap_with_cardinality(
        &mut shortage,
        empty_result.produced_objects,
        land_filter(),
        Cardinality {
            min: 0,
            max: Some(2),
        },
    )
    .unwrap();
    assert!(matches!(
        empty_count.program_value,
        Some(ProgramValue::Count(0))
    ));
    assert!(tap_events.is_empty());
}
