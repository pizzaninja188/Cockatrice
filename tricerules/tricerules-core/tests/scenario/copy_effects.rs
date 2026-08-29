use crate::helpers::*;
use tricerules_cards::primitives::{
    Color, ContinuousEffectKind, CounterKind, EffectDuration, Keyword, StaticAbilityDef,
};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone, TargetRef};

#[test]
fn copy_snapshot_does_not_require_a_registry_definition() {
    let (mut engine, source) = resolving_clone_with_source("serra_angel", 4601);
    let face = tricerules_cards::CardRegistry::global()
        .get("serra_angel")
        .unwrap()
        .primary_face()
        .clone();
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.card_id = "runtime_token_without_registry_definition".into();
    object.copiable_values = Some(tricerules_core::state::CopiableValues {
        source_card_id: String::new(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    assert_eq!(engine.effective_power(source), Some(4));
    assert_eq!(engine.effective_toughness(source), Some(4));
}

#[test]
fn token_copy_and_populate_create_independent_tokens() {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                "cackling_counterpart",
                "wake_the_reflections",
                "serra_angel",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(4602, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "serra_angel");
    let source = put_creature_on_battlefield(&mut engine, 0, "serra_angel");
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 3);
    ensure_in_hand(&mut engine, 0, "cackling_counterpart");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "cackling_counterpart");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    pass_both_players(&mut engine);
    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| *oid != source)
        .unwrap();
    assert!(engine.state.objects[&token].is_token());
    assert_eq!(engine.effective_power(token), Some(4));
    assert_eq!(engine.effective_power(source), Some(7));

    ensure_in_hand(&mut engine, 0, "wake_the_reflections");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "wake_the_reflections");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert_eq!(pending.presentation.candidates, vec![token]);
    assert_eq!(pending.presentation.min, 1);
    engine
        .apply_command(0, &submit_resolution_choice(vec![token]))
        .unwrap();
    assert_eq!(engine.state.players[0].battlefield.len(), 3);
}

fn token_copy_game(card_id: &str) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                card_id,
                "cackling_counterpart",
                "wake_the_reflections",
                "unsummon",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(4603, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, card_id);
    let source = put_creature_on_battlefield(&mut engine, 0, card_id);
    (engine, source)
}

fn cast_copy(engine: &mut GameEngine, source: u32) -> (u32, RuledEventBatch) {
    ensure_in_hand(engine, 0, "cackling_counterpart");
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, "cackling_counterpart");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    engine.apply_command(0, &pass()).unwrap();
    let batch = engine.apply_command(1, &pass()).unwrap();
    let tokens = token_created_events(&batch);
    assert_eq!(tokens.len(), 1);
    (tokens[0].object_id, batch)
}

fn begin_populate(engine: &mut GameEngine) {
    ensure_in_hand(engine, 0, "wake_the_reflections");
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, "wake_the_reflections");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(engine);
}

#[test]
fn token_copy_inline_ability_preserves_explicit_target_groups() {
    use tricerules_cards::primitives::{SpellEffectKind, TargetGroupDef, TargetingDef};
    let (mut engine, source) = token_copy_game("prodigal_sorcerer");
    let mut face = tricerules_cards::CardRegistry::global()
        .get("prodigal_sorcerer")
        .unwrap()
        .primary_face()
        .clone();
    face.activated_abilities[0].effect = vec![
        SpellEffectKind::DamageTarget {
            amount: 1.into(),
            target: Default::default(),
        },
        SpellEffectKind::DamageTarget {
            amount: 2.into(),
            target: Default::default(),
        },
    ];
    face.activated_abilities[0].targeting = Some(TargetingDef {
        groups: (0..2)
            .map(|index| TargetGroupDef {
                min: 1,
                max: 1,
                prompt: "Choose a damage recipient".into(),
                effect_indices: vec![index],
                distinct_from: vec![],
                same_graveyard: false,
            })
            .collect(),
    });
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.card_id = "inline_pinger".into();
    object.token_origin = Some(tricerules_core::state::CopiableValues {
        source_card_id: String::new(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    let (copy, _) = cast_copy(&mut engine, source);
    engine.state.objects.get_mut(&copy).unwrap().summoning_sick = false;
    let mut targets = target_player(0);
    let mut other = target_player(1);
    other[0].group_index = 1;
    targets.extend(other);
    apply_ability(&mut engine, 0, copy, 0, targets).unwrap();
    engine.state.objects.remove(&copy);
    engine.state.players[0]
        .battlefield
        .retain(|oid| *oid != copy);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].life, 19);
    assert_eq!(engine.state.players[1].life, 18);
}

#[test]
fn cackling_counterpart_flashback_creates_a_token_and_exiles_the_spell() {
    let (mut engine, source) = token_copy_game("serra_angel");
    let (first, _) = cast_copy(&mut engine, source);
    let spell = *engine.state.players[0]
        .graveyard
        .iter()
        .find(|oid| engine.state.objects[oid].card_id == "cackling_counterpart")
        .unwrap();
    grant_pool(&mut engine, 0);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    cast_method: tricerules_proto::ruled::v1::CastMethod::Flashback as i32,
                    source: Some(graveyard_cast_source(
                        spell,
                        engine
                            .state
                            .zone_change_generation
                            .get(&spell)
                            .copied()
                            .unwrap_or(0),
                    )),
                    targets: target_object(first),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    engine.apply_command(0, &pass()).unwrap();
    let batch = engine.apply_command(1, &pass()).unwrap();
    let tokens = token_created_events(&batch);
    assert_eq!(tokens.len(), 1);
    assert_ne!(tokens[0].object_id, first);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].battlefield.len(), 3);
}

#[test]
fn populate_resumes_effect_tail_after_a_copied_entry_choice() {
    use tricerules_cards::primitives::SpellEffectKind;
    let (mut engine, source) = token_copy_game("prodigal_sorcerer");
    let mut face = tricerules_cards::CardRegistry::global()
        .get("prodigal_sorcerer")
        .unwrap()
        .primary_face()
        .clone();
    face.activated_abilities[0].effect = vec![
        SpellEffectKind::Populate,
        SpellEffectKind::GainLife { amount: 3.into() },
    ];
    face.static_abilities = tricerules_cards::CardRegistry::global()
        .get("clone")
        .unwrap()
        .primary_face()
        .static_abilities
        .clone();
    engine.state.objects.get_mut(&source).unwrap().token_origin =
        Some(tricerules_core::state::CopiableValues {
            source_card_id: "prodigal_sorcerer".into(),
            source_face_index: 0,
            display_name: face.name.clone(),
            face,
            room_faces: None,
        });
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = false;
    apply_ability(&mut engine, 0, source, 0, vec![]).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .unwrap();
    assert_eq!(
        engine.state.players[0].life, 20,
        "tail waits for entry choice"
    );
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
        .apply_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    assert_eq!(engine.state.players[0].life, 23);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].battlefield.len(), 2);
}

#[test]
fn token_copy_and_populate_replay_identically() {
    fn replay() -> (Vec<RuledEventBatch>, u64) {
        let (mut engine, source) = token_copy_game("serra_angel");
        let (copy, batch) = cast_copy(&mut engine, source);
        begin_populate(&mut engine);
        let chosen = engine
            .apply_command(0, &submit_resolution_choice(vec![copy]))
            .unwrap();
        (vec![batch, chosen], engine.state.command_index)
    }
    assert_eq!(replay(), replay());
}

#[test]
fn issue_164_token_copy_has_its_own_cap_and_accepted_commands_replay_identically() {
    fn setup() -> (GameEngine, u32) {
        let (mut engine, source) = token_copy_game("soul_warden");
        let mut face = tricerules_cards::CardRegistry::global()
            .get("soul_warden")
            .unwrap()
            .primary_face()
            .clone();
        face.triggered_abilities[0].max_triggers_per_turn = Some(1);
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .copiable_values = Some(tricerules_core::state::CopiableValues {
            source_card_id: "soul_warden".into(),
            source_face_index: 0,
            display_name: face.name.clone(),
            face,
            room_faces: None,
        });
        for _ in 0..3 {
            inject_card_into_hand(&mut engine, 0, "raise_the_alarm");
        }
        ensure_in_hand(&mut engine, 0, "cackling_counterpart");
        grant_pool(&mut engine, 0);
        (engine, source)
    }
    let (mut engine, source) = setup();
    let mut commands = Vec::new();
    let mut batches = Vec::new();
    for (card, expected_life) in [
        ("raise_the_alarm", 21),
        ("cackling_counterpart", 21),
        ("raise_the_alarm", 22),
        ("raise_the_alarm", 22),
    ] {
        let targets = if card == "cackling_counterpart" {
            target_object(source)
        } else {
            vec![]
        };
        let command = cast_spell(hand_index_for_card(&engine, 0, card), targets);
        batches.push(engine.apply_command(0, &command).unwrap());
        commands.push((0, command));
        while !engine.state.stack.is_empty() {
            assert!(
                engine.state.pending_trigger_order.is_none(),
                "caps suppress repeated entry occurrences before ordering"
            );
            for player in [0, 1] {
                let command = pass();
                batches.push(engine.apply_command(player, &command).unwrap());
                commands.push((player, command));
            }
        }
        assert_eq!(engine.state.players[0].life, expected_life);
    }
    assert_eq!(
        engine.state.trigger_uses_this_turn.len(),
        2,
        "original and copied watcher have independent allowances"
    );
    let (mut replay, _) = setup();
    let replayed = commands
        .iter()
        .map(|(player, command)| replay.apply_command(*player, command).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(replayed, batches);
    assert_eq!(
        replay.state.trigger_uses_this_turn,
        engine.state.trigger_uses_this_turn
    );
    assert_eq!(replay.state.command_index, engine.state.command_index);
    assert_eq!(replay.state.players[0].life, engine.state.players[0].life);
}

#[test]
fn token_copy_inline_ability_revalidates_targets_after_source_disappears() {
    let (mut engine, source) = token_copy_game("prodigal_sorcerer");
    let mut face = tricerules_cards::CardRegistry::global()
        .get("prodigal_sorcerer")
        .unwrap()
        .primary_face()
        .clone();
    face.name = "Inline Pinger".into();
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.card_id = "inline_pinger".into();
    object.token_origin = Some(tricerules_core::state::CopiableValues {
        source_card_id: String::new(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    let (copy, batch) = cast_copy(&mut engine, source);
    assert_eq!(
        token_created_events(&batch)[0]
            .identity
            .as_ref()
            .unwrap()
            .name,
        "Inline Pinger"
    );
    engine.state.objects.get_mut(&copy).unwrap().summoning_sick = false;
    apply_ability(&mut engine, 0, copy, 0, target_object(source)).unwrap();
    // A spell/ability on the stack must revalidate using its captured definition, without
    // looking up the runtime token in the registry or requiring its source to survive.
    engine.state.objects.remove(&copy);
    engine.state.players[0]
        .battlefield
        .retain(|oid| *oid != copy);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .token_origin
        .as_mut()
        .unwrap()
        .face
        .keywords
        .push(Keyword::Shroud);
    pass_both_players(&mut engine);
    assert!(
        engine.state.objects.contains_key(&source),
        "newly illegal target survives"
    );
}

#[test]
fn token_copy_inline_display_and_live_ability_survive_registry_absence() {
    let (mut engine, source) = token_copy_game("prodigal_sorcerer");
    let face = tricerules_cards::CardRegistry::global()
        .get("prodigal_sorcerer")
        .unwrap()
        .primary_face()
        .clone();
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.card_id = "inline_pinger".into();
    object.token_origin = Some(tricerules_core::state::CopiableValues {
        source_card_id: String::new(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    let (copy, batch) = cast_copy(&mut engine, source);
    let view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(sync)) => sync
                .per_player
                .iter()
                .flat_map(|p| &p.battlefield_objects)
                .find(|o| o.object_id == copy),
            _ => None,
        })
        .unwrap();
    assert_eq!(view.effective_display_name, "Prodigal Sorcerer");
    engine.state.objects.get_mut(&copy).unwrap().summoning_sick = false;
    apply_ability(&mut engine, 0, copy, 0, target_player(1)).unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[1].life, 19);
}

#[test]
fn token_copy_populate_rejects_wrong_empty_duplicate_and_stale_choices() {
    let (mut engine, source) = token_copy_game("serra_angel");
    let (copy, _) = cast_copy(&mut engine, source);
    begin_populate(&mut engine);
    let index = engine.state.command_index;
    for (player, chosen) in [
        (1, vec![copy]),
        (0, vec![]),
        (0, vec![source]),
        (0, vec![copy, copy]),
    ] {
        assert!(engine
            .apply_command(player, &submit_resolution_choice(chosen))
            .is_err());
        assert!(engine.state.pending_resolution.is_some());
        assert_eq!(engine.state.command_index, index);
    }
    *engine.state.zone_change_generation.entry(copy).or_default() += 1;
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![copy]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.players[0].battlefield.len(), 2);
}

#[test]
fn token_copy_populate_does_not_target_and_no_tokens_is_a_noop() {
    let (mut engine, source) = token_copy_game("serra_angel");
    begin_populate(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    let (copy, _) = cast_copy(&mut engine, source);
    engine
        .state
        .objects
        .get_mut(&copy)
        .unwrap()
        .token_origin
        .as_mut()
        .unwrap()
        .face
        .keywords
        .push(Keyword::Shroud);
    inject_library_card(&mut engine, 0, "wake_the_reflections");
    begin_populate(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![copy]
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![copy]))
        .unwrap();
    assert_eq!(engine.state.players[0].battlefield.len(), 3);
}

#[test]
fn token_copy_bounce_removes_only_the_token() {
    let (mut engine, source) = token_copy_game("serra_angel");
    let (copy, _) = cast_copy(&mut engine, source);
    ensure_in_hand(&mut engine, 0, "unsummon");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(copy)))
        .unwrap();
    pass_both_players(&mut engine);
    assert!(!engine.state.objects.contains_key(&copy));
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(!engine.state.players[0].hand.contains(&copy));
}

#[test]
fn token_copy_face_down_values_never_publish_the_hidden_identity() {
    let (mut engine, source) = token_copy_game("serra_angel");
    engine.state.objects.get_mut(&source).unwrap().face_down = true;
    let (copy, batch) = cast_copy(&mut engine, source);
    assert_eq!(engine.effective_power(copy), Some(2));
    assert!(!engine.state.objects[&copy].face_down);
    let token = token_created_events(&batch)[0];
    assert_eq!(token.card_id, "anonymous_creature_token");
    assert!(!format!("{token:?}").contains("serra"));
    assert!(token.identity.as_ref().unwrap().keywords.is_empty());
}

#[test]
fn token_copy_entry_replacement_is_reflected_in_the_creation_event() {
    let (mut engine, source) = token_copy_game("diregraf_ghoul");
    let (copy, batch) = cast_copy(&mut engine, source);
    assert!(engine.state.objects[&copy].tapped);
    assert!(token_created_events(&batch)[0].enters_tapped);
}

#[test]
fn token_copy_copied_etb_and_populate_etb_each_trigger_once() {
    let (mut engine, source) = token_copy_game("elvish_visionary");
    let (copy, _) = cast_copy(&mut engine, source);
    assert_eq!(engine.state.stack.len(), 1);
    assert!(engine.state.stack[0].is_triggered);
    let hand = engine.state.players[0].hand.len();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand + 1);
    begin_populate(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![copy]))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 1);
    let hand = engine.state.players[0].hand.len();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand + 1);
}

#[test]
fn token_copy_of_an_existing_copy_freezes_values_and_keeps_token_status() {
    let (mut engine, source) = token_copy_game("serra_angel");
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
    let (copy, batch) = cast_copy(&mut engine, source);
    assert_eq!(
        token_created_events(&batch)[0]
            .identity
            .as_ref()
            .unwrap()
            .name,
        "Grizzly Bears"
    );
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = None;
    assert_eq!(engine.effective_power(source), Some(4));
    assert_eq!(engine.effective_power(copy), Some(2));
    assert!(engine.state.objects[&copy].is_token());
}

#[test]
fn token_copy_populate_uses_controller_not_owner_with_three_seats() {
    let (mut engine, source) = token_copy_game("serra_angel");
    engine.state.objects.get_mut(&source).unwrap().owner = 1;
    let (copy, _) = cast_copy(&mut engine, source);
    assert_eq!(engine.state.objects[&copy].owner, 0);
    engine.state.objects.get_mut(&copy).unwrap().owner = 1;
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(7, 20));
    let third = inject_creature_on_battlefield(&mut engine, 2, "soldier_w_1_1");
    ensure_in_hand(&mut engine, 0, "wake_the_reflections");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "wake_the_reflections");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    for _ in 0..3 {
        let player = engine.state.priority_player_id();
        engine.apply_command(player, &pass()).unwrap();
    }
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![copy]
    );
    assert!(engine
        .apply_command(7, &submit_resolution_choice(vec![third]))
        .is_err());
    engine
        .apply_command(0, &submit_resolution_choice(vec![copy]))
        .unwrap();
    let copied = engine.state.players[0]
        .battlefield
        .iter()
        .find(|oid| **oid != source && **oid != copy)
        .unwrap();
    assert_eq!(engine.state.objects[copied].owner, 0);
}

#[test]
fn token_copy_target_that_leaves_before_resolution_creates_nothing() {
    let (mut engine, source) = token_copy_game("serra_angel");
    ensure_in_hand(&mut engine, 0, "cackling_counterpart");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "cackling_counterpart");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Graveyard;
    engine.state.players[0]
        .battlefield
        .retain(|oid| *oid != source);
    engine.state.players[0].graveyard.push(source);
    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_default() += 1;
    engine.apply_command(0, &pass()).unwrap();
    let batch = engine.apply_command(1, &pass()).unwrap();
    assert!(token_created_events(&batch).is_empty());
    assert!(engine.state.stack.is_empty());
}

fn resolving_clone_with_source(source_card_id: &str, seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            source_card_id.into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = put_creature_on_battlefield(&mut engine, 1, source_card_id);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let clone = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_command(0, &cast_spell(clone, vec![]))
        .expect("cast Clone");
    pass_both_players(&mut engine);
    (engine, source)
}

#[test]
fn clone_chooses_its_copy_source_during_resolution_not_casting() {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            "serra_angel".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut engine = GameEngine::new(45_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = put_creature_on_battlefield(&mut engine, 1, "serra_angel");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );

    let clone = hand_index_for_card(&engine, 0, "clone");
    let cast = engine
        .apply_command(0, &cast_spell(clone, vec![]))
        .expect("cast Clone without a target");
    assert!(
        find_resolution_choice(&cast).is_none(),
        "casting Clone must not choose what it will copy"
    );

    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Clone resolution must park before battlefield entry");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.min, 0);
    assert_eq!(pending.presentation.max, 1);
    assert_eq!(pending.presentation.candidates, vec![source]);
    assert!(matches!(
        &pending.continuation,
        ResolutionContinuation::EntryCopySource { .. }
    ));
    assert!(
        engine.state.players[0]
            .battlefield
            .iter()
            .all(|oid| engine.state.objects[oid].card_id != "clone"),
        "Clone is not committed to the battlefield until its entry choice finishes"
    );
}

#[test]
fn issue_175_clone_copies_the_life_gain_prohibition() {
    let (mut engine, source) = resolving_clone_with_source("giant_cindermaw", 175_501);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .unwrap();
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    assert_eq!(engine.effective_power(clone), Some(4));
    assert_eq!(engine.effective_toughness(clone), Some(3));
    // Leave the real copy as the only prohibition source, then activate an ordinary gain.
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 1,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        zone: DevZone::Graveyard as i32,
                        card_name: "Giant Cindermaw".into(),
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
    let gnomes = inject_creature_on_battlefield(&mut engine, 0, "bottle_gnomes");
    engine
        .apply_command(0, &activate_ability(gnomes, 0, vec![]))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
}

#[test]
fn countered_clone_never_emits_a_copy_source_choice() {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            "essence_scatter".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut engine = GameEngine::new(45_011, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );

    let clone = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_command(0, &cast_spell(clone, vec![]))
        .expect("cast Clone");
    let clone_spell = engine.state.stack.last().expect("Clone on stack").id;
    engine.apply_command(0, &pass()).expect("pass to opponent");
    let scatter = hand_index_for_card(&engine, 1, "essence_scatter");
    engine
        .apply_command(
            1,
            &cast_spell(
                scatter,
                vec![TargetRef {
                    object_id: clone_spell,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("counter Clone");
    resolve_entire_stack_two_player(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(count_card_id_in_graveyard(&engine, 0, "clone"), 1);
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .all(|oid| engine.state.objects[oid].card_id != "clone"));
}

#[test]
fn clone_copies_printed_values_but_not_source_status_counters_damage_or_pump() {
    let (mut engine, source) = resolving_clone_with_source("serra_angel", 45_002);
    {
        let source_object = engine.state.objects.get_mut(&source).expect("source");
        source_object.tapped = true;
        source_object.damage = 3;
        source_object.add_counters(CounterKind::PlusOnePlusOne, 2, 1);
        source_object.add_counters(CounterKind::Keyword(Keyword::Menace), 1, 2);
    }
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::PtModify {
            delta_power: 3,
            delta_toughness: 3,
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });

    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Serra Angel");
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    let object = &engine.state.objects[&clone];
    let values = object.copiable_values.as_ref().expect("copy snapshot");
    assert_eq!(values.face.name, "Serra Angel");
    assert_eq!(values.display_name, "Serra Angel");
    assert_eq!(values.source_card_id, "serra_angel");
    assert_eq!(object.copy_revision, 1);
    assert!(!object.tapped);
    assert_eq!(object.damage, 0);
    assert!(object.counters.is_empty());
    assert!(object.counter_timestamps.is_empty());

    let characteristics = engine
        .characteristics(clone)
        .expect("copied characteristics");
    assert_eq!(characteristics.power, Some(4));
    assert_eq!(characteristics.toughness, Some(4));
    assert!(characteristics.types.contains(&"Angel".to_string()));
    assert_eq!(characteristics.colors, vec![Color::White]);
    assert!(characteristics.has_keyword(Keyword::Flying));
    assert!(characteristics.has_keyword(Keyword::Vigilance));
    assert!(
        !characteristics.has_keyword(Keyword::Menace),
        "keyword counters are status, not copiable values"
    );

    engine
        .state
        .objects
        .get_mut(&clone)
        .expect("Clone")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);
    assert_eq!(engine.effective_power(clone), Some(5));
    assert_eq!(engine.effective_toughness(clone), Some(5));
}

#[test]
fn clone_copies_safewright_cavalrys_blocker_cap() {
    let (mut engine, source) = resolving_clone_with_source("safewright_cavalry", 160_008);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Safewright Cavalry");
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    assert!(matches!(
        engine.state.objects[&clone].copiable_values.as_ref().unwrap().face.static_abilities.as_slice(),
        [StaticAbilityDef::SelfCombatRestriction { restriction, .. }] if restriction.maximum_blockers == Some(1)
    ));
}

#[test]
fn conditional_characteristics_clone_inherits_the_copied_static_ability() {
    let (mut engine, source) = resolving_clone_with_source("gearsmith_guardian", 78_008);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Gearsmith Guardian");
    let clone = battlefield_object_for_card(&engine, 0, "clone");

    assert_eq!(engine.effective_power(clone), Some(3));
    inject_creature_on_battlefield(&mut engine, 0, "air_elemental");
    assert_eq!(
        engine.effective_power(clone),
        Some(5),
        "the copied conditional static ability tracks its new controller's battlefield"
    );
}

#[test]
fn clone_re_evaluates_entry_replacements_from_the_copied_face() {
    let (mut engine, source) = resolving_clone_with_source("diregraf_ghoul", 45_003);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Diregraf Ghoul");

    let clone = battlefield_object_for_card(&engine, 0, "clone");
    assert!(engine.state.objects[&clone].tapped);
    assert_eq!(engine.effective_power(clone), Some(2));
    assert_eq!(
        engine.state.objects[&clone]
            .copiable_values
            .as_ref()
            .expect("copy snapshot")
            .face
            .name,
        "Diregraf Ghoul"
    );
}

#[test]
fn clone_decline_finishes_entry_and_zero_toughness_sba() {
    let (mut engine, _) = resolving_clone_with_source("grizzly_bears", 45_004);
    let clone = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "clone")
        .expect("Clone object")
        .id;
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("decline copy");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&clone].zone, Zone::Graveyard);
    assert!(engine.state.objects[&clone].copiable_values.is_none());
    assert!(engine.state.players[0].graveyard.contains(&clone));
}

#[test]
fn stale_copy_source_is_rejected_without_clearing_the_choice() {
    let (mut engine, source) = resolving_clone_with_source("grizzly_bears", 45_005);
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != source);
    engine.state.objects.get_mut(&source).expect("source").zone = Zone::Graveyard;

    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .is_err());
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("stale answer preserves pending choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert_eq!(pending.presentation.candidates, vec![source]);
}

#[test]
fn copying_an_already_copied_clone_uses_its_layer_one_values() {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "clone".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            "serra_angel".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut engine = GameEngine::new(45_006, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let angel = put_creature_on_battlefield(&mut engine, 1, "serra_angel");

    for source in [angel, 0] {
        grant_pool(&mut engine, 0);
        let hand_clone = hand_index_for_card(&engine, 0, "clone");
        engine
            .apply_command(0, &cast_spell(hand_clone, vec![]))
            .expect("cast Clone");
        pass_both_players(&mut engine);
        let actual_source = if source == 0 {
            engine.state.players[0]
                .battlefield
                .iter()
                .copied()
                .find(|oid| engine.state.objects[oid].card_id == "clone")
                .expect("first Clone")
        } else {
            source
        };
        engine
            .apply_command(0, &submit_resolution_choice(vec![actual_source]))
            .expect("choose copy source");
    }

    let clones: Vec<_> = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|oid| engine.state.objects[oid].card_id == "clone")
        .collect();
    assert_eq!(clones.len(), 2);
    for clone in clones {
        let values = engine.state.objects[&clone]
            .copiable_values
            .as_ref()
            .expect("copy snapshot");
        assert_eq!(values.source_card_id, "serra_angel");
        assert_eq!(values.face.name, "Serra Angel");
        assert_eq!(engine.effective_power(clone), Some(4));
    }
}

#[test]
fn clone_can_copy_a_registry_backed_token() {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "raise_the_alarm".into(),
            "island".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(45_007, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    let alarm = hand_index_for_card(&engine, 0, "raise_the_alarm");
    engine
        .apply_command(0, &cast_spell(alarm, vec![]))
        .expect("cast Raise the Alarm");
    pass_both_players(&mut engine);
    let soldier = battlefield_token_oids(&engine, 0, "soldier_w_1_1")[0];

    grant_pool(&mut engine, 0);
    let clone = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_command(0, &cast_spell(clone, vec![]))
        .expect("cast Clone");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![soldier]))
        .expect("copy Soldier token");

    let clone = battlefield_object_for_card(&engine, 0, "clone");
    assert!(
        !engine.state.objects[&clone].is_token(),
        "copying a token does not make a card a token"
    );
    let values = engine.state.objects[&clone]
        .copiable_values
        .as_ref()
        .expect("copy snapshot");
    assert_eq!(values.source_card_id, "soldier_w_1_1");
    assert_eq!(values.face.name, "Soldier");
    assert_eq!(engine.effective_power(clone), Some(1));
    assert_eq!(engine.effective_toughness(clone), Some(1));
}

#[test]
fn copied_activated_ability_uses_the_effective_face() {
    let (mut engine, source) = resolving_clone_with_source("prodigal_sorcerer", 45_008);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Prodigal Sorcerer");
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    engine
        .state
        .objects
        .get_mut(&clone)
        .expect("Clone")
        .summoning_sick = false;

    apply_ability(&mut engine, 0, clone, 0, target_player(1)).expect("activate copied ability");
    let ability = engine.state.stack.last().expect("ability on stack");
    assert_eq!(ability.card_id, "prodigal_sorcerer");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[1].life, 19);
}

#[test]
fn copied_enters_trigger_is_put_on_the_stack_from_the_effective_face() {
    let (mut engine, source) = resolving_clone_with_source("elvish_visionary", 45_010);
    inject_library_card(&mut engine, 0, "forest");
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Elvish Visionary");

    let trigger = engine.state.stack.last().expect("copied ETB trigger");
    assert!(trigger.is_triggered);
    assert_eq!(trigger.card_id, "elvish_visionary");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
}

#[test]
fn leaving_the_battlefield_clears_copy_values_and_restores_clone() {
    let (mut engine, source) = resolving_clone_with_source("serra_angel", 45_009);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Serra Angel");
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    engine.enable_dev_commands();
    let command = RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 0,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                zone: DevZone::Graveyard as i32,
                card_name: "Clone".to_string(),
                ready: false,
            })),
        })),
    };
    engine.apply_command(0, &command).expect("move Clone");

    let object = &engine.state.objects[&clone];
    assert_eq!(object.zone, Zone::Graveyard);
    assert!(object.copiable_values.is_none());
    assert_eq!(object.copy_revision, 0);
    assert_eq!(engine.effective_power(clone), Some(0));
    assert_eq!(engine.effective_toughness(clone), Some(0));
}
