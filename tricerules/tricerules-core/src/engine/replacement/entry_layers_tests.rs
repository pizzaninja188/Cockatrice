use super::*;

fn engine() -> GameEngine {
    let mut engine = GameEngine::new_with_default_decks(305_030, &[0, 1], 20).unwrap();
    engine.state.opening = None;
    engine.state.priority_idx = 0;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.command_index = 5;
    engine
}

fn object(engine: &mut GameEngine, card: &str, zone: Zone, controller: PlayerId) -> ObjectId {
    let oid = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let mut object = engine.state.objects.values().next().unwrap().clone();
    object.id = oid;
    object.card_id = card.into();
    object.zone = zone;
    object.controller = controller;
    object.base_controller = controller;
    object.owner = controller;
    object.face_up_index = 0;
    object.face_down = false;
    object.copiable_values = None;
    object.token_origin = None;
    object.token_faces = None;
    object.power = None;
    object.toughness = None;
    engine.state.objects.insert(oid, object);
    if zone == Zone::Battlefield {
        engine.state.players[controller as usize]
            .battlefield
            .push(oid);
    }
    oid
}

fn event(engine: &GameEngine, oid: ObjectId) -> BattlefieldEntryEvent {
    BattlefieldEntryEvent {
        mana_colors_spent_to_cast: Default::default(),
        prepared: false,
        object_id: oid,
        deciding_player: 0,
        destination_controller: 0,
        battle_protector: None,
        face_index: 0,
        unlock_room_door: None,
        chosen_x: 0,
        cast_by: None,
        cast_cost_receipts: Vec::new(),
        player_life_snapshot: engine.player_life_snapshot(),
        tapped: false,
        set_types: None,
        chosen_basic_land_type: None,
        entry_counters: BTreeMap::new(),
        entry_modifiers: Vec::new(),
        attached_to: None,
        pending_copy_candidate: None,
        pending_aura_recipient: None,
        applied_effects: Vec::new(),
    }
}

fn effect(engine: &mut GameEngine, affected: AffectedScope, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected,
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: 1,
    });
}

fn land_scope(controller: tricerules_cards::primitives::TargetController) -> AffectedScope {
    AffectedScope::PermanentsMatching {
        reference_player: 1,
        exclude: None,
        filter: Box::new(tricerules_cards::primitives::TargetFilter {
            kind: tricerules_cards::primitives::TargetKind::AnyPermanent,
            controller,
            permanent_types: vec![PermanentTypeFilter::Land],
            ..Default::default()
        }),
    }
}

fn land() -> tricerules_cards::TypeLineReplacement {
    tricerules_cards::TypeLineReplacement {
        card_types: vec![PermanentTypeFilter::Land],
        creature_types: Vec::new(),
        land_types: Vec::new(),
    }
}

#[test]
fn entry_early_layers_land_making_precedes_existing_land_scope_in_both_forms() {
    for replace in [false, true] {
        let mut engine = engine();
        let oid = object(&mut engine, "hill_giant", Zone::Stack, 0);
        effect(
            &mut engine,
            land_scope(tricerules_cards::primitives::TargetController::Any),
            ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
                land_types: vec![BasicLandType::Forest],
                ..Default::default()
            }),
        );
        let mut entry = event(&engine, oid);
        entry.entry_modifiers = vec![if replace {
            ResolvingPermanentModifier::SetTypeLine(land())
        } else {
            ResolvingPermanentModifier::AddTypes(tricerules_cards::TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Land],
                ..Default::default()
            })
        }];
        let preview = engine
            .battlefield_entry_characteristics_through_layer_5(&entry)
            .unwrap();
        assert!(
            preview.has_type("Forest"),
            "new land scope must see the projected entrant"
        );
        engine.commit_battlefield_entry(entry, None).unwrap();
        assert_eq!(
            engine.characteristics_through_layer_5(oid).unwrap(),
            preview
        );
    }
}

#[test]
fn entry_early_layers_destination_controller_is_visible_to_land_scope() {
    let mut engine = engine();
    let oid = object(&mut engine, "forest", Zone::Stack, 0);
    effect(
        &mut engine,
        land_scope(tricerules_cards::primitives::TargetController::You),
        ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Artifact],
            ..Default::default()
        }),
    );
    let mut entry = event(&engine, oid);
    entry.destination_controller = 1;
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.has_type("Artifact"));
    engine.commit_battlefield_entry(entry, None).unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_face_down_values_precede_type_and_color_effects() {
    let mut engine = engine();
    let oid = object(&mut engine, "hill_giant", Zone::Stack, 0);
    engine.state.objects.get_mut(&oid).unwrap().face_down = true;
    effect(
        &mut engine,
        land_scope(tricerules_cards::primitives::TargetController::Any),
        ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
    );
    let mut entry = event(&engine, oid);
    entry.set_types = Some(land());
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.names.is_empty());
    assert_eq!(preview.colors, [Color::Blue]);
    assert_eq!(preview.power, Some(2));
    engine.commit_battlefield_entry(entry, None).unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_old_single_effect_is_not_carried_to_new_incarnation() {
    let mut engine = engine();
    let oid = object(&mut engine, "forest", Zone::Graveyard, 0);
    effect(
        &mut engine,
        AffectedScope::Single(oid),
        ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
    );
    let entry = event(&engine, oid);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.colors.is_empty());
    engine.commit_battlefield_entry(entry, None).unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_selected_face_and_copied_own_static_are_used() {
    let mut engine = engine();
    let oid = object(
        &mut engine,
        "cragcrown_pathway_timbercrown_pathway",
        Zone::Hand,
        0,
    );
    let mut entry = event(&engine, oid);
    entry.face_index = 1;
    assert_eq!(
        engine
            .battlefield_entry_characteristics_through_layer_5(&entry)
            .unwrap()
            .names,
        ["Timbercrown Pathway"]
    );
    let source = r#"(id: "own_entry_land", name: "Own Entry Land", face_id: "own_entry_land", types: ["Land"],
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: ConditionalSelfModifier(condition: ActivePlayer(players: Controller),
                add_types: (card_types: [Artifact])))])"#;
    let face = CardRegistry::from_chunks_and_tokens(&[source], &[])
        .unwrap()
        .get("own_entry_land")
        .unwrap()
        .primary_face()
        .clone();
    engine.state.objects.get_mut(&oid).unwrap().copiable_values = Some(CopiableValues {
        source_card_id: "own_entry_land".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    let mut entry = event(&engine, oid);
    assert!(engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap()
        .has_type("Artifact"));
    // CR 613.7n: entrant-owned statics precede same-timestamp resolving entry effects.
    entry.set_types = Some(land());
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(!preview.has_type("Artifact"));
    engine.commit_battlefield_entry(entry, None).unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_chosen_basic_and_type_override_have_commit_order() {
    let mut engine = engine();
    let oid = object(&mut engine, "forest", Zone::Stack, 0);
    let mut entry = event(&engine, oid);
    entry.chosen_basic_land_type = Some(BasicLandType::Island);
    entry.set_types = Some(land());
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    engine.commit_battlefield_entry(entry, None).unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_provisional_entrant_does_not_satisfy_battlefield_count() {
    let mut engine = engine();
    let oid = object(&mut engine, "forest", Zone::Stack, 0);
    let source = r#"(id: "count_entry_land", name: "Count Entry Land", face_id: "count_entry_land", types: ["Land"],
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: ConditionalSelfModifier(condition: BattlefieldAggregate(
                filter: (controllers: All, card_type: Some(Land)), aggregate: Count, min: Some(1)),
                add_types: (card_types: [Artifact])))])"#;
    let face = CardRegistry::from_chunks_and_tokens(&[source], &[])
        .unwrap()
        .get("count_entry_land")
        .unwrap()
        .primary_face()
        .clone();
    engine.state.objects.get_mut(&oid).unwrap().copiable_values = Some(CopiableValues {
        source_card_id: "count_entry_land".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    let entry = event(&engine, oid);
    assert!(!engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap()
        .has_type("Artifact"));
    let existing = object(&mut engine, "forest", Zone::Battlefield, 1);
    assert!(engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap()
        .has_type("Artifact"));
    engine.state.players[1]
        .battlefield
        .retain(|&candidate| candidate != existing);
    engine.state.objects.get_mut(&existing).unwrap().zone = Zone::Graveyard;
    assert!(!engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap()
        .has_type("Artifact"));
}

#[test]
fn entry_early_layers_type_setting_suppresses_intrinsic_entry_replacements() {
    let mut engine = engine();
    let oid = object(&mut engine, "fetid_pools", Zone::Stack, 0);
    let mut entry = event(&engine, oid);
    assert!(!engine.battlefield_entry_candidates(&entry).is_empty());
    let mut types = land();
    types.land_types = vec![BasicLandType::Forest];
    entry.set_types = Some(types);
    assert!(
        engine.battlefield_entry_candidates(&entry).is_empty(),
        "the resulting Forest has no printed enters-tapped replacement"
    );
    entry.set_types = None;
    assert!(!engine.battlefield_entry_candidates(&entry).is_empty());
}

fn copy_face(engine: &mut GameEngine, oid: ObjectId, source: &str, card: &str) {
    let face = CardRegistry::from_chunks_and_tokens(&[source], &[])
        .unwrap()
        .get(card)
        .unwrap()
        .primary_face()
        .clone();
    engine.state.objects.get_mut(&oid).unwrap().copiable_values = Some(CopiableValues {
        source_card_id: card.into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
}

#[test]
fn entry_early_layers_own_global_static_is_restricted_to_entrant_during_preview() {
    let mut engine = engine();
    let existing = object(&mut engine, "forest", Zone::Battlefield, 1);
    let oid = object(&mut engine, "forest", Zone::Stack, 0);
    let source = r#"(id: "global_entry_land", name: "Global Entry Land", face_id: "global_entry_land", types: ["Land"],
        static_abilities: [
            (ability_id: "static_01", presentation: Fallback, definition: AddTypesToPermanents(
                filter: (kind: AnyPermanent, permanent_types: [Land]), addition: (card_types: [Artifact]))),
            (ability_id: "static_02", presentation: Fallback, definition: ConditionalSelfModifier(
                condition: BattlefieldAggregate(filter: (controllers: All, card_type: Some(Artifact)),
                    aggregate: Count, min: Some(1)), add_types: (card_types: [Enchantment])))])"#;
    copy_face(&mut engine, oid, source, "global_entry_land");
    let entry = event(&engine, oid);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.has_type("Artifact"));
    assert!(
        !preview.has_type("Enchantment"),
        "own global statics cannot alter existing permanents during entry preview"
    );
    assert!(!engine
        .characteristics(existing)
        .unwrap()
        .has_type("Artifact"));
    engine.commit_battlefield_entry(entry, None).unwrap();
    assert!(engine
        .characteristics(existing)
        .unwrap()
        .has_type("Artifact"));
    assert!(engine.characteristics(oid).unwrap().has_type("Enchantment"));
}

#[test]
fn entry_early_layers_own_conditions_use_projected_counters_and_tapped_status() {
    let mut engine = engine();
    let oid = object(&mut engine, "forest", Zone::Stack, 0);
    let source = r#"(id: "condition_entry_land", name: "Condition Entry Land", face_id: "condition_entry_land", types: ["Land"],
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: ConditionalSelfModifier(condition: AllOf([
                SourceCounterCount(counter: Charge, min: Some(2)), ObjectTapped(object: Source, tapped: true)]),
                add_types: (card_types: [Artifact])))])"#;
    copy_face(&mut engine, oid, source, "condition_entry_land");
    let mut entry = event(&engine, oid);
    entry.tapped = true;
    entry.entry_counters.insert(CounterKind::Charge, 2);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.has_type("Artifact"));
    engine.commit_battlefield_entry(entry, None).unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_room_door_statics_and_locked_copy_match_commitment() {
    let mut engine = engine();
    let source = r#"(id: "first_room_second_room", name: "First Room // Second Room", layout: Room, faces: [
        (name: "First Room", face_id: "first_room", mana_cost: "{1}{U}", types: ["Enchantment", "Room"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ConditionalSelfModifier(
                condition: ActivePlayer(players: Controller), add_types: (card_types: [Artifact])))]),
        (name: "Second Room", face_id: "second_room", mana_cost: "{2}{R}", types: ["Enchantment", "Room"])])"#;
    engine.registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(&[source], &[]).unwrap(),
    ));
    let oid = object(&mut engine, "first_room_second_room", Zone::Stack, 0);
    let mut entry = event(&engine, oid);
    entry.unlock_room_door = Some(0);
    let raw = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert_eq!(raw.names, ["First Room"]);
    assert_eq!(raw.colors, [Color::Blue]);
    assert_eq!(raw.mana_value, 2);
    assert!(raw.has_type("Artifact"));
    entry.set_types = Some(land());
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(!preview.has_type("Artifact"));
    engine.commit_battlefield_entry(entry, None).unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
    let own = engine
        .state
        .continuous_effects
        .iter()
        .filter(|effect| effect.source_id == Some(oid) && effect.trigger_grant_origin.is_some())
        .collect::<Vec<_>>();
    assert_eq!(own.len(), 1);
    let Some(TriggerAbilityOrigin::StaticGrant {
        source_zone_change,
        definition,
        ..
    }) = &own[0].trigger_grant_origin
    else {
        panic!("door provenance");
    };
    assert_eq!(
        *source_zone_change,
        engine.state.zone_change_generation[&oid]
    );
    assert_eq!(definition.face_id.as_str(), "first_room");
    let copy = object(&mut engine, "first_room_second_room", Zone::Stack, 0);
    let card = engine.registry.get("first_room_second_room").unwrap();
    engine.state.objects.get_mut(&copy).unwrap().copiable_values = Some(CopiableValues {
        source_card_id: card.id.clone(),
        source_face_index: 0,
        display_name: card.name.clone(),
        face: card.primary_face().clone(),
        room_faces: Some(card.faces.clone()),
    });
    let mut entry = event(&engine, copy);
    entry.unlock_room_door = Some(0);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.names.is_empty() && preview.colors.is_empty());
    assert_eq!(preview.mana_value, 0);
    assert!(!preview.has_type("Artifact"));
    engine.commit_battlefield_entry(entry, None).unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(copy).unwrap(),
        preview
    );
    assert!(!engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| effect.source_id == Some(copy) && effect.trigger_grant_origin.is_some()));
}

#[test]
fn entry_early_layers_kaito_uses_entry_loyalty_before_commitment() {
    let mut engine = engine();
    let oid = object(&mut engine, "kaito,_bane_of_nightmares", Zone::Stack, 0);
    assert_eq!(
        engine.state.objects[&oid].counter_count(CounterKind::Loyalty),
        0
    );
    let mut entry = event(&engine, oid);
    entry.entry_counters.insert(CounterKind::Loyalty, 4);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.is_creature() && preview.has_type("Ninja"));
    assert!(!preview.has_type("Planeswalker"));
    engine.commit_battlefield_entry(entry, None).unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}
