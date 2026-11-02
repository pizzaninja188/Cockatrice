use super::*;
use crate::engine::replacement::{
    CounterPlacementApplication, CounterPlacementRecipient, CounterPlacementReplacement,
    CounterReplacementInstance, PendingReplacementEvent,
};
use crate::engine::ScenarioCommandExt;
use tricerules_card_model::primitives::CounterKind;
use tricerules_card_model::{AbilityDefinitionId, AbilityId, CardFaceId, ProgramOwnerId};
use tricerules_cards::CardRegistry;
use tricerules_proto::ruled::v1 as ruled_v1;

const RETURNED_SOURCE_FIXTURE: &str = r#"(
    id: "tekuthal_proliferate_return_fixture",
    name: "Tekuthal Proliferate Return Fixture",
    face_id: "tekuthal_proliferate_return_fixture",
    types: ["Creature", "Phyrexian", "Horror"],
    power: Some(3),
    toughness: Some(5),
    static_abilities: [(
        ability_id: "tekuthal_static",
        presentation: Fallback,
        definition: DoubleControllerProliferations,
    )],
)"#;

fn identity(
    source_object_id: ObjectId,
    generation: u64,
    occurrence: &str,
) -> ProliferateReplacementIdentity {
    ProliferateReplacementIdentity {
        source_object_id,
        zone_change_generation: generation,
        ability_occurrence: AbilityDefinitionId {
            card_id: "tekuthal_proliferate_fixture".into(),
            face_id: CardFaceId::new("front").unwrap(),
            ability_path: vec![AbilityId::new(occurrence).unwrap()],
        },
    }
}

fn battlefield_instance(application: &CounterPlacementApplication) -> &CounterReplacementInstance {
    match &application.replacement {
        CounterPlacementReplacement::Battlefield {
            instance,
            member_event_indices,
            source_controller,
        } => {
            assert_eq!(member_event_indices.len(), 1);
            assert_eq!(member_event_indices[0], application.event_index);
            assert!(source_controller.is_none());
            instance
        }
        CounterPlacementReplacement::Compleated { .. } => {
            panic!("Proliferate counter replacement must be a battlefield operation")
        }
    }
}

#[test]
fn replacement_lineage_branches_independently_and_advances_depth_first() {
    let first = identity(41, 3, "first_occurrence");
    let second = identity(42, 7, "second_occurrence");
    let mut transaction = PendingProliferateTransaction::new(0);
    transaction.current_leaf = transaction.pending_nodes.pop_front();

    GameEngine::split_current_proliferate_node(&mut transaction, first.clone())
        .expect("first replacement splits the root");
    assert_eq!(transaction.pending_nodes.len(), 2);
    let first_child = transaction.pending_nodes.pop_front().unwrap();
    let sibling_child = transaction.pending_nodes.pop_front().unwrap();
    assert_eq!(
        first_child.applied_replacements,
        BTreeSet::from([first.clone()])
    );
    assert_eq!(
        sibling_child.applied_replacements,
        BTreeSet::from([first.clone()])
    );

    transaction.pending_nodes.push_back(sibling_child);
    transaction.current_leaf = Some(first_child);
    GameEngine::split_current_proliferate_node(&mut transaction, second.clone())
        .expect("a different occurrence splits the first child");
    assert_eq!(transaction.pending_nodes.len(), 3);
    let nested_first = transaction.pending_nodes.pop_front().unwrap();
    let nested_second = transaction.pending_nodes.pop_front().unwrap();
    let parent_other_child = transaction.pending_nodes.pop_front().unwrap();
    let nested_lineage = BTreeSet::from([first.clone(), second]);
    assert_eq!(nested_first.applied_replacements, nested_lineage);
    assert_eq!(
        nested_second.applied_replacements,
        nested_first.applied_replacements
    );
    assert_eq!(
        parent_other_child.applied_replacements,
        BTreeSet::from([first.clone()])
    );

    let mut isolated_sibling = parent_other_child;
    isolated_sibling
        .applied_replacements
        .insert(identity(43, 1, "third_occurrence"));
    assert_eq!(nested_first.applied_replacements, nested_lineage);
}

#[test]
fn source_incarnation_and_effective_ability_path_are_distinct_lineage_keys() {
    let original = identity(41, 3, "first_occurrence");
    let new_incarnation = identity(41, 4, "first_occurrence");
    let sibling_occurrence = identity(41, 3, "second_occurrence");

    assert_ne!(original, new_incarnation);
    assert_ne!(original, sibling_occurrence);
}

fn put_local_fixture_on_battlefield(engine: &mut GameEngine, card_id: &str) -> ObjectId {
    let player = engine.state.players[0].id;
    let object_id = *engine.state.players[0]
        .hand
        .first()
        .expect("fixture deck has another Forest in hand");
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("fixture card object")
        .card_id = card_id.into();
    crate::engine::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        object_id,
        Zone::Battlefield,
        Some(player),
    )
    .expect("fixture permanent enters through central zone-change handling");
    object_id
}

fn submit_choice(chosen_object_ids: Vec<ObjectId>) -> ruled_v1::RuledCommand {
    ruled_v1::RuledCommand {
        cmd: Some(ruled_v1::ruled_command::Cmd::SubmitResolutionChoice(
            ruled_v1::SubmitResolutionChoice {
                resolution_prompt_id: 0,
                chosen_object_ids,
                decision: ruled_v1::ResolutionChoiceDecision::Unspecified as i32,
                ..Default::default()
            },
        )),
    }
}

fn published_choice(
    batch: &ruled_v1::RuledEventBatch,
    choice_kind: ruled_v1::ChoiceKind,
) -> ruled_v1::ResolutionChoiceRequired {
    batch
        .events
        .iter()
        .rev()
        .find_map(|event| match event.ev.as_ref()? {
            ruled_v1::ruled_event::Ev::ResolutionChoiceRequired(choice)
                if choice.choice_kind == choice_kind as i32 =>
            {
                Some(choice.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("the command batch publishes the {choice_kind:?} choice"))
}

fn pass_live_players_with_batch(engine: &mut GameEngine) -> ruled_v1::RuledEventBatch {
    let live_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let mut batch = ruled_v1::RuledEventBatch::default();
    for _ in 0..live_players {
        let actor = engine.state.priority_player_id();
        batch = engine
            .apply_scenario_command(
                actor,
                &ruled_v1::RuledCommand {
                    cmd: Some(ruled_v1::ruled_command::Cmd::PassPriority(
                        ruled_v1::PassPriority {},
                    )),
                },
            )
            .expect("each live player passes priority");
    }
    batch
}

fn proliferate_log_count(batch: &ruled_v1::RuledEventBatch, actor: PlayerId) -> usize {
    let expected = format!("P{actor} proliferates.");
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_v1::ruled_event::Ev::Log(log) => Some(&log.text),
            _ => None,
        })
        .filter(|text| text.as_str() == expected)
        .count()
}

fn stack_resolved_count(batch: &ruled_v1::RuledEventBatch) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.ev.as_ref(),
                Some(ruled_v1::ruled_event::Ev::StackResolved(_))
            )
        })
        .count()
}

fn saved_observer_registry(include_ezuri: bool) -> &'static CardRegistry {
    let mut chunks = vec![
        include_str!("../../../../tricerules-cards/data/forest.ron"),
        include_str!("../../../../tricerules-cards/data/karns_bastion.ron"),
        include_str!("../../../../tricerules-cards/data/ripples_of_potential.ron"),
        include_str!("../../../../tricerules-cards/data/pacifism.ron"),
        include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
        RETURNED_SOURCE_FIXTURE,
    ];
    if include_ezuri {
        chunks.push(include_str!(
            "../../../../tricerules-cards/data/ezuri_stalker_of_spheres.ron"
        ));
    }
    Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(&chunks, &[])
            .expect("unchanged local cards and the Proliferate fixture validate"),
    ))
}

fn saved_observer_registry_with_doubling_season() -> &'static CardRegistry {
    let chunks = [
        include_str!("../../../../tricerules-cards/data/forest.ron"),
        include_str!("../../../../tricerules-cards/data/karns_bastion.ron"),
        include_str!("../../../../tricerules-cards/data/ripples_of_potential.ron"),
        include_str!("../../../../tricerules-cards/data/pacifism.ron"),
        include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
        include_str!("../../../../tricerules-cards/data/doubling_season.ron"),
        RETURNED_SOURCE_FIXTURE,
    ];
    Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(&chunks, &[])
            .expect("unchanged local cards and the Proliferate fixture validate"),
    ))
}

fn saved_observer_registry_with_doubling_season_and_ezuri() -> &'static CardRegistry {
    let chunks = [
        include_str!("../../../../tricerules-cards/data/forest.ron"),
        include_str!("../../../../tricerules-cards/data/karns_bastion.ron"),
        include_str!("../../../../tricerules-cards/data/ripples_of_potential.ron"),
        include_str!("../../../../tricerules-cards/data/pacifism.ron"),
        include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
        include_str!("../../../../tricerules-cards/data/doubling_season.ron"),
        include_str!("../../../../tricerules-cards/data/ezuri_stalker_of_spheres.ron"),
        RETURNED_SOURCE_FIXTURE,
    ];
    Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(&chunks, &[])
            .expect("unchanged local cards and the Proliferate fixture validate"),
    ))
}

fn saved_observer_engine(seed: u64, registry: &'static CardRegistry) -> GameEngine {
    let deck = vec!["forest".to_string(); 40];
    let mut engine = GameEngine::new(registry, seed, &[0, 1, 2, 3], 20, Some(vec![deck; 4]), true)
        .expect("four-player saved-observer fixture engine");
    engine.state.turn_step = crate::state::TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine
}

fn put_saved_observer_card(
    engine: &mut GameEngine,
    player_index: usize,
    card_id: &str,
    destination: Zone,
) -> ObjectId {
    let player = engine.state.players[player_index].id;
    let object_id = *engine.state.players[player_index]
        .hand
        .first()
        .expect("fixture deck has another Forest in hand");
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("fixture card object")
        .card_id = card_id.into();
    crate::engine::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        object_id,
        destination,
        (destination == Zone::Battlefield).then_some(player),
    )
    .expect("prepared card moves through central zone-change handling");
    object_id
}

fn cast_saved_ripples(engine: &mut GameEngine) -> (ObjectId, ruled_v1::RuledEventBatch) {
    let player = engine.state.players[0].id;
    let spell = *engine.state.players[0]
        .hand
        .first()
        .expect("P0 has a card available for the actual Ripples cast");
    engine
        .state
        .objects
        .get_mut(&spell)
        .expect("actual Ripples card object")
        .card_id = "ripples_of_potential".into();
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|&object_id| object_id == spell)
        .expect("Ripples remains in P0's hand before casting");
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    let cast = ruled_v1::RuledCommand {
        cmd: Some(ruled_v1::ruled_command::Cmd::CastSpell(
            ruled_v1::CastSpell {
                source: Some(ruled_v1::CastSource {
                    expected_zone_change_generation: None,
                    location: Some(ruled_v1::cast_source::Location::HandIndex(
                        hand_index as u32,
                    )),
                }),
                cast_method: ruled_v1::CastMethod::Normal as i32,
                ..Default::default()
            },
        )),
    };
    engine
        .apply_scenario_command(player, &cast)
        .expect("cast the actual Ripples of Potential from P0's hand");
    assert_eq!(engine.state.stack.last().map(|item| item.id), Some(spell));

    let mut batch = pass_live_players_with_batch(engine);
    while engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| {
            pending.presentation.choice_kind == ruled_v1::ChoiceKind::ReplacementEffect
        })
    {
        let choice = published_choice(&batch, ruled_v1::ChoiceKind::ReplacementEffect);
        assert_eq!(choice.deciding_player_id, player);
        assert!(!choice.replacement_options.is_empty());
        batch = engine
            .apply_scenario_command(
                player,
                &submit_choice_with_prompt_id(
                    choice.resolution_prompt_id,
                    vec![choice.replacement_options[0].application_id],
                ),
            )
            .expect("P0 answers any actual Proliferate replacement order");
    }
    let first_choice = published_choice(&batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(first_choice.deciding_player_id, player);
    (spell, batch)
}

fn cast_saved_ripples_until_outer_replacement_order(
    engine: &mut GameEngine,
) -> (ObjectId, ruled_v1::RuledEventBatch) {
    let player = engine.state.players[0].id;
    let spell = *engine.state.players[0]
        .hand
        .first()
        .expect("P0 has a card available for the actual Ripples cast");
    engine
        .state
        .objects
        .get_mut(&spell)
        .expect("actual Ripples card object")
        .card_id = "ripples_of_potential".into();
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|&object_id| object_id == spell)
        .expect("Ripples remains in P0's hand before casting");
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_scenario_command(
            player,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::CastSpell(
                    ruled_v1::CastSpell {
                        source: Some(ruled_v1::CastSource {
                            expected_zone_change_generation: None,
                            location: Some(ruled_v1::cast_source::Location::HandIndex(
                                hand_index as u32,
                            )),
                        }),
                        cast_method: ruled_v1::CastMethod::Normal as i32,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("cast the actual Ripples of Potential from P0's hand");
    assert_eq!(engine.state.stack.last().map(|item| item.id), Some(spell));
    let batch = pass_live_players_with_batch(engine);
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| &pending.continuation),
        Some(ResolutionContinuation::ProliferateReplacement { .. })
    ));
    (spell, batch)
}

fn queue_saved_pacifism_return(
    engine: &mut GameEngine,
    pacifism: ObjectId,
) -> ruled_v1::RuledEventBatch {
    let generation = engine.state.zone_change_generation[&pacifism];
    engine.state.pending_immediate_observer_actions.push(
        crate::state::ImmediateObserverAction::ReturnExiledObject {
            exiled: crate::state::TriggerObjectRef {
                object_id: pacifism,
                zone_change_generation: generation,
                controller_at_event: 2,
            },
        },
    );
    let mut events = Vec::new();
    assert!(engine
        .drain_immediate_observer_actions(None, &mut events)
        .expect("drain the actual owed Pacifism observer return"));
    engine
        .publish_resolution_prompt_ids(&mut events)
        .expect("publish the independent Aura choice");
    ruled_v1::RuledEventBatch {
        events,
        ..Default::default()
    }
}

fn find_published_choice(
    batch: &ruled_v1::RuledEventBatch,
    choice_kind: ruled_v1::ChoiceKind,
) -> Option<ruled_v1::ResolutionChoiceRequired> {
    batch
        .events
        .iter()
        .rev()
        .find_map(|event| match event.ev.as_ref()? {
            ruled_v1::ruled_event::Ev::ResolutionChoiceRequired(choice)
                if choice.choice_kind == choice_kind as i32 =>
            {
                Some(choice.clone())
            }
            _ => None,
        })
}

fn ripples_resolution_log_count(batch: &ruled_v1::RuledEventBatch) -> usize {
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_v1::ruled_event::Ev::Log(log) => Some(&log.text),
            _ => None,
        })
        .filter(|text| text.as_str() == "Ripples of Potential resolves.")
        .count()
}

#[test]
fn returned_source_incarnation_doubles_unstarted_sibling_without_replaying_first_leaf() {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/karns_bastion.ron"),
            RETURNED_SOURCE_FIXTURE,
        ],
        &[],
    )
    .expect("local source fixture and unchanged Forest/Bastion definitions validate");
    let registry = Box::leak(Box::new(registry));
    let deck = vec!["forest".to_string(); 40];
    let mut engine = GameEngine::new(
        registry,
        820_001,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("private returned-incarnation fixture engine");
    engine.state.turn_step = crate::state::TurnStep::Main1;
    engine.state.priority_idx = 0;

    let source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let recipient = put_local_fixture_on_battlefield(&mut engine, "forest");
    let bastion = put_local_fixture_on_battlefield(&mut engine, "karns_bastion");
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let source_snapshot = engine
        .copiable_values_for(source)
        .expect("the local source has registry-backed copiable values");
    {
        let source_object = engine
            .state
            .objects
            .get_mut(&source)
            .expect("original source incarnation");
        source_object.copiable_values = Some(source_snapshot);
        source_object.copy_revision += 1;
        source_object.set_counter(CounterKind::Charge, 1);
    }
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("counter-bearing Forest recipient")
        .set_counter(CounterKind::Charge, 1);
    engine.reconcile_activated_ability_slots();
    engine.state.players[0].mana_pool.colorless = 4;

    let activation = ruled_v1::ActivateAbility {
        source_object_id: bastion,
        ability_index: 1,
        expected_zone_change_generation: engine.payment_object_ref(bastion).zone_change_generation,
        source_zone: ruled_v1::AbilitySourceZone::Battlefield as i32,
        ..Default::default()
    };
    engine
        .apply_scenario_command(
            0,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::ActivateAbility(activation)),
            },
        )
        .expect("P0 activates shipped Karn's Bastion with the captured generation");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let mut batch = pass_live_players_with_batch(&mut engine);
    let mut stack_resolutions = stack_resolved_count(&batch);

    while engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| {
            pending.presentation.choice_kind == ruled_v1::ChoiceKind::ReplacementEffect
        })
    {
        let choice = published_choice(&batch, ruled_v1::ChoiceKind::ReplacementEffect);
        assert!(!choice.replacement_options.is_empty());
        assert!(choice.replacement_options.iter().all(|option| {
            option.source_object_id == source
                && option.source_zone_change_generation == original_generation
        }));
        let application_id = choice.replacement_options[0].application_id;
        assert_ne!(application_id, 0);
        batch = engine
            .apply_scenario_command(0, &submit_choice(vec![application_id]))
            .expect("P0 chooses a published original-incarnation application");
        stack_resolutions += stack_resolved_count(&batch);
    }
    let first_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the original replacement has opened its first Proliferate leaf");
    assert_eq!(
        first_prompt.presentation.choice_kind,
        ruled_v1::ChoiceKind::Proliferate
    );
    assert!(first_prompt.presentation.candidates.contains(&source));
    assert!(first_prompt.presentation.candidates.contains(&recipient));

    // Prepared-edge integration: these central moves park the first leaf and return the same
    // object. This proves continuation behavior against zone-change bookkeeping, not lawful blink
    // reachability or a priority window between replacement children.
    crate::engine::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        source,
        Zone::Graveyard,
        None,
    )
    .expect("prepare the original source's departure while its first leaf is parked");
    crate::engine::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        source,
        Zone::Battlefield,
        Some(0),
    )
    .expect("return the same source object through central battlefield-entry handling");
    let returned_generation = engine.state.zone_change_generation[&source];
    assert_eq!(returned_generation, original_generation + 2);
    assert!(engine.state.objects[&source].copiable_values.is_none());
    assert_eq!(engine.state.objects[&source].copy_revision, 0);
    assert!(engine.state.objects[&source]
        .active_copy_occurrence
        .is_none());
    assert!(engine.state.objects[&source].counters.is_empty());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );

    batch = engine
        .apply_scenario_command(0, &submit_choice(vec![recipient]))
        .expect("complete the already-started first leaf exactly once");
    let mut completed_leaves = 1;
    let mut proliferate_logs = proliferate_log_count(&batch, 0);
    stack_resolutions += stack_resolved_count(&batch);
    assert_eq!(proliferate_logs, 1);

    for step in 0..24 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            break;
        };
        assert!(
            step < 23,
            "returned-source continuation exceeded its bounded test guard"
        );
        match pending.presentation.choice_kind {
            ruled_v1::ChoiceKind::ReplacementEffect => {
                let choice = published_choice(&batch, ruled_v1::ChoiceKind::ReplacementEffect);
                assert!(!choice.replacement_options.is_empty());
                assert!(choice.replacement_options.iter().all(|option| {
                    option.source_object_id == source
                        && option.source_zone_change_generation == returned_generation
                }));
                let application_id = choice.replacement_options[0].application_id;
                assert_ne!(application_id, 0);
                batch = engine
                    .apply_scenario_command(0, &submit_choice(vec![application_id]))
                    .expect("P0 chooses the returned incarnation's published application");
                stack_resolutions += stack_resolved_count(&batch);
            }
            ruled_v1::ChoiceKind::Proliferate => {
                assert_eq!(
                    pending.presentation.candidates,
                    vec![recipient],
                    "fresh siblings omit the returned source after its counters were cleared"
                );
                batch = engine
                    .apply_scenario_command(0, &submit_choice(vec![recipient]))
                    .expect("complete one returned-source child by ordinary Proliferate choice");
                assert_eq!(proliferate_log_count(&batch, 0), 1);
                proliferate_logs += 1;
                completed_leaves += 1;
                stack_resolutions += stack_resolved_count(&batch);
            }
            other => panic!("unexpected returned-source continuation choice {other:?}"),
        }
    }

    assert_eq!(
        completed_leaves, 3,
        "the returned source splits only the unstarted sibling"
    );
    assert_eq!(proliferate_logs, 3, "P0 completes three distinct leaves");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        4
    );
    assert_eq!(
        stack_resolutions, 1,
        "the original Bastion stack item exits once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.objects[&bastion].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

fn local_legacy_proliferate_item(source: ObjectId, generation: u64) -> crate::state::StackItem {
    let mut item = crate::state::StackItem {
        phyrexian_symbols_paid_with_life: 0,
        prototyped: false,
        mana_colors_spent_to_cast: Default::default(),
        id: source + 10_000,
        controller: 0,
        card_id: "capture_sphere".to_string(),
        targets: vec![],
        ability_text: Some("When this Aura enters, tap enchanted creature.".to_string()),
        source_permanent_id: Some(source),
        source_owner: Some(0),
        source_zone_change: generation,
        trigger_use_key: None,
        source_face_change: 0,
        ability_index: Some(0),
        activated_ability: None,
        activated_ability_occurrence: None,
        triggered_ability: None,
        is_triggered: true,
        is_copy: false,
        face_index: 0,
        cast_method: crate::state::SpellCastMethod::Normal,
        returned_attacker_assignment: None,
        chosen_x: 0,
        chosen_modes: vec![],
        cast_condition_results: Vec::new(),
        cast_occurrence: None,
        cast_by: None,
        cast_cost_receipts: vec![],
        payment_result: crate::state::CardResultCohort::default(),
        exiled_cohorts: Default::default(),
        chaos_warp_owner_instructions: Default::default(),
        resolution_branch_choices: Default::default(),
        blight_receipts: Vec::new(),
        trigger_context: crate::state::TriggerContext::default(),
    };
    let mut ability = tricerules_cards::registry::global()
        .get("brambleguard_captain")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.effect = vec![
        tricerules_card_model::primitives::SpellEffectKind::Proliferate,
        tricerules_card_model::primitives::SpellEffectKind::GainLife {
            amount: tricerules_card_model::primitives::Amount::Fixed(2),
            who: tricerules_card_model::primitives::PlayerRecipient::Controller,
        },
    ];
    item.triggered_ability = Some(ability);
    item
}

fn submit_choice_with_prompt_id(
    resolution_prompt_id: u64,
    chosen_object_ids: Vec<ObjectId>,
) -> ruled_v1::RuledCommand {
    let mut command = submit_choice(chosen_object_ids);
    if let Some(ruled_v1::ruled_command::Cmd::SubmitResolutionChoice(choice)) = command.cmd.as_mut()
    {
        choice.resolution_prompt_id = resolution_prompt_id;
    }
    command
}

fn submit_branch_with_prompt_id(
    resolution_prompt_id: u64,
    selected_branch_index: u32,
) -> ruled_v1::RuledCommand {
    ruled_v1::RuledCommand {
        cmd: Some(ruled_v1::ruled_command::Cmd::SubmitResolutionChoice(
            ruled_v1::SubmitResolutionChoice {
                resolution_prompt_id,
                selected_branch_index,
                decision: ruled_v1::ResolutionChoiceDecision::SelectBranch as i32,
                ..Default::default()
            },
        )),
    }
}

#[test]
fn observer_restore_keeps_committed_doubled_leaf_and_runs_legacy_tail_once() {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/karns_bastion.ron"),
            RETURNED_SOURCE_FIXTURE,
        ],
        &[],
    )
    .expect("local source fixture and unchanged Forest/Bastion definitions validate");
    let registry = Box::leak(Box::new(registry));
    let deck = vec!["forest".to_string(); 40];
    let mut engine = GameEngine::new(
        registry,
        820_005,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("private observer-restore fixture engine");
    engine.state.turn_step = crate::state::TurnStep::Main1;
    engine.state.priority_idx = 0;

    let source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let recipient = put_local_fixture_on_battlefield(&mut engine, "forest");
    let source_generation = engine.state.zone_change_generation[&source];
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("counter-bearing Forest recipient")
        .set_counter(CounterKind::Charge, 1);
    engine.reconcile_activated_ability_slots();

    let item = local_legacy_proliferate_item(source, source_generation);
    engine.state.stack.push(item.clone());
    let (effects, spell_label) = engine.build_resolution_effects(&item);
    let mut initial_events = Vec::new();
    let progress = engine
        .run_effect_list_with_previous(
            &item,
            &spell_label,
            effects,
            0,
            OperationResults::default(),
            &mut initial_events,
        )
        .expect("prepared legacy Proliferate resolution parks for its first answer");
    assert!(matches!(
        progress,
        crate::engine::resolution::ResolutionProgress::Parked
    ));
    engine
        .publish_resolution_prompt_ids(&mut initial_events)
        .unwrap();
    let mut batch = ruled_v1::RuledEventBatch {
        events: initial_events,
        ..Default::default()
    };
    let mut action_batches = vec![batch.clone()];

    while engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| {
            pending.presentation.choice_kind == ruled_v1::ChoiceKind::ReplacementEffect
        })
    {
        let choice = published_choice(&batch, ruled_v1::ChoiceKind::ReplacementEffect);
        assert_eq!(choice.replacement_options.len(), 1);
        let replacement_id = choice.replacement_options[0].application_id;
        batch = engine
            .apply_scenario_command(
                0,
                &submit_choice_with_prompt_id(choice.resolution_prompt_id, vec![replacement_id]),
            )
            .expect("P0 applies the one local doubling replacement");
        action_batches.push(batch.clone());
    }
    let first_prompt = published_choice(&batch, ruled_v1::ChoiceKind::Proliferate);
    assert!(first_prompt.candidate_object_ids.contains(&recipient));
    let first_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(first_prompt.resolution_prompt_id, vec![recipient]),
        )
        .expect("P0 chooses R for the first doubled leaf");
    action_batches.push(first_batch.clone());
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(proliferate_log_count(&first_batch, 0), 1);

    let second_prompt = published_choice(&first_batch, ruled_v1::ChoiceKind::Proliferate);
    let (parked_stack, parked_transaction, parked_prompt_id) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the second doubled leaf is parked");
        assert_eq!(
            pending.presentation.choice_kind,
            ruled_v1::ChoiceKind::Proliferate
        );
        assert_ne!(
            pending.resolution_prompt_id,
            crate::state::ResolutionPromptId::UNASSIGNED
        );
        let ResolutionContinuation::Proliferate {
            stack, transaction, ..
        } = &pending.continuation
        else {
            panic!("the second doubled leaf remains in its Proliferate transaction");
        };
        assert!(stack.program.is_none());
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 1);
        assert!(transaction.pending_nodes.is_empty());
        let current_leaf = transaction
            .current_leaf
            .as_ref()
            .expect("the active second leaf is retained");
        assert_eq!(current_leaf.applied_replacements.len(), 1);
        let applied = current_leaf.applied_replacements.iter().next().unwrap();
        assert_eq!(applied.source_object_id, source);
        assert_eq!(applied.zone_change_generation, source_generation);
        assert_eq!(transaction.counter_placements.len(), 1);
        let receipt = transaction.counter_placements[0];
        assert_eq!(receipt.object.object_id, recipient);
        assert_eq!(
            receipt.object.zone_change_generation,
            engine.state.zone_change_generation[&recipient]
        );
        assert_eq!(receipt.counter, CounterKind::Charge);
        assert_eq!(receipt.count, 1);
        assert_eq!((receipt.before, receipt.after), (1, 2));
        assert_eq!(stack.item.source_permanent_id, Some(source));
        assert_eq!(stack.item.source_zone_change, source_generation);
        assert_eq!(stack.resume_effect_index, Some(1));
        (
            format!("{stack:?}"),
            format!("{transaction:?}"),
            second_prompt.resolution_prompt_id,
        )
    };
    let global_stack = format!("{:?}", engine.state.stack);
    assert_eq!(engine.state.players[0].life, 20);

    let suspended_resolution = engine
        .state
        .pending_resolution
        .take()
        .expect("capture the actual pending observer-suspended resolution");
    let suspended_replacement = engine.state.pending_replacement_event.take();
    let mut restored_events = Vec::new();
    engine
        .restore_suspended_observer_resolution(
            Some(suspended_resolution),
            suspended_replacement,
            &mut restored_events,
        )
        .expect("restore the observer-suspended Proliferate leaf");
    engine
        .publish_resolution_prompt_ids(&mut restored_events)
        .unwrap();
    let restored_batch = ruled_v1::RuledEventBatch {
        events: restored_events,
        ..Default::default()
    };
    action_batches.push(restored_batch.clone());
    assert_eq!(proliferate_log_count(&restored_batch, 0), 0);
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(engine.state.players[0].life, 20);

    let restored_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("observer restoration keeps the active child pending");
    let ResolutionContinuation::Proliferate {
        stack, transaction, ..
    } = &restored_pending.continuation
    else {
        panic!("observer restore restarted the Proliferate root instead of preserving child two");
    };
    assert_eq!(format!("{stack:?}"), parked_stack);
    assert_eq!(format!("{transaction:?}"), parked_transaction);
    assert_eq!(format!("{:?}", engine.state.stack), global_stack);
    let restored_prompt = published_choice(&restored_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_ne!(restored_prompt.resolution_prompt_id, parked_prompt_id);
    assert_eq!(restored_prompt.candidate_object_ids, vec![recipient]);

    let pending_before_stale = format!("{restored_pending:?}");
    let charge_before_stale = engine.state.objects[&recipient].counter_count(CounterKind::Charge);
    let life_before_stale = engine.state.players[0].life;
    assert!(engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(parked_prompt_id, vec![recipient]),
        )
        .is_err());
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution.as_ref().unwrap()),
        pending_before_stale
    );
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        charge_before_stale
    );
    assert_eq!(engine.state.players[0].life, life_before_stale);
    assert_eq!(format!("{:?}", engine.state.stack), global_stack);

    let final_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(restored_prompt.resolution_prompt_id, vec![recipient]),
        )
        .expect("P0 chooses R for the restored second doubled leaf");
    action_batches.push(final_batch.clone());
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        3
    );
    assert_eq!(engine.state.players[0].life, 22);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        action_batches
            .iter()
            .map(|batch| proliferate_log_count(batch, 0))
            .sum::<usize>(),
        2
    );
    assert_eq!(
        action_batches
            .iter()
            .map(stack_resolved_count)
            .sum::<usize>(),
        1
    );
}

#[test]
fn delegated_observer_restore_keeps_committed_doubled_leaf_after_actor_concedes() {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/karns_bastion.ron"),
            RETURNED_SOURCE_FIXTURE,
        ],
        &[],
    )
    .expect("local source fixture and unchanged Forest/Bastion definitions validate");
    let registry = Box::leak(Box::new(registry));
    let deck = vec!["forest".to_string(); 40];
    let mut engine = GameEngine::new(
        registry,
        820_006,
        &[0, 1, 2, 3],
        20,
        Some(vec![deck.clone(), deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("private four-player delegated observer fixture engine");
    engine.state.turn_step = crate::state::TurnStep::Main1;
    engine.state.priority_idx = 0;

    let source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let recipient_player = engine.state.players[1].id;
    let recipient = *engine.state.players[1]
        .hand
        .first()
        .expect("P1 fixture deck has another Forest in hand");
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("P1's physical Forest object")
        .card_id = "forest".into();
    crate::engine::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        recipient,
        Zone::Battlefield,
        Some(recipient_player),
    )
    .expect("P1's recipient enters through central zone-change handling");
    let source_generation = engine.state.zone_change_generation[&source];
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("P1's counter-bearing Forest recipient")
        .set_counter(CounterKind::Charge, 1);
    engine.reconcile_activated_ability_slots();

    let item = local_legacy_proliferate_item(source, source_generation);
    engine.state.stack.push(item.clone());
    let (effects, spell_label) = engine.build_resolution_effects(&item);
    let mut initial_events = Vec::new();
    let progress = engine
        .run_effect_list_with_previous(
            &item,
            &spell_label,
            effects,
            0,
            OperationResults::default(),
            &mut initial_events,
        )
        .expect("prepared legacy Proliferate resolution parks for its first answer");
    assert!(matches!(
        progress,
        crate::engine::resolution::ResolutionProgress::Parked
    ));
    engine
        .publish_resolution_prompt_ids(&mut initial_events)
        .unwrap();
    let mut batch = ruled_v1::RuledEventBatch {
        events: initial_events,
        ..Default::default()
    };
    let mut action_batches = vec![batch.clone()];

    while engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| {
            pending.presentation.choice_kind == ruled_v1::ChoiceKind::ReplacementEffect
        })
    {
        let choice = published_choice(&batch, ruled_v1::ChoiceKind::ReplacementEffect);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.replacement_options.len(), 1);
        batch = engine
            .apply_scenario_command(
                0,
                &submit_choice_with_prompt_id(
                    choice.resolution_prompt_id,
                    vec![choice.replacement_options[0].application_id],
                ),
            )
            .expect("P0 applies the one local doubling replacement");
        action_batches.push(batch.clone());
    }
    let first_prompt = published_choice(&batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(first_prompt.deciding_player_id, 0);
    assert!(first_prompt.candidate_object_ids.contains(&recipient));
    let first_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(first_prompt.resolution_prompt_id, vec![recipient]),
        )
        .expect("P0 chooses P1's R for the first doubled leaf");
    action_batches.push(first_batch.clone());
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(proliferate_log_count(&first_batch, 0), 1);
    let second_prompt = published_choice(&first_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(second_prompt.deciding_player_id, 0);

    let concede_batch = engine
        .apply_scenario_command(
            0,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::Concede(ruled_v1::Concede {})),
            },
        )
        .expect("P0 concedes while the second doubled leaf is parked");
    action_batches.push(concede_batch.clone());
    assert!(engine.state.players[0].has_lost);
    let delegate_prompt = published_choice(&concede_batch, ruled_v1::ChoiceKind::ResolutionBranch);
    assert_eq!(delegate_prompt.deciding_player_id, 1);
    let delegate = delegate_prompt
        .resolution_branches
        .iter()
        .find(|branch| branch.label == "P2 makes the choice")
        .expect("P1 can delegate the Proliferate choice to P2");
    assert!(delegate.selectable);
    let delegate_batch = engine
        .apply_scenario_command(
            1,
            &submit_branch_with_prompt_id(
                delegate_prompt.resolution_prompt_id,
                delegate.branch_index,
            ),
        )
        .expect("P1 delegates P0's Proliferate choice to P2");
    action_batches.push(delegate_batch.clone());
    let delegated_prompt = published_choice(&delegate_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(delegated_prompt.deciding_player_id, 2);
    assert_eq!(delegated_prompt.candidate_object_ids, vec![recipient]);

    let (parked_stack, parked_transaction, old_prompt_id) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("P2's delegated recipient choice is parked");
        assert_eq!(
            pending.presentation.choice_kind,
            ruled_v1::ChoiceKind::Proliferate
        );
        assert_eq!(pending.deciding_player, 2);
        let ResolutionContinuation::Proliferate {
            stack, transaction, ..
        } = &pending.continuation
        else {
            panic!("the delegated second leaf remains in the flat Proliferate transaction");
        };
        assert!(stack.program.is_none());
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 1);
        assert!(transaction.pending_nodes.is_empty());
        assert!(transaction.current_leaf.is_some());
        assert_eq!(transaction.counter_placements.len(), 1);
        let receipt = transaction.counter_placements[0];
        assert_eq!(receipt.object.object_id, recipient);
        assert_eq!(
            receipt.object.zone_change_generation,
            engine.state.zone_change_generation[&recipient]
        );
        assert_eq!(receipt.counter, CounterKind::Charge);
        assert_eq!((receipt.before, receipt.after), (1, 2));
        (
            format!("{stack:?}"),
            format!("{transaction:?}"),
            delegated_prompt.resolution_prompt_id,
        )
    };
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        2
    );
    let global_stack = format!("{:?}", engine.state.stack);

    let suspended_resolution = engine
        .state
        .pending_resolution
        .take()
        .expect("capture the actual delegated pending observer-suspended resolution");
    let suspended_replacement = engine.state.pending_replacement_event.take();
    let mut restored_events = Vec::new();
    engine
        .restore_suspended_observer_resolution(
            Some(suspended_resolution),
            suspended_replacement,
            &mut restored_events,
        )
        .expect("restore P2's delegated Proliferate recipient choice after observer return");
    engine
        .publish_resolution_prompt_ids(&mut restored_events)
        .unwrap();
    let restored_batch = ruled_v1::RuledEventBatch {
        events: restored_events,
        ..Default::default()
    };
    action_batches.push(restored_batch.clone());
    assert_eq!(proliferate_log_count(&restored_batch, 0), 0);
    let restored_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("observer restoration retains P2's active child");
    let ResolutionContinuation::Proliferate {
        stack, transaction, ..
    } = &restored_pending.continuation
    else {
        panic!("observer restore rebuilt the doubled Proliferate root");
    };
    assert_eq!(restored_pending.deciding_player, 2);
    assert_eq!(format!("{stack:?}"), parked_stack);
    assert_eq!(format!("{transaction:?}"), parked_transaction);
    assert_eq!(format!("{:?}", engine.state.stack), global_stack);
    let restored_prompt = published_choice(&restored_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(restored_prompt.deciding_player_id, 2);
    assert_ne!(restored_prompt.resolution_prompt_id, old_prompt_id);
    assert_eq!(restored_prompt.candidate_object_ids, vec![recipient]);

    let pending_before_stale = format!("{restored_pending:?}");
    assert!(engine
        .apply_scenario_command(
            2,
            &submit_choice_with_prompt_id(old_prompt_id, vec![recipient]),
        )
        .is_err());
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution.as_ref().unwrap()),
        pending_before_stale
    );
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(format!("{:?}", engine.state.stack), global_stack);

    let final_batch = engine
        .apply_scenario_command(
            2,
            &submit_choice_with_prompt_id(restored_prompt.resolution_prompt_id, vec![recipient]),
        )
        .expect("P2 chooses P1's R for the restored second doubled leaf");
    action_batches.push(final_batch);
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        3
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        action_batches
            .iter()
            .map(|batch| proliferate_log_count(batch, 0))
            .sum::<usize>(),
        2
    );
    assert_eq!(
        action_batches
            .iter()
            .map(stack_resolved_count)
            .sum::<usize>(),
        1
    );
}

#[test]
fn saved_program_proliferate_keeps_first_receipt_when_actor_concedes_under_observer() {
    let registry = saved_observer_registry(false);
    let mut engine = saved_observer_engine(820_021, registry);
    let source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let source_generation = engine.state.zone_change_generation[&source];
    let recipient = put_saved_observer_card(&mut engine, 3, "forest", Zone::Battlefield);
    let recipient_generation = engine.state.zone_change_generation[&recipient];
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("P3's Forest recipient")
        .set_counter(CounterKind::Charge, 1);
    let pacifism = put_saved_observer_card(&mut engine, 2, "pacifism", Zone::Exile);
    let pacifism_generation = engine.state.zone_change_generation[&pacifism];
    let bears = put_saved_observer_card(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
    engine.reconcile_activated_ability_slots();
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.pending_resolution.is_none());

    let (spell, first_offer_batch) = cast_saved_ripples(&mut engine);
    let first_offer = published_choice(&first_offer_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(first_offer.candidate_object_ids, vec![recipient]);
    let first_leaf = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(first_offer.resolution_prompt_id, vec![recipient]),
        )
        .expect("P0 chooses P3's Forest for Ripples' first actual leaf");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(proliferate_log_count(&first_leaf, 0), 1);

    let second_offer = published_choice(&first_leaf, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(second_offer.deciding_player_id, 0);
    assert_eq!(second_offer.candidate_object_ids, vec![recipient]);
    let (saved_parent, saved_stack, saved_transaction, spell_generation) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Ripples' second doubled leaf is parked");
        assert_eq!(pending.deciding_player, 0);
        assert_eq!(pending.presentation.source_object_id, spell);
        assert_eq!(
            pending.presentation.choice_kind,
            ruled_v1::ChoiceKind::Proliferate
        );
        let ResolutionContinuation::Proliferate {
            stack,
            candidate_generations,
            stage,
            transaction,
            ..
        } = &pending.continuation
        else {
            panic!("the actual Ripples Program retains its second Proliferate child");
        };
        assert!(matches!(
            stage,
            crate::state::PendingProliferateStage::SelectingRecipients
        ));
        assert_eq!(
            candidate_generations,
            &vec![(recipient, recipient_generation)]
        );
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 1);
        assert!(
            transaction.pending_nodes.is_empty(),
            "the replacement root is not replayed"
        );
        let leaf = transaction
            .current_leaf
            .as_ref()
            .expect("the second doubled child is the active leaf");
        assert_eq!(leaf.applied_replacements.len(), 1);
        let applied = leaf.applied_replacements.iter().next().unwrap();
        assert_eq!(applied.source_object_id, source);
        assert_eq!(applied.zone_change_generation, source_generation);
        assert_eq!(transaction.counter_placements.len(), 1);
        let receipt = transaction.counter_placements[0];
        assert_eq!(receipt.object.object_id, recipient);
        assert_eq!(receipt.object.zone_change_generation, recipient_generation);
        assert_eq!(receipt.object.controller_at_event, 3);
        assert_eq!(receipt.counter, CounterKind::Charge);
        assert_eq!(receipt.count, 1);
        assert_eq!((receipt.before, receipt.after), (1, 2));
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.card_id, "ripples_of_potential");
        assert_eq!(stack.item.controller, 0);
        assert!(stack.item.source_owner.is_none());
        assert_eq!(stack.resume_effect_index, None);
        let program = stack
            .program
            .as_ref()
            .expect("the actual spell's Program cursor remains parked");
        assert_eq!(program.frames.len(), 1);
        assert_eq!(program.frames[0].next_instruction, 1);
        assert!(program.in_flight.is_some());
        let physical_spell = engine
            .state
            .objects
            .get(&spell)
            .expect("the physical Ripples spell is still on the stack");
        assert_eq!(physical_spell.owner, 0);
        assert_eq!(physical_spell.zone, Zone::Stack);
        (
            format!("{pending:?}"),
            format!("{stack:?}"),
            format!("{transaction:?}"),
            engine.state.zone_change_generation[&spell],
        )
    };
    let global_stack = format!("{:?}", engine.state.stack);
    assert_eq!(engine.state.objects[&pacifism].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&bears].owner, 2);
    assert_eq!(engine.state.objects[&bears].controller, 2);

    let observer_batch = queue_saved_pacifism_return(&mut engine, pacifism);
    let aura_offer = published_choice(&observer_batch, ruled_v1::ChoiceKind::AuraPermanent);
    assert_eq!(aura_offer.deciding_player_id, 2);
    assert_ne!(aura_offer.resolution_prompt_id, 0);
    assert!(aura_offer.candidate_object_ids.contains(&bears));
    {
        let batch = engine
            .state
            .pending_observer_return_batch
            .as_ref()
            .expect("the actual independent Aura return parks in its observer batch");
        assert!(batch.resume_stack.is_none());
        assert!(batch.suspended_replacement_event.is_none());
        assert_eq!(
            format!("{:?}", batch.suspended_resolution.as_ref().unwrap()),
            saved_parent,
            "the original active Program choice is held under AuraReturn"
        );
    }
    let aura_pending = engine.state.pending_resolution.as_ref().unwrap();
    let ResolutionContinuation::AuraReturn {
        stack: None,
        exiled,
    } = &aura_pending.continuation
    else {
        panic!("the independent AuraReturn has no competing resume stack");
    };
    assert_eq!(exiled.object_id, pacifism);
    assert_eq!(exiled.zone_change_generation, pacifism_generation);

    let concede_batch = engine
        .apply_scenario_command(
            0,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::Concede(ruled_v1::Concede {})),
            },
        )
        .expect("P0 concedes through the real command while P2's Aura prompt is active");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.objects.contains_key(&spell));
    assert!(!engine.state.objects.contains_key(&source));
    assert!(
        spell_generation > 0,
        "the captured spell incarnation was on the stack"
    );
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(proliferate_log_count(&concede_batch, 0), 0);
    let batch = engine
        .state
        .pending_observer_return_batch
        .as_ref()
        .expect("the independent Aura return remains active after concession");
    let saved = batch
        .suspended_resolution
        .as_ref()
        .expect("the begun Program child and its committed first receipt survive below AuraReturn");
    assert_eq!(format!("{saved:?}"), saved_parent);
    let ResolutionContinuation::Proliferate {
        stack, transaction, ..
    } = &saved.continuation
    else {
        panic!("the saved parent remains the original Program Proliferate continuation");
    };
    assert_eq!(format!("{stack:?}"), saved_stack);
    assert_eq!(format!("{transaction:?}"), saved_transaction);
    assert_eq!(format!("{:?}", engine.state.stack), global_stack);

    let current_aura_offer =
        find_published_choice(&concede_batch, ruled_v1::ChoiceKind::AuraPermanent)
            .unwrap_or(aura_offer);
    assert_eq!(current_aura_offer.deciding_player_id, 2);
    assert_eq!(current_aura_offer.candidate_object_ids, vec![bears]);
    let aura_batch = engine
        .apply_scenario_command(
            2,
            &submit_choice_with_prompt_id(current_aura_offer.resolution_prompt_id, vec![bears]),
        )
        .expect("P2 answers the currently published Pacifism return choice");
    assert_eq!(engine.state.objects[&pacifism].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&pacifism].attached_to,
        Some(crate::state::AttachmentRecipient::Object(bears))
    );
    assert!(engine.state.pending_observer_return_batch.is_none());
    let active = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the saved Proliferate child resumes after Pacifism attaches");
    assert_eq!(
        active.presentation.choice_kind,
        ruled_v1::ChoiceKind::ResolutionBranch
    );
    assert_eq!(active.deciding_player, 1);
    let delegate_offer = published_choice(&aura_batch, ruled_v1::ChoiceKind::ResolutionBranch);
    assert_eq!(delegate_offer.deciding_player_id, 1);
    let p2_branch = delegate_offer
        .resolution_branches
        .iter()
        .find(|branch| branch.label == "P2 makes the choice")
        .expect("the current delegate prompt offers live P2");
    assert!(p2_branch.selectable);
    let delegate_batch = engine
        .apply_scenario_command(
            1,
            &submit_branch_with_prompt_id(
                delegate_offer.resolution_prompt_id,
                p2_branch.branch_index,
            ),
        )
        .expect("P1 selects the actual offered P2 Proliferate delegate branch");
    let restored_offer = published_choice(&delegate_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(restored_offer.deciding_player_id, 2);
    assert_ne!(restored_offer.resolution_prompt_id, 0);
    assert_eq!(restored_offer.candidate_object_ids, vec![recipient]);
    let restored = engine.state.pending_resolution.as_ref().unwrap();
    let ResolutionContinuation::Proliferate {
        stack, transaction, ..
    } = &restored.continuation
    else {
        panic!("delegation retains the saved child of the original Program");
    };
    assert_eq!(format!("{stack:?}"), saved_stack);
    assert_eq!(transaction.actor, 0);
    assert_eq!(transaction.completed_leaves, 1);
    assert_eq!(transaction.counter_placements.len(), 1);

    let second_leaf = engine
        .apply_scenario_command(
            2,
            &submit_choice_with_prompt_id(restored_offer.resolution_prompt_id, vec![recipient]),
        )
        .expect("P2 chooses P3's R for the restored second actual leaf");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        3
    );
    assert_eq!(proliferate_log_count(&second_leaf, 0), 1);
    assert_eq!(ripples_resolution_log_count(&second_leaf), 1);
    assert_eq!(stack_resolved_count(&second_leaf), 0);
    assert!(engine.state.objects[&recipient].phase_status.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.staged_trigger_groups.is_empty());
}

#[test]
fn saved_program_proliferate_refreshes_departed_recipient_after_independent_observer() {
    let registry = saved_observer_registry(true);
    for (row, seed, keep_survivor) in [
        ("surviving_candidate", 820_022, true),
        ("no_candidates", 820_023, false),
    ] {
        let mut engine = saved_observer_engine(seed, registry);
        let source =
            put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
        let source_generation = engine.state.zone_change_generation[&source];
        let ezuri = put_saved_observer_card(
            &mut engine,
            0,
            "ezuri,_stalker_of_spheres",
            Zone::Battlefield,
        );
        let recipient = put_saved_observer_card(&mut engine, 1, "forest", Zone::Battlefield);
        let recipient_generation = engine.state.zone_change_generation[&recipient];
        engine
            .state
            .objects
            .get_mut(&recipient)
            .expect("P1's counter-bearing Forest")
            .set_counter(CounterKind::Charge, 1);
        let survivor = keep_survivor
            .then(|| put_saved_observer_card(&mut engine, 0, "forest", Zone::Battlefield));
        let survivor_generation =
            survivor.map(|object_id| engine.state.zone_change_generation[&object_id]);
        if let Some(survivor) = survivor {
            engine
                .state
                .objects
                .get_mut(&survivor)
                .expect("P0's counter-bearing surviving Forest")
                .set_counter(CounterKind::Charge, 1);
        }
        let pacifism = put_saved_observer_card(&mut engine, 2, "pacifism", Zone::Exile);
        let bears = put_saved_observer_card(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
        engine.reconcile_activated_ability_slots();
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.pending_triggers.is_empty());
        assert!(engine.state.pending_trigger_order.is_none());
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.staged_trigger_groups.is_empty());

        let (spell, first_offer_batch) = cast_saved_ripples(&mut engine);
        let first_offer = published_choice(&first_offer_batch, ruled_v1::ChoiceKind::Proliferate);
        let mut original_candidates = vec![recipient];
        if let Some(survivor) = survivor {
            original_candidates.push(survivor);
        }
        original_candidates.sort_unstable();
        assert_eq!(
            first_offer.candidate_object_ids, original_candidates,
            "{row}"
        );
        let hand_at_first_offer = engine.state.players[0].hand.len();
        let first_leaf = engine
            .apply_scenario_command(
                0,
                &submit_choice_with_prompt_id(first_offer.resolution_prompt_id, vec![recipient]),
            )
            .expect("P0 chooses P1's R for the first actual Proliferate leaf");
        assert_eq!(
            engine.state.objects[&recipient].counter_count(CounterKind::Charge),
            2
        );
        assert_eq!(proliferate_log_count(&first_leaf, 0), 1);
        assert_eq!(engine.state.players[0].hand.len(), hand_at_first_offer);
        let staged: Vec<_> = engine
            .state
            .staged_trigger_groups
            .iter()
            .flat_map(|group| group.triggers.iter())
            .collect();
        assert_eq!(
            staged.len(),
            1,
            "{row}: only the first real event has fired"
        );
        assert_eq!(staged[0].source_permanent_id, ezuri);
        assert_eq!(staged[0].controller, 0);
        assert_eq!(staged[0].card_id, "ezuri,_stalker_of_spheres");
        assert_eq!(staged[0].ability_index, 1);
        assert!(engine.state.pending_trigger_order.is_none());

        let second_offer = published_choice(&first_leaf, ruled_v1::ChoiceKind::Proliferate);
        assert_eq!(second_offer.deciding_player_id, 0, "{row}");
        assert_eq!(
            second_offer.candidate_object_ids, original_candidates,
            "{row}"
        );
        let (saved_parent, saved_stack, saved_transaction) = {
            let pending = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("the second real child is parked");
            assert_eq!(pending.presentation.source_object_id, spell, "{row}");
            let ResolutionContinuation::Proliferate {
                stack,
                candidate_generations,
                stage,
                transaction,
                ..
            } = &pending.continuation
            else {
                panic!("{row}: the actual Ripples Program retains its second child");
            };
            assert!(matches!(
                stage,
                crate::state::PendingProliferateStage::SelectingRecipients
            ));
            let mut expected_generations = vec![(recipient, recipient_generation)];
            if let (Some(survivor), Some(generation)) = (survivor, survivor_generation) {
                expected_generations.push((survivor, generation));
            }
            expected_generations.sort_unstable();
            assert_eq!(candidate_generations, &expected_generations, "{row}");
            assert_eq!(transaction.actor, 0, "{row}");
            assert_eq!(transaction.completed_leaves, 1, "{row}");
            assert!(
                transaction.pending_nodes.is_empty(),
                "{row}: no replay root"
            );
            let current_leaf = transaction
                .current_leaf
                .as_ref()
                .expect("the second child is the active leaf");
            assert_eq!(current_leaf.applied_replacements.len(), 1, "{row}");
            let applied = current_leaf.applied_replacements.iter().next().unwrap();
            assert_eq!(applied.source_object_id, source, "{row}");
            assert_eq!(applied.zone_change_generation, source_generation, "{row}");
            assert_eq!(transaction.counter_placements.len(), 1, "{row}");
            let receipt = transaction.counter_placements[0];
            assert_eq!(receipt.object.object_id, recipient, "{row}");
            assert_eq!(
                receipt.object.zone_change_generation, recipient_generation,
                "{row}"
            );
            assert_eq!(receipt.counter, CounterKind::Charge, "{row}");
            assert_eq!((receipt.before, receipt.after), (1, 2), "{row}");
            assert_eq!(stack.item.id, spell, "{row}");
            assert_eq!(stack.item.card_id, "ripples_of_potential", "{row}");
            assert_eq!(stack.item.controller, 0, "{row}");
            assert_eq!(
                stack.program.as_ref().unwrap().frames[0].next_instruction,
                1
            );
            assert!(stack.program.as_ref().unwrap().in_flight.is_some());
            (
                format!("{pending:?}"),
                format!("{stack:?}"),
                format!("{transaction:?}"),
            )
        };
        let global_stack = format!("{:?}", engine.state.stack);
        let observer_batch = queue_saved_pacifism_return(&mut engine, pacifism);
        let aura_offer = published_choice(&observer_batch, ruled_v1::ChoiceKind::AuraPermanent);
        assert_eq!(aura_offer.deciding_player_id, 2, "{row}");
        assert!(aura_offer.candidate_object_ids.contains(&bears), "{row}");
        let observer_state = engine.state.pending_observer_return_batch.as_ref().unwrap();
        assert!(observer_state.resume_stack.is_none(), "{row}");
        assert!(
            observer_state.suspended_replacement_event.is_none(),
            "{row}"
        );
        assert_eq!(
            format!(
                "{:?}",
                observer_state.suspended_resolution.as_ref().unwrap()
            ),
            saved_parent,
            "{row}: the real observer owns the exact saved Program continuation"
        );
        let ResolutionContinuation::AuraReturn { stack: None, .. } = &engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation
        else {
            panic!("{row}: independent Pacifism uses its actual AuraReturn choice");
        };

        let concede_batch = engine
            .apply_scenario_command(
                1,
                &ruled_v1::RuledCommand {
                    cmd: Some(ruled_v1::ruled_command::Cmd::Concede(ruled_v1::Concede {})),
                },
            )
            .expect("P1 concedes through apply_command while P2's Aura prompt is active");
        assert!(engine.state.players[1].has_lost, "{row}");
        assert!(
            !engine.state.objects.contains_key(&recipient),
            "{row}: R physically leaves"
        );
        assert_eq!(proliferate_log_count(&concede_batch, 0), 0, "{row}");
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_at_first_offer,
            "{row}"
        );
        let saved_batch = engine.state.pending_observer_return_batch.as_ref().unwrap();
        let saved = saved_batch
            .suspended_resolution
            .as_ref()
            .expect("the original Program prefix remains saved after R's controller concedes");
        assert_eq!(format!("{saved:?}"), saved_parent, "{row}");
        let ResolutionContinuation::Proliferate {
            stack, transaction, ..
        } = &saved.continuation
        else {
            panic!("{row}: the saved object is still the original Program Proliferate");
        };
        assert_eq!(format!("{stack:?}"), saved_stack, "{row}");
        assert_eq!(format!("{transaction:?}"), saved_transaction, "{row}");
        assert_eq!(format!("{:?}", engine.state.stack), global_stack, "{row}");
        if let Some(survivor) = survivor {
            assert_eq!(
                engine.state.objects[&survivor].counter_count(CounterKind::Charge),
                1
            );
        }

        let current_aura_offer =
            find_published_choice(&concede_batch, ruled_v1::ChoiceKind::AuraPermanent)
                .unwrap_or(aura_offer);
        assert_eq!(current_aura_offer.deciding_player_id, 2, "{row}");
        assert!(
            current_aura_offer.candidate_object_ids.contains(&bears),
            "{row}"
        );
        let aura_batch = engine
            .apply_scenario_command(
                2,
                &submit_choice_with_prompt_id(current_aura_offer.resolution_prompt_id, vec![bears]),
            )
            .expect("P2 answers the current actual Pacifism return offer");
        assert_eq!(
            engine.state.objects[&pacifism].zone,
            Zone::Battlefield,
            "{row}"
        );
        assert_eq!(
            engine.state.objects[&pacifism].attached_to,
            Some(crate::state::AttachmentRecipient::Object(bears)),
            "{row}"
        );
        assert!(
            engine.state.pending_observer_return_batch.is_none(),
            "{row}"
        );

        if let Some(survivor) = survivor {
            let restored_offer =
                find_published_choice(&aura_batch, ruled_v1::ChoiceKind::Proliferate)
                    .expect("observer answer publishes the refreshed current Proliferate offer");
            let restored = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("surviving S keeps the second child answerable");
            assert_eq!(
                restored.presentation.choice_kind,
                ruled_v1::ChoiceKind::Proliferate
            );
            assert_eq!(restored.deciding_player, 0);
            assert_ne!(restored_offer.resolution_prompt_id, 0);
            assert_eq!(restored_offer.deciding_player_id, 0);
            assert_eq!(restored_offer.candidate_object_ids, vec![survivor]);
            let ResolutionContinuation::Proliferate {
                stack, transaction, ..
            } = &restored.continuation
            else {
                panic!("the restored current offer remains the actual Program child");
            };
            assert_eq!(transaction.actor, 0);
            assert_eq!(transaction.completed_leaves, 1);
            assert_eq!(transaction.counter_placements.len(), 1);
            assert_eq!(
                transaction.counter_placements[0].object.object_id,
                recipient
            );
            assert_eq!(
                transaction.counter_placements[0]
                    .object
                    .zone_change_generation,
                recipient_generation
            );
            assert_eq!(format!("{stack:?}"), saved_stack);
            let old_state = format!("{restored:?}");
            let counter_before_stale =
                engine.state.objects[&survivor].counter_count(CounterKind::Charge);
            let stack_before_stale = format!("{:?}", engine.state.stack);
            assert!(engine
                .apply_scenario_command(
                    0,
                    &submit_choice_with_prompt_id(
                        second_offer.resolution_prompt_id,
                        vec![survivor]
                    ),
                )
                .is_err());
            assert_eq!(
                format!("{:?}", engine.state.pending_resolution.as_ref().unwrap()),
                old_state,
                "the retired pre-observer prompt is rejected atomically"
            );
            assert_eq!(
                engine.state.objects[&survivor].counter_count(CounterKind::Charge),
                counter_before_stale
            );
            assert_eq!(format!("{:?}", engine.state.stack), stack_before_stale);

            let second_leaf = engine
                .apply_scenario_command(
                    0,
                    &submit_choice_with_prompt_id(
                        restored_offer.resolution_prompt_id,
                        vec![survivor],
                    ),
                )
                .expect("P0 chooses only live S for the restored child");
            assert_eq!(
                engine.state.objects[&survivor].counter_count(CounterKind::Charge),
                2
            );
            assert_eq!(proliferate_log_count(&second_leaf, 0), 1);
            let tail_offer = published_choice(&second_leaf, ruled_v1::ChoiceKind::PermanentObjects);
            assert_eq!(tail_offer.deciding_player_id, 0);
            assert_eq!(tail_offer.candidate_object_ids, vec![survivor]);
            assert!(!tail_offer.candidate_object_ids.contains(&recipient));
            let phase_batch = engine
                .apply_scenario_command(
                    0,
                    &submit_choice_with_prompt_id(tail_offer.resolution_prompt_id, vec![survivor]),
                )
                .expect("P0 phases only S from Ripples' live receipt projection");
            assert!(engine.state.objects[&survivor].phase_status.is_some());
            assert_eq!(ripples_resolution_log_count(&phase_batch), 1);
            assert_eq!(stack_resolved_count(&phase_batch), 1);
        } else {
            assert!(
                find_published_choice(&aura_batch, ruled_v1::ChoiceKind::Proliferate).is_none(),
                "the empty current leaf has no fabricated recipient prompt"
            );
            assert!(engine.state.pending_resolution.is_none());
            let batch_logs = ripples_resolution_log_count(&aura_batch);
            assert_eq!(
                batch_logs, 1,
                "the original Program completes inline exactly once"
            );
            assert_eq!(stack_resolved_count(&aura_batch), 1);
            assert_eq!(proliferate_log_count(&aura_batch, 0), 0);
        }

        assert!(engine.state.pending_resolution.is_none(), "{row}");
        assert!(engine.state.stack.is_empty(), "{row}");
        assert!(
            engine.state.pending_observer_return_batch.is_none(),
            "{row}"
        );
        assert!(engine.state.pending_trigger_order.is_some(), "{row}");
        let pending_order = engine.state.pending_trigger_order.as_ref().unwrap();
        assert_eq!(pending_order.deciding_player, 0, "{row}");
        assert_eq!(pending_order.candidates.len(), 2, "{row}");
        for trigger in &pending_order.candidates {
            assert_eq!(trigger.source_permanent_id, ezuri, "{row}");
            assert_eq!(trigger.controller, 0, "{row}");
            assert_eq!(trigger.card_id, "ezuri,_stalker_of_spheres", "{row}");
            assert_eq!(trigger.ability_index, 1, "{row}");
        }
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_at_first_offer,
            "{row}"
        );

        let mut order_submissions = 0;
        while let Some(order) = engine.state.pending_trigger_order.as_ref() {
            assert!(
                order_submissions < 2,
                "{row}: two actual triggers have a bounded order"
            );
            let deciding_player = order.deciding_player;
            let trigger_object_id = order.candidates[0].object_id;
            engine
                .apply_scenario_command(
                    deciding_player,
                    &ruled_v1::RuledCommand {
                        cmd: Some(ruled_v1::ruled_command::Cmd::SubmitTriggerOrder(
                            ruled_v1::SubmitTriggerOrder { trigger_object_id },
                        )),
                    },
                )
                .expect("submit the actual first published Ezuri trigger identity");
            order_submissions += 1;
        }
        assert!(
            order_submissions > 0,
            "{row}: actual simultaneous triggers were ordered"
        );
        assert_eq!(
            engine.state.stack.len(),
            2,
            "{row}: only Ezuri's two triggers are stacked"
        );
        assert!(engine.state.stack.iter().all(|item| {
            item.is_triggered
                && item.source_permanent_id == Some(ezuri)
                && item.controller == 0
                && item.card_id == "ezuri,_stalker_of_spheres"
                && item.ability_index == Some(1)
        }));
        assert!(engine.state.pending_triggers.is_empty(), "{row}");
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_at_first_offer,
            "{row}"
        );
        pass_live_players_with_batch(&mut engine);
        pass_live_players_with_batch(&mut engine);
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_at_first_offer + 2,
            "{row}"
        );
        assert!(engine.state.pending_trigger_order.is_none(), "{row}");
        assert!(engine.state.pending_triggers.is_empty(), "{row}");
        assert!(engine.state.staged_trigger_groups.is_empty(), "{row}");
        assert!(engine.state.pending_resolution.is_none(), "{row}");
        assert!(
            engine.state.pending_observer_return_batch.is_none(),
            "{row}"
        );
        assert!(engine.state.stack.is_empty(), "{row}");
    }
}

#[test]
fn saved_proliferate_outer_and_nested_work_survives_actor_departure_under_observer() {
    let registry = saved_observer_registry(false);
    let mut engine = saved_observer_engine(820_024, registry);
    let first_source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let second_source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let first_source_generation = engine.state.zone_change_generation[&first_source];
    let second_source_generation = engine.state.zone_change_generation[&second_source];
    let recipient = put_saved_observer_card(&mut engine, 3, "forest", Zone::Battlefield);
    let recipient_generation = engine.state.zone_change_generation[&recipient];
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("P3's Forest recipient")
        .set_counter(CounterKind::Charge, 1);
    let pacifism = put_saved_observer_card(&mut engine, 2, "pacifism", Zone::Exile);
    let pacifism_generation = engine.state.zone_change_generation[&pacifism];
    let bears = put_saved_observer_card(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
    engine.reconcile_activated_ability_slots();
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.pending_resolution.is_none());

    let (spell, outer_batch) = cast_saved_ripples_until_outer_replacement_order(&mut engine);
    let outer_offer = published_choice(&outer_batch, ruled_v1::ChoiceKind::ReplacementEffect);
    assert_eq!(outer_offer.deciding_player_id, 0);
    assert_ne!(outer_offer.resolution_prompt_id, 0);
    assert_eq!(outer_offer.replacement_options.len(), 2);
    let outer_handles = outer_offer
        .replacement_options
        .iter()
        .map(|option| option.application_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(outer_handles.len(), 2);
    assert_eq!(
        outer_offer
            .candidate_object_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([first_source, second_source])
    );
    let (saved_outer, saved_stack, saved_transaction, spell_generation) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Ripples' real outer action-order choice is parked");
        assert_eq!(pending.deciding_player, 0);
        assert_eq!(pending.presentation.source_object_id, spell);
        assert_eq!(
            pending.presentation.choice_kind,
            ruled_v1::ChoiceKind::ReplacementEffect
        );
        let ResolutionContinuation::ProliferateReplacement { stack, transaction } =
            &pending.continuation
        else {
            panic!("the actual action-order offer retains Ripples' transaction");
        };
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 0);
        assert!(transaction.pending_nodes.is_empty());
        assert!(transaction.counter_placements.is_empty());
        assert_eq!(transaction.offered_replacements.len(), 2);
        assert_eq!(
            transaction
                .offered_replacements
                .iter()
                .map(|application| application.identity.source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_source, second_source])
        );
        assert!(transaction.offered_replacements.iter().any(|application| {
            application.identity.source_object_id == first_source
                && application.identity.zone_change_generation == first_source_generation
        }));
        assert!(transaction.offered_replacements.iter().any(|application| {
            application.identity.source_object_id == second_source
                && application.identity.zone_change_generation == second_source_generation
        }));
        assert!(transaction
            .current_leaf
            .as_ref()
            .expect("the actual root node is parked before any replacement applies")
            .applied_replacements
            .is_empty());
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.card_id, "ripples_of_potential");
        assert_eq!(stack.item.controller, 0);
        assert!(stack.item.source_owner.is_none());
        assert_eq!(stack.resume_effect_index, None);
        let program = stack
            .program
            .as_ref()
            .expect("the original Ripples Program remains parked");
        assert_eq!(program.frames.len(), 1);
        assert!(program.in_flight.is_some());
        let physical_spell = engine
            .state
            .objects
            .get(&spell)
            .expect("the physical Ripples card is still on the stack");
        assert_eq!(physical_spell.owner, 0);
        assert_eq!(physical_spell.zone, Zone::Stack);
        (
            format!("{pending:?}"),
            format!("{stack:?}"),
            format!("{transaction:?}"),
            engine.state.zone_change_generation[&spell],
        )
    };
    let original_global_stack = format!("{:?}", engine.state.stack);
    let observer_batch = queue_saved_pacifism_return(&mut engine, pacifism);
    let aura_offer = published_choice(&observer_batch, ruled_v1::ChoiceKind::AuraPermanent);
    assert_eq!(aura_offer.deciding_player_id, 2);
    assert_ne!(aura_offer.resolution_prompt_id, 0);
    assert!(aura_offer.candidate_object_ids.contains(&bears));
    let observer = engine
        .state
        .pending_observer_return_batch
        .as_ref()
        .expect("the actual independent Aura return owns its saved continuation");
    assert!(observer.resume_stack.is_none());
    assert!(observer.suspended_replacement_event.is_none());
    assert_eq!(
        format!("{:?}", observer.suspended_resolution.as_ref().unwrap()),
        saved_outer
    );
    let ResolutionContinuation::AuraReturn {
        stack: None,
        exiled,
    } = &engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Pacifism's return is now the active choice")
        .continuation
    else {
        panic!("the independent AuraReturn has no competing resume stack");
    };
    assert_eq!(exiled.object_id, pacifism);
    assert_eq!(exiled.zone_change_generation, pacifism_generation);

    let concede_batch = engine
        .apply_scenario_command(
            0,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::Concede(ruled_v1::Concede {})),
            },
        )
        .expect("P0 concedes while P2's independent Aura choice is active");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.objects.contains_key(&spell));
    assert!(!engine.state.objects.contains_key(&first_source));
    assert!(!engine.state.objects.contains_key(&second_source));
    assert!(spell_generation > 0);
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        1
    );
    assert_eq!(proliferate_log_count(&concede_batch, 0), 0);
    assert_eq!(format!("{:?}", engine.state.stack), original_global_stack);
    let suspended = engine
        .state
        .pending_observer_return_batch
        .as_ref()
        .expect("the independent Aura return remains active after P0 concedes");
    assert_eq!(
        format!("{:?}", suspended.suspended_resolution.as_ref().unwrap()),
        saved_outer
    );
    let ResolutionContinuation::ProliferateReplacement { stack, transaction } = &suspended
        .suspended_resolution
        .as_ref()
        .unwrap()
        .continuation
    else {
        panic!("the outer action-order continuation remains saved after concession");
    };
    assert_eq!(format!("{stack:?}"), saved_stack);
    assert_eq!(format!("{transaction:?}"), saved_transaction);

    let current_aura_offer =
        find_published_choice(&concede_batch, ruled_v1::ChoiceKind::AuraPermanent)
            .unwrap_or(aura_offer);
    assert_eq!(current_aura_offer.deciding_player_id, 2);
    assert!(current_aura_offer.candidate_object_ids.contains(&bears));
    let aura_batch = engine
        .apply_scenario_command(
            2,
            &submit_choice_with_prompt_id(current_aura_offer.resolution_prompt_id, vec![bears]),
        )
        .expect("P2 answers the currently published Pacifism return offer");
    assert_eq!(engine.state.objects[&pacifism].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&pacifism].attached_to,
        Some(crate::state::AttachmentRecipient::Object(bears))
    );
    assert!(engine.state.pending_observer_return_batch.is_none());

    let delegate_offer = published_choice(&aura_batch, ruled_v1::ChoiceKind::ResolutionBranch);
    assert_eq!(delegate_offer.deciding_player_id, 1);
    assert_ne!(delegate_offer.resolution_prompt_id, 0);
    let p2_branch = delegate_offer
        .resolution_branches
        .iter()
        .find(|branch| branch.label == "P2 makes the choice")
        .expect("the live delegate selector offers P2");
    assert!(p2_branch.selectable);
    let delegate_batch = engine
        .apply_scenario_command(
            1,
            &submit_branch_with_prompt_id(
                delegate_offer.resolution_prompt_id,
                p2_branch.branch_index,
            ),
        )
        .expect("P1 selects the actual offered P2 Proliferate delegate branch");
    let restored_offer = published_choice(&delegate_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(restored_offer.deciding_player_id, 2);
    assert_ne!(restored_offer.resolution_prompt_id, 0);
    assert_eq!(restored_offer.candidate_object_ids, vec![recipient]);
    let restored = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the one ordinary original action leaf is now offered to P2");
    assert_eq!(restored.deciding_player, 2);
    let ResolutionContinuation::Proliferate {
        stack,
        candidate_generations,
        transaction,
        ..
    } = &restored.continuation
    else {
        panic!("the new recipient offer remains the original Ripples Program child");
    };
    assert_eq!(format!("{stack:?}"), saved_stack);
    assert_eq!(
        candidate_generations,
        &vec![(recipient, recipient_generation)]
    );
    assert_eq!(transaction.actor, 0);
    assert_eq!(transaction.completed_leaves, 0);
    assert!(transaction.pending_nodes.is_empty());
    assert!(transaction.counter_placements.is_empty());
    assert!(transaction.offered_replacements.is_empty());
    assert!(transaction
        .current_leaf
        .as_ref()
        .expect("the original action root advanced to one ordinary leaf")
        .applied_replacements
        .is_empty());
    let restored_snapshot = format!("{restored:?}");
    let recipient_count_before_stale =
        engine.state.objects[&recipient].counter_count(CounterKind::Charge);
    let stale_handle = outer_handles
        .iter()
        .copied()
        .find(|handle| *handle != recipient)
        .expect("at least one outer application handle differs from the physical R object");
    assert!(engine
        .apply_scenario_command(
            2,
            &submit_choice_with_prompt_id(outer_offer.resolution_prompt_id, vec![stale_handle]),
        )
        .is_err());
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution.as_ref().unwrap()),
        restored_snapshot,
        "the actor-scoped pre-observer order prompt and handle cannot answer the new leaf"
    );
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        recipient_count_before_stale
    );
    assert_eq!(format!("{:?}", engine.state.stack), original_global_stack);

    let ordinary_leaf = engine
        .apply_scenario_command(
            2,
            &submit_choice_with_prompt_id(restored_offer.resolution_prompt_id, vec![recipient]),
        )
        .expect("P2 chooses the only live R for the ordinary original action leaf");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(proliferate_log_count(&ordinary_leaf, 0), 1);
    assert_eq!(ripples_resolution_log_count(&ordinary_leaf), 1);
    assert_eq!(stack_resolved_count(&ordinary_leaf), 0);
    assert!(engine.state.objects[&recipient].phase_status.is_none());
    assert!(
        find_published_choice(&ordinary_leaf, ruled_v1::ChoiceKind::PermanentObjects).is_none()
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.staged_trigger_groups.is_empty());

    let registry = saved_observer_registry_with_doubling_season();
    let mut engine = saved_observer_engine(820_025, registry);
    let action_source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let action_source_generation = engine.state.zone_change_generation[&action_source];
    let recipient = put_saved_observer_card(&mut engine, 3, "forest", Zone::Battlefield);
    let recipient_generation = engine.state.zone_change_generation[&recipient];
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("P3's Forest recipient")
        .set_counter(CounterKind::Charge, 1);
    let first_season =
        put_saved_observer_card(&mut engine, 3, "doubling_season", Zone::Battlefield);
    let second_season =
        put_saved_observer_card(&mut engine, 3, "doubling_season", Zone::Battlefield);
    let first_season_generation = engine.state.zone_change_generation[&first_season];
    let second_season_generation = engine.state.zone_change_generation[&second_season];
    let pacifism = put_saved_observer_card(&mut engine, 2, "pacifism", Zone::Exile);
    let pacifism_generation = engine.state.zone_change_generation[&pacifism];
    let bears = put_saved_observer_card(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
    engine.reconcile_activated_ability_slots();
    assert_eq!(engine.state.objects[&action_source].owner, 0);
    for season in [first_season, second_season] {
        assert_eq!(engine.state.objects[&season].owner, 3);
        assert_eq!(engine.state.objects[&season].controller, 3);
    }
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_trigger_order.is_none());

    let max_object_id = engine
        .state
        .objects
        .keys()
        .copied()
        .max()
        .expect("prepared state contains physical objects");
    let above_objects = max_object_id
        .checked_add(1)
        .expect("room remains above physical object IDs");
    let first_handle_id = engine
        .state
        .next_replacement_application_id
        .max(above_objects);
    let _reserved_handle_limit = first_handle_id
        .checked_add(8)
        .expect("the fixed nested counter choices have checked handle headroom");
    assert!(first_handle_id > max_object_id);
    let object_id_limit = max_object_id;
    engine.state.next_replacement_application_id = first_handle_id;

    let (spell, initial_leaf_batch) = cast_saved_ripples(&mut engine);
    let initial_leaf_offer =
        published_choice(&initial_leaf_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(initial_leaf_offer.deciding_player_id, 0);
    assert_eq!(initial_leaf_offer.candidate_object_ids, vec![recipient]);
    let first_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(initial_leaf_offer.resolution_prompt_id, vec![recipient]),
        )
        .expect("P0 chooses P3's R for the first actual action leaf");
    let first_nested_offer =
        published_choice(&first_leaf_batch, ruled_v1::ChoiceKind::ReplacementEffect);
    assert_eq!(first_nested_offer.deciding_player_id, 3);
    assert_ne!(first_nested_offer.resolution_prompt_id, 0);
    assert_eq!(first_nested_offer.replacement_options.len(), 2);
    assert!(first_nested_offer
        .replacement_options
        .iter()
        .all(|option| option.application_id > object_id_limit));
    assert!(first_nested_offer.replacement_options.iter().all(|option| {
        !engine
            .state
            .objects
            .contains_key(&(option.application_id as ObjectId))
    }));
    let PendingReplacementEvent::Counters(first_counter_batch) = engine
        .state
        .pending_replacement_event
        .as_ref()
        .expect("the first actual Doubling Season order parks its counter batch")
    else {
        panic!("the real first nested choice owns a Counters batch");
    };
    assert_eq!(first_counter_batch.event_index, 0);
    assert_eq!(first_counter_batch.applications.len(), 2);
    assert_eq!(
        first_counter_batch
            .applications
            .iter()
            .map(|application| battlefield_instance(application).source_object_id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([first_season, second_season])
    );
    assert!(first_counter_batch.applications.iter().any(|application| {
        battlefield_instance(application).source_object_id == first_season
            && battlefield_instance(application).source_zone_change_generation
                == first_season_generation
    }));
    assert!(first_counter_batch.applications.iter().any(|application| {
        battlefield_instance(application).source_object_id == second_season
            && battlefield_instance(application).source_zone_change_generation
                == second_season_generation
    }));

    let first_order_answer = engine
        .apply_scenario_command(
            3,
            &submit_choice_with_prompt_id(
                first_nested_offer.resolution_prompt_id,
                vec![first_nested_offer.replacement_options[0].application_id],
            ),
        )
        .expect("P3 answers the first real nested replacement order");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(proliferate_log_count(&first_order_answer, 0), 1);
    let second_leaf_offer =
        published_choice(&first_order_answer, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(second_leaf_offer.deciding_player_id, 0);
    assert_eq!(second_leaf_offer.candidate_object_ids, vec![recipient]);
    let second_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(second_leaf_offer.resolution_prompt_id, vec![recipient]),
        )
        .expect("P0 chooses R for Ripples' second actual leaf");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5,
        "the second leaf has not committed before its real order is answered"
    );
    let second_nested_offer =
        published_choice(&second_leaf_batch, ruled_v1::ChoiceKind::ReplacementEffect);
    assert_eq!(second_nested_offer.deciding_player_id, 3);
    assert_ne!(second_nested_offer.resolution_prompt_id, 0);
    assert_eq!(second_nested_offer.replacement_options.len(), 2);
    let second_nested_handles = second_nested_offer
        .replacement_options
        .iter()
        .map(|option| option.application_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(second_nested_handles.len(), 2);
    assert!(second_nested_handles
        .iter()
        .all(|handle| *handle > object_id_limit));
    assert!(second_nested_handles
        .iter()
        .all(|handle| !engine.state.objects.contains_key(&(*handle as ObjectId))));
    let (saved_nested, saved_stack, saved_transaction, saved_counter_batch) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Ripples' second leaf is parked at P3's real nested order");
        assert_eq!(pending.deciding_player, 3);
        assert_eq!(pending.presentation.source_object_id, spell);
        assert_eq!(
            pending.presentation.choice_kind,
            ruled_v1::ChoiceKind::ReplacementEffect
        );
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
        } = &pending.continuation
        else {
            panic!("the actual second order retains typed Proliferate completion");
        };
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 1);
        assert!(transaction.pending_nodes.is_empty());
        assert!(transaction.offered_replacements.is_empty());
        let leaf = transaction
            .current_leaf
            .as_ref()
            .expect("the second action child is the active lineage");
        assert_eq!(leaf.applied_replacements.len(), 1);
        let applied = leaf.applied_replacements.iter().next().unwrap();
        assert_eq!(applied.source_object_id, action_source);
        assert_eq!(applied.zone_change_generation, action_source_generation);
        assert_eq!(transaction.counter_placements.len(), 1);
        let receipt = transaction.counter_placements[0];
        assert_eq!(receipt.object.object_id, recipient);
        assert_eq!(receipt.object.zone_change_generation, recipient_generation);
        assert_eq!(receipt.object.controller_at_event, 3);
        assert_eq!(receipt.counter, CounterKind::Charge);
        assert_eq!(receipt.count, 4);
        assert_eq!((receipt.before, receipt.after), (1, 5));
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.card_id, "ripples_of_potential");
        assert_eq!(stack.item.controller, 0);
        assert_eq!(stack.resume_effect_index, None);
        assert!(stack.program.as_ref().unwrap().in_flight.is_some());
        let PendingReplacementEvent::Counters(counter_batch) = engine
            .state
            .pending_replacement_event
            .as_ref()
            .expect("the typed completion has its paired current event")
        else {
            panic!("the current nested order remains paired with Counters");
        };
        assert_eq!(counter_batch.event_index, 0);
        assert_eq!(counter_batch.applications.len(), 2);
        assert_eq!(
            counter_batch
                .applications
                .iter()
                .map(|application| application.choice_id)
                .collect::<BTreeSet<_>>(),
            second_nested_handles
        );
        assert_eq!(
            counter_batch
                .applications
                .iter()
                .map(|application| battlefield_instance(application).source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_season, second_season])
        );
        let event = &counter_batch.events[counter_batch.event_index];
        assert_eq!(event.actor, 0);
        assert!(matches!(
            event.recipient,
            CounterPlacementRecipient::Object {
                object_id,
                zone_change_generation
            } if object_id == recipient && zone_change_generation == recipient_generation
        ));
        assert_eq!(event.counters.get(&CounterKind::Charge), Some(&1));
        (
            format!("{pending:?}"),
            format!("{stack:?}"),
            format!("{transaction:?}"),
            format!(
                "{:?}",
                engine.state.pending_replacement_event.as_ref().unwrap()
            ),
        )
    };
    assert_eq!(engine.state.objects[&first_season].owner, 3);
    assert_eq!(engine.state.objects[&second_season].owner, 3);
    assert_eq!(engine.state.objects[&first_season].controller, 3);
    assert_eq!(engine.state.objects[&second_season].controller, 3);
    let observer_batch = queue_saved_pacifism_return(&mut engine, pacifism);
    let aura_offer = published_choice(&observer_batch, ruled_v1::ChoiceKind::AuraPermanent);
    assert_eq!(aura_offer.deciding_player_id, 2);
    assert_ne!(aura_offer.resolution_prompt_id, 0);
    assert!(aura_offer.candidate_object_ids.contains(&bears));
    let observer = engine
        .state
        .pending_observer_return_batch
        .as_ref()
        .expect("the independent Aura return saves the actual nested transaction");
    assert!(observer.resume_stack.is_none());
    assert_eq!(
        format!("{:?}", observer.suspended_resolution.as_ref().unwrap()),
        saved_nested
    );
    assert_eq!(
        format!(
            "{:?}",
            observer.suspended_replacement_event.as_ref().unwrap()
        ),
        saved_counter_batch
    );
    let ResolutionContinuation::AuraReturn {
        stack: None,
        exiled,
    } = &engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the Aura return now owns the active choice")
        .continuation
    else {
        panic!("the independent AuraReturn has no competing resume stack");
    };
    assert_eq!(exiled.object_id, pacifism);
    assert_eq!(exiled.zone_change_generation, pacifism_generation);

    let concede_batch = engine
        .apply_scenario_command(
            0,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::Concede(ruled_v1::Concede {})),
            },
        )
        .expect("P0 concedes while P2's independent Aura choice is active");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.objects.contains_key(&spell));
    assert!(!engine.state.objects.contains_key(&action_source));
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    assert!(engine.state.objects.contains_key(&first_season));
    assert!(engine.state.objects.contains_key(&second_season));
    assert_eq!(proliferate_log_count(&concede_batch, 0), 0);
    let saved = engine
        .state
        .pending_observer_return_batch
        .as_ref()
        .expect("the independent Aura return remains active after P0 concedes");
    assert_eq!(
        format!("{:?}", saved.suspended_resolution.as_ref().unwrap()),
        saved_nested
    );
    assert_eq!(
        format!("{:?}", saved.suspended_replacement_event.as_ref().unwrap()),
        saved_counter_batch
    );
    let ResolutionContinuation::CounterReplacement {
        stack: Some(stack), ..
    } = &saved.suspended_resolution.as_ref().unwrap().continuation
    else {
        panic!("the saved nested counter stage remains paired after concession");
    };
    assert_eq!(format!("{stack:?}"), saved_stack);
    assert!(
        format!("{:?}", saved.suspended_resolution.as_ref().unwrap()).contains(&saved_transaction)
    );

    let current_aura_offer =
        find_published_choice(&concede_batch, ruled_v1::ChoiceKind::AuraPermanent)
            .unwrap_or(aura_offer);
    assert_eq!(current_aura_offer.deciding_player_id, 2);
    assert!(current_aura_offer.candidate_object_ids.contains(&bears));
    let aura_batch = engine
        .apply_scenario_command(
            2,
            &submit_choice_with_prompt_id(current_aura_offer.resolution_prompt_id, vec![bears]),
        )
        .expect("P2 answers the currently published Pacifism return offer");
    assert_eq!(engine.state.objects[&pacifism].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&pacifism].attached_to,
        Some(crate::state::AttachmentRecipient::Object(bears))
    );
    assert!(engine.state.pending_observer_return_batch.is_none());
    let restored_nested_offer =
        find_published_choice(&aura_batch, ruled_v1::ChoiceKind::ReplacementEffect)
            .expect("observer restoration republishes the current opaque nested counter order");
    assert_eq!(restored_nested_offer.deciding_player_id, 3);
    assert_ne!(restored_nested_offer.resolution_prompt_id, 0);
    assert_eq!(
        restored_nested_offer
            .replacement_options
            .iter()
            .map(|option| option.application_id)
            .collect::<BTreeSet<_>>(),
        second_nested_handles
    );
    assert!(restored_nested_offer
        .replacement_options
        .iter()
        .all(|option| !engine
            .state
            .objects
            .contains_key(&(option.application_id as ObjectId))));
    let restored = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P3's unchanged current nested order is restored");
    assert_eq!(restored.deciding_player, 3);
    let restored_snapshot = format!("{restored:?}");
    let current_counter_batch = format!(
        "{:?}",
        engine.state.pending_replacement_event.as_ref().unwrap()
    );
    let stale_handle = *second_nested_handles.iter().next().unwrap();
    assert!(engine
        .apply_scenario_command(
            3,
            &submit_choice_with_prompt_id(
                second_nested_offer.resolution_prompt_id,
                vec![stale_handle],
            ),
        )
        .is_err());
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution.as_ref().unwrap()),
        restored_snapshot,
        "the same live P3 cannot reuse the pre-observer prompt/handle pair"
    );
    assert_eq!(
        format!(
            "{:?}",
            engine.state.pending_replacement_event.as_ref().unwrap()
        ),
        current_counter_batch
    );
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );

    let final_leaf = engine
        .apply_scenario_command(
            3,
            &submit_choice_with_prompt_id(
                restored_nested_offer.resolution_prompt_id,
                vec![restored_nested_offer.replacement_options[0].application_id],
            ),
        )
        .expect("P3 answers the freshly published source-bound counter order");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        9
    );
    assert_eq!(proliferate_log_count(&first_order_answer, 0), 1);
    assert_eq!(proliferate_log_count(&final_leaf, 0), 1);
    assert_eq!(ripples_resolution_log_count(&final_leaf), 1);
    assert_eq!(stack_resolved_count(&final_leaf), 0);
    assert!(engine.state.objects[&recipient].phase_status.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn saved_nested_proliferate_recipient_retirement_finishes_leaf_before_remaining_sibling_and_cohort()
{
    let registry = saved_observer_registry_with_doubling_season_and_ezuri();
    let mut engine = saved_observer_engine(820_026, registry);
    let first_source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let second_source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let first_source_generation = engine.state.zone_change_generation[&first_source];
    let second_source_generation = engine.state.zone_change_generation[&second_source];
    let ezuri = put_saved_observer_card(
        &mut engine,
        0,
        "ezuri,_stalker_of_spheres",
        Zone::Battlefield,
    );
    let a = put_saved_observer_card(&mut engine, 0, "forest", Zone::Battlefield);
    let a_generation = engine.state.zone_change_generation[&a];
    engine
        .state
        .objects
        .get_mut(&a)
        .expect("P0's A recipient")
        .set_counter(CounterKind::Charge, 1);
    let b = put_saved_observer_card(&mut engine, 1, "forest", Zone::Battlefield);
    let b_generation = engine.state.zone_change_generation[&b];
    engine
        .state
        .objects
        .get_mut(&b)
        .expect("P1's B recipient")
        .set_counter(CounterKind::Charge, 1);
    let first_season =
        put_saved_observer_card(&mut engine, 1, "doubling_season", Zone::Battlefield);
    let second_season =
        put_saved_observer_card(&mut engine, 1, "doubling_season", Zone::Battlefield);
    let first_season_generation = engine.state.zone_change_generation[&first_season];
    let second_season_generation = engine.state.zone_change_generation[&second_season];
    let pacifism = put_saved_observer_card(&mut engine, 2, "pacifism", Zone::Exile);
    let pacifism_generation = engine.state.zone_change_generation[&pacifism];
    let bears = put_saved_observer_card(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
    engine.reconcile_activated_ability_slots();
    assert_eq!(engine.state.objects[&a].owner, 0);
    assert_eq!(engine.state.objects[&a].controller, 0);
    assert_eq!(engine.state.objects[&b].owner, 1);
    assert_eq!(engine.state.objects[&b].controller, 1);
    assert_eq!(engine.state.objects[&ezuri].owner, 0);
    assert_eq!(engine.state.objects[&ezuri].controller, 0);
    assert_eq!(engine.state.objects[&first_season].owner, 1);
    assert_eq!(engine.state.objects[&second_season].owner, 1);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.staged_trigger_groups.is_empty());

    let (spell, initial_batch) = cast_saved_ripples(&mut engine);
    let initial_offer = published_choice(&initial_batch, ruled_v1::ChoiceKind::Proliferate);
    let mut both_candidates = vec![a, b];
    both_candidates.sort_unstable();
    assert_eq!(initial_offer.deciding_player_id, 0);
    assert_eq!(initial_offer.candidate_object_ids, both_candidates);
    assert_ne!(initial_offer.resolution_prompt_id, 0);
    let hand_at_first_offer = engine.state.players[0].hand.len();
    let first_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(initial_offer.resolution_prompt_id, vec![a]),
        )
        .expect("P0 chooses only A for the first actual action leaf");
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&b].counter_count(CounterKind::Charge),
        1
    );
    assert_eq!(proliferate_log_count(&first_leaf_batch, 0), 1);
    assert_eq!(engine.state.players[0].hand.len(), hand_at_first_offer);
    let second_leaf_offer = published_choice(&first_leaf_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(second_leaf_offer.deciding_player_id, 0);
    assert_eq!(second_leaf_offer.candidate_object_ids, both_candidates);
    {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the second actual leaf is ready for recipient selection");
        let ResolutionContinuation::Proliferate {
            stack, transaction, ..
        } = &pending.continuation
        else {
            panic!("the saved Ripples Program retains the action transaction");
        };
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 1);
        assert_eq!(transaction.pending_nodes.len(), 1);
        let leaf = transaction.current_leaf.as_ref().unwrap();
        assert_eq!(leaf.applied_replacements.len(), 2);
        assert_eq!(
            leaf.applied_replacements
                .iter()
                .map(|identity| identity.source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_source, second_source])
        );
        assert!(leaf.applied_replacements.iter().any(|identity| {
            identity.source_object_id == first_source
                && identity.zone_change_generation == first_source_generation
        }));
        assert!(leaf.applied_replacements.iter().any(|identity| {
            identity.source_object_id == second_source
                && identity.zone_change_generation == second_source_generation
        }));
        assert_eq!(transaction.counter_placements.len(), 1);
        let receipt = transaction.counter_placements[0];
        assert_eq!(receipt.object.object_id, a);
        assert_eq!(receipt.object.zone_change_generation, a_generation);
        assert_eq!(receipt.object.controller_at_event, 0);
        assert_eq!(receipt.counter, CounterKind::Charge);
        assert_eq!(receipt.count, 1);
        assert_eq!((receipt.before, receipt.after), (1, 2));
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.controller, 0);
        assert!(stack.program.as_ref().unwrap().in_flight.is_some());
    }

    let second_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(second_leaf_offer.resolution_prompt_id, vec![b]),
        )
        .expect("P0 chooses only B for the second actual action leaf");
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&b].counter_count(CounterKind::Charge),
        1
    );
    let nested_offer =
        published_choice(&second_leaf_batch, ruled_v1::ChoiceKind::ReplacementEffect);
    assert_eq!(nested_offer.deciding_player_id, 1);
    assert_ne!(nested_offer.resolution_prompt_id, 0);
    assert_eq!(nested_offer.replacement_options.len(), 2);
    let nested_handles = nested_offer
        .replacement_options
        .iter()
        .map(|option| option.application_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(nested_handles.len(), 2);
    let (saved_nested, saved_stack, saved_transaction, saved_counter_batch) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("P1's actual nested B counter order is parked");
        assert_eq!(pending.deciding_player, 1);
        assert_eq!(pending.presentation.source_object_id, spell);
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
        } = &pending.continuation
        else {
            panic!("the nested B order retains a typed Proliferate completion");
        };
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 1);
        assert_eq!(
            transaction.pending_nodes.len(),
            1,
            "the remaining source-A sibling still awaits the other live action source"
        );
        let leaf = transaction.current_leaf.as_ref().unwrap();
        assert_eq!(leaf.applied_replacements.len(), 2);
        assert_eq!(
            leaf.applied_replacements
                .iter()
                .map(|identity| identity.source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_source, second_source])
        );
        assert_eq!(transaction.counter_placements.len(), 1);
        let receipt = transaction.counter_placements[0];
        assert_eq!(receipt.object.object_id, a);
        assert_eq!(receipt.object.zone_change_generation, a_generation);
        assert_eq!(receipt.object.controller_at_event, 0);
        assert_eq!(receipt.count, 1);
        assert_eq!((receipt.before, receipt.after), (1, 2));
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.controller, 0);
        assert!(stack.program.as_ref().unwrap().in_flight.is_some());
        let PendingReplacementEvent::Counters(batch) = engine
            .state
            .pending_replacement_event
            .as_ref()
            .expect("the typed Proliferate completion retains its paired B event")
        else {
            panic!("the active B nested order is paired with Counters");
        };
        assert_eq!(batch.event_index, 0);
        assert_eq!(batch.applications.len(), 2);
        assert_eq!(
            batch
                .applications
                .iter()
                .map(|application| application.choice_id)
                .collect::<BTreeSet<_>>(),
            nested_handles
        );
        assert_eq!(
            batch
                .applications
                .iter()
                .map(|application| battlefield_instance(application).source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_season, second_season])
        );
        assert!(batch.applications.iter().any(|application| {
            battlefield_instance(application).source_object_id == first_season
                && battlefield_instance(application).source_zone_change_generation
                    == first_season_generation
        }));
        assert!(batch.applications.iter().any(|application| {
            battlefield_instance(application).source_object_id == second_season
                && battlefield_instance(application).source_zone_change_generation
                    == second_season_generation
        }));
        let event = &batch.events[batch.event_index];
        assert_eq!(event.actor, 0);
        assert!(matches!(
            event.recipient,
            CounterPlacementRecipient::Object {
                object_id,
                zone_change_generation
            } if object_id == b && zone_change_generation == b_generation
        ));
        assert_eq!(event.counters.get(&CounterKind::Charge), Some(&1));
        (
            format!("{pending:?}"),
            format!("{stack:?}"),
            format!("{transaction:?}"),
            format!(
                "{:?}",
                engine.state.pending_replacement_event.as_ref().unwrap()
            ),
        )
    };
    assert_eq!(proliferate_log_count(&second_leaf_batch, 0), 0);
    assert_eq!(engine.state.staged_trigger_groups.len(), 1);
    let staged_before_retirement: Vec<_> = engine
        .state
        .staged_trigger_groups
        .iter()
        .flat_map(|group| group.triggers.iter())
        .collect();
    assert_eq!(staged_before_retirement.len(), 1);
    assert_eq!(staged_before_retirement[0].source_permanent_id, ezuri);
    assert_eq!(staged_before_retirement[0].controller, 0);
    assert_eq!(staged_before_retirement[0].ability_index, 1);

    let observer_batch = queue_saved_pacifism_return(&mut engine, pacifism);
    let aura_offer = published_choice(&observer_batch, ruled_v1::ChoiceKind::AuraPermanent);
    assert_eq!(aura_offer.deciding_player_id, 2);
    assert_ne!(aura_offer.resolution_prompt_id, 0);
    assert!(aura_offer.candidate_object_ids.contains(&bears));
    let observer = engine
        .state
        .pending_observer_return_batch
        .as_ref()
        .expect("the independent Aura return saves the B nested transaction");
    assert!(observer.resume_stack.is_none());
    assert_eq!(
        format!("{:?}", observer.suspended_resolution.as_ref().unwrap()),
        saved_nested
    );
    assert_eq!(
        format!(
            "{:?}",
            observer.suspended_replacement_event.as_ref().unwrap()
        ),
        saved_counter_batch
    );
    let ResolutionContinuation::AuraReturn {
        stack: None,
        exiled,
    } = &engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the Aura return now owns the active choice")
        .continuation
    else {
        panic!("the independent AuraReturn has no competing resume stack");
    };
    assert_eq!(exiled.object_id, pacifism);
    assert_eq!(exiled.zone_change_generation, pacifism_generation);

    let concede_batch = engine
        .apply_scenario_command(
            1,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::Concede(ruled_v1::Concede {})),
            },
        )
        .expect("P1 concedes while P2's independent Aura choice is active");
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.objects.contains_key(&b));
    assert!(!engine.state.objects.contains_key(&first_season));
    assert!(!engine.state.objects.contains_key(&second_season));
    assert!(engine.state.objects.contains_key(&a));
    assert!(engine.state.objects.contains_key(&ezuri));
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(proliferate_log_count(&concede_batch, 0), 0);
    let saved = engine
        .state
        .pending_observer_return_batch
        .as_ref()
        .expect("the independent Aura return remains active after P1 concedes");
    assert_eq!(
        format!("{:?}", saved.suspended_resolution.as_ref().unwrap()),
        saved_nested
    );
    assert_eq!(
        format!("{:?}", saved.suspended_replacement_event.as_ref().unwrap()),
        saved_counter_batch
    );
    let ResolutionContinuation::CounterReplacement {
        stack: Some(stack), ..
    } = &saved.suspended_resolution.as_ref().unwrap().continuation
    else {
        panic!("the saved nested B counter stage remains paired after concession");
    };
    assert_eq!(format!("{stack:?}"), saved_stack);
    assert!(
        format!("{:?}", saved.suspended_resolution.as_ref().unwrap()).contains(&saved_transaction)
    );

    let current_aura_offer =
        find_published_choice(&concede_batch, ruled_v1::ChoiceKind::AuraPermanent)
            .unwrap_or(aura_offer);
    assert_eq!(current_aura_offer.deciding_player_id, 2);
    assert!(current_aura_offer.candidate_object_ids.contains(&bears));
    let aura_batch = engine
        .apply_scenario_command(
            2,
            &submit_choice_with_prompt_id(current_aura_offer.resolution_prompt_id, vec![bears]),
        )
        .expect("P2 answers the currently published Pacifism return offer");
    assert_eq!(engine.state.objects[&pacifism].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&pacifism].attached_to,
        Some(crate::state::AttachmentRecipient::Object(bears))
    );
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        2
    );
    assert!(!engine.state.objects.contains_key(&b));
    let third_leaf_offer = find_published_choice(&aura_batch, ruled_v1::ChoiceKind::Proliferate);
    let Some(third_leaf_offer) = third_leaf_offer else {
        let active = engine.state.pending_resolution.as_ref().map(|pending| {
            (
                pending.deciding_player,
                pending.presentation.choice_kind,
                pending.presentation.source_object_id,
                format!("{:?}", pending.continuation),
            )
        });
        let emitted_choices = aura_batch
            .events
            .iter()
            .filter_map(|event| match event.ev.as_ref()? {
                ruled_v1::ruled_event::Ev::ResolutionChoiceRequired(choice) => Some(format!(
                    "kind={} decider={} candidates={:?}",
                    choice.choice_kind, choice.deciding_player_id, choice.candidate_object_ids
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        panic!(
            "retiring B must finish the second action leaf before the sibling; pending={active:?}; paired_event={:?}; emitted_choices={emitted_choices:?}",
            engine.state.pending_replacement_event
        );
    };
    assert_eq!(third_leaf_offer.deciding_player_id, 0);
    assert_ne!(third_leaf_offer.resolution_prompt_id, 0);
    assert_eq!(third_leaf_offer.candidate_object_ids, vec![a]);
    assert!(find_published_choice(&aura_batch, ruled_v1::ChoiceKind::PermanentObjects).is_none());
    assert_eq!(proliferate_log_count(&aura_batch, 0), 1);
    assert_eq!(ripples_resolution_log_count(&aura_batch), 0);
    {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the third actual leaf follows the retired B leaf");
        let ResolutionContinuation::Proliferate {
            stack,
            candidate_generations,
            stage,
            transaction,
            ..
        } = &pending.continuation
        else {
            panic!("the actor's original Program still owns its third leaf");
        };
        assert_eq!(pending.deciding_player, 0);
        assert!(matches!(
            stage,
            crate::state::PendingProliferateStage::SelectingRecipients
        ));
        assert_eq!(candidate_generations, &vec![(a, a_generation)]);
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 2);
        assert_eq!(transaction.pending_nodes.len(), 1);
        assert_eq!(transaction.counter_placements.len(), 1);
        let receipt = transaction.counter_placements[0];
        assert_eq!(receipt.object.object_id, a);
        assert_eq!(receipt.object.zone_change_generation, a_generation);
        assert_eq!(receipt.count, 1);
        assert_eq!((receipt.before, receipt.after), (1, 2));
        assert!(transaction
            .counter_placements
            .iter()
            .all(|receipt| receipt.object.object_id != b));
        let leaf = transaction.current_leaf.as_ref().unwrap();
        assert_eq!(leaf.applied_replacements.len(), 2);
        assert_eq!(
            leaf.applied_replacements
                .iter()
                .map(|identity| identity.source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_source, second_source])
        );
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.controller, 0);
    }

    let third_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(third_leaf_offer.resolution_prompt_id, Vec::new()),
        )
        .expect("P0 selects the empty subset for the actual third Proliferate leaf");
    assert_eq!(proliferate_log_count(&third_leaf_batch, 0), 1);
    let fourth_leaf_offer = published_choice(&third_leaf_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(fourth_leaf_offer.deciding_player_id, 0);
    assert_eq!(fourth_leaf_offer.candidate_object_ids, vec![a]);
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        2
    );
    {
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let ResolutionContinuation::Proliferate { transaction, .. } = &pending.continuation else {
            panic!("the actual fourth leaf remains in the original Program");
        };
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 3);
        assert_eq!(transaction.counter_placements.len(), 1);
        assert_eq!(transaction.counter_placements[0].object.object_id, a);
    }

    let fourth_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(fourth_leaf_offer.resolution_prompt_id, Vec::new()),
        )
        .expect("P0 selects the empty subset for the actual fourth Proliferate leaf");
    assert_eq!(proliferate_log_count(&fourth_leaf_batch, 0), 1);
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        2
    );
    let tail_offer = published_choice(&fourth_leaf_batch, ruled_v1::ChoiceKind::PermanentObjects);
    assert_eq!(tail_offer.deciding_player_id, 0);
    assert_eq!(tail_offer.candidate_object_ids, vec![a]);
    assert!(!tail_offer.candidate_object_ids.contains(&b));
    let phase_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(tail_offer.resolution_prompt_id, vec![a]),
        )
        .expect("P0 phases only A from the first committed positive receipt");
    assert!(engine.state.objects[&a].phase_status.is_some());
    assert_eq!(ripples_resolution_log_count(&phase_batch), 1);
    assert_eq!(stack_resolved_count(&phase_batch), 1);
    assert_eq!(
        proliferate_log_count(&first_leaf_batch, 0)
            + proliferate_log_count(&aura_batch, 0)
            + proliferate_log_count(&third_leaf_batch, 0)
            + proliferate_log_count(&fourth_leaf_batch, 0),
        4,
        "the four actual action leaves each emit exactly one Proliferated event"
    );
    assert!(!engine.state.objects.contains_key(&b));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert!(engine
        .state
        .staged_trigger_groups
        .iter()
        .all(|group| group.triggers.is_empty()));
    assert!(engine.state.pending_triggers.is_empty());
    let pending_order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("all four real Proliferated events bind to Ezuri's passive ability");
    assert_eq!(pending_order.deciding_player, 0);
    assert_eq!(pending_order.candidates.len(), 4);
    for trigger in &pending_order.candidates {
        assert_eq!(trigger.source_permanent_id, ezuri);
        assert_eq!(trigger.controller, 0);
        assert_eq!(trigger.card_id, "ezuri,_stalker_of_spheres");
        assert_eq!(trigger.ability_index, 1);
    }
    assert_eq!(engine.state.players[0].hand.len(), hand_at_first_offer);
    let mut order_submissions = 0;
    while let Some(order) = engine.state.pending_trigger_order.as_ref() {
        assert!(order_submissions < 4);
        let deciding_player = order.deciding_player;
        let trigger_object_id = order.candidates[0].object_id;
        engine
            .apply_scenario_command(
                deciding_player,
                &ruled_v1::RuledCommand {
                    cmd: Some(ruled_v1::ruled_command::Cmd::SubmitTriggerOrder(
                        ruled_v1::SubmitTriggerOrder { trigger_object_id },
                    )),
                },
            )
            .expect("submit an actual Ezuri trigger identity in the current order offer");
        order_submissions += 1;
    }
    assert_eq!(
        order_submissions, 3,
        "the final single trigger is automatically placed"
    );
    assert_eq!(engine.state.stack.len(), 4);
    assert!(engine.state.stack.iter().all(|item| {
        item.is_triggered
            && item.source_permanent_id == Some(ezuri)
            && item.controller == 0
            && item.card_id == "ezuri,_stalker_of_spheres"
            && item.ability_index == Some(1)
    }));
    assert!(engine.state.pending_triggers.is_empty());
    assert_eq!(engine.state.players[0].hand.len(), hand_at_first_offer);
    for remaining in (0..4).rev() {
        let hand_before_resolution = engine.state.players[0].hand.len();
        pass_live_players_with_batch(&mut engine);
        assert_eq!(engine.state.stack.len(), remaining);
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before_resolution + 1,
            "each actual Ezuri Proliferate trigger draws one card"
        );
    }
    assert_eq!(engine.state.players[0].hand.len(), hand_at_first_offer + 4);
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.staged_trigger_groups.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert!(engine.state.stack.is_empty());
}

fn flat_gambit_saved_row(registry: &'static CardRegistry) {
    let mut engine = saved_observer_engine(820_032, registry);
    let action_source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let action_source_generation = engine.state.zone_change_generation[&action_source];
    let recipient = put_saved_observer_card(&mut engine, 3, "forest", Zone::Battlefield);
    let recipient_generation = engine.state.zone_change_generation[&recipient];
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("P3's real Forest recipient")
        .set_counter(CounterKind::Charge, 1);
    let first_season =
        put_saved_observer_card(&mut engine, 3, "doubling_season", Zone::Battlefield);
    let first_season_generation = engine.state.zone_change_generation[&first_season];
    let second_season =
        put_saved_observer_card(&mut engine, 3, "doubling_season", Zone::Battlefield);
    let second_season_generation = engine.state.zone_change_generation[&second_season];
    let pacifism = put_saved_observer_card(&mut engine, 2, "pacifism", Zone::Exile);
    let pacifism_generation = engine.state.zone_change_generation[&pacifism];
    let bears = put_saved_observer_card(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
    engine.reconcile_activated_ability_slots();

    let object_id_limit = engine
        .state
        .objects
        .keys()
        .copied()
        .max()
        .expect("prepared state contains physical objects");
    let above_objects = object_id_limit
        .checked_add(1)
        .expect("room remains above physical object IDs");
    let first_handle_id = engine
        .state
        .next_replacement_application_id
        .max(above_objects);
    first_handle_id
        .checked_add(8)
        .expect("the fixed saved nested choices have checked handle headroom");
    assert!(first_handle_id > object_id_limit);
    engine.state.next_replacement_application_id = first_handle_id;

    let spell = *engine.state.players[0]
        .hand
        .first()
        .expect("P0 has a physical card to turn into actual Gambit");
    engine
        .state
        .objects
        .get_mut(&spell)
        .expect("P0's physical Gambit card")
        .card_id = "tezzerets_gambit".into();
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 3;
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|&object_id| object_id == spell)
        .expect("Gambit remains in P0's hand before casting");
    let draws_before_cast = engine.state.turn_history.current.player(0).cards_drawn;
    let hand_before_cast = engine.state.players[0].hand.len();
    engine
        .apply_scenario_command(
            0,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::CastSpell(
                    ruled_v1::CastSpell {
                        source: Some(ruled_v1::CastSource {
                            expected_zone_change_generation: None,
                            location: Some(ruled_v1::cast_source::Location::HandIndex(
                                hand_index as u32,
                            )),
                        }),
                        cast_method: ruled_v1::CastMethod::Normal as i32,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("cast the shipped Tezzeret's Gambit from P0's actual hand");
    assert_eq!(engine.state.stack.last().map(|item| item.id), Some(spell));

    let outer_batch = pass_live_players_with_batch(&mut engine);
    assert_eq!(flat_gambit_draw_log_count(&outer_batch), 1);
    let draws_after_prefix = engine.state.turn_history.current.player(0).cards_drawn;
    assert_eq!(draws_after_prefix, draws_before_cast + 2);
    assert_eq!(engine.state.players[0].hand.len(), hand_before_cast + 1);
    let first_leaf_offer = published_choice(&outer_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(first_leaf_offer.deciding_player_id, 0);
    assert!(first_leaf_offer.candidate_object_ids.contains(&recipient));

    let first_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(first_leaf_offer.resolution_prompt_id, vec![recipient]),
        )
        .expect("P0 selects P3's R for the first actual Gambit leaf");
    let first_nested_offer =
        published_choice(&first_leaf_batch, ruled_v1::ChoiceKind::ReplacementEffect);
    assert_eq!(first_nested_offer.deciding_player_id, 3);
    assert_ne!(first_nested_offer.resolution_prompt_id, 0);
    assert_eq!(first_nested_offer.replacement_options.len(), 2);
    assert!(first_nested_offer
        .replacement_options
        .iter()
        .all(|option| option.application_id > object_id_limit));
    assert!(first_nested_offer.replacement_options.iter().all(|option| {
        !engine
            .state
            .objects
            .contains_key(&(option.application_id as ObjectId))
    }));
    let (first_stack_snapshot, first_resume_index, first_previous_result) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the first actual P3 Season order is parked");
        assert_eq!(pending.deciding_player, 3);
        assert_eq!(
            pending.resolution_prompt_id.get(),
            first_nested_offer.resolution_prompt_id
        );
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
        } = &pending.continuation
        else {
            panic!("the first Season order retains the flat typed Proliferate completion");
        };
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 0);
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.card_id, "tezzerets_gambit");
        assert_eq!(stack.item.controller, 0);
        assert!(
            stack.program.is_none(),
            "actual Gambit remains on the flat path"
        );
        let PendingReplacementEvent::Counters(counter_batch) = engine
            .state
            .pending_replacement_event
            .as_ref()
            .expect("the first Season order owns the real counter batch")
        else {
            panic!("the first nested order is paired with Counters");
        };
        assert_eq!(counter_batch.event_index, 0);
        assert_eq!(counter_batch.applications.len(), 2);
        assert_eq!(
            counter_batch
                .applications
                .iter()
                .map(|application| battlefield_instance(application).source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_season, second_season])
        );
        assert!(counter_batch.applications.iter().any(|application| {
            battlefield_instance(application).source_object_id == first_season
                && battlefield_instance(application).source_zone_change_generation
                    == first_season_generation
        }));
        assert!(counter_batch.applications.iter().any(|application| {
            battlefield_instance(application).source_object_id == second_season
                && battlefield_instance(application).source_zone_change_generation
                    == second_season_generation
        }));
        (
            format!("{stack:?}"),
            stack.resume_effect_index,
            format!("{:?}", stack.previous_result),
        )
    };

    let first_order_batch = engine
        .apply_scenario_command(
            3,
            &submit_choice_with_prompt_id(
                first_nested_offer.resolution_prompt_id,
                vec![first_nested_offer.replacement_options[0].application_id],
            ),
        )
        .expect("P3 applies one actual Doubling Season to R's first counter event");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(proliferate_log_count(&first_order_batch, 0), 1);
    let second_leaf_offer = published_choice(&first_order_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(second_leaf_offer.deciding_player_id, 0);
    assert_eq!(second_leaf_offer.candidate_object_ids, vec![recipient]);

    let second_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(second_leaf_offer.resolution_prompt_id, vec![recipient]),
        )
        .expect("P0 selects P3's R for Gambit's second actual doubled leaf");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    let second_nested_offer =
        published_choice(&second_leaf_batch, ruled_v1::ChoiceKind::ReplacementEffect);
    assert_eq!(second_nested_offer.deciding_player_id, 3);
    assert_ne!(second_nested_offer.resolution_prompt_id, 0);
    assert_eq!(second_nested_offer.replacement_options.len(), 2);
    let nested_handles = second_nested_offer
        .replacement_options
        .iter()
        .map(|option| option.application_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(nested_handles.len(), 2);
    assert!(nested_handles
        .iter()
        .all(|handle| *handle > object_id_limit));
    assert!(nested_handles
        .iter()
        .all(|handle| !engine.state.objects.contains_key(&(*handle as ObjectId))));
    let (saved_pending, saved_stack, saved_transaction, saved_counter_batch) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Gambit's second actual leaf is parked at P3's nested order");
        assert_eq!(pending.deciding_player, 3);
        assert_eq!(pending.presentation.source_object_id, spell);
        assert_eq!(
            pending.presentation.choice_kind,
            ruled_v1::ChoiceKind::ReplacementEffect
        );
        assert_eq!(
            pending.resolution_prompt_id.get(),
            second_nested_offer.resolution_prompt_id
        );
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
        } = &pending.continuation
        else {
            panic!("the second Season order retains the flat typed Proliferate completion");
        };
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 1);
        assert!(transaction.pending_nodes.is_empty());
        assert_eq!(transaction.counter_placements.len(), 1);
        let receipt = transaction.counter_placements[0];
        assert_eq!(receipt.object.object_id, recipient);
        assert_eq!(receipt.object.zone_change_generation, recipient_generation);
        assert_eq!(receipt.object.controller_at_event, 3);
        assert_eq!(receipt.counter, CounterKind::Charge);
        assert_eq!(receipt.count, 4);
        assert_eq!((receipt.before, receipt.after), (1, 5));
        let leaf = transaction.current_leaf.as_ref().unwrap();
        assert_eq!(leaf.applied_replacements.len(), 1);
        let applied = leaf.applied_replacements.iter().next().unwrap();
        assert_eq!(applied.source_object_id, action_source);
        assert_eq!(applied.zone_change_generation, action_source_generation);
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.card_id, "tezzerets_gambit");
        assert_eq!(stack.item.controller, 0);
        assert!(
            stack.program.is_none(),
            "actual Gambit remains on the flat path"
        );
        assert_eq!(format!("{stack:?}"), first_stack_snapshot);
        assert_eq!(stack.resume_effect_index, first_resume_index);
        assert_eq!(
            format!("{:?}", stack.previous_result),
            first_previous_result
        );
        let PendingReplacementEvent::Counters(counter_batch) = engine
            .state
            .pending_replacement_event
            .as_ref()
            .expect("the second nested stage retains its paired Counters batch")
        else {
            panic!("the second nested order is paired with Counters");
        };
        assert_eq!(counter_batch.event_index, 0);
        assert_eq!(counter_batch.applications.len(), 2);
        assert_eq!(
            counter_batch
                .applications
                .iter()
                .map(|application| application.choice_id)
                .collect::<BTreeSet<_>>(),
            nested_handles
        );
        assert_eq!(
            counter_batch
                .applications
                .iter()
                .map(|application| battlefield_instance(application).source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_season, second_season])
        );
        assert!(counter_batch.applications.iter().any(|application| {
            battlefield_instance(application).source_object_id == first_season
                && battlefield_instance(application).source_zone_change_generation
                    == first_season_generation
        }));
        assert!(counter_batch.applications.iter().any(|application| {
            battlefield_instance(application).source_object_id == second_season
                && battlefield_instance(application).source_zone_change_generation
                    == second_season_generation
        }));
        let event = &counter_batch.events[counter_batch.event_index];
        assert_eq!(event.actor, 0);
        assert!(matches!(
            event.recipient,
            CounterPlacementRecipient::Object {
                object_id,
                zone_change_generation
            } if object_id == recipient && zone_change_generation == recipient_generation
        ));
        assert_eq!(event.counters.get(&CounterKind::Charge), Some(&1));
        (
            format!("{pending:?}"),
            format!("{stack:?}"),
            format!("{transaction:?}"),
            format!(
                "{:?}",
                engine.state.pending_replacement_event.as_ref().unwrap()
            ),
        )
    };

    let observer_batch = queue_saved_pacifism_return(&mut engine, pacifism);
    let aura_offer = published_choice(&observer_batch, ruled_v1::ChoiceKind::AuraPermanent);
    assert_eq!(aura_offer.deciding_player_id, 2);
    assert_ne!(aura_offer.resolution_prompt_id, 0);
    assert!(aura_offer.candidate_object_ids.contains(&bears));
    let observer = engine
        .state
        .pending_observer_return_batch
        .as_ref()
        .expect("the independent Aura return saves the flat nested transaction");
    assert!(observer.resume_stack.is_none());
    assert_eq!(
        format!("{:?}", observer.suspended_resolution.as_ref().unwrap()),
        saved_pending
    );
    assert_eq!(
        format!(
            "{:?}",
            observer.suspended_replacement_event.as_ref().unwrap()
        ),
        saved_counter_batch
    );
    let ResolutionContinuation::AuraReturn {
        stack: None,
        exiled,
    } = &engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the independent Aura return owns the active P2 choice")
        .continuation
    else {
        panic!("the independent AuraReturn has no competing resume stack");
    };
    assert_eq!(exiled.object_id, pacifism);
    assert_eq!(exiled.zone_change_generation, pacifism_generation);

    let concede_batch = engine
        .apply_scenario_command(
            0,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::Concede(ruled_v1::Concede {})),
            },
        )
        .expect("P0 concedes while P2's independent Aura choice is active");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.objects.contains_key(&action_source));
    assert!(!engine.state.objects.contains_key(&spell));
    assert!(engine.state.objects.contains_key(&recipient));
    assert!(engine.state.objects.contains_key(&first_season));
    assert!(engine.state.objects.contains_key(&second_season));
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(proliferate_log_count(&concede_batch, 0), 0);
    assert_eq!(flat_gambit_draw_log_count(&concede_batch), 0);
    assert_eq!(flat_gambit_resolution_log_count(&concede_batch), 0);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_after_prefix
    );
    let saved = engine
        .state
        .pending_observer_return_batch
        .as_ref()
        .expect("the flat nested continuation survives the saved-route departure filter");
    assert_eq!(
        format!("{:?}", saved.suspended_resolution.as_ref().unwrap()),
        saved_pending
    );
    assert_eq!(
        format!("{:?}", saved.suspended_replacement_event.as_ref().unwrap()),
        saved_counter_batch
    );
    let ResolutionContinuation::CounterReplacement {
        stack: Some(stack),
        completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
    } = &saved.suspended_resolution.as_ref().unwrap().continuation
    else {
        panic!("the saved flat continuation retains its typed Proliferate transaction");
    };
    assert_eq!(format!("{stack:?}"), saved_stack);
    assert_eq!(format!("{:?}", transaction), saved_transaction);
    assert!(stack.program.is_none());

    let current_aura_offer =
        find_published_choice(&concede_batch, ruled_v1::ChoiceKind::AuraPermanent)
            .unwrap_or(aura_offer);
    assert_eq!(current_aura_offer.deciding_player_id, 2);
    assert!(current_aura_offer.candidate_object_ids.contains(&bears));
    let aura_batch = engine
        .apply_scenario_command(
            2,
            &submit_choice_with_prompt_id(current_aura_offer.resolution_prompt_id, vec![bears]),
        )
        .expect("P2 answers the currently published Pacifism return offer");
    assert_eq!(engine.state.objects[&pacifism].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&pacifism].attached_to,
        Some(crate::state::AttachmentRecipient::Object(bears))
    );
    assert!(engine.state.pending_observer_return_batch.is_none());
    let restored_nested_offer =
        find_published_choice(&aura_batch, ruled_v1::ChoiceKind::ReplacementEffect)
            .expect("observer restoration republishes the current flat nested counter order");
    assert_eq!(restored_nested_offer.deciding_player_id, 3);
    assert_ne!(restored_nested_offer.resolution_prompt_id, 0);
    assert_ne!(
        restored_nested_offer.resolution_prompt_id,
        second_nested_offer.resolution_prompt_id
    );
    assert_eq!(
        restored_nested_offer
            .replacement_options
            .iter()
            .map(|option| option.application_id)
            .collect::<BTreeSet<_>>(),
        nested_handles
    );
    assert!(restored_nested_offer
        .replacement_options
        .iter()
        .all(|option| option.application_id > object_id_limit));
    assert!(restored_nested_offer
        .replacement_options
        .iter()
        .all(|option| !engine
            .state
            .objects
            .contains_key(&(option.application_id as ObjectId))));
    let restored = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P3's actual current nested order is restored");
    assert_eq!(restored.deciding_player, 3);
    assert_eq!(
        restored.resolution_prompt_id.get(),
        restored_nested_offer.resolution_prompt_id
    );
    let ResolutionContinuation::CounterReplacement {
        stack: Some(stack),
        completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
    } = &restored.continuation
    else {
        panic!("restoration keeps the typed flat Proliferate continuation");
    };
    assert_eq!(format!("{stack:?}"), saved_stack);
    assert_eq!(format!("{:?}", transaction), saved_transaction);
    assert_eq!(
        format!(
            "{:?}",
            engine.state.pending_replacement_event.as_ref().unwrap()
        ),
        saved_counter_batch
    );
    let restored_snapshot = format!("{restored:?}");
    let current_counter_batch = format!(
        "{:?}",
        engine.state.pending_replacement_event.as_ref().unwrap()
    );
    let stale_handle = *nested_handles.iter().next().unwrap();
    assert!(engine
        .apply_scenario_command(
            3,
            &submit_choice_with_prompt_id(
                second_nested_offer.resolution_prompt_id,
                vec![stale_handle],
            ),
        )
        .is_err());
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution.as_ref().unwrap()),
        restored_snapshot
    );
    assert_eq!(
        format!(
            "{:?}",
            engine.state.pending_replacement_event.as_ref().unwrap()
        ),
        current_counter_batch
    );
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_after_prefix
    );

    let final_leaf = engine
        .apply_scenario_command(
            3,
            &submit_choice_with_prompt_id(
                restored_nested_offer.resolution_prompt_id,
                vec![restored_nested_offer.replacement_options[0].application_id],
            ),
        )
        .expect("P3 answers the freshly published source-bound counter order");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        9
    );
    assert_eq!(proliferate_log_count(&first_order_batch, 0), 1);
    assert_eq!(proliferate_log_count(&final_leaf, 0), 1);
    assert_eq!(
        proliferate_log_count(&first_order_batch, 0) + proliferate_log_count(&final_leaf, 0),
        2
    );
    assert_eq!(flat_gambit_draw_log_count(&outer_batch), 1);
    assert_eq!(flat_gambit_draw_log_count(&first_leaf_batch), 0);
    assert_eq!(flat_gambit_draw_log_count(&first_order_batch), 0);
    assert_eq!(flat_gambit_draw_log_count(&second_leaf_batch), 0);
    assert_eq!(flat_gambit_draw_log_count(&observer_batch), 0);
    assert_eq!(flat_gambit_draw_log_count(&concede_batch), 0);
    assert_eq!(flat_gambit_draw_log_count(&aura_batch), 0);
    assert_eq!(flat_gambit_draw_log_count(&final_leaf), 0);
    assert_eq!(flat_gambit_resolution_log_count(&final_leaf), 1);
    assert_eq!(stack_resolved_count(&final_leaf), 0);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_after_prefix
    );
    assert!(!engine.state.objects.contains_key(&action_source));
    assert!(!engine.state.objects.contains_key(&spell));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert!(engine.state.stack.is_empty());
}

fn flat_nested_gambit_registry() -> &'static CardRegistry {
    let chunks = [
        include_str!("../../../../tricerules-cards/data/forest.ron"),
        include_str!("../../../../tricerules-cards/data/doubling_season.ron"),
        include_str!("../../../../tricerules-cards/data/tezzerets_gambit.ron"),
        include_str!("../../../../tricerules-cards/data/pacifism.ron"),
        include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
        RETURNED_SOURCE_FIXTURE,
    ];
    Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(&chunks, &[])
            .expect("unchanged Gambit, Season, Aura and fixture definitions validate"),
    ))
}

fn flat_gambit_draw_log_count(batch: &ruled_v1::RuledEventBatch) -> usize {
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_v1::ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .filter(|text| *text == "P0 draws 2 cards (Tezzeret's Gambit).")
        .count()
}

fn flat_gambit_resolution_log_count(batch: &ruled_v1::RuledEventBatch) -> usize {
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_v1::ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .filter(|text| *text == "Tezzeret's Gambit resolves.")
        .count()
}

#[test]
fn flat_second_counter_batch_survives_actor_departure_in_active_and_saved_routes() {
    let registry = flat_nested_gambit_registry();
    let mut engine = saved_observer_engine(820_031, registry);
    let action_source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let action_source_generation = engine.state.zone_change_generation[&action_source];
    let recipient = put_saved_observer_card(&mut engine, 3, "forest", Zone::Battlefield);
    let recipient_generation = engine.state.zone_change_generation[&recipient];
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("P3's real Forest recipient")
        .set_counter(CounterKind::Charge, 1);
    let first_season =
        put_saved_observer_card(&mut engine, 3, "doubling_season", Zone::Battlefield);
    let first_season_generation = engine.state.zone_change_generation[&first_season];
    let second_season =
        put_saved_observer_card(&mut engine, 3, "doubling_season", Zone::Battlefield);
    let second_season_generation = engine.state.zone_change_generation[&second_season];
    engine.reconcile_activated_ability_slots();

    let spell = *engine.state.players[0]
        .hand
        .first()
        .expect("P0 has a physical card to turn into actual Gambit");
    engine
        .state
        .objects
        .get_mut(&spell)
        .expect("P0's physical Gambit card")
        .card_id = "tezzerets_gambit".into();
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 3;
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|&object_id| object_id == spell)
        .expect("Gambit remains in P0's hand before casting");
    let draws_before_cast = engine.state.turn_history.current.player(0).cards_drawn;
    let hand_before_cast = engine.state.players[0].hand.len();
    engine
        .apply_scenario_command(
            0,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::CastSpell(
                    ruled_v1::CastSpell {
                        source: Some(ruled_v1::CastSource {
                            expected_zone_change_generation: None,
                            location: Some(ruled_v1::cast_source::Location::HandIndex(
                                hand_index as u32,
                            )),
                        }),
                        cast_method: ruled_v1::CastMethod::Normal as i32,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("cast the shipped Tezzeret's Gambit from P0's actual hand");
    assert_eq!(engine.state.stack.last().map(|item| item.id), Some(spell));

    let outer_batch = pass_live_players_with_batch(&mut engine);
    assert_eq!(flat_gambit_draw_log_count(&outer_batch), 1);
    let draws_after_prefix = engine.state.turn_history.current.player(0).cards_drawn;
    assert_eq!(draws_after_prefix, draws_before_cast + 2);
    assert_eq!(engine.state.players[0].hand.len(), hand_before_cast + 1);
    let first_leaf_offer = published_choice(&outer_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(first_leaf_offer.deciding_player_id, 0);
    assert!(first_leaf_offer.candidate_object_ids.contains(&recipient));

    let first_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(first_leaf_offer.resolution_prompt_id, vec![recipient]),
        )
        .expect("P0 selects P3's R for the first actual Gambit leaf");
    assert_eq!(flat_gambit_draw_log_count(&first_leaf_batch), 0);
    let first_nested_offer =
        published_choice(&first_leaf_batch, ruled_v1::ChoiceKind::ReplacementEffect);
    assert_eq!(first_nested_offer.deciding_player_id, 3);
    assert_ne!(first_nested_offer.resolution_prompt_id, 0);
    assert_eq!(first_nested_offer.replacement_options.len(), 2);
    let (first_stack_snapshot, first_item_snapshot, first_resume_index, first_previous_result) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the first actual P3 Season order is parked");
        assert_eq!(pending.deciding_player, 3);
        assert_eq!(
            pending.resolution_prompt_id.get(),
            first_nested_offer.resolution_prompt_id
        );
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
        } = &pending.continuation
        else {
            panic!("the first Season order retains the flat typed Proliferate completion");
        };
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 0);
        assert!(transaction.counter_placements.is_empty());
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.card_id, "tezzerets_gambit");
        assert_eq!(stack.item.controller, 0);
        assert!(
            stack.program.is_none(),
            "actual Gambit remains on the flat path"
        );
        let PendingReplacementEvent::Counters(counter_batch) = engine
            .state
            .pending_replacement_event
            .as_ref()
            .expect("the first Season order owns the real counter batch")
        else {
            panic!("the first nested order is paired with Counters");
        };
        assert_eq!(counter_batch.event_index, 0);
        assert_eq!(counter_batch.applications.len(), 2);
        assert_eq!(
            counter_batch
                .applications
                .iter()
                .map(|application| battlefield_instance(application).source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_season, second_season])
        );
        (
            format!("{stack:?}"),
            format!("{:?}", stack.item),
            stack.resume_effect_index,
            format!("{:?}", stack.previous_result),
        )
    };

    let first_order_batch = engine
        .apply_scenario_command(
            3,
            &submit_choice_with_prompt_id(
                first_nested_offer.resolution_prompt_id,
                vec![first_nested_offer.replacement_options[0].application_id],
            ),
        )
        .expect("P3 applies one actual Doubling Season to R's first counter event");
    assert_eq!(flat_gambit_draw_log_count(&first_order_batch), 0);
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(proliferate_log_count(&first_order_batch, 0), 1);
    let second_leaf_offer = published_choice(&first_order_batch, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(second_leaf_offer.deciding_player_id, 0);
    assert_eq!(second_leaf_offer.candidate_object_ids, vec![recipient]);

    let second_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(second_leaf_offer.resolution_prompt_id, vec![recipient]),
        )
        .expect("P0 selects P3's R for Gambit's second actual doubled leaf");
    assert_eq!(flat_gambit_draw_log_count(&second_leaf_batch), 0);
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    let second_nested_offer =
        published_choice(&second_leaf_batch, ruled_v1::ChoiceKind::ReplacementEffect);
    assert_eq!(second_nested_offer.deciding_player_id, 3);
    assert_ne!(second_nested_offer.resolution_prompt_id, 0);
    assert_eq!(second_nested_offer.replacement_options.len(), 2);
    let nested_handles = second_nested_offer
        .replacement_options
        .iter()
        .map(|option| option.application_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(nested_handles.len(), 2);
    let (saved_pending, saved_stack, saved_transaction, saved_counter_batch) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Gambit's second actual leaf is parked at P3's nested order");
        assert_eq!(pending.deciding_player, 3);
        assert_eq!(pending.presentation.source_object_id, spell);
        assert_eq!(
            pending.presentation.choice_kind,
            ruled_v1::ChoiceKind::ReplacementEffect
        );
        assert_eq!(
            pending.resolution_prompt_id.get(),
            second_nested_offer.resolution_prompt_id
        );
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
        } = &pending.continuation
        else {
            panic!("the second Season order retains the flat typed Proliferate completion");
        };
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 1);
        assert!(
            transaction.pending_nodes.is_empty(),
            "the second leaf has no sibling behind it"
        );
        assert_eq!(transaction.counter_placements.len(), 1);
        let receipt = transaction.counter_placements[0];
        assert_eq!(receipt.object.object_id, recipient);
        assert_eq!(receipt.object.zone_change_generation, recipient_generation);
        assert_eq!(receipt.object.controller_at_event, 3);
        assert_eq!(receipt.counter, CounterKind::Charge);
        assert_eq!(receipt.count, 4);
        assert_eq!((receipt.before, receipt.after), (1, 5));
        let leaf = transaction.current_leaf.as_ref().unwrap();
        assert_eq!(leaf.applied_replacements.len(), 1);
        let applied = leaf.applied_replacements.iter().next().unwrap();
        assert_eq!(applied.source_object_id, action_source);
        assert_eq!(applied.zone_change_generation, action_source_generation);
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.card_id, "tezzerets_gambit");
        assert_eq!(stack.item.controller, 0);
        assert!(
            stack.program.is_none(),
            "actual Gambit remains on the flat path"
        );
        assert_eq!(format!("{:?}", stack.item), first_item_snapshot);
        assert_eq!(format!("{stack:?}"), first_stack_snapshot);
        assert_eq!(stack.resume_effect_index, first_resume_index);
        assert_eq!(
            format!("{:?}", stack.previous_result),
            first_previous_result
        );
        let PendingReplacementEvent::Counters(counter_batch) = engine
            .state
            .pending_replacement_event
            .as_ref()
            .expect("the second nested stage retains its paired Counters batch")
        else {
            panic!("the second nested order is paired with Counters");
        };
        assert_eq!(counter_batch.event_index, 0);
        assert_eq!(counter_batch.applications.len(), 2);
        assert_eq!(
            counter_batch
                .applications
                .iter()
                .map(|application| application.choice_id)
                .collect::<BTreeSet<_>>(),
            nested_handles
        );
        assert_eq!(
            counter_batch
                .applications
                .iter()
                .map(|application| battlefield_instance(application).source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_season, second_season])
        );
        assert!(counter_batch.applications.iter().any(|application| {
            battlefield_instance(application).source_object_id == first_season
                && battlefield_instance(application).source_zone_change_generation
                    == first_season_generation
        }));
        assert!(counter_batch.applications.iter().any(|application| {
            battlefield_instance(application).source_object_id == second_season
                && battlefield_instance(application).source_zone_change_generation
                    == second_season_generation
        }));
        let event = &counter_batch.events[counter_batch.event_index];
        assert_eq!(event.actor, 0);
        assert!(matches!(
            event.recipient,
            CounterPlacementRecipient::Object {
                object_id,
                zone_change_generation
            } if object_id == recipient && zone_change_generation == recipient_generation
        ));
        assert_eq!(event.counters.get(&CounterKind::Charge), Some(&1));
        (
            format!("{pending:?}"),
            format!("{stack:?}"),
            format!("{transaction:?}"),
            format!(
                "{:?}",
                engine.state.pending_replacement_event.as_ref().unwrap()
            ),
        )
    };
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(flat_gambit_draw_log_count(&outer_batch), 1);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_after_prefix
    );
    assert!(engine.state.objects.contains_key(&first_season));
    assert!(engine.state.objects.contains_key(&second_season));

    let concede_batch = engine
        .apply_scenario_command(
            0,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::Concede(ruled_v1::Concede {})),
            },
        )
        .expect("P0 concedes while P3's actual second nested order is active");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.objects.contains_key(&action_source));
    assert!(!engine.state.objects.contains_key(&spell));
    assert!(engine.state.objects.contains_key(&recipient));
    assert!(engine.state.objects.contains_key(&first_season));
    assert!(engine.state.objects.contains_key(&second_season));
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(proliferate_log_count(&concede_batch, 0), 0);
    assert_eq!(flat_gambit_draw_log_count(&concede_batch), 0);
    assert_eq!(flat_gambit_resolution_log_count(&concede_batch), 0);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_after_prefix
    );
    let active_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("active flat nested Proliferate work survives actor departure");
    assert_eq!(active_pending.deciding_player, 3);
    assert_eq!(
        active_pending.resolution_prompt_id.get(),
        second_nested_offer.resolution_prompt_id
    );
    let ResolutionContinuation::CounterReplacement {
        stack: Some(stack),
        completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
    } = &active_pending.continuation
    else {
        panic!("the active current offer keeps the flat typed continuation");
    };
    assert_eq!(format!("{active_pending:?}"), saved_pending);
    assert_eq!(format!("{stack:?}"), saved_stack);
    assert_eq!(format!("{:?}", transaction), saved_transaction);
    assert_eq!(
        format!(
            "{:?}",
            engine.state.pending_replacement_event.as_ref().unwrap()
        ),
        saved_counter_batch
    );

    let final_leaf = engine
        .apply_scenario_command(
            3,
            &submit_choice_with_prompt_id(
                second_nested_offer.resolution_prompt_id,
                vec![second_nested_offer.replacement_options[0].application_id],
            ),
        )
        .expect("P3 answers the same current opaque offer after P0 leaves");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        9
    );
    assert_eq!(proliferate_log_count(&first_order_batch, 0), 1);
    assert_eq!(proliferate_log_count(&final_leaf, 0), 1);
    assert_eq!(flat_gambit_draw_log_count(&final_leaf), 0);
    assert_eq!(flat_gambit_resolution_log_count(&final_leaf), 1);
    assert_eq!(stack_resolved_count(&final_leaf), 0);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_after_prefix
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert!(engine.state.stack.is_empty());
    flat_gambit_saved_row(registry);
}

#[test]
fn ripples_second_counter_batch_keeps_first_child_receipt_after_actor_concedes() {
    let registry = saved_observer_registry_with_doubling_season();
    let mut engine = saved_observer_engine(820_041, registry);
    let source =
        put_local_fixture_on_battlefield(&mut engine, "tekuthal_proliferate_return_fixture");
    let source_generation = engine.state.zone_change_generation[&source];
    let a = put_saved_observer_card(&mut engine, 3, "forest", Zone::Battlefield);
    let a_generation = engine.state.zone_change_generation[&a];
    engine
        .state
        .objects
        .get_mut(&a)
        .expect("P3's A recipient")
        .set_counter(CounterKind::Charge, 1);
    let b = put_saved_observer_card(&mut engine, 3, "forest", Zone::Battlefield);
    let b_generation = engine.state.zone_change_generation[&b];
    engine
        .state
        .objects
        .get_mut(&b)
        .expect("P3's B recipient")
        .set_counter(CounterKind::Charge, 1);
    let first_season =
        put_saved_observer_card(&mut engine, 3, "doubling_season", Zone::Battlefield);
    let second_season =
        put_saved_observer_card(&mut engine, 3, "doubling_season", Zone::Battlefield);
    let first_season_generation = engine.state.zone_change_generation[&first_season];
    let second_season_generation = engine.state.zone_change_generation[&second_season];
    engine.reconcile_activated_ability_slots();

    assert_eq!(engine.state.objects[&source].controller, 0);
    for permanent in [a, b, first_season, second_season] {
        assert_eq!(engine.state.objects[&permanent].controller, 3);
    }
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert!(engine.state.pending_triggers.is_empty());

    let (spell, initial_batch) = cast_saved_ripples(&mut engine);
    let first_offer = published_choice(&initial_batch, ruled_v1::ChoiceKind::Proliferate);
    let mut recipients = vec![a, b];
    recipients.sort_unstable();
    assert_eq!(first_offer.deciding_player_id, 0);
    assert_eq!(first_offer.candidate_object_ids, recipients);
    assert_ne!(first_offer.resolution_prompt_id, 0);

    let first_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(first_offer.resolution_prompt_id, vec![a]),
        )
        .expect("P0 chooses only A for the first actual action leaf");
    let first_nested_offer =
        published_choice(&first_leaf_batch, ruled_v1::ChoiceKind::ReplacementEffect);
    assert_eq!(first_nested_offer.deciding_player_id, 3);
    assert_eq!(first_nested_offer.replacement_options.len(), 2);
    let first_completed = engine
        .apply_scenario_command(
            3,
            &submit_choice_with_prompt_id(
                first_nested_offer.resolution_prompt_id,
                vec![first_nested_offer.replacement_options[0].application_id],
            ),
        )
        .expect("P3 applies both actual Doubling Season replacements to A");
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(
        engine.state.objects[&b].counter_count(CounterKind::Charge),
        1
    );
    assert_eq!(proliferate_log_count(&first_completed, 0), 1);
    assert_eq!(ripples_resolution_log_count(&first_completed), 0);
    let second_offer = published_choice(&first_completed, ruled_v1::ChoiceKind::Proliferate);
    assert_eq!(second_offer.deciding_player_id, 0);
    assert_eq!(second_offer.candidate_object_ids, recipients);
    assert_ne!(second_offer.resolution_prompt_id, 0);

    let second_leaf_batch = engine
        .apply_scenario_command(
            0,
            &submit_choice_with_prompt_id(second_offer.resolution_prompt_id, vec![b]),
        )
        .expect("P0 chooses only B for the active second action leaf");
    let nested_offer =
        published_choice(&second_leaf_batch, ruled_v1::ChoiceKind::ReplacementEffect);
    assert_eq!(nested_offer.deciding_player_id, 3);
    assert_ne!(nested_offer.resolution_prompt_id, 0);
    assert_eq!(nested_offer.replacement_options.len(), 2);
    assert!(
        find_published_choice(&second_leaf_batch, ruled_v1::ChoiceKind::PermanentObjects).is_none()
    );
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(
        engine.state.objects[&b].counter_count(CounterKind::Charge),
        1
    );

    let nested_handles = nested_offer
        .replacement_options
        .iter()
        .map(|option| option.application_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(nested_handles.len(), 2);
    let (
        pending_snapshot,
        stack_snapshot,
        item_snapshot,
        previous_result_snapshot,
        program_snapshot,
        transaction_snapshot,
        counter_batch_snapshot,
    ) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("P3's actual second-leaf counter order is active");
        assert_eq!(pending.deciding_player, 3);
        assert_eq!(pending.presentation.source_object_id, spell);
        assert_eq!(
            pending.resolution_prompt_id.get(),
            nested_offer.resolution_prompt_id
        );
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
        } = &pending.continuation
        else {
            panic!("the active counter order retains its typed Proliferate completion");
        };
        assert_eq!(transaction.actor, 0);
        assert_eq!(transaction.completed_leaves, 1);
        assert!(transaction.pending_nodes.is_empty());
        let leaf = transaction
            .current_leaf
            .as_ref()
            .expect("B is the active second leaf");
        assert_eq!(leaf.applied_replacements.len(), 1);
        let applied = leaf.applied_replacements.iter().next().unwrap();
        assert_eq!(applied.source_object_id, source);
        assert_eq!(applied.zone_change_generation, source_generation);
        assert_eq!(
            applied.ability_occurrence.ability_path,
            vec![AbilityId::new("tekuthal_static").unwrap()]
        );
        assert_eq!(transaction.counter_placements.len(), 1);
        let receipt = transaction.counter_placements[0];
        assert_eq!(receipt.object.object_id, a);
        assert_eq!(receipt.object.zone_change_generation, a_generation);
        assert_eq!(receipt.object.controller_at_event, 3);
        assert_eq!(receipt.counter, CounterKind::Charge);
        assert_eq!(receipt.count, 4);
        assert_eq!((receipt.before, receipt.after), (1, 5));
        assert_eq!(stack.item.id, spell);
        assert_eq!(stack.item.card_id, "ripples_of_potential");
        assert_eq!(stack.item.controller, 0);
        assert_eq!(stack.item.source_owner, None);
        assert_eq!(
            stack.item.cast_occurrence,
            Some(crate::state::StackObjectRef {
                object_id: spell,
                zone_change_generation: Some(engine.state.zone_change_generation[&spell]),
            })
        );
        assert_eq!(stack.resume_effect_index, None);
        let program = stack
            .program
            .as_ref()
            .expect("the actual Ripples Program remains parked");
        assert_eq!(
            program.owner,
            ProgramOwnerId::face(
                "ripples_of_potential",
                CardFaceId::new("ripples_of_potential").unwrap()
            )
        );
        assert_eq!(program.frames.len(), 1);
        assert_eq!(program.frames[0].next_instruction, 1);
        assert!(program.in_flight.is_some());
        let PendingReplacementEvent::Counters(batch) = engine
            .state
            .pending_replacement_event
            .as_ref()
            .expect("the typed completion is paired with its current counter batch")
        else {
            panic!("the active nested event is a counter placement");
        };
        assert_eq!(batch.event_index, 0);
        assert_eq!(batch.applications.len(), 2);
        assert_eq!(
            batch
                .applications
                .iter()
                .map(|application| application.choice_id)
                .collect::<BTreeSet<_>>(),
            nested_handles
        );
        assert_eq!(
            batch
                .applications
                .iter()
                .map(|application| battlefield_instance(application).source_object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first_season, second_season])
        );
        assert!(batch.applications.iter().any(|application| {
            battlefield_instance(application).source_object_id == first_season
                && battlefield_instance(application).source_zone_change_generation
                    == first_season_generation
        }));
        assert!(batch.applications.iter().any(|application| {
            battlefield_instance(application).source_object_id == second_season
                && battlefield_instance(application).source_zone_change_generation
                    == second_season_generation
        }));
        let event = &batch.events[batch.event_index];
        assert_eq!(event.actor, 0);
        assert!(matches!(
            event.recipient,
            CounterPlacementRecipient::Object {
                object_id,
                zone_change_generation
            } if object_id == b && zone_change_generation == b_generation
        ));
        assert_eq!(event.counters.get(&CounterKind::Charge), Some(&1));
        (
            format!("{pending:?}"),
            format!("{stack:?}"),
            format!("{:?}", stack.item),
            format!("{:?}", stack.previous_result),
            format!("{program:?}"),
            format!("{transaction:?}"),
            format!(
                "{:?}",
                engine.state.pending_replacement_event.as_ref().unwrap()
            ),
        )
    };
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert_eq!(proliferate_log_count(&second_leaf_batch, 0), 0);
    assert_eq!(ripples_resolution_log_count(&second_leaf_batch), 0);

    let concede_batch = engine
        .apply_scenario_command(
            0,
            &ruled_v1::RuledCommand {
                cmd: Some(ruled_v1::ruled_command::Cmd::Concede(ruled_v1::Concede {})),
            },
        )
        .expect("the original actor concedes while P3's second counter order is active");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.objects.contains_key(&source));
    assert!(!engine.state.objects.contains_key(&spell));
    for permanent in [a, b, first_season, second_season] {
        assert!(engine.state.objects.contains_key(&permanent));
    }
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(
        engine.state.objects[&b].counter_count(CounterKind::Charge),
        1
    );
    assert_eq!(proliferate_log_count(&concede_batch, 0), 0);
    assert_eq!(ripples_resolution_log_count(&concede_batch), 0);
    let active_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the active second counter order remains parked after actor departure");
    assert_eq!(format!("{active_pending:?}"), pending_snapshot);
    assert_eq!(active_pending.deciding_player, 3);
    assert_eq!(
        active_pending.resolution_prompt_id.get(),
        nested_offer.resolution_prompt_id
    );
    let ResolutionContinuation::CounterReplacement {
        stack: Some(stack),
        completion: crate::state::CounterPlacementCompletion::Proliferate { transaction },
    } = &active_pending.continuation
    else {
        panic!("actor departure leaves the original typed continuation intact");
    };
    assert_eq!(format!("{stack:?}"), stack_snapshot);
    assert_eq!(format!("{:?}", stack.item), item_snapshot);
    assert_eq!(
        format!("{:?}", stack.previous_result),
        previous_result_snapshot
    );
    assert_eq!(
        format!("{:?}", stack.program.as_ref().unwrap()),
        program_snapshot
    );
    assert_eq!(format!("{transaction:?}"), transaction_snapshot);
    assert_eq!(
        format!(
            "{:?}",
            engine.state.pending_replacement_event.as_ref().unwrap()
        ),
        counter_batch_snapshot
    );
    if let Some(republished) =
        find_published_choice(&concede_batch, ruled_v1::ChoiceKind::ReplacementEffect)
    {
        assert_eq!(
            republished.resolution_prompt_id,
            nested_offer.resolution_prompt_id
        );
        assert_eq!(
            republished
                .replacement_options
                .iter()
                .map(|option| option.application_id)
                .collect::<BTreeSet<_>>(),
            nested_handles
        );
    }
    assert!(engine.state.pending_observer_return_batch.is_none());

    let final_leaf = engine
        .apply_scenario_command(
            3,
            &submit_choice_with_prompt_id(
                nested_offer.resolution_prompt_id,
                vec![nested_offer.replacement_options[0].application_id],
            ),
        )
        .expect("P3 answers the same current counter offer after P0 leaves");
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(
        engine.state.objects[&b].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(proliferate_log_count(&final_leaf, 0), 1);
    assert_eq!(ripples_resolution_log_count(&final_leaf), 1);
    assert_eq!(stack_resolved_count(&final_leaf), 0);
    assert!(find_published_choice(&final_leaf, ruled_v1::ChoiceKind::PermanentObjects).is_none());
    assert!(engine.state.objects[&a].phase_status.is_none());
    assert!(engine.state.objects[&b].phase_status.is_none());
    assert!(!engine.state.objects.contains_key(&source));
    assert!(!engine.state.objects.contains_key(&spell));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}
