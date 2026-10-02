//! Anger's graveyard grant keeps the timestamp of entry even while its condition is false.
use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

fn setup() -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine =
        GameEngine::new(26_100_802, &[0, 1, 2], 20, Some(vec![deck; 3]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine.enable_dev_commands();
    engine
}

fn move_card(engine: &mut GameEngine, player: i32, name: &str, zone: DevZone) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
}

fn haste(engine: &GameEngine, object: u32) -> bool {
    engine
        .characteristics(object)
        .unwrap()
        .has_keyword(Keyword::Haste)
}

#[test]
fn anger_inactive_graveyard_entry_timestamp_survives_later_mountain_and_reentry_is_new() {
    let mut engine = setup();
    let anger = inject_card_into_hand(&mut engine, 0, "anger");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    move_card(&mut engine, 0, "Anger", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&anger].zone, Zone::Graveyard);
    assert!(!haste(&engine, bear));
    // This removal is later than graveyard entry but earlier than the Mountain condition.
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(bear),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    inject_card_into_hand(&mut engine, 0, "mountain");
    move_card(&mut engine, 0, "Mountain", DevZone::Battlefield);
    assert!(
        !haste(&engine, bear),
        "condition activation must not retimestamp the grant"
    );
    let generation = engine.state.zone_change_generation[&anger];
    move_card(&mut engine, 0, "Anger", DevZone::Exile);
    assert!(!haste(&engine, bear));
    move_card(&mut engine, 0, "Anger", DevZone::Graveyard);
    assert_eq!(engine.state.zone_change_generation[&anger], generation + 2);
    assert!(
        haste(&engine, bear),
        "the new graveyard incarnation grants after removal"
    );
}

fn resolve_one(engine: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let mut batch = Default::default();
    for _ in 0..engine.state.players.len() {
        batch = engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    batch
}

fn modify(engine: &mut GameEngine, object: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn graveyard_source(engine: &mut GameEngine, player: usize) -> u32 {
    let source = inject_card_into_hand(engine, player, "anger");
    move_card(engine, player as i32, "Anger", DevZone::Graveyard);
    source
}

fn projected_haste(engine: &mut GameEngine, object: u32) -> bool {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|candidate| candidate.object_id == object)
                .map(|candidate| candidate.keywords.iter().any(|keyword| keyword == "Haste")),
            _ => None,
        })
        .unwrap()
}

#[test]
fn anger_actual_cast_printed_haste_cost_and_graveyard_only_public_grant() {
    let mut engine = setup();
    let source = inject_card_into_hand(&mut engine, 0, "anger");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    assert!(!haste(&engine, bear));
    let slot = hand_index_for_card(&engine, 0, "anger");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let before = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect_err("Anger costs 3R");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Stack);
    assert!(!haste(&engine, bear));
    resolve_one(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(haste(&engine, source));
    assert!(projected_haste(&mut engine, source));
    assert!(!projected_haste(&mut engine, bear));
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let before = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, vec![]))
        .expect_err("a static ability cannot be activated");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    move_card(&mut engine, 0, "Anger", DevZone::Graveyard);
    assert!(projected_haste(&mut engine, bear));
    for zone in [DevZone::Exile, DevZone::Hand, DevZone::Battlefield] {
        move_card(&mut engine, 0, "Anger", zone);
        assert!(!projected_haste(&mut engine, bear));
    }
}

#[test]
fn anger_condition_uses_current_land_subtype_control_and_current_creatures() {
    use tricerules_cards::{
        BasicLandType, ControllerReference, PermanentTypeFilter, TypeLineReplacement,
    };
    let mut engine = setup();
    graveyard_source(&mut engine, 0);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    inject_permanent_on_battlefield(&mut engine, 1, "mountain");
    assert!(
        !haste(&engine, own),
        "an opponent's Mountain does not count"
    );
    let land = inject_permanent_on_battlefield(&mut engine, 0, "taiga");
    assert!(haste(&engine, own), "a nonbasic Mountain counts");
    assert!(!haste(&engine, other));
    assert!(!haste(&engine, artifact));
    modify(
        &mut engine,
        land,
        ContinuousEffectKind::Layer4SetBasicLandType(BasicLandType::Island),
    );
    assert!(!haste(&engine, own));
    modify(
        &mut engine,
        land,
        ContinuousEffectKind::Layer4SetBasicLandType(BasicLandType::Mountain),
    );
    assert!(haste(&engine, own));
    modify(
        &mut engine,
        land,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(2),
        },
    );
    assert!(!haste(&engine, own));
    modify(
        &mut engine,
        land,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(0),
        },
    );
    assert!(haste(&engine, own));
    modify(
        &mut engine,
        artifact,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Artifact, PermanentTypeFilter::Creature],
            creature_types: vec![],
        }),
    );
    assert!(haste(&engine, artifact));
    modify(
        &mut engine,
        own,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
    );
    assert!(!haste(&engine, own));
    modify(
        &mut engine,
        other,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(0),
        },
    );
    assert!(haste(&engine, other));
    modify(
        &mut engine,
        land,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
        }),
    );
    assert!(
        !haste(&engine, other),
        "a former Mountain that is no longer a land does not count"
    );
}

#[test]
fn anger_stolen_death_grants_owners_creatures_and_not_former_controllers() {
    let mut engine = setup();
    let source = inject_creature_under_foreign_control(&mut engine, 0, 1, "anger");
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let thief = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    for player in 0..3 {
        inject_permanent_on_battlefield(&mut engine, player, "mountain");
    }
    engine.state.objects.get_mut(&source).unwrap().damage = 2;
    engine.apply_command(0, &pass()).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&source));
    assert!(haste(&engine, own));
    assert!(!haste(&engine, thief));
    assert!(projected_haste(&mut engine, own));
}

#[test]
fn anger_actual_mill_and_discard_create_the_graveyard_static() {
    for discard in [false, true] {
        let mut engine = setup();
        let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        inject_permanent_on_battlefield(&mut engine, 0, "mountain");
        let source;
        if discard {
            source = inject_card_into_hand(&mut engine, 0, "anger");
            let other = engine.state.players[0].hand[0];
            inject_card_into_hand(&mut engine, 0, "frantic_search");
            give_mana(
                &mut engine,
                0,
                ManaGift {
                    u: 1,
                    c: 2,
                    ..Default::default()
                },
            );
            let slot = hand_index_for_card(&engine, 0, "frantic_search");
            engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
            let batch = resolve_one(&mut engine);
            let choice = find_resolution_choice(&batch).unwrap();
            assert!(choice.candidate_object_ids.contains(&source));
            engine
                .apply_command(0, &submit_resolution_choice(vec![source, other]))
                .unwrap();
            engine
                .apply_command(0, &submit_resolution_choice(vec![]))
                .unwrap();
        } else {
            source = inject_library_card(&mut engine, 0, "anger");
            engine.state.players[0].library.retain(|&id| id != source);
            engine.state.players[0].library.push_front(source);
            let shredder = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
            apply_ability(&mut engine, 0, shredder, 0, target_player(0)).unwrap();
            resolve_one(&mut engine);
        }
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert!(haste(&engine, bear));
        assert!(projected_haste(&mut engine, bear));
    }
}

#[test]
fn anger_source_provenance_and_reentry_fail_closed() {
    let mut engine = setup();
    let source = graveyard_source(&mut engine, 0);
    inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert!(haste(&engine, bear));
    let original = engine
        .state
        .continuous_effects
        .iter()
        .find(|effect| effect.source_id == Some(source))
        .unwrap()
        .clone();
    for corruption in 0..4 {
        let mut corrupt = original.clone();
        let Some(tricerules_core::state::TriggerAbilityOrigin::StaticGrant {
            source_zone_change,
            definition,
            ..
        }) = &mut corrupt.trigger_grant_origin
        else {
            panic!("exact source provenance")
        };
        match corruption {
            0 => *source_zone_change += 1,
            1 => definition.card_id = "grizzly_bears".into(),
            2 => {
                definition.face_id =
                    serde_json::from_value(serde_json::json!("wrong_face")).unwrap()
            }
            _ => {
                definition.ability_path =
                    vec![serde_json::from_value(serde_json::json!("wrong_ability")).unwrap()]
            }
        }
        engine.state.continuous_effects = vec![corrupt];
        assert!(!haste(&engine, bear), "bad provenance {corruption}");
    }
    engine.state.continuous_effects = vec![original.clone()];
    let generation = engine.state.zone_change_generation[&source];
    move_card(&mut engine, 0, "Anger", DevZone::Exile);
    move_card(&mut engine, 0, "Anger", DevZone::Graveyard);
    assert_eq!(engine.state.zone_change_generation[&source], generation + 2);
    assert_eq!(
        engine
            .state
            .continuous_effects
            .iter()
            .filter(|effect| effect.source_id == Some(source))
            .count(),
        1
    );
    assert!(haste(&engine, bear));
    assert!(engine.state.continuous_effects[0].timestamp > original.timestamp);
    assert!(
        serde_json::from_str::<tricerules_cards::ResolvingEffectDuration>(
            r#""WhileSourceInGraveyard""#
        )
        .is_err()
    );
}

#[test]
fn anger_clone_loses_copied_static_but_physical_anger_recovers_its_own_after_death() {
    let mut engine = setup();
    inject_card_into_hand(&mut engine, 0, "anger");
    let source = move_ready_to_battlefield(&mut engine, 0, "anger");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    let copy = inject_card_into_hand(&mut engine, 0, "clone");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "clone");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    resolve_one(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .unwrap();
    assert!(haste(&engine, copy));
    assert!(!haste(&engine, bear));
    move_card(&mut engine, 0, "Clone", DevZone::Graveyard);
    assert!(engine.state.objects[&copy].copiable_values.is_none());
    assert!(
        !haste(&engine, bear),
        "physical Clone has no graveyard anthem"
    );
    let face = tricerules_cards::CardRegistry::global()
        .get("grizzly_bears")
        .unwrap()
        .primary_face()
        .clone();
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        source_card_id: "grizzly_bears".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    assert!(!haste(&engine, source));
    engine.state.objects.get_mut(&source).unwrap().damage = 2;
    engine.apply_command(0, &pass()).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(engine.state.objects[&source].copiable_values.is_none());
    assert!(
        haste(&engine, bear),
        "physical Anger regains its printed graveyard static"
    );
}

#[test]
fn anger_actual_token_copy_ceases_and_leaves_no_graveyard_grant() {
    let mut engine = setup();
    inject_card_into_hand(&mut engine, 0, "anger");
    let source = move_ready_to_battlefield(&mut engine, 0, "anger");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    inject_card_into_hand(&mut engine, 0, "cackling_counterpart");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "cackling_counterpart");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    resolve_one(&mut engine);
    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|id| engine.state.objects[id].is_token())
        .unwrap();
    assert!(haste(&engine, token));
    assert_eq!(
        engine.state.objects[&token]
            .token_origin
            .as_ref()
            .unwrap()
            .source_card_id,
        "anger"
    );
    engine.state.objects.get_mut(&token).unwrap().damage = 2;
    engine.apply_command(0, &pass()).unwrap();
    assert!(!engine.state.objects.contains_key(&token));
    assert!(!engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| effect.source_id == Some(token)
            && effect.duration == EffectDuration::WhileSourceInGraveyard));
    assert!(!haste(&engine, bear));
}

#[test]
fn anger_two_sources_keep_distinct_timestamps_against_recipient_removal() {
    let mut engine = setup();
    let older = graveyard_source(&mut engine, 0);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    modify(
        &mut engine,
        bear,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    let newer = graveyard_source(&mut engine, 0);
    inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    assert!(haste(&engine, bear));
    let earlier = engine
        .state
        .continuous_effects
        .iter()
        .find(|effect| effect.source_id == Some(older))
        .unwrap()
        .timestamp;
    let later = engine
        .state
        .continuous_effects
        .iter()
        .find(|effect| effect.source_id == Some(newer))
        .unwrap()
        .timestamp;
    assert!(later > earlier);
    // The logged move picks the first graveyard source (the older source); leave it in exile
    // so a second move selects the newer graveyard source.
    move_card(&mut engine, 0, "Anger", DevZone::Exile);
    assert!(haste(&engine, bear));
    move_card(&mut engine, 0, "Anger", DevZone::Exile);
    assert!(!haste(&engine, bear));
}

#[test]
fn anger_haste_controls_summoning_sick_tap_legality_and_projects_loss() {
    let mut engine = setup();
    inject_card_into_hand(&mut engine, 0, "llanowar_elves");
    let elf = move_ready_to_battlefield(&mut engine, 0, "llanowar_elves");
    engine.state.objects.get_mut(&elf).unwrap().summoning_sick = true;
    inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    let before = serde_json::to_value(&engine.state).unwrap();
    apply_ability(&mut engine, 0, elf, 0, vec![]).expect_err("summoning sick without haste");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    graveyard_source(&mut engine, 0);
    assert!(projected_haste(&mut engine, elf));
    let before = serde_json::to_value(&engine.state).unwrap();
    apply_ability(&mut engine, 1, elf, 0, vec![]).expect_err("other player cannot tap our elf");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    apply_ability(&mut engine, 0, elf, 0, vec![]).unwrap();
    assert!(engine.state.objects[&elf].tapped);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    engine.state.objects.get_mut(&elf).unwrap().tapped = false;
    move_card(&mut engine, 0, "Anger", DevZone::Exile);
    assert!(!projected_haste(&mut engine, elf));
    let before = serde_json::to_value(&engine.state).unwrap();
    apply_ability(&mut engine, 0, elf, 0, vec![])
        .expect_err("haste loss restores summoning restriction");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
}

#[test]
fn anger_grant_and_loss_refresh_summoning_sick_attack_legality() {
    let mut engine = setup();
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&bear).unwrap().summoning_sick = true;
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    inject_card_into_hand(&mut engine, 0, "anger");
    engine.apply_command(0, &primitive_yield()).unwrap();
    resolve_one(&mut engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    let before = engine.initial_response_batch();
    let legal = &before.legal_by_player[&0];
    assert!(!legal.selectable_attacker_ids.contains(&bear));
    let mut assignment = legal.legal_attack_assignments[0];
    assignment.attacker_object_id = bear;
    assignment.attacker_zone_change_generation = 0;
    let command = RuledCommand {
        cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
            assignments: vec![assignment],
        })),
    };
    let state_before = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_command(0, &command)
        .expect_err("summoning sick attacker without haste");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), state_before);
    move_card(&mut engine, 0, "Anger", DevZone::Graveyard);
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .selectable_attacker_ids
        .contains(&bear));
    move_card(&mut engine, 0, "Anger", DevZone::Exile);
    assert!(!engine.initial_response_batch().legal_by_player[&0]
        .selectable_attacker_ids
        .contains(&bear));
    move_card(&mut engine, 0, "Anger", DevZone::Graveyard);
    let batch = engine.apply_command(0, &command).unwrap();
    assert!(!attackers_declared_in(&batch).is_empty());
    assert!(engine.state.objects[&bear].tapped);
}

#[test]
fn anger_logged_zone_condition_changes_replay_deterministically() {
    let play = || {
        let mut engine = setup();
        inject_card_into_hand(&mut engine, 0, "anger");
        inject_card_into_hand(&mut engine, 0, "mountain");
        inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let mut batches = Vec::new();
        for (name, zone) in [
            ("Anger", DevZone::Graveyard),
            ("Mountain", DevZone::Battlefield),
            ("Mountain", DevZone::Exile),
            ("Mountain", DevZone::Battlefield),
            ("Anger", DevZone::Exile),
            ("Anger", DevZone::Graveyard),
        ] {
            batches.push(
                engine
                    .apply_command(
                        0,
                        &RuledCommand {
                            cmd: Some(Cmd::DevCommand(DevCommand {
                                target_player_id: 0,
                                dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                                    card_name: name.into(),
                                    zone: zone as i32,
                                    ready: false,
                                })),
                            })),
                        },
                    )
                    .unwrap(),
            );
        }
        let objects = engine
            .state
            .objects
            .iter()
            .map(|(&id, object)| (id, serde_json::to_value(object).unwrap()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let generations = engine
            .state
            .zone_change_generation
            .iter()
            .map(|(&id, &generation)| (id, generation))
            .collect::<std::collections::BTreeMap<_, _>>();
        (
            batches,
            engine.initial_response_batch(),
            objects,
            generations,
            engine.state.continuous_effects.clone(),
            engine.state.command_index,
            serde_json::to_value(&engine.state.players).unwrap(),
            engine.state.turn_history.clone(),
        )
    };
    assert_eq!(play(), play());
}
