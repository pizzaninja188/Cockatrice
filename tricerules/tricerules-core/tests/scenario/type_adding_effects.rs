use crate::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{AttachmentRecipient, Zone};

// Rebellious Captives and Badgermole share this action. A granted test ability isolates
// the mechanic before the calibration card data is added.
fn earthbend_fixture() -> (GameEngine, u32, u32) {
    earthbend_fixture_count(2)
}

fn earthbend_fixture_count(count: u32) -> (GameEngine, u32, u32) {
    use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
    use tricerules_core::{AffectedScope, ContinuousEffect};
    let fixture = r#"(id: "test", name: "Test", face_id: "test", types: ["Creature"], power: 1, toughness: 1,
            activated_abilities: [(ability_id: "activated_01", presentation: Fallback, costs: [], effect: [Earthbend(count: AMOUNT)])])"#.replace("AMOUNT", &count.to_string());
    let registry = tricerules_cards::CardRegistry::from_chunks_and_tokens(&[&fixture], &[])
        .expect("Earthbend must be a reusable authored action");
    let mut engine = GameEngine::new(
        150,
        &[0, 1],
        20,
        Some(vec![deck_with("mountain", &[]), deck_with("plains", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantActivatedAbility(Box::new(
            registry
                .get("test")
                .unwrap()
                .primary_face()
                .activated_abilities[0]
                .clone(),
        )),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    (engine, source, land)
}

fn earthbend_move(engine: &mut GameEngine, name: &str, zone: tricerules_proto::ruled::v1::DevZone) {
    use tricerules_proto::ruled::v1::{dev_command::Dev, DevCommand, DevMoveCard};
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
}

#[test]
fn earthbend_return_uses_stack_and_only_the_immediate_destination_generation() {
    use tricerules_proto::ruled::v1::DevZone;
    for destination in [
        DevZone::Graveyard,
        DevZone::Exile,
        DevZone::Hand,
        DevZone::Library,
    ] {
        for stale in [false, true] {
            let (mut e, source, land) = earthbend_fixture();
            e.apply_command(0, &activate_ability(source, 0, target_object(land)))
                .unwrap();
            resolve_entire_stack_two_player(&mut e);
            let generation = e
                .state
                .zone_change_generation
                .get(&land)
                .copied()
                .unwrap_or(0);
            earthbend_move(&mut e, "Forest", destination);
            assert!(e.state.active_event_observers.is_empty());
            let should_return = matches!(destination, DevZone::Graveyard | DevZone::Exile);
            assert_eq!(e.state.stack.len(), usize::from(should_return));
            assert_ne!(e.state.objects[&land].zone, Zone::Battlefield);
            if let Some(trigger) = e.state.stack.last() {
                assert_eq!(trigger.source_permanent_id, Some(source));
                assert_eq!(
                    trigger.trigger_context.observed_object.unwrap().object_id,
                    land
                );
            }
            if stale && destination != DevZone::Hand {
                earthbend_move(&mut e, "Forest", DevZone::Hand);
            }
            resolve_entire_stack_two_player(&mut e);
            if should_return && !stale {
                assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
                assert_eq!(e.state.zone_change_generation[&land], generation + 2);
                assert!(e.state.objects[&land].tapped);
                assert!(e.state.objects[&land].counters.is_empty());
                assert!(!e.characteristics(land).unwrap().is_creature());
                assert!(!e.effective_has_keyword(land, Keyword::Haste));
            } else {
                assert_ne!(e.state.objects[&land].zone, Zone::Battlefield);
            }
        }
    }
}

#[test]
fn earthbend_source_departure_does_not_cancel_animation_or_return() {
    use tricerules_proto::ruled::v1::DevZone;
    let (mut e, source, land) = earthbend_fixture();
    e.apply_command(0, &activate_ability(source, 0, target_object(land)))
        .unwrap();
    earthbend_move(&mut e, "Grizzly Bears", DevZone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert!(e.characteristics(land).unwrap().is_creature());
    e.state.objects.get_mut(&land).unwrap().damage = 2;
    e.apply_command(e.state.priority_player_id(), &pass())
        .unwrap();
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
}

#[test]
fn earthbend_target_leaving_and_returning_before_resolution_is_not_animated() {
    use tricerules_proto::ruled::v1::DevZone;
    let (mut e, source, land) = earthbend_fixture();
    e.apply_command(0, &activate_ability(source, 0, target_object(land)))
        .unwrap();
    earthbend_move(&mut e, "Forest", DevZone::Exile);
    earthbend_move(&mut e, "Forest", DevZone::Battlefield);
    resolve_entire_stack_two_player(&mut e);
    assert!(!e.characteristics(land).unwrap().is_creature());
    assert!(e.state.active_event_observers.is_empty());
}

#[test]
fn earthbend_animates_only_your_land_and_survives_cleanup() {
    let (mut engine, source, land) = earthbend_fixture();
    let opposing_land = inject_permanent_on_battlefield(&mut engine, 1, "island");
    for illegal in [source, opposing_land] {
        let command_index = engine.state.command_index;
        assert!(engine
            .apply_command(0, &activate_ability(source, 0, target_object(illegal)))
            .is_err());
        assert_eq!(engine.state.command_index, command_index);
    }
    engine
        .apply_command(0, &activate_ability(source, 0, target_object(land)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    let characteristics = engine.characteristics(land).unwrap();
    assert!(characteristics.has_type("Land") && characteristics.has_type("Forest"));
    assert!(characteristics.is_creature());
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(2), Some(2))
    );
    assert!(engine.effective_has_keyword(land, Keyword::Haste));
    assert_eq!(engine.state.active_event_observers.len(), 1);
    for _ in 0..20 {
        if engine.state.active_player_idx == 1 {
            break;
        }
        let player = engine.state.priority_player_id();
        engine.apply_command(player, &pass()).unwrap();
    }
    assert_eq!(engine.state.active_player_idx, 1);
    assert!(engine.characteristics(land).unwrap().is_creature());
    assert!(engine.effective_has_keyword(land, Keyword::Haste));
    assert_eq!(engine.state.active_event_observers.len(), 1);
}

#[test]
fn earthbend_zero_creates_its_watcher_before_the_sba_death() {
    let (mut e, source, land) = earthbend_fixture_count(0);
    e.apply_command(0, &activate_ability(source, 0, target_object(land)))
        .unwrap();
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(
        e.state.stack.len(),
        1,
        "the delayed return survives the zero-toughness SBA"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
    assert!(e.state.objects[&land].tapped);
}

#[test]
fn earthbend_preserves_mana_and_haste_but_cannot_return_a_token() {
    let (mut e, source, land) = earthbend_fixture();
    let face = tricerules_cards::CardRegistry::global()
        .get("forest")
        .unwrap()
        .primary_face()
        .clone();
    e.state.objects.get_mut(&land).unwrap().token_origin =
        Some(tricerules_core::state::CopiableValues {
            source_card_id: "forest".into(),
            source_face_index: 0,
            display_name: face.name.clone(),
            face,
            room_faces: None,
        });
    e.state.objects.get_mut(&land).unwrap().summoning_sick = true;
    e.apply_command(0, &activate_ability(source, 0, target_object(land)))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    let mana_before = e.state.players[0].mana_pool.green;
    e.apply_command(0, &activate_ability_for(&e, land, 0, vec![]))
        .unwrap();
    assert!(e.state.objects[&land].tapped);
    assert_eq!(e.state.players[0].mana_pool.green, mana_before + 1);
    e.state.objects.get_mut(&land).unwrap().damage = 2;
    e.apply_command(0, &pass()).unwrap();
    assert!(!e.state.objects.contains_key(&land));
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);
    assert!(!e.state.objects.contains_key(&land));
}

#[test]
fn earthbend_rebellious_captives_exhaust_resets_only_on_reentry() {
    use tricerules_proto::ruled::v1::DevZone;
    tricerules_cards::CardRegistry::global()
        .get("rebellious_captives")
        .expect("calibration card exists");
    let (mut e, _, land) = earthbend_fixture();
    let source = inject_creature_on_battlefield(&mut e, 0, "rebellious_captives");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 18,
            ..Default::default()
        },
    );
    e.apply_command(0, &activate_ability(source, 0, target_object(land)))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(source), Some(4));
    assert_eq!(e.effective_power(land), Some(2));
    assert!(e
        .apply_command(0, &activate_ability(source, 0, target_object(land)))
        .is_err());
    earthbend_move(&mut e, "Rebellious Captives", DevZone::Graveyard);
    earthbend_move(&mut e, "Rebellious Captives", DevZone::Battlefield);
    e.apply_command(0, &activate_ability_for(&e, source, 0, target_object(land)))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(source), Some(4));
    assert_eq!(e.effective_power(land), Some(4));
}

#[test]
fn earthbend_badgermole_target_and_counter_filtered_trample() {
    tricerules_cards::CardRegistry::global()
        .get("badgermole")
        .expect("calibration card exists");
    let (mut e, bear, land) = earthbend_fixture();
    inject_card_into_hand(&mut e, 0, "badgermole");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "badgermole");
    e.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut e);
    assert_eq!(e.state.pending_triggers.len(), 1);
    let choose = |oid| RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: target_object(oid),
            ..Default::default()
        })),
    };
    assert!(e.apply_command(0, &choose(bear)).is_err());
    e.apply_command(0, &choose(land)).unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(land), Some(2));
    assert!(e.effective_has_keyword(land, Keyword::Trample));
    assert!(!e.effective_has_keyword(bear, Keyword::Trample));
    earthbend_move(
        &mut e,
        "Badgermole",
        tricerules_proto::ruled::v1::DevZone::Hand,
    );
    assert!(!e.effective_has_keyword(land, Keyword::Trample));
    assert!(e.effective_has_keyword(land, Keyword::Haste));
}

#[test]
fn earthbend_dai_li_both_modes_and_discard_eligibility() {
    tricerules_cards::CardRegistry::global()
        .get("dai_li_indoctrination")
        .expect("calibration card exists");
    for mode in [0, 1] {
        let (mut e, _, land) = earthbend_fixture();
        inject_card_into_hand(&mut e, 0, "dai_li_indoctrination");
        let bear = inject_card_into_hand(&mut e, 1, "grizzly_bears");
        let bolt = inject_card_into_hand(&mut e, 1, "lightning_bolt");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "dai_li_indoctrination");
        e.apply_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(
                    mode,
                    if mode == 0 {
                        target_player(1)
                    } else {
                        target_object(land)
                    },
                )],
            ),
        )
        .unwrap();
        let spell = e.state.stack.last().unwrap().id;
        let spell_generation = e.state.zone_change_generation[&spell];
        pass_both_players(&mut e);
        if mode == 0 {
            assert!(e.state.pending_resolution.is_some());
            assert!(e
                .apply_command(0, &submit_resolution_choice(vec![bolt]))
                .is_err());
            assert!(e
                .apply_command(1, &submit_resolution_choice(vec![bear]))
                .is_err());
            e.apply_command(0, &submit_resolution_choice(vec![bear]))
                .unwrap();
            assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
            assert_eq!(e.state.objects[&bolt].zone, Zone::Hand);
            assert!(e.state.pending_resolution.is_none());
        } else {
            assert_eq!(e.effective_power(land), Some(2));
            assert_eq!(e.state.objects[&bear].zone, Zone::Hand);
            let tricerules_core::state::EventObserverPayload::StageDelayedTrigger(payload) =
                &e.state.active_event_observers.last().unwrap().payload
            else {
                panic!("delayed return")
            };
            assert_eq!(payload.source.object_id, spell);
            assert_eq!(payload.source.zone_change_generation, spell_generation);
        }
    }
}

#[test]
fn earthbend_repeated_animation_and_replaced_death_return_only_once() {
    let (mut e, source, land) = earthbend_fixture();
    for _ in 0..2 {
        e.apply_command(0, &activate_ability(source, 0, target_object(land)))
            .unwrap();
        resolve_entire_stack_two_player(&mut e);
    }
    assert_eq!(e.effective_power(land), Some(4));
    assert_eq!(e.state.active_event_observers.len(), 2);
    let generation = e
        .state
        .zone_change_generation
        .get(&land)
        .copied()
        .unwrap_or(0);
    e.state
        .death_replacement_effects
        .push(tricerules_core::state::ActiveDeathReplacement {
            object_id: land,
            zone_change_generation: generation,
        });
    e.state.objects.get_mut(&land).unwrap().damage = 4;
    e.apply_command(0, &pass()).unwrap();
    assert_eq!(e.state.objects[&land].zone, Zone::Exile);
    answer_trigger_order_in_engine_order(&mut e);
    assert_eq!(e.state.stack.len(), 2);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.zone_change_generation[&land], generation + 2);
    assert_eq!(
        e.state.players[0]
            .battlefield
            .iter()
            .filter(|&&oid| oid == land)
            .count(),
        1
    );
}

#[test]
fn earthbend_return_keeps_its_controller_after_control_and_ability_changes() {
    use tricerules_cards::primitives::{ContinuousEffectKind, ControllerReference, EffectDuration};
    use tricerules_core::{AffectedScope, ContinuousEffect};
    let (mut e, source, land) = earthbend_fixture();
    // A foreign-owned land currently controlled by the earthbending player.
    e.state.objects.get_mut(&land).unwrap().owner = 1;
    e.apply_command(0, &activate_ability(source, 0, target_object(land)))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    for kind in [
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    ] {
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(land),
            kind,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: e.state.command_index + 1,
        });
    }
    e.apply_command(0, &pass()).unwrap();
    assert_eq!(e.state.objects[&land].controller, 1);
    assert!(!e.effective_has_keyword(land, Keyword::Haste));
    e.state.objects.get_mut(&land).unwrap().damage = 2;
    e.apply_command(1, &pass()).unwrap();
    assert!(e.state.players[1].graveyard.contains(&land));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&land].controller, 0);
    assert_eq!(e.state.objects[&land].owner, 1);
    assert!(e.state.objects[&land].tapped);
}

#[test]
fn earthbend_no_longer_creature_only_returns_from_exile() {
    use tricerules_cards::primitives::ContinuousEffectKind;
    use tricerules_proto::ruled::v1::DevZone;
    for destination in [DevZone::Graveyard, DevZone::Exile] {
        let (mut e, source, land) = earthbend_fixture();
        e.apply_command(0, &activate_ability(source, 0, target_object(land)))
            .unwrap();
        resolve_entire_stack_two_player(&mut e);
        e.state
            .continuous_effects
            .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4AddTypes(_)));
        assert!(!e.characteristics(land).unwrap().is_creature());
        earthbend_move(&mut e, "Forest", destination);
        assert_eq!(
            e.state.stack.len(),
            usize::from(destination == DevZone::Exile)
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&land].zone,
            if destination == DevZone::Exile {
                Zone::Battlefield
            } else {
                Zone::Graveyard
            }
        );
    }
}

#[test]
fn dub_adds_knight_without_replacing_printed_types() {
    let decks = Some(vec![
        deck_with("plains", &["dub", "grizzly_bears"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(81_001, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "dub");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );

    let dub_slot = hand_index_for_card(&engine, 0, "dub");
    engine
        .apply_command(0, &cast_spell(dub_slot, target_object(bear)))
        .expect("cast Dub");
    resolve_entire_stack_two_player(&mut engine);

    let characteristics = engine.characteristics(bear).expect("enchanted creature");
    assert!(characteristics.has_type("Creature"));
    assert!(characteristics.has_type("Bear"));
    assert!(characteristics.has_type("Knight"));
    assert_eq!(engine.effective_power(bear), Some(4));
    assert_eq!(engine.effective_toughness(bear), Some(4));
    assert!(engine.effective_has_keyword(bear, Keyword::FirstStrike));

    let dub = battlefield_object_for_card(&engine, 0, "dub");
    assert_eq!(
        engine.state.objects[&dub].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
    engine.state.objects.get_mut(&dub).expect("Dub").zone = Zone::Graveyard;

    let restored = engine
        .characteristics(bear)
        .expect("creature after Dub leaves");
    assert!(restored.has_type("Creature"));
    assert!(restored.has_type("Bear"));
    assert!(!restored.has_type("Knight"));
    assert_eq!(engine.effective_power(bear), Some(2));
    assert_eq!(engine.effective_toughness(bear), Some(2));
    assert!(!engine.effective_has_keyword(bear, Keyword::FirstStrike));
}

#[test]
fn liquimetal_coating_adds_artifact_until_cleanup_and_updates_legality() {
    let decks = Some(vec![
        deck_with("mountain", &["liquimetal_coating"]),
        deck_with("swamp", &["go_for_the_throat", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(81_002, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let coating = relocate_to_battlefield(&mut engine, 0, "liquimetal_coating", false);
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_card_in_hand(&mut engine, 1, "go_for_the_throat");

    engine
        .apply_command(0, &activate_ability(coating, 0, target_object(bear)))
        .expect("activate Liquimetal Coating");
    resolve_entire_stack_two_player(&mut engine);

    let coated = engine.characteristics(bear).expect("coated creature");
    assert!(coated.has_type("Creature"));
    assert!(coated.has_type("Bear"));
    assert!(coated.has_type("Artifact"));

    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &pass()).expect("pass priority");
    let removal_slot = hand_index_for_card(&engine, 1, "go_for_the_throat");
    engine
        .apply_command(1, &cast_spell(removal_slot, target_object(bear)))
        .expect_err("an artifact creature is not legal for Go for the Throat");
    engine
        .apply_command(1, &pass())
        .expect("finish the main-phase priority round");
    engine
        .apply_command(0, &primitive_yield())
        .expect("begin combat to end combat");
    engine
        .apply_command(0, &primitive_yield())
        .expect("end combat to second main");
    engine
        .apply_command(0, &primitive_yield())
        .expect("second main to end step");
    engine
        .apply_command(0, &primitive_yield())
        .expect("end step to cleanup or next upkeep");
    resolve_cleanup_discards_if_any(&mut engine);
    let expired = engine
        .characteristics(bear)
        .expect("creature after cleanup");
    assert!(expired.has_type("Creature"));
    assert!(expired.has_type("Bear"));
    assert!(!expired.has_type("Artifact"));

    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let removal_slot = hand_index_for_card(&engine, 1, "go_for_the_throat");
    engine
        .apply_command(1, &cast_spell(removal_slot, target_object(bear)))
        .expect("Go for the Throat is legal after the type addition expires");
}

#[test]
fn liquimetal_coating_type_addition_does_not_follow_a_zone_change() {
    let decks = Some(vec![
        deck_with("mountain", &["liquimetal_coating"]),
        deck_with("swamp", &["murder", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(81_004, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let coating = relocate_to_battlefield(&mut engine, 0, "liquimetal_coating", false);
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_card_in_hand(&mut engine, 1, "murder");

    engine
        .apply_command(0, &activate_ability(coating, 0, target_object(bear)))
        .expect("activate Liquimetal Coating");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine
        .characteristics(bear)
        .expect("coated bear")
        .has_type("Artifact"));

    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &pass()).expect("pass priority");
    let murder_slot = hand_index_for_card(&engine, 1, "murder");
    engine
        .apply_command(1, &cast_spell(murder_slot, target_object(bear)))
        .expect("cast Murder");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);
    assert!(!engine
        .characteristics(bear)
        .expect("card after zone change")
        .has_type("Artifact"));
}
