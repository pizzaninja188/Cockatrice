//! Characterization of printed-card predicates, independent of target/choice context.
use super::resolution::move_object_to_zone;
use super::targeting::{graveyard_target_legal, TargetSourceIdentity};
use super::*;
use tricerules_card_model::primitives::{GraveyardFilter, GraveyardOwner};

fn fixture() -> GameEngine {
    let cards = [
        "grizzly_bears",
        "forest",
        "short_sword",
        "bonecrusher_giant_stomp",
    ];
    let deck = cards
        .into_iter()
        .cycle()
        .take(12)
        .map(str::to_owned)
        .collect();
    GameEngine::new(
        tricerules_cards::registry::global(),
        198_001,
        &[0, 1],
        20,
        Some(vec![deck, vec!["forest".into(); 12]]),
        true,
    )
    .expect("predicate fixture")
}

fn object(engine: &GameEngine, card: &str) -> ObjectId {
    engine
        .state
        .objects
        .values()
        .filter(|object| object.owner == 0 && object.card_id == card)
        .map(|object| object.id)
        .min()
        .expect("fixture card")
}

// Install the actual source definition through the same static emitter used by real entry.
// The test fixture changes one existing physical object's identity before moving it.
fn install_lattice(engine: &mut GameEngine) -> ObjectId {
    let source = object(engine, "short_sword");
    engine.state.objects.get_mut(&source).unwrap().card_id = "mycosynth_lattice".into();
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        source,
        Zone::Battlefield,
        None,
    )
    .expect("put Lattice on the battlefield");
    engine.emit_static_abilities_on_enter(source);
    source
}

#[test]
fn lattice_zone_color_filters_and_library_top_permission_use_current_color() {
    let mut engine = fixture();
    let mut bears: Vec<_> = engine
        .state
        .objects
        .values()
        .filter(|object| object.owner == 0 && object.card_id == "grizzly_bears")
        .map(|object| object.id)
        .collect();
    bears.sort_unstable();
    assert_eq!(bears.len(), 3);
    for (oid, zone) in bears
        .iter()
        .copied()
        .zip([Zone::Hand, Zone::Library, Zone::Graveyard])
    {
        if engine.state.objects[&oid].zone != zone {
            move_object_to_zone(&mut engine.state, engine.registry, oid, zone, None).unwrap();
        }
    }
    let colorless_creature = ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        is_colorless: Some(true),
        ..Default::default()
    };
    let disjunction = ZoneCardFilter {
        any_of: Some(vec![
            ZoneCardFilter {
                exact_name: Some("Forest".into()),
                ..Default::default()
            },
            colorless_creature.clone(),
        ]),
        ..Default::default()
    };
    for oid in &bears {
        assert!(!zone_card_matches_filter(
            &engine.state,
            engine.registry,
            *oid,
            Some(&disjunction)
        ));
    }
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Player(0),
        kind: ContinuousEffectKind::LibraryTopPermission {
            look_at_top: true,
            cast_filter: Some(colorless_creature.clone()),
            play_lands: false,
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: 0,
        trigger_grant_origin: None,
    });
    let bear_definition = engine.registry.get("grizzly_bears").unwrap();
    assert!(!engine.may_cast_library_top_face(0, bears[1], bear_definition, 0));
    install_lattice(&mut engine);
    for oid in &bears {
        assert!(zone_card_matches_filter(
            &engine.state,
            engine.registry,
            *oid,
            Some(&disjunction)
        ));
        assert!(!zone_card_matches_filter(
            &engine.state,
            engine.registry,
            *oid,
            Some(&ZoneCardFilter {
                is_colorless: Some(false),
                ..Default::default()
            })
        ));
    }
    assert!(engine.may_cast_library_top_face(0, bears[1], bear_definition, 0));
}

#[test]
fn lattice_zone_color_filters_preserve_face_down_hideaway_permissions() {
    let mut engine = fixture();
    let hidden = object(&engine, "grizzly_bears");
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        hidden,
        Zone::Exile,
        None,
    )
    .unwrap();
    engine.state.objects.get_mut(&hidden).unwrap().face_down = true;
    let generation = engine
        .state
        .zone_change_generation
        .get(&hidden)
        .copied()
        .unwrap_or(0);
    let producer = engine
        .registry
        .get("mosswort_bridge")
        .unwrap()
        .primary_face();
    let key = LinkedExileKey {
        source_object_id: 900_001,
        source_zone_change: 3,
        producer: AbilityDefinitionId {
            card_id: "mosswort_bridge".into(),
            face_id: producer.face_id.clone(),
            ability_path: vec![producer.triggered_abilities[0].ability_id.clone()],
        },
        ability_link_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
        occurrence: LinkedExileOccurrence::NativeOrTokenBase,
    };
    engine
        .state
        .hideaway_look_permissions
        .insert((key.clone(), hidden, generation), BTreeSet::from([0]));
    install_lattice(&mut engine);
    let filter = ZoneCardFilter {
        is_colorless: Some(true),
        ..Default::default()
    };
    assert!(!zone_card_matches_filter(
        &engine.state,
        engine.registry,
        hidden,
        Some(&filter)
    ));
    assert!(!card_predicates::linked_exile_card_matches_filter(
        &engine.state,
        engine.registry,
        &key,
        1,
        hidden,
        generation,
        Some(&filter)
    ));
    assert!(!card_predicates::linked_exile_card_matches_filter(
        &engine.state,
        engine.registry,
        &key,
        0,
        hidden,
        generation + 1,
        Some(&filter)
    ));
    assert!(card_predicates::linked_exile_card_matches_filter(
        &engine.state,
        engine.registry,
        &key,
        0,
        hidden,
        generation,
        Some(&filter)
    ));
}

fn fixture_effect(effect: &str) -> SpellEffectKind {
    let card = format!(
        r#"(id: "test", name: "Test", face_id: "test", types: ["Instant"], spell_effect: [{effect}])"#
    );
    let registry = CardRegistry::from_chunks_and_tokens(&[&card], &[]).expect("fixture registry");
    registry.get("test").unwrap().primary_face().spell_effect[0].clone()
}

fn zone_filter(predicate: &str) -> ZoneCardFilter {
    let card = format!(
        r#"(id: "test", name: "Test", face_id: "test", types: ["Instant"], spell_program: Some((
            body: (steps: [
                (id: "choose_card", kind: Perform(ChooseCard(
                    source: WholeCurrentGraveyard,
                    filter: {predicate},
                    destination: Hand,
                    optional: false,
                ))),
            ], yields: []),
            target_bindings: [],
        )))"#
    );
    let registry = CardRegistry::from_chunks_and_tokens(&[&card], &[])
        .expect("authored ChooseCard filter fixture registry");
    let program = registry
        .get("test")
        .expect("filter fixture card")
        .primary_face()
        .spell_program
        .as_ref()
        .expect("authored filter fixture program");
    let [instruction] = program.body.steps.as_slice() else {
        panic!("one authored filter instruction");
    };
    let tricerules_card_model::primitives::InstructionKind::Perform(effect) = &instruction.kind
    else {
        panic!("authored ChooseCard filter instruction");
    };
    let tricerules_card_model::primitives::SpellEffectKind::ChooseCard { filter, .. } =
        effect.as_ref()
    else {
        panic!("authored ChooseCard filter operation");
    };
    filter.clone()
}

fn graveyard_filter(predicate: &str) -> GraveyardFilter {
    let SpellEffectKind::MoveGraveyardCards { filter, .. } = fixture_effect(&format!(
        "MoveGraveyardCards(filter: {predicate}, destination: Hand)"
    )) else {
        panic!("graveyard filter fixture");
    };
    filter
}

#[test]
fn issue_198_existing_zone_leaves_and_or_match_printed_cards() {
    let engine = fixture();
    for (predicate, card, expected) in [
        ("(exact_name: Some(\"Grizzly Bears\"))", "grizzly_bears", true),
        ("(exact_name: Some(\"Forest\"))", "grizzly_bears", false),
        ("(card_type: Some(BasicLand))", "forest", true),
        ("(card_type: Some(Noncreature))", "grizzly_bears", false),
        ("(required_subtypes: [\"Bear\"])", "grizzly_bears", true),
        ("(required_subtypes: [\"Bear\"])", "forest", false),
        ("(printed_power: Some(AtMost(2)))", "grizzly_bears", true),
        ("(printed_power: Some(AtLeast(2)))", "grizzly_bears", true),
        ("(printed_power: Some(AtLeast(3)))", "grizzly_bears", false),
        ("(printed_power: Some(AtMost(2)))", "forest", false),
        ("(card_type: Some(Creature), required_subtypes: [\"Giant\"], printed_power: Some(AtLeast(4)))", "bonecrusher_giant_stomp", true),
        ("(any_of: Some([(card_type: Some(Creature)), (card_type: Some(Land))]))", "short_sword", false),
        ("(any_of: Some([(card_type: Some(Creature)), (card_type: Some(Land))]))", "forest", true),
    ] {
        let filter = zone_filter(predicate);
        assert_eq!(zone_card_matches_filter(&engine.state, engine.registry,
            object(&engine, card), Some(&filter)), expected, "{predicate}: {card}");
    }
    assert!(!zone_card_matches_filter(
        &engine.state,
        engine.registry,
        u32::MAX,
        Some(&ZoneCardFilter {
            card_type: Some(CardTypeFilter::Creature),
            ..Default::default()
        })
    ));
    assert!(zone_card_matches_filter(
        &engine.state,
        engine.registry,
        object(&engine, "forest"),
        None
    ));
}

#[test]
fn face_down_exile_has_no_printed_characteristics_for_zone_filters() {
    let mut engine = fixture();
    let hidden = object(&engine, "grizzly_bears");
    let object = engine.state.objects.get_mut(&hidden).unwrap();
    object.zone = Zone::Exile;
    object.face_down = true;

    let creature = zone_filter("(card_type: Some(Creature))");
    let exact_name = zone_filter("(exact_name: Some(\"Grizzly Bears\"))");
    assert!(!zone_card_matches_filter(
        &engine.state,
        engine.registry,
        hidden,
        Some(&creature)
    ));
    assert!(!zone_card_matches_filter(
        &engine.state,
        engine.registry,
        hidden,
        Some(&exact_name)
    ));
    assert!(zone_card_matches_filter(
        &engine.state,
        engine.registry,
        hidden,
        None
    ));
}

#[test]
fn issue_198_existing_graveyard_leaves_match_printed_cards() {
    let mut engine = fixture();
    for card in [
        "grizzly_bears",
        "forest",
        "short_sword",
        "bonecrusher_giant_stomp",
    ] {
        let oid = object(&engine, card);
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            oid,
            Zone::Graveyard,
            None,
        )
        .expect("move fixture card");
    }
    for (predicate, card, expected) in [
        ("()", "forest", true),
        (
            "(card: Some((card_type: Some(Creature))))",
            "grizzly_bears",
            true,
        ),
        (
            "(card: Some((excluded_card_types: [Creature, Land])))",
            "short_sword",
            true,
        ),
        (
            "(card: Some((excluded_card_types: [Creature, Land])))",
            "forest",
            false,
        ),
        (
            "(card: Some((min_mana_value: Some(2), max_mana_value: Some(2))))",
            "grizzly_bears",
            true,
        ),
        (
            "(card: Some((min_mana_value: Some(3))))",
            "grizzly_bears",
            false,
        ),
        (
            "(card: Some((max_mana_value: Some(1))))",
            "grizzly_bears",
            false,
        ),
        (
            "(card: Some((required_subtypes: [\"Bear\"])))",
            "grizzly_bears",
            true,
        ),
        (
            "(card: Some((required_subtypes: [\"Bear\", \"Giant\"])))",
            "grizzly_bears",
            false,
        ),
        (
            "(card: Some((excluded_subtypes: [\"Bear\"])))",
            "grizzly_bears",
            false,
        ),
        (
            "(card: Some((excluded_subtypes: [\"Giant\"])))",
            "grizzly_bears",
            true,
        ),
        (
            "(card: Some((has_adventure: Some(true))))",
            "bonecrusher_giant_stomp",
            true,
        ),
        (
            "(card: Some((has_adventure: Some(false))))",
            "bonecrusher_giant_stomp",
            false,
        ),
        (
            "(card: Some((has_adventure: Some(false))))",
            "grizzly_bears",
            true,
        ),
        (
            "(card: Some((any_of: Some([(card_type: Some(Creature)), (card_type: Some(Land))]))))",
            "forest",
            true,
        ),
        (
            "(card: Some((any_of: Some([(card_type: Some(Creature)), (card_type: Some(Land))]))))",
            "short_sword",
            false,
        ),
    ] {
        let filter = graveyard_filter(predicate);
        let oid = object(&engine, card);
        assert_eq!(
            graveyard_target_legal(
                &engine,
                &filter,
                oid,
                0,
                TargetSourceIdentity::current(&engine, u32::MAX),
                TriggerContext::default()
            ),
            expected,
            "{predicate}: {card}"
        );
        for zone in [Zone::Library, Zone::Hand, Zone::Graveyard] {
            move_object_to_zone(&mut engine.state, engine.registry, oid, zone, None).unwrap();
            assert_eq!(
                zone_card_matches_filter(&engine.state, engine.registry, oid, filter.card.as_ref()),
                expected,
                "{predicate}: {card} in {zone:?}"
            );
        }
    }
}

#[test]
fn issue_198_graveyard_context_rejects_non_cards_wrong_zones_and_wrong_owners() {
    let mut engine = fixture();
    let oid = object(&engine, "grizzly_bears");
    let filter = graveyard_filter(
        "(card: Some((any_of: Some([(card_type: Some(Creature)), (card_type: Some(Land))]))))",
    );
    let legal = |engine: &GameEngine, filter: &GraveyardFilter, oid, actor| {
        graveyard_target_legal(
            engine,
            filter,
            oid,
            actor,
            TargetSourceIdentity::current(engine, u32::MAX),
            TriggerContext::default(),
        )
    };
    assert!(
        !legal(&engine, &filter, oid, 0),
        "the card is still in hand or library"
    );
    assert!(!legal(&engine, &filter, u32::MAX, 0), "missing object");
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        oid,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    assert!(legal(&engine, &filter, oid, 0));
    assert!(
        !legal(&engine, &filter, oid, 1),
        "ownership applies to the whole disjunction"
    );
    let any = GraveyardFilter {
        owner: GraveyardOwner::AnyPlayer,
        ..filter.clone()
    };
    assert!(legal(&engine, &any, oid, 1));
    engine.state.objects.get_mut(&oid).unwrap().token_origin = engine.copiable_values_for(oid);
    assert!(
        !legal(&engine, &any, oid, 0),
        "a token that copied a registered card is not a graveyard card"
    );
    engine.state.objects.get_mut(&oid).unwrap().token_origin = None;
    engine.state.objects.get_mut(&oid).unwrap().card_id = "missing_definition".into();
    assert!(!legal(&engine, &GraveyardFilter::default(), oid, 0));
    assert!(!zone_card_matches_filter(
        &engine.state,
        engine.registry,
        oid,
        filter.card.as_ref()
    ));
}
