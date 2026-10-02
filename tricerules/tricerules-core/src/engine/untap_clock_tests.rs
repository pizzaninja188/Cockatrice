//! Distinguishing untap-boundary regressions using admitted static ability producers.
use super::*;

fn permanent(engine: &mut GameEngine, player: PlayerId, card: &str, tapped: bool) -> ObjectId {
    let oid = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let mut object = new_object_from_card(
        oid,
        player,
        card,
        Zone::Battlefield,
        engine.registry.get(card).unwrap().primary_face(),
    );
    object.tapped = tapped;
    engine.state.objects.insert(oid, object);
    engine.state.zone_change_generation.insert(oid, 0);
    let idx = engine.state.player_idx(player).unwrap();
    engine.state.players[idx].battlefield.push(oid);
    engine.emit_static_abilities_on_enter(oid);
    oid
}

fn conditional_removal(engine: &mut GameEngine, source: ObjectId, target: ObjectId, tapped: bool) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(source),
        affected: AffectedScope::Single(target),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: Some(GameCondition::ObjectTapped {
            object: ConditionObjectRef::Source,
            tapped,
        }),
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
}

fn next_turn(engine: &mut GameEngine) -> Vec<ObjectId> {
    engine.state.turn_step = TurnStep::EndStep;
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;
    let _ = engine.initial_response_batch();
    let active = engine.state.active_player_id();
    let batch = engine
        .apply_command(
            active,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PrimitiveYieldStructured(
                    rv1::PrimitiveYieldStructured {},
                )),
            },
        )
        .unwrap();
    let mut edges = batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::PermanentsUntapped(edge)) => Some(edge.object_ids.clone()),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    edges.sort_unstable();
    edges
}

#[test]
fn self_untap_hook_uses_conditional_source_availability_from_before_all_untaps() {
    let mut engine = GameEngine::new(502_301, &[0, 1], 20, None, true).unwrap();
    let waterskin = permanent(&mut engine, 0, "benders_waterskin", true);
    let land = permanent(&mut engine, 1, "forest", true);
    conditional_removal(&mut engine, land, waterskin, false);
    assert!(engine.untaps_during_other_players_untap_steps(waterskin));
    let mut expected = vec![waterskin, land];
    expected.sort_unstable();
    assert_eq!(next_turn(&mut engine), expected);
    assert!(!engine.state.objects[&waterskin].tapped);
    assert!(!engine.untaps_during_other_players_untap_steps(waterskin));
}

#[test]
fn clock_group_membership_does_not_change_when_active_untap_restores_its_ability() {
    let mut engine = GameEngine::new(502_302, &[0, 1], 20, None, true).unwrap();
    let clock = permanent(&mut engine, 0, "unwinding_clock", true);
    let ring = permanent(&mut engine, 0, "sol_ring", true);
    let land = permanent(&mut engine, 1, "forest", true);
    conditional_removal(&mut engine, land, clock, true);
    assert!(!characteristics::printed_static_source_is_available(
        &engine.state,
        engine.registry,
        clock
    ));
    assert_eq!(next_turn(&mut engine), [land]);
    assert!(engine.state.objects[&clock].tapped);
    assert!(engine.state.objects[&ring].tapped);
    assert!(characteristics::printed_static_source_is_available(
        &engine.state,
        engine.registry,
        clock
    ));
}

fn resolved_effect(
    engine: &mut GameEngine,
    target: ObjectId,
    kind: ContinuousEffectKind,
    duration: EffectDuration,
) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(target),
        kind,
        condition: None,
        duration,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn clock_uses_nonconsecutive_multiplayer_controllers_and_current_types() {
    use tricerules_cards::primitives::{TypeLineAddition, TypeLineReplacement};
    let mut engine = GameEngine::new(502_304, &[10, 20, 30], 20, None, true).unwrap();
    let clock = permanent(&mut engine, 10, "unwinding_clock", true);
    let own_ring = permanent(&mut engine, 10, "sol_ring", true);
    let moved_ring = permanent(&mut engine, 10, "sol_ring", true);
    let gained_artifact = permanent(&mut engine, 30, "grizzly_bears", true);
    let unrelated = permanent(&mut engine, 30, "grizzly_bears", true);
    let active_land = permanent(&mut engine, 20, "forest", true);
    resolved_effect(
        &mut engine,
        clock,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(30),
        },
        EffectDuration::Indefinite,
    );
    resolved_effect(
        &mut engine,
        clock,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Enchantment],
            creature_types: vec![],
        }),
        EffectDuration::Indefinite,
    );
    resolved_effect(
        &mut engine,
        moved_ring,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(30),
        },
        EffectDuration::Indefinite,
    );
    resolved_effect(
        &mut engine,
        gained_artifact,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
        }),
        EffectDuration::Indefinite,
    );
    assert_eq!(engine.characteristics(clock).unwrap().controller, 30);
    assert!(!engine.characteristics(clock).unwrap().is_artifact());
    let mut expected = vec![moved_ring, gained_artifact, active_land];
    expected.sort_unstable();
    assert_eq!(next_turn(&mut engine), expected);
    for untouched in [clock, own_ring, unrelated] {
        assert!(engine.state.objects[&untouched].tapped);
    }
}

#[test]
fn clock_reads_scope_after_turn_start_expiry() {
    use tricerules_cards::primitives::TypeLineAddition;
    let mut engine = GameEngine::new(502_305, &[10, 20, 30], 20, None, true).unwrap();
    let clock = permanent(&mut engine, 10, "unwinding_clock", true);
    let own_ring = permanent(&mut engine, 10, "sol_ring", true);
    let other_ring = permanent(&mut engine, 30, "sol_ring", true);
    let bear = permanent(&mut engine, 10, "grizzly_bears", true);
    resolved_effect(
        &mut engine,
        clock,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(30),
        },
        EffectDuration::UntilTurnStart(20),
    );
    resolved_effect(
        &mut engine,
        bear,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
        }),
        EffectDuration::UntilTurnStart(20),
    );
    assert_eq!(engine.characteristics(clock).unwrap().controller, 30);
    assert!(engine.characteristics(bear).unwrap().is_artifact());
    let mut expected = vec![clock, own_ring];
    expected.sort_unstable();
    assert_eq!(next_turn(&mut engine), expected);
    assert!(engine.state.objects[&other_ring].tapped);
    assert!(engine.state.objects[&bear].tapped);
    assert!(!engine.characteristics(bear).unwrap().is_artifact());
}

#[test]
fn multiple_clocks_and_self_hook_make_one_attempt_and_preserve_nonactive_skip_markers() {
    let mut engine = GameEngine::new(502_306, &[0, 1], 20, None, true).unwrap();
    permanent(&mut engine, 0, "unwinding_clock", false);
    permanent(&mut engine, 0, "unwinding_clock", false);
    let waterskin = permanent(&mut engine, 0, "benders_waterskin", true);
    let prohibited = permanent(&mut engine, 0, "sol_ring", true);
    let already_untapped = permanent(&mut engine, 0, "ornithopter", false);
    let unrestricted = permanent(&mut engine, 0, "sol_ring", true);
    let active_skipped = permanent(&mut engine, 1, "forest", true);
    let active_creature = permanent(&mut engine, 1, "grizzly_bears", true);
    engine
        .state
        .objects
        .get_mut(&waterskin)
        .unwrap()
        .set_counter(CounterKind::Stun, 2);
    engine
        .state
        .objects
        .get_mut(&prohibited)
        .unwrap()
        .set_counter(CounterKind::Stun, 2);
    engine
        .state
        .objects
        .get_mut(&already_untapped)
        .unwrap()
        .set_counter(CounterKind::Stun, 2);
    engine
        .state
        .objects
        .get_mut(&unrestricted)
        .unwrap()
        .summoning_sick = true;
    engine
        .state
        .objects
        .get_mut(&active_creature)
        .unwrap()
        .summoning_sick = true;
    engine.state.skip_next_untap.insert((waterskin, 0));
    engine.state.skip_next_untap.insert((unrestricted, 0));
    engine.state.skip_next_untap.insert((active_skipped, 0));
    resolved_effect(
        &mut engine,
        prohibited,
        ContinuousEffectKind::ProhibitUntap,
        EffectDuration::Indefinite,
    );
    resolved_effect(
        &mut engine,
        unrestricted,
        ContinuousEffectKind::DoesntUntapDuringUntapStep,
        EffectDuration::Indefinite,
    );
    let mut expected = vec![unrestricted, active_creature];
    expected.sort_unstable();
    assert_eq!(next_turn(&mut engine), expected);
    assert!(engine.state.objects[&waterskin].tapped);
    assert_eq!(
        engine.state.objects[&waterskin].counter_count(CounterKind::Stun),
        1
    );
    assert!(engine.state.objects[&prohibited].tapped);
    assert_eq!(
        engine.state.objects[&prohibited].counter_count(CounterKind::Stun),
        2
    );
    assert_eq!(
        engine.state.objects[&already_untapped].counter_count(CounterKind::Stun),
        2
    );
    assert!(engine.state.skip_next_untap.contains(&(waterskin, 0)));
    assert!(engine.state.skip_next_untap.contains(&(unrestricted, 0)));
    assert!(!engine.state.skip_next_untap.contains(&(active_skipped, 0)));
    assert!(engine.state.objects[&active_skipped].tapped);
    assert!(engine.state.objects[&unrestricted].summoning_sick);
    assert!(!engine.state.objects[&active_creature].summoning_sick);
}

#[test]
fn clock_face_down_basic_land_and_ability_suppression_disable_only_current_sources() {
    for case in 0..3 {
        let mut engine = GameEngine::new(502_307 + case, &[0, 1], 20, None, true).unwrap();
        let clock = permanent(&mut engine, 0, "unwinding_clock", true);
        let ring = permanent(&mut engine, 0, "sol_ring", true);
        match case {
            0 => engine.state.objects.get_mut(&clock).unwrap().face_down = true,
            1 => resolved_effect(
                &mut engine,
                clock,
                ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
                    card_types: vec![tricerules_cards::PermanentTypeFilter::Land],
                    creature_types: Vec::new(),
                    land_types: vec![tricerules_cards::BasicLandType::Forest],
                }),
                EffectDuration::Indefinite,
            ),
            _ => resolved_effect(
                &mut engine,
                clock,
                ContinuousEffectKind::Layer6RemoveAllAbilities,
                EffectDuration::Indefinite,
            ),
        }
        assert!(!characteristics::printed_static_source_is_available(
            &engine.state,
            engine.registry,
            clock
        ));
        assert!(next_turn(&mut engine).is_empty());
        assert!(engine.state.objects[&ring].tapped);
    }
}

#[test]
fn prepared_untap_never_changes_a_new_incarnation() {
    let mut engine = GameEngine::new(502_310, &[0, 1], 20, None, true).unwrap();
    let ring = permanent(&mut engine, 0, "sol_ring", true);
    engine
        .state
        .objects
        .get_mut(&ring)
        .unwrap()
        .set_counter(CounterKind::Stun, 2);
    let plan = prepare_untap(&engine, ring).unwrap();
    engine.state.zone_change_generation.insert(ring, 2);
    assert_eq!(commit_untap(&mut engine, plan), UntapOutcome::NoChange);
    assert!(engine.state.objects[&ring].tapped);
    assert_eq!(
        engine.state.objects[&ring].counter_count(CounterKind::Stun),
        2
    );
    assert!(engine.state.untapped_this_command.is_empty());
}

#[test]
fn group_untap_empty_filter_means_all_and_nonempty_filter_means_any_listed_type() {
    for all in [false, true] {
        let mut engine =
            GameEngine::new(502_311 + u64::from(all), &[0, 1], 20, None, true).unwrap();
        let source = permanent(&mut engine, 0, "unwinding_clock", false);
        let ring = permanent(&mut engine, 0, "sol_ring", true);
        let bear = permanent(&mut engine, 0, "grizzly_bears", true);
        let land = permanent(&mut engine, 0, "forest", true);
        let mut definition = engine.copiable_values_for(source).unwrap();
        let StaticAbilityDef::UntapControlledPermanentsDuringOtherPlayersUntapSteps {
            permanent_types,
        } = &mut definition.face.static_abilities[0].definition
        else {
            panic!("actual group hook")
        };
        *permanent_types = if all {
            vec![]
        } else {
            vec![PermanentTypeFilter::Artifact, PermanentTypeFilter::Creature]
        };
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .copiable_values = Some(definition);
        let mut expected = vec![ring, bear];
        if all {
            expected.push(land);
        }
        expected.sort_unstable();
        assert_eq!(next_turn(&mut engine), expected);
        assert_eq!(engine.state.objects[&land].tapped, !all);
    }
}

fn prohibition_is_active(engine: &GameEngine, target: ObjectId) -> bool {
    let snapshot = engine.characteristics(target).unwrap();
    engine.state.continuous_effects.iter().any(|effect| {
        effect.kind == ContinuousEffectKind::ProhibitUntap
            && characteristics::effect_affects(
                &engine.state,
                engine.registry,
                effect,
                target,
                &snapshot,
            )
    })
}

#[test]
fn clock_untap_outcomes_do_not_recheck_a_prohibition_restored_by_another_untap() {
    let mut engine = GameEngine::new(502_303, &[0, 1], 20, None, true).unwrap();
    permanent(&mut engine, 0, "unwinding_clock", false);
    let ring = permanent(&mut engine, 0, "sol_ring", true);
    let land = permanent(&mut engine, 1, "forest", true);
    let aura = permanent(&mut engine, 0, "indestructibility", false);
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(ring));
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
    conditional_removal(&mut engine, land, aura, true);
    assert!(!prohibition_is_active(&engine, ring));
    let mut expected = vec![ring, land];
    expected.sort_unstable();
    assert_eq!(next_turn(&mut engine), expected);
    assert!(!engine.state.objects[&ring].tapped);
    assert!(prohibition_is_active(&engine, ring));
}
