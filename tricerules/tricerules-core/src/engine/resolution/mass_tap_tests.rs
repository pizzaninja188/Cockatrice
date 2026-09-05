//! CR 115.10 / 608.2h / 613 / 701.26: shared untargeted selection, separate tap/untap actions.
use super::*;
use tricerules_cards::primitives::{
    Color, ControllerReference, PermanentTypeFilter, TypeLineAddition,
};

fn setup() -> GameEngine {
    let deck = ["grizzly_bears", "ornithopter", "forest", "short_sword"]
        .into_iter()
        .cycle()
        .take(20)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut engine = GameEngine::new(225_001, &[3, 11], 20, Some(vec![deck.clone(), deck]), true)
        .expect("engine");
    // Session admission is still two-player; exercise the generic resolver below that boundary.
    // Keep the extra player ID above the fixture's existing object IDs.
    engine.state.players.push(PlayerState::new(299, 20));
    engine
}

fn deploy(engine: &mut GameEngine, seat: usize, card: &str) -> ObjectId {
    let player = &mut engine.state.players[if seat == 2 { 0 } else { seat }];
    let oid = player
        .library
        .iter()
        .chain(&player.hand)
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == card)
        .expect("fixture card");
    player.library.retain(|id| *id != oid);
    player.hand.retain(|id| *id != oid);
    engine.state.players[seat].battlefield.push(oid);
    let owner = engine.state.players[seat].id;
    let object = engine.state.objects.get_mut(&oid).unwrap();
    object.zone = Zone::Battlefield;
    object.owner = owner;
    object.base_controller = owner;
    object.controller = owner;
    oid
}

fn modify(engine: &mut GameEngine, oid: ObjectId, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: 1,
    });
}

fn resolve(engine: &mut GameEngine, effect: SpellEffectKind) {
    let top = StackItem {
        id: u32::MAX,
        controller: 3,
        card_id: "cryptic_command".into(),
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
        cast_cost_receipts: vec![],
        payment_result: CardResultCohort::default(),
        search_results: Default::default(),
        resolution_branch_choices: Default::default(),
        blight_receipts: vec![],
        trigger_context: TriggerContext::default(),
    };
    let mut events = vec![];
    let mut result = EffectResult::default();
    let mut cx = EffectCx {
        engine,
        events: &mut events,
        targets: &[],
        targets_by_role: &[],
        target_damage: &[],
        target_group_indices: &[],
        top: &top,
        controller: 3,
        affected_player: 3,
        spell_label: "mass-effect fixture",
        previous_effect_result: &EffectResult::default(),
        effect_result: &mut result,
        effect_index: 0,
    };
    let outcome = match effect {
        SpellEffectKind::TapAll { .. } => tap_all(&mut cx, effect),
        SpellEffectKind::UntapAll { .. } => untap_all(&mut cx, effect),
        _ => panic!("mass tap/untap fixture only"),
    };
    assert_eq!(outcome.expect("resolution"), EffectOutcome::Continue);
}

#[test]
fn scopes_use_current_controllers_in_deterministic_battlefield_order() {
    let mut engine = setup();
    let ours = deploy(&mut engine, 0, "grizzly_bears");
    let stolen = deploy(&mut engine, 1, "grizzly_bears");
    let opponent = deploy(&mut engine, 2, "grizzly_bears");
    let filter = TargetFilter::default_creature();
    assert_eq!(
        scoped_battlefield_objects(&engine, 3, RelativePlayerSet::Opponents, &filter),
        vec![stolen, opponent]
    );
    modify(
        &mut engine,
        stolen,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(3),
        },
    );
    assert_eq!(engine.state.objects[&stolen].owner, 11);
    for (players, expected) in [
        (RelativePlayerSet::Controller, vec![ours, stolen]),
        (RelativePlayerSet::Opponents, vec![opponent]),
        (RelativePlayerSet::All, vec![ours, stolen, opponent]),
    ] {
        assert_eq!(
            scoped_battlefield_objects(&engine, 3, players, &filter),
            expected
        );
        resolve(
            &mut engine,
            SpellEffectKind::TapAll {
                players,
                filter: filter.clone(),
            },
        );
        for oid in [ours, stolen, opponent] {
            assert_eq!(engine.state.objects[&oid].tapped, expected.contains(&oid));
        }
        resolve(
            &mut engine,
            SpellEffectKind::UntapAll {
                players,
                filter: filter.clone(),
            },
        );
        assert!([ours, stolen, opponent]
            .iter()
            .all(|oid| !engine.state.objects[oid].tapped));
    }
}

#[test]
fn current_types_colors_and_protection_drive_untargeted_selection() {
    let mut engine = setup();
    let bear = deploy(&mut engine, 0, "grizzly_bears");
    let land = deploy(&mut engine, 1, "forest");
    let thopter = deploy(&mut engine, 2, "ornithopter");
    let filter = TargetFilter {
        not_color: Some(Color::White),
        ..TargetFilter::default_creature()
    };
    assert_eq!(
        scoped_battlefield_objects(&engine, 3, RelativePlayerSet::All, &filter),
        vec![bear, thopter]
    );
    modify(
        &mut engine,
        bear,
        ContinuousEffectKind::Layer5SetColors(vec![Color::White, Color::Green]),
    );
    modify(
        &mut engine,
        land,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            ..Default::default()
        }),
    );
    modify(
        &mut engine,
        land,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Hexproof),
    );
    modify(
        &mut engine,
        thopter,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Shroud),
    );
    assert_eq!(
        scoped_battlefield_objects(&engine, 3, RelativePlayerSet::All, &filter),
        vec![land, thopter]
    );
    resolve(
        &mut engine,
        SpellEffectKind::TapAll {
            players: RelativePlayerSet::All,
            filter,
        },
    );
    assert!(
        !engine.state.objects[&bear].tapped,
        "white multicolor creatures are excluded"
    );
    assert!(engine.state.objects[&land].tapped && engine.state.objects[&thopter].tapped);
    assert!(engine
        .state
        .objects
        .values()
        .filter(|object| object.zone != Zone::Battlefield)
        .all(|object| !object.tapped));
}

#[test]
fn overlapping_filters_and_no_ops_preserve_one_tap_action() {
    let mut engine = setup();
    let thopter = deploy(&mut engine, 1, "ornithopter");
    let sword = deploy(&mut engine, 1, "short_sword");
    let filter = TargetFilter {
        any_of: Some(vec![
            TargetFilter::default_creature(),
            TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Artifact],
                ..Default::default()
            },
        ]),
        ..Default::default()
    };
    assert_eq!(
        scoped_battlefield_objects(&engine, 3, RelativePlayerSet::Opponents, &filter),
        vec![thopter, sword]
    );
    engine.state.objects.get_mut(&thopter).unwrap().tapped = true;
    let action = engine.state.next_tap_action_id;
    for _ in 0..2 {
        resolve(
            &mut engine,
            SpellEffectKind::TapAll {
                players: RelativePlayerSet::Opponents,
                filter: filter.clone(),
            },
        );
        assert_eq!(
            engine.state.next_tap_action_id,
            action + 1,
            "repeated tap is a no-op"
        );
        assert!(engine.state.objects[&sword].tapped);
    }
    resolve(
        &mut engine,
        SpellEffectKind::TapAll {
            players: RelativePlayerSet::Controller,
            filter,
        },
    );
    assert_eq!(
        engine.state.next_tap_action_id,
        action + 1,
        "empty cohort is a no-op"
    );
}

#[test]
fn filtered_untap_keeps_prohibition_and_stun_execution() {
    let mut engine = setup();
    let prohibited = deploy(&mut engine, 0, "forest");
    let stunned = deploy(&mut engine, 1, "forest");
    let ordinary = deploy(&mut engine, 2, "forest");
    let excluded = deploy(&mut engine, 2, "grizzly_bears");
    for oid in [prohibited, stunned, ordinary, excluded] {
        engine.state.objects.get_mut(&oid).unwrap().tapped = true;
    }
    for oid in [prohibited, stunned] {
        engine
            .state
            .objects
            .get_mut(&oid)
            .unwrap()
            .set_counter(CounterKind::Stun, 1);
    }
    modify(&mut engine, prohibited, ContinuousEffectKind::ProhibitUntap);
    let filter = TargetFilter {
        kind: TargetKind::AnyPermanent,
        permanent_types: vec![PermanentTypeFilter::Land],
        ..Default::default()
    };
    resolve(
        &mut engine,
        SpellEffectKind::UntapAll {
            players: RelativePlayerSet::All,
            filter: filter.clone(),
        },
    );
    assert!(engine.state.objects[&prohibited].tapped && engine.state.objects[&stunned].tapped);
    assert!(!engine.state.objects[&ordinary].tapped);
    assert!(engine.state.objects[&excluded].tapped);
    assert_eq!(
        engine.state.objects[&prohibited].counter_count(CounterKind::Stun),
        1
    );
    assert_eq!(
        engine.state.objects[&stunned].counter_count(CounterKind::Stun),
        0
    );
    resolve(
        &mut engine,
        SpellEffectKind::UntapAll {
            players: RelativePlayerSet::All,
            filter,
        },
    );
    assert!(!engine.state.objects[&stunned].tapped);
}
