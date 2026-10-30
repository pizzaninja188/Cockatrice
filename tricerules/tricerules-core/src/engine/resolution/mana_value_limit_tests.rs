use super::*;
use tricerules_card_model::primitives::{ManaValueLimit, TargetFilter, TargetKind};
use tricerules_card_model::ActivatedAbilityDef;

fn setup() -> GameEngine {
    let deck = ["forest", "grizzly_bears"]
        .into_iter()
        .cycle()
        .take(20)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    GameEngine::new(
        tricerules_cards::registry::global(),
        248_071,
        &[3, 11],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("two-player mass-exile fixture")
}

fn deploy(engine: &mut GameEngine, seat: usize, card: &str) -> ObjectId {
    let player = &mut engine.state.players[seat];
    let object_id = player
        .library
        .iter()
        .chain(&player.hand)
        .copied()
        .find(|id| engine.state.objects[id].card_id == card)
        .expect("fixture card exists");
    player.library.retain(|id| *id != object_id);
    player.hand.retain(|id| *id != object_id);
    player.battlefield.push(object_id);
    let owner = player.id;
    let object = engine.state.objects.get_mut(&object_id).expect("object");
    object.zone = Zone::Battlefield;
    object.owner = owner;
    object.base_controller = owner;
    object.controller = owner;
    object_id
}

fn loyalty_x_owner() -> ActivatedAbilityDef {
    ron::from_str(
        r#"(ability_id: "minus_x", presentation: Fallback,
            costs: [LoyaltyX], effect: [ExileAll(
                kind: (kind: AnyPermanent),
                max_mana_value_limit: Some(ActivatedAbilityX)
            )])"#,
    )
    .expect("the captured X owner fixture parses")
}

fn resolve_effect(
    engine: &mut GameEngine,
    effect: SpellEffectKind,
    chosen_x: u32,
    activated_ability: Option<ActivatedAbilityDef>,
    is_triggered: bool,
) -> Result<EffectOutcome, EngineError> {
    let targets = Vec::new();
    let target_receipts = Vec::new();
    let top = StackItem {
        prototyped: false,
        mana_colors_spent_to_cast: Default::default(),
        id: u32::MAX,
        controller: 3,
        card_id: "ugin_phase_a_fixture".into(),
        targets: Vec::new(),
        ability_text: Some("X = 1".into()),
        source_permanent_id: None,
        source_owner: None,
        source_zone_change: 0,
        trigger_use_key: None,
        source_face_change: 0,
        ability_index: Some(0),
        activated_ability,
        activated_ability_occurrence: None,
        triggered_ability: None,
        is_triggered,
        is_copy: false,
        face_index: 0,
        cast_method: SpellCastMethod::Normal,
        returned_attacker_assignment: None,
        chosen_x,
        chosen_modes: Vec::new(),
        cast_cost_receipts: Vec::new(),
        cast_condition_results: Vec::new(),
        cast_occurrence: None,
        cast_by: None,
        payment_result: CardResultCohort::default(),
        exiled_cohorts: Default::default(),
        chaos_warp_owner_instructions: Default::default(),
        resolution_branch_choices: Default::default(),
        blight_receipts: Vec::new(),
        trigger_context: TriggerContext::default(),
    };
    let mut events = Vec::new();
    let mut effect_result = EffectResult::default();
    let previous_effect_result = EffectResult::default();
    let mut cx = EffectCx {
        engine,
        events: &mut events,
        targets: &targets,
        targets_by_role: &[],
        target_receipts: &target_receipts,
        target_role_group_indices: &[],
        top: &top,
        controller: 3,
        affected_player: 3,
        spell_label: "Ugin Phase A fixture",
        previous_effect_result: &previous_effect_result,
        effect_result: &mut effect_result,
        effect_index: 0,
        resume_previous_result: None,
        program_state: None,
    };
    match effect {
        SpellEffectKind::ExileAll { .. } => super::mass::exile_all(&mut cx, effect),
        SpellEffectKind::PutPermanentFromHand { .. } => {
            super::zones::put_permanent_from_hand(&mut cx, effect)
        }
        _ => Err(EngineError::Illegal(
            "unsupported mana-value-limit test effect",
        )),
    }
}

fn exile_all_effect(limit: ManaValueLimit) -> SpellEffectKind {
    exile_all_effect_with_filter(
        limit,
        TargetFilter {
            kind: TargetKind::AnyPermanent,
            ..TargetFilter::default()
        },
    )
}

fn exile_all_effect_with_filter(limit: ManaValueLimit, kind: TargetFilter) -> SpellEffectKind {
    SpellEffectKind::ExileAll {
        kind,
        max_mana_value_limit: Some(limit),
        until_source_leaves: false,
    }
}

fn exile_all_x_effect() -> SpellEffectKind {
    exile_all_effect(ManaValueLimit::ActivatedAbilityX)
}

#[test]
fn exile_all_uses_the_captured_announced_x_as_an_additional_mana_value_ceiling() {
    let mut engine = setup();
    let forest = deploy(&mut engine, 0, "forest");
    let bears = deploy(&mut engine, 1, "grizzly_bears");

    resolve_effect(
        &mut engine,
        exile_all_x_effect(),
        1,
        Some(loyalty_x_owner()),
        false,
    )
    .expect("valid announced LoyaltyX resolves");

    assert_eq!(engine.state.objects[&forest].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&bears].zone,
        Zone::Battlefield,
        "the printed AnyPermanent filter matches the bears, but MV 2 exceeds X=1"
    );
}

#[test]
fn exile_all_rejects_missing_loyalty_x_context_without_moving_any_permanent() {
    let mut engine = setup();
    let forest = deploy(&mut engine, 0, "forest");
    let bears = deploy(&mut engine, 1, "grizzly_bears");
    let before = serde_json::to_value(&engine.state).expect("state serializes");

    let result = resolve_effect(&mut engine, exile_all_x_effect(), 1, None, false);

    assert!(
        result.is_err(),
        "missing captured LoyaltyX context fails closed"
    );
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&bears].zone, Zone::Battlefield);
}

#[test]
fn exile_all_rejects_out_of_range_activated_x_context_without_moving_any_permanent() {
    let mut engine = setup();
    let forest = deploy(&mut engine, 0, "forest");
    let bears = deploy(&mut engine, 1, "grizzly_bears");
    let before = serde_json::to_value(&engine.state).expect("state serializes");

    let result = resolve_effect(
        &mut engine,
        exile_all_x_effect(),
        i32::MAX as u32 + 1,
        Some(loyalty_x_owner()),
        false,
    );

    assert!(
        result.is_err(),
        "captured LoyaltyX must remain in its valid range"
    );
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&bears].zone, Zone::Battlefield);
}

#[test]
fn exile_all_rejects_the_trigger_object_variant_without_moving_any_permanent() {
    let mut engine = setup();
    let forest = deploy(&mut engine, 0, "forest");
    let bears = deploy(&mut engine, 1, "grizzly_bears");
    let before = serde_json::to_value(&engine.state).expect("state serializes");

    let result = resolve_effect(
        &mut engine,
        exile_all_effect(ManaValueLimit::TriggerObject),
        1,
        Some(loyalty_x_owner()),
        false,
    );

    assert!(
        result.is_err(),
        "ExileAll rejects the crossed TriggerObject variant"
    );
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&bears].zone, Zone::Battlefield);
}

#[test]
fn put_permanent_from_hand_rejects_activated_x_before_any_hand_choice_or_move() {
    let mut engine = setup();
    let hand_card = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|id| engine.state.objects[id].card_id == "grizzly_bears")
        .expect("a permanent card remains in the controller's hand");
    let before = serde_json::to_value(&engine.state).expect("state serializes");
    let effect = SpellEffectKind::PutPermanentFromHand {
        filter: None,
        max_mana_value: Some(ManaValueLimit::ActivatedAbilityX),
        up_to_count: 1,
    };

    let result = resolve_effect(&mut engine, effect, 1, Some(loyalty_x_owner()), false);

    assert!(
        result.is_err(),
        "the hand-entry consumer rejects ActivatedAbilityX before offering a choice"
    );
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    assert_eq!(engine.state.objects[&hand_card].zone, Zone::Hand);
}

#[test]
fn exile_all_x_zero_uses_current_colors_and_exiles_a_multicolor_permanent_once() {
    let mut engine = setup();
    let multicolor_land = deploy(&mut engine, 0, "forest");
    let colorless_land = deploy(&mut engine, 1, "forest");
    let over_limit_creature = deploy(&mut engine, 1, "grizzly_bears");
    let before_generation = engine
        .state
        .zone_change_generation
        .get(&multicolor_land)
        .copied()
        .unwrap_or(0);
    for (object_id, colors) in [
        (multicolor_land, vec![Color::Red, Color::Blue]),
        (colorless_land, vec![]),
        (over_limit_creature, vec![Color::Red]),
    ] {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(object_id),
            kind: ContinuousEffectKind::Layer5SetColors(colors),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });
    }
    assert_eq!(
        engine.characteristics(multicolor_land).unwrap().colors,
        vec![Color::Red, Color::Blue]
    );
    assert!(engine
        .characteristics(colorless_land)
        .unwrap()
        .colors
        .is_empty());
    assert_eq!(
        engine.characteristics(multicolor_land).unwrap().mana_value,
        0
    );
    assert_eq!(
        engine.characteristics(colorless_land).unwrap().mana_value,
        0
    );
    assert_eq!(
        engine
            .characteristics(over_limit_creature)
            .unwrap()
            .mana_value,
        2
    );

    let color_union = TargetFilter {
        kind: TargetKind::AnyPermanent,
        any_of: Some(vec![
            TargetFilter {
                kind: TargetKind::AnyPermanent,
                is_color: Some(Color::Red),
                ..TargetFilter::default()
            },
            TargetFilter {
                kind: TargetKind::AnyPermanent,
                is_color: Some(Color::Blue),
                ..TargetFilter::default()
            },
        ]),
        ..TargetFilter::default()
    };
    resolve_effect(
        &mut engine,
        exile_all_effect_with_filter(ManaValueLimit::ActivatedAbilityX, color_union),
        0,
        Some(loyalty_x_owner()),
        false,
    )
    .expect("X=0 resolves without target choices");

    assert_eq!(engine.state.objects[&multicolor_land].zone, Zone::Exile);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&multicolor_land)
            .copied(),
        Some(before_generation + 1),
        "a permanent matching two color branches enters the cohort once"
    );
    assert_eq!(
        engine.state.objects[&colorless_land].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&over_limit_creature].zone,
        Zone::Battlefield,
        "a currently red MV 2 permanent exceeds the announced X=0"
    );
}

#[test]
fn exile_all_uses_current_copied_mana_value_at_resolution() {
    let mut engine = setup();
    let copied_land = deploy(&mut engine, 0, "forest");
    let copied_face = engine
        .registry
        .get("grizzly_bears")
        .expect("Grizzly Bears definition")
        .primary_face()
        .clone();
    engine
        .state
        .objects
        .get_mut(&copied_land)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "grizzly_bears".into(),
        source_face_index: 0,
        face: copied_face,
        room_faces: None,
        display_name: "Forest copying Grizzly Bears".into(),
    });
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(copied_land),
        kind: ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: 0,
    });
    assert_eq!(engine.characteristics(copied_land).unwrap().mana_value, 2);
    assert_eq!(
        engine.characteristics(copied_land).unwrap().colors,
        vec![Color::Red]
    );

    let red_filter = TargetFilter {
        kind: TargetKind::AnyPermanent,
        is_color: Some(Color::Red),
        ..TargetFilter::default()
    };
    resolve_effect(
        &mut engine,
        exile_all_effect_with_filter(ManaValueLimit::ActivatedAbilityX, red_filter),
        1,
        Some(loyalty_x_owner()),
        false,
    )
    .expect("current characteristics are evaluated at resolution");

    assert_eq!(
        engine.state.objects[&copied_land].zone,
        Zone::Battlefield,
        "the forest's printed MV is 0, but its current copied MV is 2 and exceeds X=1"
    );
}

#[test]
fn exile_all_keeps_its_literal_and_announced_x_limits_conjunctive() {
    let mut engine = setup();
    let forest = deploy(&mut engine, 0, "forest");
    let bears = deploy(&mut engine, 1, "grizzly_bears");
    let kind = TargetFilter {
        kind: TargetKind::AnyPermanent,
        max_mana_value: Some(1),
        ..TargetFilter::default()
    };

    resolve_effect(
        &mut engine,
        exile_all_effect_with_filter(ManaValueLimit::ActivatedAbilityX, kind),
        2,
        Some(loyalty_x_owner()),
        false,
    )
    .expect("the literal and dynamic bounds are both accepted");

    assert_eq!(engine.state.objects[&forest].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&bears].zone,
        Zone::Battlefield,
        "the literal max of 1 still applies when announced X=2"
    );
}

#[test]
fn exile_all_includes_a_permanent_whose_mana_value_equals_announced_x() {
    let mut engine = setup();
    let bears = deploy(&mut engine, 1, "grizzly_bears");

    resolve_effect(
        &mut engine,
        exile_all_x_effect(),
        2,
        Some(loyalty_x_owner()),
        false,
    )
    .expect("a permanent at the inclusive X ceiling resolves");

    assert_eq!(engine.state.objects[&bears].zone, Zone::Exile);
}
