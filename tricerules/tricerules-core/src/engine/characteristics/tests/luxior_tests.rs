use super::*;
use tricerules_card_model::CounterKind;
use tricerules_cards::CardRegistry;

const LUXIOR_RUNTIME_FIXTURE: &str = r#"(
    id: "luxior_runtime_fixture", name: "Luxior Runtime Fixture", face_id: "luxior_runtime_fixture",
    mana_cost: "{1}", types: ["Artifact", "Equipment"], supertypes: ["Legendary"],
    static_abilities: [
        (ability_id: "static_01", presentation: Fallback,
            definition: AttachedModifier(
                count: Some(AttachedObjectCounterCount),
                power_per_match: 1, toughness_per_match: 1)),
        (ability_id: "static_02", presentation: Fallback,
            definition: AttachedModifier(
                add_types: (card_types: [Creature]), remove_planeswalker: true)),
    ],
    activated_abilities: [
        (ability_id: "activated_01", presentation: Fallback,
            costs: [Mana("{3}")],
            effect: [Equip(target: (kind: Creature, controller: You))]),
        (ability_id: "activated_02", presentation: Fallback,
            costs: [Mana("{1}")],
            effect: [EquipPlaneswalker(target:
                (kind: AnyPermanent, controller: You, permanent_types: [Planeswalker]))]),
    ],
)"#;

const LUXIOR_PLANESWALKER_FIXTURE: &str = r#"(
    id: "luxior_planeswalker_fixture", name: "Luxior Planeswalker Fixture",
    face_id: "luxior_planeswalker_fixture",
    types: ["Planeswalker", "Jace", "Garruk", "Artifact", "Equipment",
        "Enchantment", "Land", "Forest", "Kindred", "Elf"],
    supertypes: ["Legendary"],
)"#;

fn luxior_characteristics_fixture() -> (GameEngine, ObjectId, ObjectId) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[LUXIOR_RUNTIME_FIXTURE, LUXIOR_PLANESWALKER_FIXTURE],
        &[],
    )
    .expect("isolated Luxior runtime fixtures are admitted");
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        610_301,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new engine");
    engine.registry = Box::leak(Box::new(registry));
    let luxior = insert_fixture(&mut engine, 0, "luxior_runtime_fixture", Zone::Battlefield);
    let planeswalker = insert_fixture(
        &mut engine,
        0,
        "luxior_planeswalker_fixture",
        Zone::Battlefield,
    );
    {
        let object = engine.state.objects.get_mut(&planeswalker).unwrap();
        object.power = None;
        object.toughness = None;
    }
    engine.state.zone_change_generation.insert(luxior, 1);
    engine.state.zone_change_generation.insert(planeswalker, 1);
    engine.emit_static_abilities_on_enter(luxior);
    engine.state.objects.get_mut(&luxior).unwrap().attached_to =
        Some(AttachmentRecipient::Object(planeswalker));
    (engine, luxior, planeswalker)
}

#[test]
fn luxior_attached_type_static_adds_creature_and_selectively_removes_planeswalker_types() {
    let (engine, _luxior, planeswalker) = luxior_characteristics_fixture();
    let characteristics = engine
        .characteristics(planeswalker)
        .expect("the attached planeswalker has characteristics");

    assert!(characteristics.is_creature());
    assert!(!characteristics.has_type("Planeswalker"));
    assert!(!characteristics.has_type("Jace"));
    assert!(!characteristics.has_type("Garruk"));
    for retained in [
        "Artifact",
        "Equipment",
        "Enchantment",
        "Land",
        "Forest",
        "Kindred",
        "Elf",
    ] {
        assert!(
            characteristics.has_type(retained),
            "unrelated or still-supported type {retained} remains"
        );
    }
    assert!(characteristics.is_legendary());
}

#[test]
fn luxior_attached_counter_count_is_live_and_includes_non_pt_counters() {
    let (mut engine, _luxior, planeswalker) = luxior_characteristics_fixture();
    let timestamp = engine.state.command_index;
    let object = engine.state.objects.get_mut(&planeswalker).unwrap();
    for (kind, count) in [
        (CounterKind::Loyalty, 2),
        (CounterKind::Charge, 1),
        (CounterKind::PlusOnePlusOne, 1),
        (CounterKind::MinusOneMinusOne, 1),
    ] {
        object.counters.insert(kind, count);
        object.counter_timestamps.insert(kind, timestamp);
    }

    assert_eq!(engine.effective_power(planeswalker), Some(5));
    assert_eq!(engine.effective_toughness(planeswalker), Some(5));

    let object = engine.state.objects.get_mut(&planeswalker).unwrap();
    object.counters.remove(&CounterKind::Charge);
    object.counter_timestamps.remove(&CounterKind::Charge);
    assert_eq!(engine.effective_power(planeswalker), Some(4));
    assert_eq!(engine.effective_toughness(planeswalker), Some(4));

    let object = engine.state.objects.get_mut(&planeswalker).unwrap();
    object.counters.insert(CounterKind::Loyalty, u32::MAX);
    object
        .counter_timestamps
        .insert(CounterKind::Loyalty, timestamp);
    object.counters.insert(CounterKind::Charge, u32::MAX);
    object
        .counter_timestamps
        .insert(CounterKind::Charge, timestamp);
    assert_eq!(engine.effective_power(planeswalker), Some(u32::MAX));
    assert_eq!(engine.effective_toughness(planeswalker), Some(u32::MAX));
}

fn luxior_early_and_full_types(engine: &GameEngine, object: ObjectId) -> Characteristics {
    let early_view = CharacteristicsEvaluator {
        state: &engine.state,
        registry: engine.registry,
    }
    .evaluate_early_layers(object, true)
    .expect("the prepared battlefield object has an early-layer view");
    let early = &early_view
        .objects
        .get(&object)
        .expect("queried object is in the early-layer view")
        .characteristics;
    let full = engine
        .characteristics(object)
        .expect("the prepared battlefield object has full characteristics");
    assert_eq!(early.types, full.types);
    assert_eq!(early.all_creature_types, full.all_creature_types);
    assert_eq!(early.supertypes, full.supertypes);
    full
}

#[test]
fn luxior_type_static_survives_source_ability_loss_but_distinct_counter_static_stops() {
    // Prepared-state regression: the two static definitions have separate origins, and the
    // layer-4 type component has already started before the injected layer-6 ability removal.
    let (mut engine, luxior, planeswalker) = luxior_characteristics_fixture();
    let timestamp = engine.state.command_index;
    let object = engine.state.objects.get_mut(&planeswalker).unwrap();
    for kind in [CounterKind::Loyalty, CounterKind::Charge] {
        object.counters.insert(kind, 2);
        object.counter_timestamps.insert(kind, timestamp);
    }

    let before = luxior_early_and_full_types(&engine, planeswalker);
    assert!(before.is_creature());
    assert!(!before.has_type("Planeswalker"));
    assert_eq!(engine.effective_power(planeswalker), Some(4));
    assert_eq!(engine.effective_toughness(planeswalker), Some(4));

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(luxior),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: timestamp.saturating_add(1),
    });

    let after = luxior_early_and_full_types(&engine, planeswalker);
    assert!(
        after.is_creature(),
        "the layer-4 type component already started"
    );
    assert!(!after.has_type("Planeswalker"));
    assert_eq!(engine.effective_power(planeswalker), Some(0));
    assert_eq!(engine.effective_toughness(planeswalker), Some(0));
}

#[test]
fn luxior_live_attachment_and_source_generation_recompute_both_statics() {
    // Prepared-state regression: direct link and zone changes isolate characteristic
    // recomputation; Phase A separately owns the zone-change transaction and link clearing.
    let (mut engine, luxior, planeswalker) = luxior_characteristics_fixture();
    let timestamp = engine.state.command_index;
    let object = engine.state.objects.get_mut(&planeswalker).unwrap();
    for kind in [CounterKind::Loyalty, CounterKind::Charge] {
        object.counters.insert(kind, 2);
        object.counter_timestamps.insert(kind, timestamp);
    }

    let attached = luxior_early_and_full_types(&engine, planeswalker);
    assert!(attached.is_creature());
    assert_eq!(engine.effective_power(planeswalker), Some(4));
    assert_eq!(engine.effective_toughness(planeswalker), Some(4));

    engine.state.objects.get_mut(&luxior).unwrap().attached_to = None;
    let unlinked = luxior_early_and_full_types(&engine, planeswalker);
    assert!(!unlinked.is_creature());
    assert!(unlinked.has_type("Planeswalker"));
    assert_eq!(engine.effective_power(planeswalker), None);
    assert_eq!(engine.effective_toughness(planeswalker), None);

    engine.state.objects.get_mut(&luxior).unwrap().attached_to =
        Some(AttachmentRecipient::Object(planeswalker));
    let reattached = luxior_early_and_full_types(&engine, planeswalker);
    assert!(reattached.is_creature());
    assert!(!reattached.has_type("Planeswalker"));
    assert_eq!(engine.effective_power(planeswalker), Some(4));
    assert_eq!(engine.effective_toughness(planeswalker), Some(4));

    let next_generation = engine
        .state
        .zone_change_generation
        .get(&luxior)
        .copied()
        .unwrap_or(0)
        .saturating_add(1);
    {
        let source = engine.state.objects.get_mut(&luxior).unwrap();
        source.zone = Zone::Graveyard;
        source.attached_to = None;
    }
    engine
        .state
        .zone_change_generation
        .insert(luxior, next_generation);
    let departed = luxior_early_and_full_types(&engine, planeswalker);
    assert!(!departed.is_creature());
    assert!(departed.has_type("Planeswalker"));
    assert_eq!(engine.effective_power(planeswalker), None);
    assert_eq!(engine.effective_toughness(planeswalker), None);

    {
        let source = engine.state.objects.get_mut(&luxior).unwrap();
        source.zone = Zone::Battlefield;
    }
    let returned_generation = next_generation.saturating_add(1);
    engine
        .state
        .zone_change_generation
        .insert(luxior, returned_generation);
    engine.emit_static_abilities_on_enter(luxior);
    engine.state.objects.get_mut(&luxior).unwrap().attached_to =
        Some(AttachmentRecipient::Object(planeswalker));

    let returned = luxior_early_and_full_types(&engine, planeswalker);
    assert!(returned.is_creature());
    assert!(!returned.has_type("Planeswalker"));
    assert_eq!(engine.effective_power(planeswalker), Some(4));
    assert_eq!(engine.effective_toughness(planeswalker), Some(4));
}

const LUXIOR_DEPENDENCY_PLANESWALKER_FIXTURE: &str = r#"(
    id: "luxior_dependency_planeswalker_fixture",
    name: "Luxior Dependency Planeswalker Fixture",
    face_id: "luxior_dependency_planeswalker_fixture",
    types: ["Planeswalker", "Jace"],
)"#;

#[test]
fn luxior_planeswalker_removal_precedes_a_filter_whose_membership_it_changes() {
    for (dependency_timestamp, luxior_timestamp) in [(1, 2), (3, 2)] {
        let registry = CardRegistry::from_chunks_and_tokens(
            &[
                LUXIOR_RUNTIME_FIXTURE,
                LUXIOR_DEPENDENCY_PLANESWALKER_FIXTURE,
            ],
            &[],
        )
        .expect("the isolated dependency fixtures are admitted");
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            610_302,
            &[0, 1],
            20,
            None,
            true,
        )
        .expect("new engine");
        engine.registry = Box::leak(Box::new(registry));
        let luxior = insert_fixture(&mut engine, 0, "luxior_runtime_fixture", Zone::Battlefield);
        let planeswalker = insert_fixture(
            &mut engine,
            0,
            "luxior_dependency_planeswalker_fixture",
            Zone::Battlefield,
        );
        engine.state.zone_change_generation.insert(luxior, 1);
        engine.state.zone_change_generation.insert(planeswalker, 1);
        engine.emit_static_abilities_on_enter(luxior);
        engine.state.objects.get_mut(&luxior).unwrap().attached_to =
            Some(AttachmentRecipient::Object(planeswalker));

        for effect in &mut engine.state.continuous_effects {
            if effect.source_id == Some(luxior)
                && matches!(
                    effect.kind,
                    ContinuousEffectKind::Layer4AddTypes(_)
                        | ContinuousEffectKind::Layer4RemovePlaneswalker
                )
            {
                effect.timestamp = luxior_timestamp;
            }
        }
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::PermanentsMatching {
                reference_player: 0,
                filter: Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![PermanentTypeFilter::Planeswalker],
                    ..TargetFilter::default()
                }),
                exclude: None,
            },
            kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Artifact],
                ..TypeLineAddition::default()
            }),
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: dependency_timestamp,
        });

        let result = luxior_early_and_full_types(&engine, planeswalker);
        assert!(result.is_creature());
        assert!(!result.has_type("Planeswalker"));
        assert!(
            !result.is_artifact(),
            "timestamp order does not let the planeswalker-filtered effect survive removal"
        );
    }
}
