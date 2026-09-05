//! Characterization of printed-card predicates, independent of target/choice context.
use super::resolution::move_object_to_zone;
use super::targeting::{graveyard_target_legal, TargetSourceIdentity};
use super::*;
use tricerules_cards::primitives::{GraveyardFilter, GraveyardOwner};

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

fn fixture_effect(effect: &str) -> SpellEffectKind {
    let card = format!(
        r#"(id: "test", name: "Test", face_id: "test", types: ["Instant"], spell_effect: [{effect}])"#
    );
    let registry = CardRegistry::from_chunks_and_tokens(&[&card], &[]).expect("fixture registry");
    registry.get("test").unwrap().primary_face().spell_effect[0].clone()
}

fn zone_filter(predicate: &str) -> ZoneCardFilter {
    let SpellEffectKind::ChooseGraveyardCard { filter, .. } = fixture_effect(&format!(
        "ChooseGraveyardCard(filter: {predicate}, destination: Hand)"
    )) else {
        panic!("zone filter fixture");
    };
    filter
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
