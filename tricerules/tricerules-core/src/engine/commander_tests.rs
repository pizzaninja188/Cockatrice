use super::*;

fn fixture() -> GameEngine {
    let registry = CardRegistry::from_chunks_and_tokens(&[
        r#"(id:"commander_a",name:"Commander A",face_id:"commander_a",mana_cost:"{1}{G}",types:["Creature"],supertypes:["Legendary"],power:2,toughness:2)"#,
        r#"(id:"commander_b",name:"Commander B",face_id:"commander_b",mana_cost:"{1}{G}",types:["Creature"],supertypes:["Legendary"],power:2,toughness:2)"#,
        r#"(id:"forest",name:"Forest",face_id:"forest",types:["Land","Forest"],supertypes:["Basic"])"#,
    ], &[]).unwrap();
    let mut engine = GameEngine::new_with_commander_decks(
        Box::leak(Box::new(registry)),
        903_800,
        &[0, 1, 2],
        40,
        Some(vec![
            EngineDeck {
                mainboard: vec!["forest".into(); 20],
                commanders: vec!["commander_a".into(), "commander_b".into()],
            },
            EngineDeck {
                mainboard: vec!["forest".into(); 20],
                commanders: vec![],
            },
            EngineDeck {
                mainboard: vec!["forest".into(); 20],
                commanders: vec![],
            },
        ]),
        true,
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine
}

fn command(engine: &GameEngine, object: ObjectId) -> rv1::CastSpell {
    rv1::CastSpell {
        source: Some(rv1::CastSource {
            location: Some(rv1::cast_source::Location::CommandZoneObjectId(object)),
            expected_zone_change_generation: Some(
                engine.payment_object_ref(object).zone_change_generation,
            ),
        }),
        cast_method: rv1::CastMethod::Normal as i32,
        ..Default::default()
    }
}

#[test]
fn commander_cast_offers_tax_and_rejection_preserve_exact_source() {
    let mut engine = fixture();
    let commander = engine.state.players[0].command_zone[0];
    let batch = engine.initial_response_batch();
    let offer = batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|a| a.object_id == commander)
        .unwrap();
    assert_eq!(offer.source_zone, rv1::CastSourceZone::Command as i32);
    assert_eq!(offer.cost, "{1}{G}");
    let cast = command(&engine, commander);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(cast.clone()))
            }
        )
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine.state.players[0].mana_pool.colorless = 1;
    engine.state.players[0].mana_pool.green = 1;
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(cast)),
            },
        )
        .unwrap();
    assert!(!engine.state.players[0].command_zone.contains(&commander));
    assert_eq!(
        engine.state.players[0]
            .commander_cast_counts
            .get(&commander),
        Some(&1)
    );
    while !engine.state.stack.is_empty() {
        engine
            .apply_command(
                engine.state.priority_player_id(),
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
    resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        commander,
        Zone::Command,
        None,
    )
    .unwrap();
    engine.state.priority_idx = 0;
    engine.state.turn_step = TurnStep::Main1;
    let batch = engine.initial_response_batch();
    let offer = batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|a| a.object_id == commander)
        .unwrap();
    assert_eq!(offer.cost, "{3}{G}");
    let other = engine.state.players[0]
        .command_zone
        .iter()
        .copied()
        .find(|id| *id != commander)
        .unwrap();
    assert_eq!(
        batch.legal_by_player[&0]
            .zone_cast_actions
            .iter()
            .find(|a| a.object_id == other)
            .unwrap()
            .cost,
        "{1}{G}"
    );
    let before = engine.diagnostic_snapshot().unwrap();
    let mut stale = command(&engine, commander);
    stale
        .source
        .as_mut()
        .unwrap()
        .expected_zone_change_generation = Some(0);
    assert!(engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(stale))
            }
        )
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn commander_combat_damage_is_per_card_per_victim_and_survives_zone_changes() {
    use damage::{DamageClassification, DamageEvent, DamageRecipient, DamageResult};
    let mut engine = fixture();
    let commander = engine.state.players[0].command_zone[0];
    let other = engine.state.players[0].command_zone[1];
    for (source, victim, amount, classification) in [
        (commander, 1, 10, DamageClassification::Combat),
        (other, 1, 11, DamageClassification::Combat),
        (commander, 2, 11, DamageClassification::Combat),
        (commander, 1, 1, DamageClassification::Noncombat),
    ] {
        let mut event = DamageEvent::combat(
            source,
            0,
            "Commander",
            DamageRecipient::Player(victim),
            amount,
        );
        event.classification = classification;
        engine
            .commit_damage_result(
                &event,
                DamageResult {
                    attempted: amount,
                    dealt: amount,
                    prevented: 0,
                },
                false,
                &mut vec![],
            )
            .unwrap();
    }
    assert_eq!(
        engine.state.players[1].commander_damage.get(&commander),
        Some(&10)
    );
    assert_eq!(
        engine.state.players[1].commander_damage.get(&other),
        Some(&11)
    );
    engine.apply_sbas(&mut vec![]).unwrap();
    assert!(!engine.state.players[1].has_lost);
    resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        commander,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    engine.state.objects.get_mut(&commander).unwrap().controller = 2;
    let event = DamageEvent::combat(commander, 2, "Commander", DamageRecipient::Player(1), 11);
    engine
        .commit_damage_result(
            &event,
            DamageResult {
                attempted: 11,
                dealt: 11,
                prevented: 0,
            },
            false,
            &mut vec![],
        )
        .unwrap();
    engine.apply_sbas(&mut vec![]).unwrap();
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.players[2].has_lost);
}

#[test]
fn cancelled_commander_announcement_restores_command_order_without_tax() {
    let mut engine = fixture();
    let original = engine.state.players[0].command_zone.clone();
    let oid = original[0];
    let cast = command(&engine, oid);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
                    rv1::BeginSpellCast {
                        announcement: Some(rv1::SpellCastAnnouncement {
                            source: cast.source,
                            cast_method: cast.cast_method,
                            ..Default::default()
                        }),
                    },
                )),
            },
        )
        .unwrap();
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .unwrap()
        .transaction_id;
    assert!(engine.state.players[0].commander_cast_counts.is_empty());
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
                    rv1::CancelSpellCast { transaction_id },
                )),
            },
        )
        .unwrap();
    assert_eq!(engine.state.players[0].command_zone, original);
    assert_eq!(engine.state.objects[&oid].zone, Zone::Command);
    assert!(engine.state.players[0].commander_cast_counts.is_empty());
    assert_eq!(engine.payment_object_ref(oid).zone_change_generation, 0);
}

#[test]
fn commander_unblocked_combat_uses_damage_history_and_eliminates_at_twenty_one() {
    let mut engine = fixture();
    let commander = engine.state.players[0].command_zone[0];
    resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        commander,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    engine
        .state
        .objects
        .get_mut(&commander)
        .unwrap()
        .summoning_sick = false;
    engine.state.players[1]
        .commander_damage
        .insert(commander, 19);
    engine.state.turn_step = TurnStep::DeclareAttackers;
    let batch = engine.initial_response_batch();
    let assignment = batch.legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|a| a.attacker_object_id == commander && a.defending_player_id == 1)
        .copied()
        .unwrap();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::DeclareAttackers(
                    rv1::DeclareAttackers {
                        assignments: vec![assignment],
                    },
                )),
            },
        )
        .unwrap();
    for _ in 0..30 {
        if engine.state.players[1].has_lost {
            break;
        }
        let actor = engine.state.priority_player_id();
        engine
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
    assert_eq!(
        engine.state.players[1].commander_damage.get(&commander),
        Some(&21)
    );
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.players[2].has_lost);
}
