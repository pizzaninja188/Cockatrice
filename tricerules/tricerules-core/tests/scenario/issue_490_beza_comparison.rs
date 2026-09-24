//! Actual-card scenarios for Beza, the Bounding Spring and its four resolution-time comparisons.
//!
//! Beza's effects resolve in Oracle order under CR 608.2c. Each opponent is considered separately
//! (Oracle ruling, 2024-07-26), and every clause reads current public values when it executes. A
//! Treasure that becomes a creature after entering can change the following creature comparison.

use super::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TargetFilter, TargetKind,
    TypeLineAddition,
};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::TokenCreated;

fn main_phase(seed: u64) -> GameEngine {
    semantic::main_phase(seed)
}

fn cast_beza_to_its_etb_trigger(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, "beza,_the_bounding_spring");
    let slot = hand_index_for_card(engine, 0, "beza,_the_bounding_spring");
    grant_pool(engine, 0);
    semantic::accepted(engine, 0, &cast_spell(slot, vec![]));
    for _ in 0..2 {
        let priority = engine.state.priority_player_id();
        semantic::accepted(engine, priority, &pass());
    }
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Beza's ETB trigger is on stack"
    );
    source
}

fn resolve_and_collect_tokens(engine: &mut GameEngine) -> Vec<TokenCreated> {
    let mut created = Vec::new();
    for _ in 0..16 {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            return created;
        }
        let first = engine.state.priority_player_id();
        let batch = semantic::accepted(engine, first, &pass());
        created.extend(token_created_events(&batch).into_iter().cloned());
        if engine.state.stack.is_empty() {
            continue;
        }
        let second = engine.state.priority_player_id();
        let batch = semantic::accepted(engine, second, &pass());
        created.extend(token_created_events(&batch).into_iter().cloned());
    }
    panic!("Beza stack did not resolve within the bounded scenario");
}

fn assert_token_events(engine: &GameEngine, created: &[TokenCreated], expected: &[(&str, &str)]) {
    assert_eq!(
        created.len(),
        expected.len(),
        "physical token event count; actual events: {created:?}"
    );
    for (event, (card_id, name)) in created.iter().zip(expected) {
        assert_eq!(event.controller_player_id, 0);
        assert_eq!(event.card_id, *card_id);
        assert_eq!(
            event.identity.as_ref().expect("public token identity").name,
            *name
        );
        let object = engine
            .state
            .objects
            .get(&event.object_id)
            .expect("TokenCreated object is in engine state");
        assert_eq!(object.card_id, *card_id);
        assert_eq!(object.owner, 0);
        assert_eq!(object.controller, 0);
        assert_eq!(object.zone, Zone::Battlefield);
        assert!(engine.state.players[0]
            .battlefield
            .contains(&event.object_id));
    }
}

#[test]
fn issue_490_beza_resolves_each_qualifying_bonus() {
    let mut engine = main_phase(490_101);
    inject_permanent_on_battlefield(&mut engine, 0, "forest");
    inject_permanent_on_battlefield(&mut engine, 1, "island");
    inject_permanent_on_battlefield(&mut engine, 1, "island");
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    for _ in 0..3 {
        inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    }
    engine.state.players[1].life = 21;
    inject_card_into_hand(&mut engine, 1, "island");
    let original_hand = engine.state.players[0].hand.len();
    inject_library_card(&mut engine, 0, "plains");

    cast_beza_to_its_etb_trigger(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), original_hand);
    let created = resolve_and_collect_tokens(&mut engine);

    assert_eq!(engine.state.players[0].life, 24, "Beza gains four life");
    assert_eq!(
        engine.state.players[0].hand.len(),
        original_hand + 1,
        "Beza draws one card"
    );
    assert_token_events(
        &engine,
        &created,
        &[
            ("treasure", "Treasure"),
            ("fish_u_1_1", "Fish"),
            ("fish_u_1_1", "Fish"),
        ],
    );
}

#[test]
fn issue_490_beza_comparisons_use_resolution_time_values() {
    let mut engine = main_phase(490_102);
    let original_life = engine.state.players[0].life;
    cast_beza_to_its_etb_trigger(&mut engine);
    assert_eq!(engine.state.players[1].life, original_life);
    // The entry trigger exists regardless; the life clause observes the newer value at resolution.
    engine.state.players[1].life += 1;

    let created = resolve_and_collect_tokens(&mut engine);
    assert_eq!(engine.state.players[0].life, original_life + 4);
    assert!(
        created.is_empty(),
        "the other three comparisons remain false"
    );
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
    assert!(battlefield_token_oids(&engine, 0, "fish_u_1_1").is_empty());
}

fn animate_all_artifacts_as_creatures(engine: &mut GameEngine) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::PermanentsMatching {
            reference_player: 0,
            filter: Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Artifact],
                ..TargetFilter::default()
            }),
            exclude: None,
        },
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            creature_types: vec!["Construct".into()],
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn issue_490_animated_treasure_changes_bezas_later_creature_comparison() {
    let mut animated = main_phase(490_103);
    inject_permanent_on_battlefield(&mut animated, 1, "island");
    for _ in 0..2 {
        inject_creature_on_battlefield(&mut animated, 1, "grizzly_bears");
    }
    animate_all_artifacts_as_creatures(&mut animated);
    cast_beza_to_its_etb_trigger(&mut animated);
    let created = resolve_and_collect_tokens(&mut animated);
    assert_token_events(&animated, &created, &[("treasure", "Treasure")]);
    let treasure = battlefield_token_oids(&animated, 0, "treasure")[0];
    assert!(
        animated
            .characteristics(treasure)
            .expect("animated Treasure characteristics")
            .is_creature(),
        "the Treasure is a creature before Beza's later creature comparison"
    );
    assert!(battlefield_token_oids(&animated, 0, "fish_u_1_1").is_empty());

    let mut ordinary = main_phase(490_104);
    inject_permanent_on_battlefield(&mut ordinary, 1, "island");
    for _ in 0..2 {
        inject_creature_on_battlefield(&mut ordinary, 1, "grizzly_bears");
    }
    cast_beza_to_its_etb_trigger(&mut ordinary);
    let created = resolve_and_collect_tokens(&mut ordinary);
    assert_token_events(
        &ordinary,
        &created,
        &[
            ("treasure", "Treasure"),
            ("fish_u_1_1", "Fish"),
            ("fish_u_1_1", "Fish"),
        ],
    );
}
