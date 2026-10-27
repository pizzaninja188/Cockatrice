//! Actual Oath casts and trigger choices with explicitly synthetic setup objects and mana.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{ActivationLimit, CounterKind};
use tricerules_cards::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineReplacement,
};
use tricerules_core::{AffectedScope, ContinuousEffect, EngineDeck, TurnStep, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const OATH: &str = "oath_of_teferi";

fn game(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    assert!(
        registry.get(OATH).is_some(),
        "Oath of Teferi must be registered"
    );
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("create the Oath scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_oath(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, OATH);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 3,
            w: 1,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, OATH);
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Oath with its real mana cost");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    source
}

fn choose_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn dev_move(engine: &mut GameEngine, card_name: &str, zone: DevZone) {
    dev_move_for(engine, 0, card_name, zone);
}

fn dev_move_for(engine: &mut GameEngine, owner: i32, card_name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move the fixture object through the engine");
}

fn advance_to_end_step(engine: &mut GameEngine) {
    for _ in 0..10 {
        if engine.state.turn_step == TurnStep::EndStep {
            break;
        }
        let active = engine.state.active_player_id();
        engine
            .apply_scenario_command(active, &primitive_yield())
            .expect("advance toward the next end step");
    }
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
}

#[test]
fn oath_entry_targets_another_controlled_permanent_and_returns_it_to_its_owner() {
    let mut engine = game(261_027_400);
    // Synthetic setup: P0 controls this Sol Ring, which P1 owns.
    let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    engine.state.objects.get_mut(&target).unwrap().owner = 1;
    let opponent = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let source = cast_oath(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);

    for invalid in [Vec::new(), target_object(source), target_object(opponent)] {
        assert!(engine
            .apply_scenario_command(0, &choose_targets(invalid))
            .is_err());
        assert_eq!(engine.state.pending_triggers.len(), 1);
    }
    engine
        .apply_scenario_command(0, &choose_targets(target_object(target)))
        .expect("choose another permanent controlled by the trigger controller");
    let initial_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or_default();
    dev_move(&mut engine, "Oath of Teferi", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    let exile_generation = engine.state.zone_change_generation[&target];
    assert_eq!(exile_generation, initial_generation + 1);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    let watched = engine.state.active_event_observers[0]
        .watched
        .as_ref()
        .expect("exact exiled object");
    assert_eq!(watched.object_id, target);
    assert_eq!(watched.zone_change_generation, exile_generation);

    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].owner, 1);
    assert_eq!(engine.state.objects[&target].controller, 1);
    assert_eq!(
        engine.state.zone_change_generation[&target],
        exile_generation + 1
    );
    assert!(engine.state.players[1].battlefield.contains(&target));
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn oath_entry_with_no_legal_other_permanent_cannot_be_put_on_stack() {
    let mut engine = game(261_027_401);
    let source = cast_oath(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    // CR 603.3d: the entry trigger occurs, but cannot be put on the stack without its target.
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.active_event_observers.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn oath_entry_loses_its_target_if_control_or_incarnation_changes() {
    for changed_control in [true, false] {
        let mut engine = game(261_027_402 + u64::from(changed_control));
        let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        cast_oath(&mut engine);
        engine
            .apply_scenario_command(0, &choose_targets(target_object(target)))
            .expect("choose the initially legal target");
        let before = engine
            .state
            .zone_change_generation
            .get(&target)
            .copied()
            .unwrap_or_default();
        if changed_control {
            // Synthetic control change while preserving the object's incarnation.
            engine.state.players[0]
                .battlefield
                .retain(|id| *id != target);
            engine.state.players[1].battlefield.push(target);
            let object = engine.state.objects.get_mut(&target).unwrap();
            object.base_controller = 1;
            object.controller = 1;
        } else {
            dev_move(&mut engine, "Sol Ring", DevZone::Hand);
            dev_move(&mut engine, "Sol Ring", DevZone::Battlefield);
            assert!(engine.state.zone_change_generation[&target] > before);
        }
        pass_both_players(&mut engine);
        assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&target].controller,
            if changed_control { 1 } else { 0 }
        );
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.active_event_observers.is_empty());
    }
}

fn jace_after_first_use(seed: u64, with_oath: bool) -> (GameEngine, u32, u64) {
    let mut engine = game(seed);
    if with_oath {
        // No other permanent exists at entry, so the required-target trigger cannot be stacked.
        cast_oath(&mut engine);
        assert!(engine.state.pending_triggers.is_empty());
        assert!(engine.state.stack.is_empty());
    }
    // Jace and its initial loyalty are synthetic setup; all activations below are real commands.
    let jace = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&jace)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    let generation = engine
        .state
        .zone_change_generation
        .get(&jace)
        .copied()
        .unwrap_or_default();
    assert!(zone_view_ability_flags(&mut engine, 0, jace)[0]);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("activate Jace's first +2");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    (engine, jace, generation)
}

fn loyalty_uses(engine: &GameEngine, object: u32, generation: u64) -> u32 {
    let key = tricerules_core::state::ActivationUseKey {
        object_id: object,
        zone_change_generation: generation,
        identity: tricerules_core::state::ActivationUseIdentity::Loyalty,
    };
    engine
        .state
        .activation_uses_this_turn
        .get(&key)
        .copied()
        .unwrap_or_default()
}

#[test]
fn without_oath_the_normal_single_loyalty_activation_limit_remains() {
    let (mut engine, jace, generation) = jace_after_first_use(261_027_410, false);
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("without Oath a second loyalty activation is illegal");
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
}

#[test]
fn oath_offers_and_allows_the_same_loyalty_ability_a_second_time() {
    let (mut engine, jace, generation) = jace_after_first_use(261_027_411, true);
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    assert!(
        zone_view_ability_flags(&mut engine, 0, jace)[0],
        "the same ability must still be offered after the first use"
    );
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("activate the same +2 again");
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        7
    );
    pass_both_players(&mut engine);
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("Oath does not allow a third loyalty activation");
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        7
    );
}

#[test]
fn oath_allows_a_different_loyalty_ability_for_the_same_permanent() {
    let (mut engine, jace, generation) = jace_after_first_use(261_027_412, true);
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    apply_ability(&mut engine, 0, jace, 1, target_player(0))
        .expect("activate Jace's different -1 ability as its second use");
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        4
    );
    pass_both_players(&mut engine);
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("different abilities share the permanent's two-use limit");
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        4
    );
}

fn set_fixture_controller(engine: &mut GameEngine, object: u32, from: usize, to: usize) {
    engine.state.players[from]
        .battlefield
        .retain(|id| *id != object);
    engine.state.players[to].battlefield.push(object);
    let player_id = engine.state.players[to].id;
    let object = engine.state.objects.get_mut(&object).unwrap();
    object.base_controller = player_id;
    object.controller = player_id;
}

fn add_fixture_effect(engine: &mut GameEngine, object: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
}

fn set_fixture_copy_face(
    engine: &mut GameEngine,
    object: u32,
    card_id: &str,
    face: tricerules_cards::CardFace,
) {
    engine
        .state
        .objects
        .get_mut(&object)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: card_id.into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Synthetic Oath test fixture".into(),
    });
}

#[test]
fn oath_permission_tracks_source_suppression_controller_and_current_walker_type() {
    let (mut engine, jace, generation) = jace_after_first_use(261_027_420, true);
    let oath = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == OATH && object.zone == Zone::Battlefield)
        .unwrap()
        .id;
    assert!(zone_view_ability_flags(&mut engine, 0, jace)[0]);

    // Synthetic continuous effects isolate the live static and current type checks.
    add_fixture_effect(
        &mut engine,
        oath,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("suppressed Oath grants no permission");
    engine
        .state
        .continuous_effects
        .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities));
    assert!(zone_view_ability_flags(&mut engine, 0, jace)[0]);

    set_fixture_controller(&mut engine, oath, 0, 1);
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    set_fixture_controller(&mut engine, oath, 1, 0);
    assert!(zone_view_ability_flags(&mut engine, 0, jace)[0]);

    add_fixture_effect(
        &mut engine,
        jace,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
            land_types: vec![],
        }),
    );
    assert!(!engine
        .characteristics(jace)
        .unwrap()
        .has_type("Planeswalker"));
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    engine
        .state
        .continuous_effects
        .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)));
    assert!(zone_view_ability_flags(&mut engine, 0, jace)[0]);
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect("restored permission allows the second use");
}

#[test]
fn oath_entering_after_first_use_grants_permission_and_leaving_removes_it() {
    let (mut engine, jace, generation) = jace_after_first_use(261_027_421, false);
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    inject_card_into_hand(&mut engine, 0, OATH);
    let oath = move_ready_to_battlefield(&mut engine, 0, OATH);
    engine
        .apply_scenario_command(0, &choose_targets(target_object(target)))
        .expect("choose Oath's real entry target");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    assert!(zone_view_ability_flags(&mut engine, 0, jace)[0]);
    dev_move(&mut engine, "Oath of Teferi", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&oath].zone, Zone::Graveyard);
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect_err("departed Oath grants no permission");
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
}

#[test]
fn two_oath_permissions_do_not_stack() {
    let (mut engine, jace, generation) = jace_after_first_use(261_027_422, true);
    // A synthetic nonlegendary copy permits two identical sources without a legend-rule choice.
    let copy = inject_permanent_on_battlefield(&mut engine, 0, OATH);
    let mut face = tricerules_cards::registry::global()
        .get(OATH)
        .unwrap()
        .primary_face()
        .clone();
    face.supertypes.clear();
    set_fixture_copy_face(&mut engine, copy, OATH, face);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("the second activation is allowed");
    pass_both_players(&mut engine);
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("two Oaths still permit only two uses");
}

#[test]
fn oath_preserves_history_across_control_and_face_changes_but_not_blink() {
    let (mut engine, jace, generation) = jace_after_first_use(261_027_423, true);
    // Synthetic control and face-status changes retain the same battlefield incarnation.
    set_fixture_controller(&mut engine, jace, 0, 1);
    set_fixture_controller(&mut engine, jace, 1, 0);
    *engine.state.face_change_generation.entry(jace).or_default() += 1;
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("the same incarnation has one use left");
    pass_both_players(&mut engine);
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);

    dev_move(&mut engine, "Jace Beleren", DevZone::Hand);
    dev_move(&mut engine, "Jace Beleren", DevZone::Battlefield);
    let new_generation = engine.state.zone_change_generation[&jace];
    assert!(new_generation > generation);
    assert_eq!(loyalty_uses(&engine, jace, new_generation), 0);
    assert!(zone_view_ability_flags(&mut engine, 0, jace)[0]);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("new incarnation has a fresh allowance");
    assert_eq!(loyalty_uses(&engine, jace, new_generation), 1);
}

#[test]
fn oath_keeps_loyalty_timing_cost_and_individual_ability_limits() {
    let (mut engine, jace, generation) = jace_after_first_use(261_027_424, true);
    engine.state.turn_step = TurnStep::BeginCombat; // Synthetic timing setup.
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("Oath does not change sorcery timing");
    engine.state.turn_step = TurnStep::Main1;
    apply_ability(&mut engine, 0, jace, 2, target_player(0))
        .expect_err("Oath does not pay an unaffordable loyalty cost");
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect("rejected attempts do not consume the remaining use");

    let mut limited = game(261_027_425);
    cast_oath(&mut limited);
    let walker = inject_permanent_on_battlefield(&mut limited, 0, "jace_beleren");
    let mut face = tricerules_cards::registry::global()
        .get("jace_beleren")
        .unwrap()
        .primary_face()
        .clone();
    // This synthetic copied face gives +2 its own once-per-turn restriction.
    face.activated_abilities[0].activation_limit =
        Some(ActivationLimit::PerTurn { max_activations: 1 });
    set_fixture_copy_face(&mut limited, walker, "jace_beleren", face);
    limited
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    apply_ability(&mut limited, 0, walker, 0, vec![]).expect("first explicitly limited use");
    pass_both_players(&mut limited);
    apply_ability(&mut limited, 0, walker, 0, vec![])
        .expect_err("the ability's own limit remains conjunctive");
    apply_ability(&mut limited, 0, walker, 1, target_player(0))
        .expect("another ability can use Oath's remaining allowance");
}

#[test]
fn oath_counts_a_successful_activation_even_if_tidebinder_counters_it() {
    let mut engine = game(261_027_430);
    cast_oath(&mut engine);
    let jace = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&jace)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    let generation = engine
        .state
        .zone_change_generation
        .get(&jace)
        .copied()
        .unwrap_or_default();
    let hand_before = engine.state.players[0].hand.len();
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("activate Jace's +2");
    let original = engine.state.stack.last().unwrap().id;
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass to the opponent");
    inject_card_into_hand(&mut engine, 1, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "tishanas_tidebinder");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("cast Tidebinder in response");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(
            1,
            &choose_targets(vec![TargetRef {
                object_id: original,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }]),
        )
        .expect("choose the original loyalty ability");
    pass_both_players(&mut engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == original));
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before,
        "the countered draw did not resolve"
    );
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );

    // Tidebinder's actual source-linked suppression ends when its physical card leaves.
    dev_move_for(&mut engine, 1, "Tishana's Tidebinder", DevZone::Graveyard);
    assert!(zone_view_ability_flags(&mut engine, 0, jace)[0]);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("the second use remains available");
    pass_both_players(&mut engine);
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("the countered first use still counts");
}

#[test]
fn oath_does_not_count_a_real_rings_copy_as_an_activation() {
    let mut engine = game(261_027_431);
    cast_oath(&mut engine);
    let rings = inject_permanent_on_battlefield(&mut engine, 0, "rings_of_brighthearth");
    let jace = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&jace)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    let generation = engine
        .state
        .zone_change_generation
        .get(&jace)
        .copied()
        .unwrap_or_default();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("activate Jace and trigger Rings");
    assert_eq!(engine.state.stack.len(), 2);
    let batch = pass_both_players_with_batch(&mut engine);
    assert_eq!(
        find_resolution_choice(&batch).unwrap().choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose to pay for Rings");
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {2} and copy the activation");
    assert!(engine.state.stack.last().unwrap().is_copy);
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(loyalty_uses(&engine, jace, generation), 1);
    dev_move(&mut engine, "Rings of Brighthearth", DevZone::Hand);
    assert_eq!(engine.state.objects[&rings].zone, Zone::Hand);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect("a second real activation is still allowed");
    pass_both_players(&mut engine);
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("the third actual activation is illegal");
}

#[test]
fn oath_loyalty_allowance_resets_on_the_next_turn() {
    let (mut engine, jace, generation) = jace_after_first_use(261_027_432, true);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("second use on this turn");
    pass_both_players(&mut engine);
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
    let starting_turn = engine.state.turn_instance;
    for _ in 0..160 {
        if engine.state.turn_instance > starting_turn
            && engine.state.active_player_id() == 0
            && engine.state.turn_step == TurnStep::Main1
        {
            break;
        }
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(player) => {
                let index = engine.state.player_idx(player).unwrap();
                let excess = engine.state.players[index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("pass through actual turns and cleanup");
    }
    assert!(engine.state.turn_instance > starting_turn);
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(loyalty_uses(&engine, jace, generation), 0);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect("the same permanent may activate on the new turn");
    pass_both_players(&mut engine);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect("Oath also permits a second activation on the new turn");
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
}

#[test]
fn oath_two_activations_replay_as_accepted_wire_commands() {
    use prost::Message;

    fn fixture() -> (GameEngine, u32, u64) {
        let mut engine = game(261_027_433);
        cast_oath(&mut engine);
        let jace = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
        engine
            .state
            .objects
            .get_mut(&jace)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 3);
        let generation = engine
            .state
            .zone_change_generation
            .get(&jace)
            .copied()
            .unwrap_or_default();
        (engine, jace, generation)
    }
    fn record(
        engine: &mut GameEngine,
        commands: &mut Vec<(i32, RuledCommand, RuledEventBatch)>,
        actor: i32,
        command: RuledCommand,
    ) {
        let batch = engine
            .apply_scenario_command(actor, &command)
            .expect("record an accepted command");
        commands.push((actor, command, batch));
    }
    let (mut engine, jace, generation) = fixture();
    let mut commands = Vec::new();
    for index in [0, 1] {
        let targets = if index == 0 { vec![] } else { target_player(0) };
        let command = activate_ability_for(&engine, jace, index, targets);
        record(&mut engine, &mut commands, 0, command);
        for _ in 0..2 {
            let actor = engine.state.priority_player_id();
            record(&mut engine, &mut commands, actor, pass());
        }
    }
    assert_eq!(loyalty_uses(&engine, jace, generation), 2);
    let active = engine.state.active_player_id();
    record(&mut engine, &mut commands, active, primitive_yield());

    let (mut replay, replay_jace, replay_generation) = fixture();
    assert_eq!((replay_jace, replay_generation), (jace, generation));
    for (actor, command, expected) in commands {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(
            replay.apply_scenario_command(actor, &decoded).unwrap(),
            expected
        );
    }
    assert_eq!(loyalty_uses(&replay, jace, generation), 2);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

fn resolve_oath_entry(engine: &mut GameEngine, target: u32) {
    engine
        .apply_scenario_command(0, &choose_targets(target_object(target)))
        .expect("choose Oath's required target");
    pass_priority_round(engine);
}

fn counter_oath_trigger_with_tidebinder(engine: &mut GameEngine, trigger: u32) {
    if engine.state.priority_player_id() == 0 {
        engine
            .apply_scenario_command(0, &pass())
            .expect("pass to Tidebinder's controller");
    }
    inject_card_into_hand(engine, 1, "tishanas_tidebinder");
    give_mana(
        engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 1, "tishanas_tidebinder");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("cast Tidebinder in response");
    pass_priority_round(engine);
    engine
        .apply_scenario_command(
            1,
            &choose_targets(vec![TargetRef {
                object_id: trigger,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }]),
        )
        .expect("target Oath's trigger with Tidebinder");
    pass_priority_round(engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == trigger));
}

#[test]
fn oath_delayed_return_ignores_a_later_exile_incarnation_and_a_ceased_token() {
    let mut stale = game(261_027_440);
    let target = inject_permanent_on_battlefield(&mut stale, 0, "sol_ring");
    cast_oath(&mut stale);
    resolve_oath_entry(&mut stale, target);
    let captured = stale.state.zone_change_generation[&target];
    dev_move(&mut stale, "Sol Ring", DevZone::Hand);
    dev_move(&mut stale, "Sol Ring", DevZone::Exile);
    assert!(stale.state.zone_change_generation[&target] > captured);
    advance_to_end_step(&mut stale);
    pass_priority_round(&mut stale);
    assert_eq!(stale.state.objects[&target].zone, Zone::Exile);
    assert!(stale.state.active_event_observers.is_empty());

    let mut token_game = game(261_027_441);
    let token = inject_creature_on_battlefield(&mut token_game, 0, "soldier_w_1_1");
    cast_oath(&mut token_game);
    resolve_oath_entry(&mut token_game, token);
    assert!(!token_game.state.objects.contains_key(&token));
    assert_eq!(token_game.state.active_event_observers.len(), 1);
    advance_to_end_step(&mut token_game);
    pass_priority_round(&mut token_game);
    assert!(!token_game.state.objects.contains_key(&token));
    assert!(token_game.state.active_event_observers.is_empty());
}

#[test]
fn oath_original_and_delayed_triggers_are_separately_counterable() {
    for delayed in [false, true] {
        let mut engine = game(261_027_442 + u64::from(delayed));
        let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        cast_oath(&mut engine);
        engine
            .apply_scenario_command(0, &choose_targets(target_object(target)))
            .expect("choose target");
        if delayed {
            pass_priority_round(&mut engine);
            assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
            advance_to_end_step(&mut engine);
        }
        let trigger = engine
            .state
            .stack
            .last()
            .expect("counterable Oath trigger")
            .id;
        counter_oath_trigger_with_tidebinder(&mut engine, trigger);
        assert_eq!(
            engine.state.objects[&target].zone,
            if delayed {
                Zone::Exile
            } else {
                Zone::Battlefield
            }
        );
        assert!(engine.state.active_event_observers.is_empty());
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn oath_trigger_keeps_its_controller_when_the_source_changes_control() {
    let mut engine = game(261_027_444);
    let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let oath = cast_oath(&mut engine);
    engine
        .apply_scenario_command(0, &choose_targets(target_object(target)))
        .expect("choose the controlled target");
    // Synthetic source-control change occurs after the trigger chose its target.
    set_fixture_controller(&mut engine, oath, 0, 1);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    let tricerules_core::state::EventObserverPayload::StageDelayedTrigger(payload) =
        &engine.state.active_event_observers[0].payload
    else {
        panic!("delayed trigger observer")
    };
    assert_eq!(payload.controller, 0);
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.last().unwrap().controller, 0);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].controller, 0);
}

#[test]
fn oath_trigger_created_during_end_step_waits_for_the_next_end_step() {
    let mut engine = game(261_027_445);
    let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    advance_to_end_step(&mut engine);
    let created_turn = engine.state.turn_instance;
    inject_card_into_hand(&mut engine, 0, OATH);
    move_ready_to_battlefield(&mut engine, 0, OATH);
    resolve_oath_entry(&mut engine, target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.active_event_observers.len(), 1);
    for _ in 0..80 {
        if engine.state.turn_instance > created_turn {
            break;
        }
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(player) => {
                let index = engine.state.player_idx(player).unwrap();
                let excess = engine.state.players[index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("finish the current turn");
    }
    assert!(engine.state.turn_instance > created_turn);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
}

fn commander_oath_game(seed: u64) -> (GameEngine, u32) {
    let registry = tricerules_cards::registry::global();
    let decks = vec![
        EngineDeck {
            mainboard: vec!["plains".into(); 40],
            commanders: vec![],
        },
        EngineDeck {
            mainboard: vec!["forest".into(); 40],
            commanders: vec!["kami_of_the_crescent_moon".into()],
        },
    ];
    let mut engine =
        GameEngine::new_with_commander_decks(registry, seed, &[0, 1], 20, Some(decks), true)
            .expect("declared commander game");
    advance_to_main1_from_game_start(&mut engine);
    let commander = engine.state.players[1].command_zone[0];
    dev_move_for(
        &mut engine,
        1,
        "Kami of the Crescent Moon",
        DevZone::Battlefield,
    );
    // Synthetic stolen commander: owner P1, current controller P0.
    set_fixture_controller(&mut engine, commander, 1, 0);
    (engine, commander)
}

#[test]
fn oath_observes_the_commander_exile_incarnation_only_if_its_owner_keeps_it_there() {
    for to_command in [false, true] {
        let (mut engine, commander) = commander_oath_game(261_027_446 + u64::from(to_command));
        cast_oath(&mut engine);
        resolve_oath_entry(&mut engine, commander);
        assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
        assert_eq!(engine.state.active_event_observers.len(), 1);
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("commander owner chooses after exile");
        assert_eq!(pending.deciding_player, 1);
        let branch = if to_command { 0 } else { 1 };
        engine
            .apply_scenario_command(
                1,
                &RuledCommand {
                    cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                        decision: ResolutionChoiceDecision::SelectBranch as i32,
                        selected_branch_index: branch,
                        ..Default::default()
                    })),
                },
            )
            .expect("answer the command-zone state-based choice");
        advance_to_end_step(&mut engine);
        pass_priority_round(&mut engine);
        assert_eq!(
            engine.state.objects[&commander].zone,
            if to_command {
                Zone::Command
            } else {
                Zone::Battlefield
            }
        );
        if !to_command {
            assert_eq!(engine.state.objects[&commander].controller, 1);
        }
        assert!(engine.state.active_event_observers.is_empty());
    }
}

#[test]
fn oath_returning_aura_is_attached_by_its_owner_without_targeting_hexproof() {
    use tricerules_core::AttachmentRecipient;
    let mut engine = game(261_027_450);
    let host = inject_creature_on_battlefield(&mut engine, 0, "gladecover_scout");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "pacifism");
    // Synthetic foreign ownership and initial attachment; subsequent moves and choices are real.
    let object = engine.state.objects.get_mut(&aura).unwrap();
    object.owner = 1;
    object.attached_to = Some(AttachmentRecipient::Object(host));
    cast_oath(&mut engine);
    resolve_oath_entry(&mut engine, aura);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    advance_to_end_step(&mut engine);
    let delayed = engine.state.stack.last().unwrap().id;
    let before_choice = pass_both_players_with_batch(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura entry asks its owner to attach it");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(
        pending.presentation.candidates.contains(&host),
        "attachment does not target the opponent's hexproof creature"
    );
    assert!(!before_choice.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed)));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![host]))
        .expect_err("the trigger controller cannot answer the owner's choice");
    let returned = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![host]))
        .expect("the owner chooses the attachment");
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&aura].owner, 1);
    assert_eq!(engine.state.objects[&aura].controller, 1);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(host))
    );
    assert_eq!(
        returned
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed))
            .count(),
        1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn oath_leaves_a_returning_aura_in_exile_without_a_legal_host() {
    use tricerules_core::AttachmentRecipient;
    let mut engine = game(261_027_451);
    let host = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(host));
    cast_oath(&mut engine);
    resolve_oath_entry(&mut engine, aura);
    dev_move(&mut engine, "Grizzly Bears", DevZone::Graveyard);
    advance_to_end_step(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.active_event_observers.is_empty());
}

fn parked_foreign_aura(seed: u64) -> (GameEngine, u32, u32, u32) {
    use tricerules_core::AttachmentRecipient;
    let registry = tricerules_cards::registry::global();
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("plains", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("three-player Oath game");
    advance_to_main1_from_game_start(&mut engine);
    let host = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "pacifism");
    // The Aura is owned by P1 and controlled by P0; the host is controlled by P2.
    let object = engine.state.objects.get_mut(&aura).unwrap();
    object.owner = 1;
    object.attached_to = Some(AttachmentRecipient::Object(host));
    let source = inject_card_into_hand(&mut engine, 0, OATH);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            w: 1,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, OATH);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Oath in the three-player game");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    resolve_oath_entry(&mut engine, aura);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    advance_to_end_step(&mut engine);
    let delayed = engine.state.stack.last().unwrap().id;
    let mut events = Vec::new();
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        events.extend(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("resolve the delayed trigger to its entry choice")
                .events,
        );
    }
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura owner has a parked entry choice");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(pending.presentation.candidates.contains(&host));
    assert!(!events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed)));
    (engine, aura, host, delayed)
}

#[test]
fn oath_begun_aura_return_survives_trigger_controller_departure_and_replays() {
    use prost::Message;
    use tricerules_core::AttachmentRecipient;
    fn apply_wire(
        original: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
    ) -> RuledEventBatch {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let actual = original
            .apply_scenario_command(actor, &command)
            .expect("accepted original command");
        assert_eq!(
            actual,
            replay
                .apply_scenario_command(actor, &decoded)
                .expect("accepted replay command")
        );
        actual
    }
    let (mut engine, aura, host, delayed) = parked_foreign_aura(261_027_452);
    let (mut replay, other_aura, other_host, other_delayed) = parked_foreign_aura(261_027_452);
    assert_eq!(
        (aura, host, delayed),
        (other_aura, other_host, other_delayed)
    );
    let mut events = apply_wire(&mut engine, &mut replay, 0, concede()).events;
    assert!(engine.state.players[0].has_lost);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the begun return still waits for its surviving owner");
    assert_eq!(pending.deciding_player, 1);
    assert!(pending.presentation.candidates.contains(&host));
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    events.extend(
        apply_wire(
            &mut engine,
            &mut replay,
            1,
            submit_resolution_choice(vec![host]),
        )
        .events,
    );
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&aura].controller, 1);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(host))
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed))
            .count(),
        1
    );
    assert_eq!(events.iter().filter(|event| matches!(&event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == aura
            && moved.destination() == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield)).count(), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    let actor = engine.state.priority_player_id();
    apply_wire(&mut engine, &mut replay, actor, pass());
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn oath_parked_aura_return_cleans_up_when_the_aura_owner_departs() {
    let (mut engine, aura, _, delayed) = parked_foreign_aura(261_027_453);
    let batch = engine
        .apply_scenario_command(1, &concede())
        .expect("the Aura owner leaves during its entry choice");
    assert!(!engine.state.objects.contains_key(&aura));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed))
            .count(),
        1
    );
}

fn three_player_game(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("plains", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("three-player Oath game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn exile_foreign_permanent_with_oath(engine: &mut GameEngine, target: u32) {
    inject_card_into_hand(engine, 0, OATH);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 3,
            w: 1,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, OATH);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast actual Oath");
    pass_priority_round(engine);
    resolve_oath_entry(engine, target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}

fn parked_foreign_clone(seed: u64) -> (GameEngine, u32, u32, u32) {
    let mut engine = three_player_game(seed);
    let host = inject_creature_on_battlefield(&mut engine, 2, "hill_giant");
    inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let clone = inject_permanent_on_battlefield(&mut engine, 0, "clone");
    // Synthetic foreign ownership and pre-existing copy; Clone's return and new copy choice are real.
    let face = tricerules_cards::registry::global()
        .get("hill_giant")
        .unwrap()
        .primary_face()
        .clone();
    set_fixture_copy_face(&mut engine, clone, "hill_giant", face);
    engine.state.objects.get_mut(&clone).unwrap().owner = 1;
    exile_foreign_permanent_with_oath(&mut engine, clone);
    advance_to_end_step(&mut engine);
    let delayed = engine.state.stack.last().unwrap().id;
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("returning Clone offers a copy-source choice");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert!(pending.presentation.candidates.contains(&host));
    (engine, clone, host, delayed)
}

fn assert_one_return_and_completion(batch: &RuledEventBatch, object: u32, delayed: u32) {
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed))
            .count(),
        1
    );
    assert_eq!(batch.events.iter().filter(|event| matches!(&event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == object
            && moved.destination() == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield)).count(), 1);
}

#[test]
fn oath_begun_clone_return_survives_trigger_controller_departure() {
    let (mut engine, clone, host, delayed) = parked_foreign_clone(261_027_454);
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![u32::MAX]))
        .expect_err("a copy source outside the offered identities is illegal");
    engine
        .apply_scenario_command(0, &concede())
        .expect("trigger controller leaves");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the surviving owner keeps its Clone choice");
    assert_eq!(pending.deciding_player, 1);
    assert!(pending.presentation.candidates.contains(&host));
    let batch = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![host]))
        .expect("owner chooses the surviving exact copy source");
    assert_eq!(engine.state.objects[&clone].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&clone].controller, 1);
    assert_eq!(
        engine.state.objects[&clone]
            .copiable_values
            .as_ref()
            .unwrap()
            .source_card_id,
        "hill_giant"
    );
    assert_one_return_and_completion(&batch, clone, delayed);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn oath_parked_clone_return_cleans_up_when_the_owner_departs() {
    let (mut engine, clone, _, delayed) = parked_foreign_clone(261_027_455);
    let batch = engine
        .apply_scenario_command(1, &concede())
        .expect("Clone owner leaves during its entry choice");
    assert!(!engine.state.objects.contains_key(&clone));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed))
            .count(),
        1
    );
}

fn parked_foreign_walker_with_entry_replacements(seed: u64) -> (GameEngine, u32, u32) {
    let mut engine = three_player_game(seed);
    // The Seasons enter through the engine under P1's control. Synthetic ownership of the
    // second by P2 lets that replacement source leave independently of the returning owner.
    for index in 0..2 {
        inject_card_into_hand(&mut engine, 1, "doubling_season");
        let season = move_ready_to_battlefield(&mut engine, 1, "doubling_season");
        if index == 1 {
            engine.state.objects.get_mut(&season).unwrap().owner = 2;
        }
    }
    let walker = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
    // Synthetic foreign owner/current controller and initial loyalty.
    let object = engine.state.objects.get_mut(&walker).unwrap();
    object.owner = 1;
    object.set_counter(CounterKind::Loyalty, 3);
    exile_foreign_permanent_with_oath(&mut engine, walker);
    advance_to_end_step(&mut engine);
    let delayed = engine.state.stack.last().unwrap().id;
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("owner orders the two entry-counter replacements");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(pending.presentation.candidates.len(), 2);
    (engine, walker, delayed)
}

#[test]
fn oath_begun_counter_entry_survives_trigger_controller_departure() {
    let (mut engine, walker, delayed) = parked_foreign_walker_with_entry_replacements(261_027_456);
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![u32::MAX]))
        .expect_err("an unoffered replacement identity is illegal");
    engine
        .apply_scenario_command(0, &concede())
        .expect("trigger controller leaves");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("surviving owner keeps entry-replacement choice");
    assert_eq!(pending.deciding_player, 1);
    let chosen = pending.presentation.candidates[0];
    let batch = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![chosen]))
        .expect("owner orders the actual Doubling Season replacements");
    assert_eq!(engine.state.objects[&walker].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&walker].controller, 1);
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        12
    );
    assert_one_return_and_completion(&batch, walker, delayed);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn oath_parked_counter_entry_cleans_up_when_the_owner_departs() {
    let (mut engine, walker, delayed) = parked_foreign_walker_with_entry_replacements(261_027_457);
    let batch = engine
        .apply_scenario_command(1, &concede())
        .expect("walker owner leaves during its entry choice");
    assert!(!engine.state.objects.contains_key(&walker));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed))
            .count(),
        1
    );
}

#[test]
fn oath_return_refreshes_copy_sources_when_a_candidate_owner_leaves() {
    let (mut engine, clone, departed_host, delayed) = parked_foreign_clone(261_027_458);
    let surviving_host = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "grizzly_bears" && object.zone == Zone::Battlefield)
        .unwrap()
        .id;
    engine
        .apply_scenario_command(2, &concede())
        .expect("copy source owner leaves");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("returning owner still chooses");
    assert_eq!(pending.deciding_player, 1);
    assert!(!pending.presentation.candidates.contains(&departed_host));
    assert!(pending.presentation.candidates.contains(&surviving_host));
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![departed_host]))
        .expect_err("departed copy source is no longer a legal choice");
    let batch = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![surviving_host]))
        .expect("select the surviving copy source");
    assert_eq!(
        engine.state.objects[&clone]
            .copiable_values
            .as_ref()
            .unwrap()
            .source_card_id,
        "grizzly_bears"
    );
    assert_one_return_and_completion(&batch, clone, delayed);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn oath_return_refreshes_only_live_counter_replacements_after_source_departure() {
    let (mut engine, walker, delayed) = parked_foreign_walker_with_entry_replacements(261_027_459);
    let batch = engine
        .apply_scenario_command(2, &concede())
        .expect("one replacement source owner leaves");
    assert_eq!(engine.state.objects[&walker].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6
    );
    assert_one_return_and_completion(&batch, walker, delayed);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn oath_parked_return_retires_a_stale_surviving_incarnation_without_restoring_old_copy() {
    let (mut engine, clone, _, delayed) = parked_foreign_clone(261_027_460);
    let captured = engine.state.zone_change_generation[&clone];
    // Synthetic zone change while the choice is parked, to distinguish the old entry receipt
    // from a new incarnation that happens to retain the same object ID.
    engine.state.players[1].exile.retain(|id| *id != clone);
    engine.state.players[1].hand.push(clone);
    engine.state.objects.get_mut(&clone).unwrap().zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(clone)
        .or_default() += 1;
    let face = tricerules_cards::registry::global()
        .get("grizzly_bears")
        .unwrap()
        .primary_face()
        .clone();
    set_fixture_copy_face(&mut engine, clone, "grizzly_bears", face);
    let batch = engine
        .apply_scenario_command(2, &concede())
        .expect("unrelated player leaves");
    assert_eq!(engine.state.zone_change_generation[&clone], captured + 1);
    assert_eq!(engine.state.objects[&clone].zone, Zone::Hand);
    assert_eq!(
        engine.state.objects[&clone]
            .copiable_values
            .as_ref()
            .unwrap()
            .source_card_id,
        "grizzly_bears"
    );
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed))
            .count(),
        1
    );
    assert!(!batch.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == clone)));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn oath_unbegun_return_is_removed_when_its_controller_leaves() {
    for (seed, stack_trigger) in [(261_027_461, false), (261_027_462, true)] {
        let mut engine = three_player_game(seed);
        let target = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        engine.state.objects.get_mut(&target).unwrap().owner = 1;
        exile_foreign_permanent_with_oath(&mut engine, target);
        if stack_trigger {
            advance_to_end_step(&mut engine);
            assert_eq!(engine.state.stack.len(), 1);
        } else {
            assert_eq!(engine.state.active_event_observers.len(), 1);
        }
        engine
            .apply_scenario_command(0, &concede())
            .expect("trigger controller leaves before return begins");
        assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.active_event_observers.is_empty());
    }
}

#[test]
fn oath_parked_entry_does_not_preserve_a_different_captured_leaf() {
    let (mut engine, clone, _, delayed) = parked_foreign_clone(261_027_463);
    // Synthetic corruption: the parked choice's saved triggered operation no longer matches the
    // exact-owner return, so the special preservation guard must not claim it.
    let stack = engine
        .state
        .pending_resolution
        .as_mut()
        .unwrap()
        .continuation
        .stack_mut()
        .unwrap();
    stack.item.triggered_ability.as_mut().unwrap().effect =
        vec![tricerules_card_model::SpellEffectKind::None];
    engine
        .apply_scenario_command(0, &concede())
        .expect("controller leaves with the unrelated saved leaf");
    assert_eq!(engine.state.objects[&clone].zone, Zone::Exile);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.stack.iter().any(|item| item.id == delayed));
}

fn parked_koya_under_departing_aura_observer(
    seed: u64,
    counter_entry: bool,
) -> (GameEngine, u32, u32, u32, u32) {
    use tricerules_core::AttachmentRecipient;
    // Actual Koya's admitted Program is unchanged except for this synthetic observable tail.
    let source = include_str!("../../../tricerules-cards/data/koya,_death_from_above.ron")
        .replace("\r\n", "\n");
    let needle = "                ))\n              ])))";
    assert_eq!(source.matches(needle).count(), 1);
    let koya = source.replacen(needle,
        "                )),\n                (id: \"tail\", kind: Perform(GainLife(who: Controller, amount: 1)))\n              ])))", 1);
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                &koya,
                include_str!("../../../tricerules-cards/data/banishing_light.ron"),
                include_str!("../../../tricerules-cards/data/pacifism.ron"),
                include_str!("../../../tricerules-cards/data/clone.ron"),
                include_str!("../../../tricerules-cards/data/jace_beleren.ron"),
                include_str!("../../../tricerules-cards/data/doubling_season.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/generated/h/hill_giant.ron"),
                include_str!("../../../tricerules-cards/data/plains.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
            ],
            &[],
        )
        .expect("Koya return-tail and independent observer registry"),
    ));
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1, 2, 3],
        20,
        Some(vec![
            deck_with("plains", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
            deck_with("plains", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);

    let host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(host));
    // Synthetic priority setup; actual Banishing Light cast and ETB create the observer.
    engine.state.active_player_idx = 3;
    engine.state.priority_idx = 3;
    let light = inject_card_into_hand(&mut engine, 3, "banishing_light");
    give_mana(
        &mut engine,
        3,
        ManaGift {
            c: 2,
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 3, "banishing_light");
    engine
        .apply_scenario_command(3, &cast_spell(slot, vec![]))
        .expect("cast actual Banishing Light");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(3, &choose_targets(target_object(aura)))
        .expect("choose the Aura");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&light].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    assert_eq!(engine.state.active_event_observers.len(), 1);

    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    if counter_entry {
        // Both real Seasons enter under P2's control; synthetic P3 ownership makes one depart
        // while the independent Aura observer holds the replacement-order choice.
        for index in 0..2 {
            inject_card_into_hand(&mut engine, 2, "doubling_season");
            let season = move_ready_to_battlefield(&mut engine, 2, "doubling_season");
            if index == 1 {
                engine.state.objects.get_mut(&season).unwrap().owner = 3;
            }
        }
    }
    let target = inject_creature_on_battlefield(
        &mut engine,
        2,
        if counter_entry {
            "jace_beleren"
        } else {
            "clone"
        },
    );
    let face = tricerules_cards::registry::global()
        .get("hill_giant")
        .unwrap()
        .primary_face()
        .clone();
    // A synthetic creature copy makes the Jace fixture a valid Koya target; the real Jace face
    // returns after the Exile zone change. The Clone case is likewise a pre-existing copy.
    set_fixture_copy_face(&mut engine, target, "hill_giant", face);
    inject_card_into_hand(&mut engine, 0, "koya,_death_from_above");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "koya,_death_from_above");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Koya with synthetic tail");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(0, &choose_targets(target_object(target)))
        .expect("choose opposing creature");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    advance_to_end_step(&mut engine);
    let delayed = engine.state.stack.last().unwrap().id;
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Koya's optional payment");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Koya's begun entry is parked");
    assert_eq!(pending.deciding_player, 2);
    assert_eq!(
        pending.presentation.choice_kind,
        if counter_entry {
            ChoiceKind::ReplacementEffect
        } else {
            ChoiceKind::CopySource
        }
    );
    if counter_entry {
        assert_eq!(pending.presentation.candidates.len(), 2);
    }
    assert_eq!(engine.state.players[0].life, 20);
    (engine, target, aura, host, delayed)
}

#[test]
fn koya_begun_return_restores_its_typed_tail_after_an_independent_observer_choice() {
    use tricerules_core::AttachmentRecipient;
    let (mut engine, clone, aura, host, delayed) =
        parked_koya_under_departing_aura_observer(261_027_464, false);
    let mut events = engine
        .apply_scenario_command(3, &concede())
        .expect("Light owner leaves")
        .events;
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("independent Aura observer suspends the Clone choice");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(pending.presentation.candidates.contains(&host));
    assert_eq!(engine.state.objects[&clone].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].life, 20);
    events.extend(
        engine
            .apply_scenario_command(2, &concede())
            .expect("Clone owner leaves under observer choice")
            .events,
    );
    assert!(!engine.state.objects.contains_key(&clone));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::AuraPermanent
    );
    events.extend(
        engine
            .apply_scenario_command(1, &submit_resolution_choice(vec![host]))
            .expect("finish independent Aura return and restore typed continuation")
            .events,
    );
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(host))
    );
    assert_eq!(
        engine.state.players[0].life, 21,
        "surviving controller's typed tail runs once"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed))
            .count(),
        1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn koya_suspended_counter_entry_reconciles_departed_replacement_and_runs_tail_once() {
    use tricerules_core::AttachmentRecipient;
    let (mut engine, walker, aura, host, delayed) =
        parked_koya_under_departing_aura_observer(261_027_465, true);
    let mut events = engine
        .apply_scenario_command(3, &concede())
        .expect("Light and one Season owner leaves")
        .events;
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("independent Aura return suspends the counter choice");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert_eq!(engine.state.objects[&walker].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].life, 20);
    events.extend(
        engine
            .apply_scenario_command(1, &submit_resolution_choice(vec![host]))
            .expect("restore the saved counter transaction after Aura return")
            .events,
    );
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(host))
    );
    assert_eq!(engine.state.objects[&walker].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == delayed))
            .count(),
        1
    );
    assert_eq!(events.iter().filter(|event| matches!(&event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == walker
            && moved.destination() == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield)).count(), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}
