//! Prepared targeted poison-emblem capability fixtures, not actual Ajani admission.
use super::super::replacement::{
    CounterPlacementRecipient, CounterPlacementReplacement, PendingReplacementEvent,
};
use super::*;

const WALKER: &str = r#"(
    id:"synthetic_poison_emblem_walker",name:"Synthetic Poison Emblem Walker",
    face_id:"synthetic_poison_emblem_walker",mana_cost:"{0}",types:["Planeswalker"],loyalty:6,
    activated_abilities:[(ability_id:"ultimate",presentation:Fallback,costs:[Loyalty(-6)],
        effect:[CreateTriggeredEmblem(emblem_id:"synthetic_targeted_poison_emblem",
            display_name:"Synthetic Targeted Poison Emblem",ability:(
                ability_id:"emblem_01",presentation:Fallback,
                trigger:WheneverPlayerCastsSpell(caster:Controller,
                    filter:(any_of:Some([(card_type:Some(Creature)),(card_type:Some(Planeswalker))])),
                    ordinal:None,ordinal_scope:AllSpells),
                effect:[PutPoisonCountersOnTargetPlayer(count:2,target:(kind:OpponentPlayer))]))]
    )],
)"#;
const DUAL: &str = r#"(
    id:"synthetic_dual_type_spell",name:"Synthetic Dual Type Spell",face_id:"synthetic_dual_type_spell",
    mana_cost:"{0}",types:["Creature","Planeswalker"],power:2,toughness:2,loyalty:1,
)"#;
const OTHER: &str = r#"(
    id:"synthetic_emblem_other",name:"Synthetic Emblem Other",face_id:"synthetic_emblem_other",
    mana_cost:"{0}",types:["Artifact"],
)"#;
const FIXED_SPELL: &str = r#"(
    id:"synthetic_fixed_poison_spell",name:"Synthetic Fixed Poison Spell",face_id:"synthetic_fixed_poison_spell",
    mana_cost:"{0}",types:["Sorcery"],spell_effect:[
        PutPoisonCountersOnTargetPlayer(count:3,target:(kind:AnyPlayer)),GainLife(amount:7,who:EachPlayer)],
)"#;
const COPY_ANY: &str = r#"(
    id:"synthetic_emblem_copy_any",name:"Synthetic Emblem Copy Any",face_id:"synthetic_emblem_copy_any",
    mana_cost:"{0}",types:["Instant"],spell_effect:[CopyTargetSpell(count:1,spell_filter:(card_type:None))],
)"#;
const PREPARED_INSET: &str = r#"(
    id:"synthetic_emblem_prepared_inset",name:"Synthetic Emblem Preparation // Inset",layout:Preparation,
    faces:[
        (face_id:"synthetic_emblem_preparation",name:"Synthetic Emblem Preparation",mana_cost:"{0}",types:["Artifact"],
            static_abilities:[(ability_id:"static_01",presentation:Fallback,definition:EntersPrepared)]),
        (face_id:"synthetic_emblem_inset",name:"Synthetic Emblem Inset",mana_cost:"{0}",types:["Sorcery"],
            spell_effect:[GainLife(amount:1)]),
    ],
)"#;

fn fixture() -> GameEngine {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
            include_str!("../../../../tricerules-cards/data/vorinclex,_monstrous_raider.ron"),
            include_str!("../../../../tricerules-cards/data/laezel,_vlaakiths_champion.ron"),
            include_str!("../../../../tricerules-cards/data/counterspell.ron"),
            include_str!("../../../../tricerules-cards/data/tishanas_tidebinder.ron"),
            include_str!("../../../../tricerules-cards/data/pacifism.ron"),
            include_str!("../../../../tricerules-cards/data/generated/r/reclamation_sage.ron"),
            WALKER,
            DUAL,
            OTHER,
            FIXED_SPELL,
            COPY_ANY,
            PREPARED_INSET,
        ],
        &[],
    )
    .unwrap();
    let mut engine = GameEngine::new_with_registry(
        731_600,
        &[17, 29, 41],
        20,
        Some(
            (0..3)
                .map(|_| EngineDeck {
                    mainboard: vec!["forest".into(); 40],
                    commanders: vec![],
                })
                .collect(),
        ),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine
}

fn pass_round(engine: &mut GameEngine) -> rv1::RuledEventBatch {
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let mut batch = None;
    for _ in 0..count {
        let actor = engine.state.priority_player_id();
        batch = Some(
            engine
                .apply_command(
                    actor,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .unwrap(),
        );
    }
    batch.unwrap()
}

fn create_emblem(engine: &mut GameEngine) -> (ObjectId, ObjectId) {
    let source = place_ready(engine, 17, "Synthetic Poison Emblem Walker");
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: source,
                        expected_zone_change_generation: engine
                            .payment_object_ref(source)
                            .zone_change_generation,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Graveyard,
        "zero-loyalty source leaves before its ability resolves"
    );
    pass_round(engine);
    (
        source,
        engine.state.static_emblems.last().unwrap().object_id,
    )
}

fn cast_prepared(
    engine: &mut GameEngine,
    actor: PlayerId,
    name: &str,
) -> (ObjectId, rv1::RuledEventBatch) {
    cast_prepared_targets(engine, actor, name, vec![])
}

fn cast_prepared_targets(
    engine: &mut GameEngine,
    actor: PlayerId,
    name: &str,
    targets: Vec<rv1::TargetRef>,
) -> (ObjectId, rv1::RuledEventBatch) {
    engine
        .apply_dev_command(
            actor,
            &rv1::DevCommand {
                target_player_id: actor,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: name.into(),
                        zone: rv1::DevZone::Hand as i32,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    let object = engine.state.next_object_id - 1;
    let index = engine.state.player_idx(actor).unwrap();
    let hand_index = engine.state.players[index]
        .hand
        .iter()
        .position(|id| *id == object)
        .unwrap() as u32;
    engine.state.priority_idx = index;
    engine.state.active_player_idx = index;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.players[index].mana_pool.green = 10;
    engine.state.players[index].mana_pool.blue = 10;
    engine.state.players[index].mana_pool.colorless = 10;
    let batch = engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    targets,
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        expected_zone_change_generation: Some(
                            engine.payment_object_ref(object).zone_change_generation,
                        ),
                        location: Some(rv1::cast_source::Location::HandIndex(hand_index)),
                    }),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    (object, batch)
}

fn player_target(player: PlayerId) -> rv1::TargetRef {
    rv1::TargetRef {
        object_id: player as u32,
        kind: rv1::TargetRefKind::Player as i32,
        ..Default::default()
    }
}

fn choose_replacement(engine: &mut GameEngine, source: ObjectId) -> rv1::RuledEventBatch {
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let Some(PendingReplacementEvent::Counters(batch)) = &engine.state.pending_replacement_event
    else {
        panic!("counter replacement")
    };
    let application = batch
        .applications
        .iter()
        .find(|application| {
            matches!(&application.replacement,
        CounterPlacementReplacement::Battlefield{instance,..} if instance.source_object_id==source)
        })
        .unwrap()
        .choice_id;
    let actor = pending.deciding_player;
    let prompt_id = pending.resolution_prompt_id.get();
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![application],
                        resolution_prompt_id: prompt_id,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap()
}

fn parked_poison(spell: bool) -> (GameEngine, [ObjectId; 3]) {
    let mut engine = fixture();
    if !spell {
        create_emblem(&mut engine);
    }
    let sources =
        [17, 29, 41].map(|owner| place_ready(&mut engine, owner, "Vorinclex, Monstrous Raider"));
    // Replacement handles are opaque identities, independent of physical object IDs.
    engine.state.next_replacement_application_id = engine.state.next_object_id + 1000;
    if spell {
        cast_prepared_targets(
            &mut engine,
            17,
            "Synthetic Fixed Poison Spell",
            vec![player_target(41)],
        );
    } else {
        cast_prepared(&mut engine, 17, "Grizzly Bears");
        choose_opponent(&mut engine, 41);
    }
    pass_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Poison),
        None
    );
    (engine, sources)
}

#[test]
fn two_emblems_order_and_target_independently_for_one_committed_cast() {
    let mut engine = fixture();
    let (_, first) = create_emblem(&mut engine);
    let (_, second) = create_emblem(&mut engine);
    cast_prepared(&mut engine, 17, "Synthetic Dual Type Spell");
    let pending = engine.state.pending_trigger_order.as_ref().unwrap();
    assert_eq!(
        pending.candidates.len(),
        2,
        "dual type is one trigger per emblem"
    );
    let selected = pending
        .candidates
        .iter()
        .find(|trigger| trigger.source_permanent_id == first)
        .unwrap()
        .object_id;
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitTriggerOrder(
                    rv1::SubmitTriggerOrder {
                        trigger_object_id: selected,
                    },
                )),
            },
        )
        .unwrap();
    assert_eq!(engine.state.pending_triggers[0].source_permanent_id, first);
    choose_opponent(&mut engine, 29);
    assert_eq!(engine.state.pending_triggers[0].source_permanent_id, second);
    choose_opponent(&mut engine, 41);
    pass_round(&mut engine);
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Poison),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        None
    );
    pass_round(&mut engine);
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&2)
    );
}

#[test]
fn replacement_order_can_reduce_fixed_poison_to_zero_without_restarting_the_event() {
    for halve_first in [false, true] {
        let (mut engine, sources) = parked_poison(false);
        choose_replacement(
            &mut engine,
            if halve_first { sources[1] } else { sources[0] },
        );
        let Some(PendingReplacementEvent::Counters(batch)) =
            &engine.state.pending_replacement_event
        else {
            panic!("two unapplied replacements remain")
        };
        assert_eq!(batch.events[0].original_counters[&CounterKind::Poison], 2);
        assert_eq!(batch.events[0].applied_replacements.len(), 1);
        choose_replacement(
            &mut engine,
            if halve_first { sources[2] } else { sources[1] },
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
        assert_eq!(
            engine.state.players[2]
                .counters
                .get(&CounterKind::Poison)
                .copied()
                .unwrap_or(0),
            u32::from(!halve_first)
        );
    }
}

#[test]
fn fixed_spell_player_counter_tail_survives_actor_and_recipient_departure() {
    for departed in [17, 41] {
        let (mut engine, sources) = parked_poison(true);
        choose_replacement(&mut engine, sources[0]);
        concede(&mut engine, departed);
        if departed == 17 {
            assert!(
                !engine.state.objects.contains_key(&sources[0]),
                "accepted source departed"
            );
            choose_replacement(&mut engine, sources[1]);
            assert_eq!(
                engine.state.players[2].counters.get(&CounterKind::Poison),
                Some(&1)
            );
        }
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
        for player in &engine.state.players {
            if !player.has_lost {
                assert_eq!(player.life, 27, "tail runs once after departure");
            }
        }
    }
}

#[test]
fn fixed_poison_rejects_forged_scope_and_accepted_history_atomically() {
    let (mut engine, sources) = parked_poison(false);
    choose_replacement(&mut engine, sources[0]);
    let ResolutionContinuation::CounterReplacement {
        stack: Some(frame),
        completion,
    } = &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .continuation
    else {
        unreachable!()
    };
    let frame = frame.clone();
    let completion = completion.clone();
    let Some(PendingReplacementEvent::Counters(batch)) = &engine.state.pending_replacement_event
    else {
        unreachable!()
    };
    let proposal = batch.events[0].clone();
    let before = engine.diagnostic_snapshot().unwrap();
    let mut output = vec![];
    assert!(engine
        .commit_counter_placement_batch(
            std::slice::from_ref(&proposal),
            &completion,
            None,
            &mut output
        )
        .is_err());
    assert!(engine
        .commit_counter_placement_batch(&[], &completion, Some(&frame), &mut output)
        .is_err());
    assert!(engine
        .commit_counter_placement_batch(
            &[proposal.clone(), proposal.clone()],
            &completion,
            Some(&frame),
            &mut output
        )
        .is_err());
    for mutation in 0..10 {
        let mut event = proposal.clone();
        let mut changed = frame.clone();
        match mutation {
            0 => event.actor = 29,
            1 => {
                event.original_counters.insert(CounterKind::Poison, 3);
            }
            2 => {
                event.counters.insert(CounterKind::Poison, 5);
            }
            3 => event.recipient = CounterPlacementRecipient::Player { player_id: 29 },
            4 => event.cost_debit_index = Some(0),
            5 => event.effect_created_counters = false,
            6 => event
                .applied_replacements
                .push(event.applied_replacements[0].clone()),
            7 => changed.item.targets[0].counter_amount = 1,
            8 => changed.item.targets[0].zone_change_generation = Some(0),
            9 => changed.set_flat_tail(Some(2), Default::default()),
            _ => unreachable!(),
        }
        assert!(
            engine
                .commit_counter_placement_batch(&[event], &completion, Some(&changed), &mut output)
                .is_err(),
            "mutation{mutation}"
        );
        assert!(output.is_empty());
        assert_eq!(
            engine.diagnostic_snapshot().unwrap(),
            before,
            "mutation{mutation}"
        );
    }
}

#[test]
fn copying_a_matching_permanent_spell_without_casting_does_not_trigger_the_emblem() {
    let mut engine = fixture();
    create_emblem(&mut engine);
    let (original, _) = cast_prepared(&mut engine, 17, "Grizzly Bears");
    choose_opponent(&mut engine, 29);
    pass_round(&mut engine);
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 2);
    cast_prepared_targets(
        &mut engine,
        17,
        "Synthetic Emblem Copy Any",
        vec![rv1::TargetRef {
            object_id: original,
            kind: rv1::TargetRefKind::Stack as i32,
            ..Default::default()
        }],
    );
    pass_round(&mut engine);
    let copy = engine.state.stack.last().unwrap();
    assert!(copy.is_copy);
    assert!(copy.cast_occurrence.is_none());
    assert_ne!(copy.id, original);
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_trigger_order.is_none());
    assert_eq!(engine.state.turn_history.current.spells_cast, 2);
    pass_round(&mut engine);
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 2);
}

#[test]
fn countering_the_original_cast_spell_does_not_remove_its_captured_poison_trigger() {
    let mut engine = fixture();
    create_emblem(&mut engine);
    let (original, _) = cast_prepared(&mut engine, 17, "Grizzly Bears");
    choose_opponent(&mut engine, 29);
    let poison_trigger = engine.state.stack.last().unwrap().id;
    cast_prepared_targets(
        &mut engine,
        29,
        "Counterspell",
        vec![rv1::TargetRef {
            object_id: original,
            kind: rv1::TargetRefKind::Stack as i32,
            ..Default::default()
        }],
    );
    pass_round(&mut engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == original));
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == poison_trigger));
    pass_round(&mut engine);
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 2);
}

#[test]
fn the_captured_poison_trigger_can_be_countered_without_removing_the_emblem() {
    let mut engine = fixture();
    let (_, marker) = create_emblem(&mut engine);
    cast_prepared(&mut engine, 17, "Grizzly Bears");
    choose_opponent(&mut engine, 29);
    let poison_trigger = engine.state.stack.last().unwrap().id;
    cast_prepared(&mut engine, 29, "Tishana's Tidebinder");
    pass_round(&mut engine);
    engine
        .apply_command(
            29,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ChooseTriggerTarget(
                    rv1::ChooseTriggerTarget {
                        targets: vec![rv1::TargetRef {
                            object_id: poison_trigger,
                            kind: rv1::TargetRefKind::Stack as i32,
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    pass_round(&mut engine);
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == poison_trigger));
    assert!(engine
        .state
        .static_emblems
        .iter()
        .any(|emblem| emblem.object_id == marker));
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        None
    );
}

#[test]
fn nested_observer_return_preserves_begun_poison_after_controller_departure() {
    let (mut engine, sources) = parked_poison(false);
    choose_replacement(&mut engine, sources[0]);
    let parent_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    let Some(PendingReplacementEvent::Counters(batch)) = &engine.state.pending_replacement_event
    else {
        panic!("saved poison replacement batch")
    };
    let application_ids = batch
        .applications
        .iter()
        .map(|application| application.choice_id)
        .collect::<Vec<_>>();
    assert!(application_ids
        .iter()
        .all(|id| !engine.state.objects.contains_key(id)));
    let bearer = place_ready(&mut engine, 29, "Grizzly Bears");
    let aura = engine.state.players[1].hand[0];
    engine.state.objects.get_mut(&aura).unwrap().card_id = "pacifism".into();
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        aura,
        Zone::Exile,
        None,
    )
    .unwrap();
    engine.state.pending_immediate_observer_actions.push(
        crate::state::ImmediateObserverAction::ReturnExiledObject {
            exiled: crate::state::TriggerObjectRef {
                object_id: aura,
                zone_change_generation: engine.state.zone_change_generation[&aura],
                controller_at_event: 29,
            },
        },
    );
    let mut events = vec![];
    assert!(engine
        .drain_immediate_observer_actions(None, &mut events)
        .unwrap());
    engine.publish_resolution_prompt_ids(&mut events).unwrap();
    assert!(engine.state.pending_observer_return_batch.is_some());
    concede(&mut engine, 17);
    let saved = engine.state.pending_observer_return_batch.as_ref().unwrap();
    assert!(
        engine.begun_targeted_poison_placement(
            saved.suspended_resolution.as_ref().unwrap(),
            saved.suspended_replacement_event.as_ref()
        ),
        "the saved transaction remains authenticated after its controller leaves"
    );
    let prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    let returned = engine
        .apply_command(
            29,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![bearer],
                        resolution_prompt_id: prompt,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    assert!(engine.state.pending_observer_return_batch.is_none());
    let refreshed = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    assert_ne!(refreshed, parent_prompt);
    assert_ne!(refreshed, prompt);
    assert!(returned.events.iter().any(|event| matches!(
        &event.ev,
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.resolution_prompt_id == refreshed && choice.candidate_object_ids == application_ids
    )));
    for (actor, old_prompt, chosen) in [
        (41, parent_prompt, application_ids[0]),
        (29, prompt, bearer),
    ] {
        let before = ron::to_string(&engine.state).unwrap();
        assert!(engine
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![chosen],
                            resolution_prompt_id: old_prompt,
                            ..Default::default()
                        }
                    ))
                }
            )
            .is_err());
        assert_eq!(ron::to_string(&engine.state).unwrap(), before);
    }
    let Some(PendingReplacementEvent::Counters(batch)) = &engine.state.pending_replacement_event
    else {
        panic!("saved poison order restored")
    };
    assert_eq!(batch.events[0].original_counters[&CounterKind::Poison], 2);
    assert_eq!(batch.events[0].counters[&CounterKind::Poison], 4);
    assert_eq!(batch.events[0].applied_replacements.len(), 1);
    choose_replacement(&mut engine, sources[1]);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Poison),
        Some(&1)
    );
}

fn choose_opponent(engine: &mut GameEngine, opponent: PlayerId) -> rv1::RuledEventBatch {
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ChooseTriggerTarget(
                    rv1::ChooseTriggerTarget {
                        targets: vec![rv1::TargetRef {
                            object_id: opponent as u32,
                            kind: rv1::TargetRefKind::Player as i32,
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap()
}

#[test]
fn waiting_emblem_offer_refreshes_after_unrelated_concession_and_retires_on_controller_loss() {
    for departing in [41, 17] {
        let mut engine = fixture();
        let (_, marker) = create_emblem(&mut engine);
        cast_prepared(&mut engine, 17, "Grizzly Bears");
        let reserved = engine.state.pending_triggers.front().unwrap().object_id;
        if departing == 17 {
            engine.state.priority_idx = 1; // The surviving holder still needs a visible handoff.
        }
        let response = engine
            .apply_command(
                departing,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .unwrap();
        assert!(!engine.state.is_terminal());
        let offers = response
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::TriggerNeedsTarget(offer)) => Some(offer),
                _ => None,
            })
            .collect::<Vec<_>>();
        if departing == 41 {
            assert_eq!(
                offers.len(),
                1,
                "each authoritative batch republishes the final current offer"
            );
            assert_eq!(offers[0].source_permanent_id, marker);
            assert_eq!(
                engine.state.pending_triggers.front().unwrap().object_id,
                reserved
            );
            assert!(offers[0].targets.as_ref().unwrap().groups[0].can_target_opponent);
            assert!(!response
                .events
                .iter()
                .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::PriorityChanged(_)))));
            choose_opponent(&mut engine, 29);
            pass_round(&mut engine);
            assert_eq!(
                engine.state.players[1].counters.get(&CounterKind::Poison),
                Some(&2)
            );
        } else {
            assert!(offers.is_empty());
            assert!(engine.state.pending_triggers.is_empty());
            assert!(response.events.iter().any(|event| matches!(event.ev,
                Some(rv1::ruled_event::Ev::PriorityChanged(ref priority)) if priority.player_id == 29
            )));
        }
    }
}

#[test]
fn terminal_publication_removes_an_earlier_same_batch_trigger_offer() {
    let mut engine = fixture();
    create_emblem(&mut engine);
    let (_, batch) = cast_prepared(&mut engine, 17, "Grizzly Bears");
    assert!(!engine.state.pending_triggers.is_empty());
    // Exercise the terminal normalizer with the genuine offer emitted earlier in this batch.
    engine.state.outcome = Some(crate::state::GameOutcome::Winner(17));
    let terminal = engine.finish_terminal_batch(batch);
    assert!(engine.state.pending_triggers.is_empty());
    assert!(!terminal
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::TriggerNeedsTarget(_)))));
}

#[test]
fn departure_retires_an_impossible_captured_target_and_final_opponent_loss_is_terminal() {
    let mut engine = fixture();
    place_ready(&mut engine, 29, "Synthetic Emblem Other");
    cast_prepared(&mut engine, 17, "Reclamation Sage");
    pass_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let response = engine
        .apply_command(
            29,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    assert!(!engine.state.is_terminal());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
    assert!(!response
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::TriggerNeedsTarget(_)))));
    assert!(response
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::PriorityChanged(_)))));

    let mut engine = fixture();
    create_emblem(&mut engine);
    cast_prepared(&mut engine, 17, "Grizzly Bears");
    concede(&mut engine, 41);
    let before = ron::to_string(&engine.state).unwrap();
    assert!(engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ChooseTriggerTarget(
                    rv1::ChooseTriggerTarget {
                        targets: vec![player_target(41)],
                        ..Default::default()
                    }
                ))
            }
        )
        .is_err());
    assert_eq!(ron::to_string(&engine.state).unwrap(), before);
    let terminal = engine
        .apply_command(
            29,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    assert!(engine.state.is_terminal());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(!terminal
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::TriggerNeedsTarget(_)))));
}

#[test]
fn retiring_an_impossible_front_publishes_the_next_staged_offer_exactly_once() {
    let mut engine = fixture();
    create_emblem(&mut engine);
    place_ready(&mut engine, 29, "Synthetic Emblem Other");
    cast_prepared(&mut engine, 17, "Reclamation Sage");
    let captured = engine.state.pending_triggers.front().unwrap().clone();
    choose_opponent(&mut engine, 41);
    pass_round(&mut engine);
    pass_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1); // Sage awaits its sole artifact target.
    let reserved = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    // Prepared placement graph: reuse a genuinely captured emblem definition as the remaining
    // ordered candidate. This isolates retirement/flush publication, not another cast producer.
    engine.state.pending_trigger_order = Some(crate::state::PendingTriggerOrder {
        deciding_player: 17,
        prompt_emitted: true,
        candidates: vec![crate::state::StagedTrigger {
            object_id: reserved,
            source_permanent_id: captured.source_permanent_id,
            source_owner: captured.source_owner,
            source_face_index: captured.source_face_index,
            source_zone_change: captured.source_zone_change,
            source_face_change: captured.source_face_change,
            card_id: captured.card_id,
            card_name: captured.source_label,
            controller: captured.controller,
            ability_index: captured.ability_index,
            ability_origin: captured.ability_origin,
            ability: captured.ability,
            ability_text: captured.ability_text,
            presentation: captured.presentation,
            trigger_context: captured.trigger_context,
            may: captured.may,
        }],
    });
    let response = engine
        .apply_command(
            29,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    assert!(!engine.state.is_terminal());
    assert!(engine.state.pending_trigger_order.is_none());
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert_eq!(
        engine.state.pending_triggers.front().unwrap().object_id,
        reserved
    );
    assert_eq!(
        response
            .events
            .iter()
            .filter(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::TriggerNeedsTarget(_))))
            .count(),
        1
    );
    choose_opponent(&mut engine, 41);
    assert_eq!(engine.state.stack.last().unwrap().id, reserved);
}

#[test]
fn actual_preparation_cast_copy_has_a_cast_occurrence_but_does_not_match_the_emblem() {
    let mut engine = fixture();
    create_emblem(&mut engine);
    let (base, _) = cast_prepared(&mut engine, 17, "Synthetic Emblem Preparation // Inset");
    assert!(engine.state.pending_triggers.is_empty());
    pass_round(&mut engine);
    let copy = engine.state.prepared_permanents[&base]
        .active_exile_copy
        .unwrap()
        .object_id;
    let permission = engine
        .state
        .active_exile_play_permissions
        .iter()
        .find(|permission| permission.object_id == copy)
        .unwrap()
        .group_id;
    let response = engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    face_index: 1,
                    casting_permission_id: Some(permission),
                    source: Some(rv1::CastSource {
                        expected_zone_change_generation: Some(
                            engine.state.zone_change_generation[&copy],
                        ),
                        location: Some(rv1::cast_source::Location::ExileObjectId(copy)),
                    }),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    let item = engine.state.stack.last().unwrap();
    assert_eq!(item.id, copy);
    assert!(item.is_copy);
    assert!(item.cast_occurrence.is_some());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(!response
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::TriggerNeedsTarget(_)))));
    let life = engine.state.players[0].life;
    pass_round(&mut engine);
    assert_eq!(engine.state.players[0].life, life + 1);
}

#[test]
fn captured_emblem_targets_one_opponent_and_places_two_poison_after_source_left() {
    let mut engine = fixture();
    let (source, marker) = create_emblem(&mut engine);
    let (spell, cast) = cast_prepared(&mut engine, 17, "Grizzly Bears");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert_eq!(engine.state.pending_triggers[0].source_permanent_id, marker);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    let offer = cast
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::TriggerNeedsTarget(offer)) => Some(offer),
            _ => None,
        })
        .unwrap();
    assert_eq!(offer.controller_player_id, 17);
    assert!(offer.targets.as_ref().unwrap().groups[0].can_target_opponent);
    assert!(!offer.targets.as_ref().unwrap().groups[0].can_target_self);
    let targeted = choose_opponent(&mut engine, 29);
    let pushed = targeted
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::StackPushed(push)) => Some(push),
            _ => None,
        })
        .unwrap();
    assert!(
        pushed.ability_source.is_none(),
        "an emblem is not a battlefield source"
    );
    assert_eq!(pushed.targets[0].kind, rv1::TargetRefKind::Player as i32);
    assert_eq!(pushed.targets[0].counter_amount, 0);
    pass_round(&mut engine);
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Poison),
        None
    );
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Poison),
        None
    );
    assert_eq!(engine.state.stack.last().unwrap().id, spell);
    assert!(engine
        .state
        .static_emblems
        .iter()
        .any(|emblem| emblem.object_id == marker));
}

#[test]
fn emblem_cast_filter_matches_its_controller_once_for_either_or_both_types() {
    for (actor, name, expected) in [
        (17, "Grizzly Bears", true),
        (17, "Synthetic Poison Emblem Walker", true),
        (17, "Synthetic Dual Type Spell", true),
        (17, "Synthetic Emblem Other", false),
        (29, "Grizzly Bears", false),
    ] {
        let mut engine = fixture();
        create_emblem(&mut engine);
        let (_, batch) = cast_prepared(&mut engine, actor, name);
        let offers = batch
            .events
            .iter()
            .filter(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::TriggerNeedsTarget(_))))
            .count();
        assert_eq!(offers, usize::from(expected), "{actor} casts {name}");
        assert_eq!(engine.state.pending_triggers.len(), usize::from(expected));
        if expected {
            choose_opponent(&mut engine, 29);
            pass_round(&mut engine);
            assert_eq!(
                engine.state.players[1].counters.get(&CounterKind::Poison),
                Some(&2)
            );
        }
    }
}

#[test]
fn malformed_poison_target_choices_reject_without_consuming_the_captured_trigger() {
    for mutation in 0..9 {
        let mut engine = fixture();
        create_emblem(&mut engine);
        cast_prepared(&mut engine, 17, "Grizzly Bears");
        let mut actor = 17;
        let mut target = rv1::TargetRef {
            object_id: 29,
            kind: rv1::TargetRefKind::Player as i32,
            ..Default::default()
        };
        let mut choice = rv1::ChooseTriggerTarget::default();
        match mutation {
            0 => actor = 29,
            1 => target.object_id = 17,
            2 => target.kind = rv1::TargetRefKind::Permanent as i32,
            3 => target.damage_amount = 1,
            4 => target.counter_amount = 1,
            5 => target.expected_zone_change_generation = Some(0),
            6 => target.group_index = 1,
            7 => choice.decline = true,
            8 => {}
            _ => unreachable!(),
        }
        choice.targets = if mutation == 8 { vec![] } else { vec![target] };
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(
            engine
                .apply_command(
                    actor,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::ChooseTriggerTarget(choice))
                    }
                )
                .is_err(),
            "mutation{mutation}"
        );
        assert_eq!(
            engine.diagnostic_snapshot().unwrap(),
            before,
            "mutation{mutation}"
        );
    }
}

#[test]
fn fixed_poison_hits_ten_only_after_the_trigger_resolves() {
    let mut engine = fixture();
    create_emblem(&mut engine);
    engine.state.players[1]
        .counters
        .insert(CounterKind::Poison, 8);
    cast_prepared(&mut engine, 17, "Grizzly Bears");
    choose_opponent(&mut engine, 29);
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 8);
    assert!(!engine.state.players[1].has_lost);
    pass_round(&mut engine);
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 10);
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.is_terminal());
}

#[test]
fn poison_offer_does_not_publish_physical_targets_when_a_player_id_is_also_an_object_id() {
    let mut engine = fixture();
    let (source, _) = create_emblem(&mut engine);
    let mut creature = engine.state.objects[&source].clone();
    creature.id = 29;
    creature.card_id = "grizzly_bears".into();
    creature.zone = Zone::Battlefield;
    for player in &mut engine.state.players {
        player.hand.retain(|id| *id != 29);
        player.library.retain(|id| *id != 29);
        player.battlefield.retain(|id| *id != 29);
    }
    engine.state.objects.insert(29, creature);
    engine.state.zone_change_generation.insert(29, 0);
    engine.state.players[0].battlefield.push(29);
    let (_, batch) = cast_prepared(&mut engine, 17, "Grizzly Bears");
    let offer = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::TriggerNeedsTarget(offer)) => Some(offer),
            _ => None,
        })
        .unwrap();
    let group = &offer.targets.as_ref().unwrap().groups[0];
    assert!(
        group.valid_permanent_ids.is_empty(),
        "player-only poison cannot offer the physical creature sharing PlayerId29"
    );
    assert!(group.valid_stack_ids.is_empty());
    assert!(group.valid_graveyard_ids.is_empty());
    assert!(group.can_target_opponent);
    choose_opponent(&mut engine, 29);
    pass_round(&mut engine);
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 2);
    assert_eq!(
        engine.state.objects[&29].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}
