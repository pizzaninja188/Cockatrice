//! Issue #196: characterize the three destruction callers before sharing execution.
use super::*;
use crate::state::ActiveDeathReplacement;
use tricerules_cards::primitives::{CastTriggerPlayer, ControllerReference};
use tricerules_proto::ruled::v1::ruled_event::Ev;

#[derive(Clone, Copy, Debug)]
enum Caller {
    Chosen,
    Source,
    Mass,
    Attached,
}

const CALLERS: [Caller; 4] = [
    Caller::Chosen,
    Caller::Source,
    Caller::Mass,
    Caller::Attached,
];

fn setup() -> GameEngine {
    let deck = ["grizzly_bears", "ornithopter", "forest", "short_sword"]
        .into_iter()
        .cycle()
        .take(24)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut engine = GameEngine::new(196_001, &[3, 11], 20, Some(vec![deck.clone(), deck]), true)
        .expect("engine");
    // Test the resolver's player-set contract below the two-seat session-admission boundary.
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
    let effect = ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: 1,
    };
    if matches!(effect.kind, ContinuousEffectKind::GrantTriggeredAbility(_)) {
        engine.state.add_triggered_ability_grant(effect);
    } else {
        engine.state.continuous_effects.push(effect);
    }
}

fn grant_trigger(engine: &mut GameEngine, oid: ObjectId, trigger: TriggerCondition) {
    let mut ability = engine
        .registry
        .get("ajanis_pridemate")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = trigger;
    modify(
        engine,
        oid,
        ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
    );
}

fn subject(engine: &mut GameEngine, caller: Caller, seat: usize) -> ObjectId {
    deploy(
        engine,
        seat,
        if matches!(caller, Caller::Attached) {
            "short_sword"
        } else {
            "grizzly_bears"
        },
    )
}

fn resolve(
    engine: &mut GameEngine,
    caller: Caller,
    subjects: &[ObjectId],
    prevent_regeneration: bool,
) -> (Vec<rv1::RuledEvent>, EffectResult) {
    let mut targets = subjects.to_vec();
    if matches!(caller, Caller::Attached) {
        let recipient = deploy(engine, 0, "forest");
        for oid in subjects {
            engine.state.objects.get_mut(oid).unwrap().attached_to =
                Some(AttachmentRecipient::Object(recipient));
        }
        targets = vec![recipient];
    }
    let top = StackItem {
        id: u32::MAX,
        controller: 3,
        card_id: "murder".into(),
        targets: vec![],
        ability_text: None,
        source_permanent_id: subjects.first().copied(),
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
        targets: &targets,
        targets_by_role: &[],
        target_damage: &[],
        target_group_indices: &[],
        top: &top,
        controller: 3,
        affected_player: 3,
        spell_label: "Destruction fixture",
        previous_effect_result: &EffectResult::default(),
        effect_result: &mut result,
        effect_index: 0,
    };
    let outcome = match caller {
        Caller::Chosen | Caller::Source => misc::destroy(
            &mut cx,
            SpellEffectKind::Destroy {
                subject: if matches!(caller, Caller::Source) {
                    EffectSubject::Source
                } else {
                    EffectSubject::Chosen(Box::new(TargetFilter::default_creature()))
                },
            },
        ),
        Caller::Mass => mass::destroy_all(
            &mut cx,
            SpellEffectKind::DestroyAll {
                kind: TargetFilter::default_creature(),
                prevent_regeneration,
            },
        ),
        Caller::Attached => mass::destroy_attached(
            &mut cx,
            SpellEffectKind::DestroyAttached {
                target: TargetFilter::default_creature(),
                attachments: AttachmentFilter {
                    kinds: vec![AttachmentKind::Equipment],
                },
            },
        ),
    };
    assert_eq!(outcome.expect("resolution"), EffectOutcome::Continue);
    (events, result)
}

fn moves(events: &[rv1::RuledEvent]) -> Vec<&rv1::PermanentMoved> {
    events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::PermanentMoved(moved)) => Some(moved),
            _ => None,
        })
        .collect()
}

#[test]
fn issue_196_destroy_receipts_and_public_moves_use_the_actual_destination_and_owner() {
    for caller in CALLERS {
        for exile in [false, true] {
            let mut engine = setup();
            let oid = subject(&mut engine, caller, 2);
            modify(
                &mut engine,
                oid,
                ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::Fixed(11),
                },
            );
            if exile {
                engine
                    .state
                    .death_replacement_effects
                    .push(ActiveDeathReplacement {
                        object_id: oid,
                        zone_change_generation: 0,
                    });
            }
            let (events, result) = resolve(&mut engine, caller, &[oid], false);
            assert_eq!(events.len(), 2, "{caller:?}: one log then one move");
            assert!(
                matches!(&events[0].ev, Some(Ev::Log(log)) if log.text.starts_with("Destruction fixture destroys "))
            );
            let moved = moves(&events);
            assert_eq!(moved.len(), 1, "{caller:?}");
            assert_eq!(
                (
                    moved[0].object_id,
                    moved[0].owner_player_id,
                    moved[0].controller_player_id
                ),
                (oid, 299, 299)
            );
            assert_eq!(
                moved[0].destination,
                if exile {
                    rv1::permanent_moved::Destination::Exile
                } else {
                    rv1::permanent_moved::Destination::Graveyard
                } as i32
            );
            assert_eq!(engine.state.zone_change_generation[&oid], 1);
            assert_eq!(
                engine.state.turn_history.current.creatures_died,
                u32::from(!exile && !matches!(caller, Caller::Attached))
            );
            if matches!(caller, Caller::Chosen | Caller::Source) {
                assert_eq!(result.cards.len(), 1);
                let receipt = &result.cards[0];
                assert_eq!(receipt.action, CardResultAction::Destroy);
                assert_eq!(
                    (
                        receipt.object_id,
                        receipt.affected_player,
                        receipt.zone_change_generation
                    ),
                    (oid, 299, 1)
                );
            } else {
                assert!(
                    result.cards.is_empty(),
                    "mass/attachment effects do not publish receipts"
                );
            }
        }
    }
}

#[test]
fn issue_196_survivors_keep_identity_and_only_regeneration_consumes_a_shield() {
    for caller in CALLERS {
        for indestructible in [false, true] {
            let mut engine = setup();
            let oid = subject(&mut engine, caller, 1);
            let object = engine.state.objects.get_mut(&oid).unwrap();
            object.regeneration_shields = 2;
            object.damage = 1;
            object.deathtouch_damage = true;
            grant_trigger(
                &mut engine,
                oid,
                TriggerCondition::WheneverSelfBecomesTapped,
            );
            if indestructible {
                modify(
                    &mut engine,
                    oid,
                    ContinuousEffectKind::Layer6AddKeyword(Keyword::Indestructible),
                );
            }
            let name = object_display_name(&engine.state, engine.registry, oid);
            let (events, result) = resolve(&mut engine, caller, &[oid], false);
            let expected_log = if !indestructible {
                format!("{name} regenerates.")
            } else if matches!(caller, Caller::Chosen | Caller::Source) {
                format!("Destruction fixture has no effect: {name} is indestructible.")
            } else {
                format!("{name} is indestructible and survives Destruction fixture.")
            };
            assert_eq!(events.len(), 1);
            assert!(matches!(&events[0].ev, Some(Ev::Log(log)) if log.text == expected_log));
            assert!(moves(&events).is_empty());
            assert!(result.cards.is_empty());
            let object = &engine.state.objects[&oid];
            assert_eq!(object.zone, Zone::Battlefield);
            assert_eq!(
                object.regeneration_shields,
                if indestructible { 2 } else { 1 }
            );
            assert_eq!(object.tapped, !indestructible);
            assert_eq!(object.damage, u32::from(indestructible));
            assert_eq!(object.deathtouch_damage, indestructible);
            assert_eq!(
                engine
                    .state
                    .zone_change_generation
                    .get(&oid)
                    .copied()
                    .unwrap_or(0),
                0
            );
            assert_eq!(
                engine.state.staged_trigger_groups.len(),
                usize::from(!indestructible)
            );
            if !indestructible {
                assert_eq!(engine.state.staged_trigger_groups[0].triggers.len(), 1);
                engine.state.staged_trigger_groups.clear();
                resolve(&mut engine, caller, &[oid], false);
                assert_eq!(engine.state.objects[&oid].regeneration_shields, 0);
                assert!(
                    engine.state.staged_trigger_groups.is_empty(),
                    "already tapped regeneration has no new tap trigger"
                );
            }
        }
    }
}

#[test]
fn issue_196_regeneration_removes_combat_participants_before_logging() {
    for caller in [Caller::Chosen, Caller::Source, Caller::Mass] {
        for blocking in [false, true] {
            let mut engine = setup();
            let oid = subject(&mut engine, caller, 1);
            let other = deploy(&mut engine, 0, "ornithopter");
            let (attacker, blocker) = if blocking { (other, oid) } else { (oid, other) };
            // Mass destruction must leave the other combatant alone so we can inspect exactly
            // which combat relation regeneration removed.
            modify(
                &mut engine,
                other,
                ContinuousEffectKind::Layer6AddKeyword(Keyword::Indestructible),
            );
            engine
                .state
                .objects
                .get_mut(&oid)
                .unwrap()
                .regeneration_shields = 1;
            engine.state.combat = Some(CombatState {
                attacking: vec![attacker],
                attack_assignments: HashMap::new(),
                blockers: HashMap::from([(attacker, vec![blocker])]),
                damage_assignments: HashMap::new(),
                trample_player_damage: HashMap::new(),
                damage_assignment_needed: false,
                attackers_declared: true,
                blockers_declared: true,
                assign_combat_damage_phase: false,
                first_strike_attackers: vec![],
                first_strike_blockers: HashMap::new(),
                first_strike_damage_done: false,
            });
            let (events, result) = resolve(&mut engine, caller, &[oid], false);
            assert!(result.cards.is_empty());
            let removal = events.iter().position(|event| matches!(&event.ev, Some(Ev::RemovedFromCombat(removed)) if removed.object_ids == vec![oid])).expect("removal event");
            assert!(
                matches!(&events[removal + 1].ev, Some(Ev::Log(log)) if log.text == "Grizzly Bears regenerates.")
            );
            let combat = engine.state.combat.as_ref().unwrap();
            assert!(!combat.attacking.contains(&oid));
            assert!(!combat.blockers.contains_key(&oid));
            assert!(combat
                .blockers
                .values()
                .all(|blockers| !blockers.contains(&oid)));
            assert_eq!(combat.attacking.contains(&other), blocking);
        }
    }
}

#[test]
fn issue_196_mass_cant_regenerate_does_not_tap_or_trigger_shielded_permanents() {
    let mut engine = setup();
    let oid = subject(&mut engine, Caller::Mass, 1);
    engine
        .state
        .objects
        .get_mut(&oid)
        .unwrap()
        .regeneration_shields = 2;
    grant_trigger(
        &mut engine,
        oid,
        TriggerCondition::WheneverSelfBecomesTapped,
    );
    let (events, result) = resolve(&mut engine, Caller::Mass, &[oid], true);
    assert_eq!(moves(&events).len(), 1);
    assert!(result.cards.is_empty());
    assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
    assert!(engine.state.staged_trigger_groups.is_empty());
}

#[test]
fn issue_196_cohorts_preserve_move_order_and_departing_observers_in_one_trigger_group() {
    for caller in [Caller::Chosen, Caller::Mass, Caller::Attached] {
        let mut engine = setup();
        let dies = subject(&mut engine, caller, 0);
        let regenerates = subject(&mut engine, caller, 1);
        let survives = subject(&mut engine, caller, 2);
        let exiles = subject(&mut engine, caller, 1);
        let token = subject(&mut engine, caller, 0);
        engine.state.objects.get_mut(&token).unwrap().token_origin =
            engine.copiable_values_for(token);
        engine
            .state
            .objects
            .get_mut(&regenerates)
            .unwrap()
            .regeneration_shields = 1;
        modify(
            &mut engine,
            survives,
            ContinuousEffectKind::Layer6AddKeyword(Keyword::Indestructible),
        );
        engine
            .state
            .death_replacement_effects
            .push(ActiveDeathReplacement {
                object_id: exiles,
                zone_change_generation: 0,
            });
        grant_trigger(
            &mut engine,
            regenerates,
            TriggerCondition::WheneverSelfBecomesTapped,
        );
        grant_trigger(&mut engine, dies, TriggerCondition::WhenSelfDies);
        if !matches!(caller, Caller::Attached) {
            grant_trigger(
                &mut engine,
                dies,
                TriggerCondition::WheneverCreatureDies {
                    controller: CastTriggerPlayer::AnyPlayer,
                    filter: Default::default(),
                },
            );
        }
        let subjects = [dies, regenerates, survives, exiles, token];
        let mut expected_order = match caller {
            Caller::Mass => {
                battlefield_objects_matching(&engine, &TargetFilter::default_creature())
            }
            _ => subjects.to_vec(),
        };
        if matches!(caller, Caller::Attached) {
            expected_order.sort_unstable();
        }
        expected_order.retain(|oid| [dies, exiles, token].contains(oid));
        let (events, result) = resolve(&mut engine, caller, &subjects, false);
        assert_eq!(
            moves(&events)
                .iter()
                .map(|event| event.object_id)
                .collect::<Vec<_>>(),
            expected_order
        );
        assert_eq!(
            engine.state.staged_trigger_groups.len(),
            1,
            "{caller:?}: taps and deaths share one group"
        );
        assert_eq!(
            engine.state.staged_trigger_groups[0].triggers.len(),
            if matches!(caller, Caller::Attached) {
                2
            } else {
                4
            },
            "{caller:?}: departing source sees itself and the token die, never the exiled object"
        );
        assert_eq!(engine.state.objects[&survives].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&regenerates].zone, Zone::Battlefield);
        assert_eq!(
            result.cards.len(),
            if matches!(caller, Caller::Chosen) {
                3
            } else {
                0
            }
        );
    }
}

#[test]
fn issue_196_departed_or_new_generation_source_is_not_destroyed() {
    for zone in [Zone::Hand, Zone::Battlefield] {
        let mut engine = setup();
        let oid = subject(&mut engine, Caller::Source, 1);
        move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Hand, None).unwrap();
        if zone == Zone::Battlefield {
            move_object_to_zone(&mut engine.state, engine.registry, oid, zone, None).unwrap();
        }
        let generation = engine.state.zone_change_generation[&oid];
        let (events, result) = resolve(&mut engine, Caller::Source, &[oid], false);
        assert!(events.is_empty());
        assert!(result.cards.is_empty());
        assert_eq!(engine.state.objects[&oid].zone, zone);
        assert_eq!(engine.state.zone_change_generation[&oid], generation);
    }
}
