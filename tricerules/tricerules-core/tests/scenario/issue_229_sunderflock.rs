use super::helpers::*;
use tricerules_core::state::Zone;

fn cast_sunderflock(engine: &mut GameEngine) -> u32 {
    let slot = hand_index_for_card(engine, 0, "sunderflock");
    let source = engine.state.players[0].hand[slot];
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    source
}

fn resolve_top(engine: &mut GameEngine) -> RuledEventBatch {
    let count = engine.state.players.len();
    let mut batch = None;
    for _ in 0..count {
        batch = Some(
            engine
                .apply_command(engine.state.priority_player_id(), &pass())
                .unwrap(),
        );
    }
    batch.unwrap()
}

fn setup() -> GameEngine {
    let mut engine = GameEngine::new(
        229_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["sunderflock"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Sunderflock is registered for scenario coverage");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "sunderflock");
    engine.state.players[0].mana_pool.blue = 9;
    engine
}

#[test]
fn issue_229_sunderflock_cast_returns_non_elementals_to_owners() {
    let mut engine = setup();
    let ours = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let borrowed = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    engine.state.objects.get_mut(&borrowed).unwrap().owner = 1;
    let elemental = inject_creature_on_battlefield(&mut engine, 1, "air_elemental");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "island");
    let slot = hand_index_for_card(&engine, 0, "sunderflock");
    let source = engine.state.players[0].hand[slot];
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.stack.len(), 1, "cast-conditioned ETB");
    assert!(engine.state.stack[0].targets.is_empty());
    assert_eq!(engine.state.objects[&ours].zone, Zone::Battlefield);
    pass_both_players(&mut engine);
    for (oid, owner) in [(ours, 0), (theirs, 1), (borrowed, 1)] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Hand);
        assert!(engine.state.players[owner].hand.contains(&oid));
    }
    for oid in [source, elemental, land] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
    }
}

#[test]
fn issue_229_sunderflock_reduction_cannot_pay_colored_mana() {
    let mut engine = setup();
    inject_creature_on_battlefield(&mut engine, 0, "air_elemental");
    let slot = hand_index_for_card(&engine, 0, "sunderflock");
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 10;
    let hand = engine.state.players[0].hand.clone();
    assert!(engine.apply_command(0, &cast_spell(slot, vec![])).is_err());
    assert_eq!(engine.state.players[0].hand, hand);
    assert!(engine.state.stack.is_empty());
    engine.state.players[0].mana_pool.blue = 2;
    engine.state.players[0].mana_pool.colorless = 2;
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("five mana reduction");
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn issue_229_sunderflock_trigger_survives_source_removal() {
    let mut engine = setup();
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let source = cast_sunderflock(&mut engine);
    resolve_top(&mut engine);
    let source_generation = engine.state.zone_change_generation[&source];
    inject_card_into_hand(&mut engine, 0, "unsummon");
    engine.state.players[0].mana_pool.blue = 1;
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert!(engine.state.zone_change_generation[&source] > source_generation);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Hand);
}

#[test]
fn issue_229_sunderflock_uses_actual_caster_not_spell_controller() {
    for restore_control in [false, true] {
        let mut engine = setup();
        let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let source = cast_sunderflock(&mut engine);
        assert_eq!(engine.state.stack[0].cast_by, Some(0));
        engine.state.stack[0].controller = 1;
        if restore_control {
            engine.state.stack[0].controller = 0;
        }
        resolve_top(&mut engine);
        assert_eq!(
            engine.state.objects[&source].controller,
            if restore_control { 0 } else { 1 }
        );
        assert_eq!(engine.state.stack.len(), usize::from(restore_control));
        if restore_control {
            resolve_top(&mut engine);
        }
        assert_eq!(
            engine.state.objects[&bear].zone,
            if restore_control {
                Zone::Hand
            } else {
                Zone::Battlefield
            }
        );
    }
}

#[test]
fn issue_229_uncast_permanent_spell_copy_does_not_inherit_cast_entry() {
    let mut engine = setup();
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let source = cast_sunderflock(&mut engine);
    let mut copy = engine.state.stack[0].clone();
    copy.id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    copy.is_copy = true;
    let copied = copy.id;
    engine.state.stack.push(copy);
    resolve_top(&mut engine);
    assert!(engine.state.objects[&copied].is_token());
    assert_eq!(engine.state.stack.len(), 1, "only original spell remains");
    assert_eq!(engine.state.objects[&bear].zone, Zone::Battlefield);
    assert_eq!(engine.state.stack[0].id, source);
    resolve_top(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "original cast does trigger");
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Hand);
}

#[test]
fn issue_229_reanimated_sunderflock_does_not_trigger() {
    let mut engine = setup();
    let source = inject_graveyard_card(&mut engine, 0, "sunderflock");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "zombify");
    engine.state.players[0].mana_pool.black = 1;
    let slot = hand_index_for_card(&engine, 0, "zombify");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&bear].zone, Zone::Battlefield);
}

#[test]
fn issue_229_cast_clone_retains_caster_through_copy_entry_choice() {
    let mut engine = setup();
    let model = inject_creature_on_battlefield(&mut engine, 0, "sunderflock");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let clone = inject_card_into_hand(&mut engine, 0, "clone");
    let slot = hand_index_for_card(&engine, 0, "clone");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&clone].zone, Zone::Stack);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::CopySource
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![model]))
        .unwrap();
    assert_eq!(engine.state.objects[&clone].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "cast Clone becomes Sunderflock and triggers"
    );
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Hand);
}

#[test]
fn issue_229_mass_return_preserves_simultaneous_leave_observers() {
    let mut engine = setup();
    let scribe = inject_creature_on_battlefield(&mut engine, 0, "three_tree_scribe");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let source = cast_sunderflock(&mut engine);
    resolve_top(&mut engine);
    resolve_top(&mut engine);
    for oid in [scribe, bear] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Hand);
    }
    for _ in 0..2 {
        answer_trigger_order_in_engine_order(&mut engine);
        assert!(
            !engine.state.pending_triggers.is_empty(),
            "observer sees both departures"
        );
        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                        targets: target_object(source),
                        ..Default::default()
                    })),
                },
            )
            .unwrap();
    }
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        2
    );
}

#[test]
fn issue_229_mass_return_is_player_generic_and_publishes_owner_movements() {
    let mut engine = setup();
    // Exercise the engine's generic player sets below the current session-admission boundary.
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(299, 20));
    let third = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let stolen = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    engine.state.objects.get_mut(&stolen).unwrap().owner = 299;
    let token = inject_creature_on_battlefield(&mut engine, 2, "soldier_w_1_1");
    assert!(engine.state.objects[&token].is_token());
    cast_sunderflock(&mut engine);
    resolve_top(&mut engine);
    let batch = resolve_top(&mut engine);
    for oid in [third, stolen] {
        assert!(engine.state.players[2].hand.contains(&oid));
        assert!(batch.events.iter().any(|event| matches!(&event.ev,
            Some(Ev::PermanentMoved(moved)) if moved.object_id == oid && moved.controller_player_id == 299
        )));
    }
    assert!(!engine
        .state
        .players
        .iter()
        .any(|player| player.hand.contains(&token)));
    assert!(engine
        .state
        .objects
        .get(&token)
        .is_none_or(|object| object.zone != Zone::Hand));
}

#[test]
fn issue_229_blink_resets_entry_fact_without_changing_the_old_trigger() {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
    let mut engine = setup();
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let source = cast_sunderflock(&mut engine);
    resolve_top(&mut engine);
    let old_generation = engine.state.zone_change_generation[&source];
    engine.enable_dev_commands();
    for zone in [DevZone::Exile, DevZone::Battlefield] {
        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 0,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            zone: zone as i32,
                            card_name: "Sunderflock".into(),
                            ready: false,
                        })),
                    })),
                },
            )
            .unwrap();
    }
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&source],
        old_generation + 2
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "noncast reentry adds no trigger"
    );
    assert_eq!(engine.state.stack[0].source_zone_change, old_generation);
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Hand);
}

fn copy_characteristics(engine: &mut GameEngine, oid: u32, card: &str) {
    let definition = tricerules_cards::CardRegistry::global().get(card).unwrap();
    engine.state.objects.get_mut(&oid).unwrap().copiable_values =
        Some(tricerules_core::state::CopiableValues {
            source_card_id: card.into(),
            source_face_index: 0,
            face: definition.primary_face().clone(),
            room_faces: None,
            display_name: definition.name.clone(),
        });
}

#[test]
fn issue_229_mass_return_uses_resolution_types_and_ignores_target_protection() {
    use tricerules_cards::{ContinuousEffectKind, EffectDuration, Keyword};
    use tricerules_core::state::{AffectedScope, ContinuousEffect};
    let mut engine = setup();
    let becomes_elemental = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let loses_elemental = inject_creature_on_battlefield(&mut engine, 1, "air_elemental");
    let protected = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_sunderflock(&mut engine);
    resolve_top(&mut engine);
    copy_characteristics(&mut engine, becomes_elemental, "air_elemental");
    copy_characteristics(&mut engine, loses_elemental, "grizzly_bears");
    for keyword in [Keyword::Hexproof, Keyword::Shroud] {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(protected),
            kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
    }
    resolve_top(&mut engine);
    assert_eq!(
        engine.state.objects[&becomes_elemental].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&loses_elemental].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&protected].zone, Zone::Hand);
}

#[test]
fn issue_229_created_token_copy_does_not_copy_cast_history() {
    let mut engine = setup();
    let source = cast_sunderflock(&mut engine);
    resolve_entire_stack_two_player(&mut engine);
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "cackling_counterpart");
    engine.state.players[0].mana_pool.blue = 3;
    let slot = hand_index_for_card(&engine, 0, "cackling_counterpart");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    resolve_top(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].battlefield.len(), 2);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Battlefield);
}

#[test]
fn issue_229_sunderflock_commands_replay_identically_and_reject_invalid_sources() {
    fn run() -> Vec<RuledEventBatch> {
        let mut engine = setup();
        inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let slot = hand_index_for_card(&engine, 0, "sunderflock");
        let mut command = cast_spell(slot, vec![]);
        if let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() {
            cast.source.as_mut().unwrap().location =
                Some(tricerules_proto::ruled::v1::cast_source::Location::HandIndex(u32::MAX));
        }
        let before = serde_json::to_value(&engine.state).unwrap();
        assert!(engine.apply_command(0, &command).is_err());
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
        let cast = engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
        vec![cast, resolve_top(&mut engine), resolve_top(&mut engine)]
    }
    assert_eq!(run(), run());
}

#[test]
fn issue_229_exile_cast_uses_permission_caster_and_rejects_stale_generations() {
    use tricerules_core::state::{
        ActiveExilePlayPermission, ExilePermissionCastCost, ExilePlayPermissionOrigin,
        ExilePlayPermissionScope,
    };
    use tricerules_proto::ruled::v1::{cast_source, dev_command, DevCommand, DevMoveCard, DevZone};
    let mut engine = setup();
    let source = engine.state.players[0].hand[hand_index_for_card(&engine, 0, "sunderflock")];
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        zone: DevZone::Exile as i32,
                        card_name: "Sunderflock".into(),
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
    let generation = engine.state.zone_change_generation[&source];
    engine
        .state
        .active_exile_play_permissions
        .push(ActiveExilePlayPermission {
            group_id: 1,
            player_id: 1,
            source_label: "cast permission fixture".into(),
            object_id: source,
            zone_change_generation: generation,
            scope: ExilePlayPermissionScope::CastCard,
            cast_cost: ExilePermissionCastCost::AlternativeManaCost(
                tricerules_cards::mana::ManaCost::parse("{U}").unwrap(),
            ),
            origin: ExilePlayPermissionOrigin::Effect,
            available_after_turn_instance: None,
            expires_at_cleanup_turn_instance: None,
        });
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.players[1].mana_pool.blue = 1;
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let mut cast = CastSpell {
        source: Some(CastSource {
            location: Some(cast_source::Location::ExileObjectId(source)),
            expected_zone_change_generation: Some(generation + 1),
        }),
        cast_method: CastMethod::Permission as i32,
        casting_permission_id: Some(1),
        ..Default::default()
    };
    let before = serde_json::to_value(&engine.state).unwrap();
    assert!(engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(cast.clone()))
            }
        )
        .is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    cast.source
        .as_mut()
        .unwrap()
        .expected_zone_change_generation = Some(generation);
    engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(cast)),
            },
        )
        .unwrap();
    let fact = engine
        .state
        .turn_history
        .current
        .spell_casts
        .last()
        .unwrap();
    assert_eq!(fact.mana_value, 9);
    assert_eq!(fact.mana_spent, 1);
    assert_eq!(fact.caster, 1);
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&source].owner, 0);
    assert_eq!(engine.state.objects[&source].controller, 1);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Hand);
}
