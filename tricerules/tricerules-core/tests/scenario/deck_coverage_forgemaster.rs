//! Exact three-artifact activation payments for Kuldotha Forgemaster.
use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_core::Zone;

fn game(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, Some(vec![deck; 2]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_creature(engine: &mut GameEngine, card: &str, cost: u32) -> u32 {
    assert!(
        CardRegistry::global().get(card).is_some(),
        "missing exact {card}"
    );
    let source = inject_card_into_hand(engine, 0, card);
    give_mana(
        engine,
        0,
        ManaGift {
            c: cost,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, card);
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    source
}

#[test]
fn forgemaster_actual_cast_has_exact_three_five() {
    let mut engine = game(2026100111);
    let source = cast_creature(&mut engine, "kuldotha_forgemaster", 5);
    let stats = engine.characteristics(source).unwrap();
    assert_eq!((stats.power, stats.toughness), (Some(3), Some(5)));
    assert!(stats.is_artifact() && stats.is_creature());
    let a = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let b = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let command = forge_command(&engine, source, &[source, a, b]);
    rejected_unchanged(&mut engine, 0, &command); // Newly cast creature cannot pay {T}.
}

fn rejected_unchanged(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = format!("{:?}", engine.state);
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}

fn resolve_one(engine: &mut GameEngine) -> RuledEventBatch {
    let mut result = RuledEventBatch::default();
    let count = engine.state.players.iter().filter(|p| !p.has_lost).count()
        - engine.state.passes_since_stack_change as usize;
    for _ in 0..count {
        let actor = engine.state.priority_player_id();
        result = engine.apply_command(actor, &pass()).unwrap();
    }
    result
}

fn forge_command(engine: &GameEngine, source: u32, objects: &[u32]) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.cost_selections = vec![CostSelection {
        cost_index: 1,
        selection: Some(
            tricerules_proto::ruled::v1::cost_selection::Selection::BattlefieldObjects(
                tricerules_proto::ruled::v1::CostObjectRefs {
                    objects: objects
                        .iter()
                        .map(|oid| tricerules_proto::ruled::v1::CostObjectRef {
                            object_id: *oid,
                            zone_change_generation: engine
                                .state
                                .zone_change_generation
                                .get(oid)
                                .copied()
                                .unwrap_or(0),
                        })
                        .collect(),
                },
            ),
        ),
    }];
    command
}

#[test]
fn forgemaster_three_artifact_cohort_includes_source_then_search_and_shuffle() {
    let mut engine = game(2026100114);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "kuldotha_forgemaster");
    let a = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let b = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let foreign = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let nonartifact = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let hit = inject_library_card(&mut engine, 0, "pentavus");
    let land = inject_library_card(&mut engine, 0, "forest");
    let foreign_hit = inject_library_card(&mut engine, 1, "sol_ring");
    let count = engine.state.objects.len();
    let generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let offer = engine.initial_response_batch();
    let costs = &offer.legal_by_player[&0].cost_choices_by_ability[&(u64::from(source) << 32)];
    assert!(costs.non_mana_costs_payable);
    assert_eq!(costs.choices.len(), 1);
    for (i, choice) in costs.choices.iter().enumerate() {
        assert_eq!(choice.cost_index, i as u32 + 1);
        assert_eq!((choice.min, choice.max), (3, 3));
        let candidates: Vec<_> = choice
            .candidate_objects
            .iter()
            .filter_map(|c| c.object.map(|o| o.object_id))
            .collect();
        assert!(candidates.contains(&source) && candidates.contains(&a) && candidates.contains(&b));
        assert!(!candidates.contains(&foreign) && !candidates.contains(&nonartifact));
    }
    for objects in [
        vec![source, a],
        vec![source, a, b, foreign],
        vec![source, a, a],
        vec![source, a, foreign],
        vec![source, a, nonartifact],
    ] {
        let command = forge_command(&engine, source, &objects);
        rejected_unchanged(&mut engine, 0, &command);
    }
    let command = forge_command(&engine, source, &[source, a, b]);
    rejected_unchanged(&mut engine, 1, &command);
    let mut stale = command.clone();
    let Some(Cmd::ActivateAbility(activation)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation += 1;
    rejected_unchanged(&mut engine, 0, &stale);
    let mut stale_member = command.clone();
    let Some(Cmd::ActivateAbility(activation)) = stale_member.cmd.as_mut() else {
        unreachable!()
    };
    let Some(tricerules_proto::cost_selection::Selection::BattlefieldObjects(selected)) =
        activation.cost_selections[0].selection.as_mut()
    else {
        unreachable!()
    };
    selected.objects[1].zone_change_generation += 1;
    rejected_unchanged(&mut engine, 0, &stale_member);
    let mut duplicate_index = command.clone();
    let Some(Cmd::ActivateAbility(activation)) = duplicate_index.cmd.as_mut() else {
        unreachable!()
    };
    activation
        .cost_selections
        .push(activation.cost_selections[0].clone());
    rejected_unchanged(&mut engine, 0, &duplicate_index);
    let mut scalar = command.clone();
    let Some(Cmd::ActivateAbility(activation)) = scalar.cmd.as_mut() else {
        unreachable!()
    };
    activation.cost_selections[0] = permanent_cost_selection(1, source);
    rejected_unchanged(&mut engine, 0, &scalar);
    engine.state.objects.get_mut(&source).unwrap().tapped = true;
    rejected_unchanged(&mut engine, 0, &command);
    engine.state.objects.get_mut(&source).unwrap().tapped = false;
    engine.apply_command(0, &command).unwrap();
    for oid in [source, a, b] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
        assert!(engine.state.players[0].graveyard.contains(&oid));
    }
    assert_eq!(engine.state.zone_change_generation[&source], generation + 1);
    assert_eq!(engine.state.objects[&hit].zone, Zone::Library);
    assert_eq!(engine.state.objects[&foreign].zone, Zone::Battlefield);
    assert_eq!(engine.state.stack.len(), 1);
    let batch = resolve_one(&mut engine);
    let choice = find_resolution_choice(&batch).unwrap();
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&hit));
    assert!(!choice.candidate_object_ids.contains(&land));
    assert!(!choice.candidate_object_ids.contains(&foreign_hit));
    for (actor, objects) in [
        (1, vec![hit]),
        (0, vec![land]),
        (0, vec![foreign_hit]),
        (0, vec![hit, hit]),
    ] {
        rejected_unchanged(&mut engine, actor, &submit_resolution_choice(objects));
    }
    let completed = engine
        .apply_command(0, &submit_resolution_choice(vec![hit]))
        .unwrap();
    assert_eq!(engine.state.objects[&hit].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&hit].controller, 0);
    assert!(!engine.state.objects[&hit].tapped);
    assert_eq!(
        engine.state.objects[&hit].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        5
    );
    assert!(!engine.state.players[0].library.contains(&hit));
    assert_eq!(shuffle_count(&completed), 1);
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects.len(),
        count,
        "physical cards are moved, not recreated"
    );
}

fn shuffle_count(batch: &RuledEventBatch) -> usize {
    batch
        .events
        .iter()
        .filter(|e| matches!(&e.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
        .count()
}

#[test]
fn forgemaster_single_three_artifact_cost_has_one_simultaneous_departure_group() {
    let mut engine = game(2026100116);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "kuldotha_forgemaster");
    let artist = inject_permanent_on_battlefield(&mut engine, 0, "blood_artist");
    let gnomes = inject_permanent_on_battlefield(&mut engine, 0, "bottle_gnomes");
    engine
        .state
        .continuous_effects
        .push(tricerules_core::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: tricerules_core::AffectedScope::Single(artist),
            kind: tricerules_cards::ContinuousEffectKind::Layer4AddTypes(
                tricerules_cards::TypeLineAddition {
                    card_types: vec![tricerules_cards::PermanentTypeFilter::Artifact],
                    ..Default::default()
                },
            ),
            condition: None,
            duration: tricerules_cards::EffectDuration::Indefinite,
            timestamp: 1,
        });
    let command = forge_command(&engine, source, &[source, artist, gnomes]);
    engine.apply_command(0, &command).unwrap();
    let captured = engine.state.pending_triggers.len()
        + engine
            .state
            .pending_trigger_order
            .as_ref()
            .map_or(0, |order| order.candidates.len());
    assert_eq!(
        captured, 3,
        "Blood Artist sees all three simultaneous creature deaths, regardless of selection order"
    );
}

#[test]
fn forgemaster_fewer_artifacts_unpayable_and_failure_to_find_still_shuffles() {
    let mut engine = game(2026100115);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "kuldotha_forgemaster");
    let a = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let command = forge_command(&engine, source, &[source, a, a]);
    rejected_unchanged(&mut engine, 0, &command);
    let offer = engine.initial_response_batch();
    assert!(
        !offer.legal_by_player[&0].cost_choices_by_ability[&(u64::from(source) << 32)]
            .non_mana_costs_payable
    );
    let b = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    // Hidden qualified searches may fail to find even an existing eligible card.
    let hit = inject_library_card(&mut engine, 0, "sol_ring");
    let command = forge_command(&engine, source, &[source, a, b]);
    engine.apply_command(0, &command).unwrap();
    resolve_one(&mut engine);
    let completed = engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    assert_eq!(engine.state.objects[&hit].zone, Zone::Library);
    assert_eq!(shuffle_count(&completed), 1);
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
}

#[test]
fn forgemaster_departing_observer_distinguishes_each_object_from_one_or_more() {
    use tricerules_cards::primitives::{CastTriggerPlayer, TriggerCondition, ZoneEventCardinality};
    for (cardinality, expected) in [
        (ZoneEventCardinality::OneOrMore, 1),
        (ZoneEventCardinality::EachObject, 3),
    ] {
        let deck = deck_with("forest", &[]);
        let mut engine =
            GameEngine::new(50202, &[10, 20, 30], 20, Some(vec![deck; 3]), true).unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let source = inject_permanent_on_battlefield(&mut engine, 0, "kuldotha_forgemaster");
        let a = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let b = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
        engine.state.objects.get_mut(&b).unwrap().owner = 30;
        let mut ability = CardRegistry::global()
            .get("ajanis_pridemate")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        ability.trigger = TriggerCondition::WheneverPermanentLeavesBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: Default::default(),
            destination: Default::default(),
            cardinality,
        };
        engine
            .state
            .add_triggered_ability_grant(tricerules_core::ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: tricerules_core::AffectedScope::Single(source),
                kind: tricerules_cards::ContinuousEffectKind::GrantTriggeredAbility(Box::new(
                    ability,
                )),
                condition: None,
                duration: tricerules_cards::EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index,
            });
        let command = forge_command(&engine, source, &[source, a, b]);
        rejected_unchanged(&mut engine, 20, &command);
        engine.apply_command(10, &command).unwrap();
        let captured = engine.state.pending_triggers.len()
            + engine
                .state
                .pending_trigger_order
                .as_ref()
                .map_or(0, |order| order.candidates.len());
        assert_eq!(captured + engine.state.stack.len() - 1, expected);
        assert!(engine.state.players[2].graveyard.contains(&b));
        assert_eq!(engine.state.objects[&b].controller, 30);
        assert!(engine.state.players[0].graveyard.contains(&source));
    }
}
