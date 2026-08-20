//! Issue #98: CR 701.40 / 708 face-down permanent characteristics and lifecycle.

use crate::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{AttachmentRecipient, Zone};
use tricerules_proto::ruled::v1::ruled_event::Ev;

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for &object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(object_id);
    }
    object_ids
}
#[test]
fn manifested_permanent_has_public_face_down_characteristics() {
    let decks = Some(vec![
        deck_with("forest", &["serra_angel"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(98_001, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let angel = relocate_to_battlefield(&mut engine, 0, "serra_angel", false);
    engine
        .state
        .objects
        .get_mut(&angel)
        .expect("angel")
        .face_down = true;

    let characteristics = engine.characteristics(angel).expect("face-down permanent");
    assert_eq!(characteristics.types, vec!["Creature"]);
    assert!(characteristics.supertypes.is_empty());
    assert!(characteristics.colors.is_empty());
    assert!(characteristics.keywords.is_empty());
    assert!(characteristics.protections.is_empty());
    assert!(characteristics.evasions.is_empty());
    assert_eq!(characteristics.power, Some(2));
    assert_eq!(characteristics.toughness, Some(2));
}

#[test]
fn controller_gets_generation_bound_turn_face_up_action_and_keeps_priority() {
    let decks = Some(vec![
        deck_with("plains", &["serra_angel", "flight"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(98_002, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let angel = relocate_to_battlefield(&mut engine, 0, "serra_angel", false);
    engine
        .state
        .objects
        .get_mut(&angel)
        .expect("angel")
        .face_down = true;
    let generation = engine
        .state
        .zone_change_generation
        .get(&angel)
        .copied()
        .unwrap_or(0);
    let aura = relocate_to_battlefield(&mut engine, 0, "flight", false);
    {
        let object = engine.state.objects.get_mut(&angel).expect("angel");
        object.tapped = true;
        object.damage = 1;
        object.counters.insert(CounterKind::PlusOnePlusOne, 1);
    }
    engine
        .state
        .objects
        .get_mut(&aura)
        .expect("aura")
        .attached_to = Some(AttachmentRecipient::Object(angel));

    let legal = engine.initial_response_batch();
    let action = legal.legal_by_player[&0]
        .permanent_actions
        .iter()
        .find(|action| action.object_id == angel)
        .expect("controller action");
    assert_eq!(action.zone_change_generation, generation);
    assert_eq!(action.mana_cost, "{3}{W}{W}");
    assert!(legal.legal_by_player[&1].permanent_actions.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 2,
            c: 3,
            ..Default::default()
        },
    );
    let pool_before = engine.state.players[0].mana_pool;
    let tampered = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::TurnFaceUp(TurnFaceUp {
                    object_id: angel,
                    expected_zone_change_generation: generation,
                    flex_payments: vec![],
                    restricted_mana: vec![ManaSpendSelection {
                        restriction_group_id: 999,
                        w: 1,
                        ..Default::default()
                    }],
                })),
            },
        )
        .expect_err("restricted mana is not eligible for this special action");
    assert!(tampered.to_string().contains("restricted"));
    let pool_after = &engine.state.players[0].mana_pool;
    assert_eq!(
        (
            pool_after.white,
            pool_after.blue,
            pool_after.black,
            pool_after.red,
            pool_after.green,
            pool_after.colorless,
        ),
        (
            pool_before.white,
            pool_before.blue,
            pool_before.black,
            pool_before.red,
            pool_before.green,
            pool_before.colorless,
        )
    );
    assert!(engine.state.objects[&angel].face_down);
    let batch = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::TurnFaceUp(TurnFaceUp {
                    object_id: angel,
                    expected_zone_change_generation: generation,
                    flex_payments: vec![],
                    restricted_mana: vec![],
                })),
            },
        )
        .expect("turn face up");

    assert!(!engine.state.objects[&angel].face_down);
    assert!(
        engine.state.stack.is_empty(),
        "special action does not use stack"
    );
    assert_eq!(engine.state.priority_player_id(), 0, "priority is retained");
    assert_eq!(engine.effective_power(angel), Some(5));
    assert_eq!(engine.effective_toughness(angel), Some(5));
    assert!(engine.effective_has_keyword(angel, Keyword::Flying));
    let object = &engine.state.objects[&angel];
    assert!(object.tapped);
    assert_eq!(object.damage, 1);
    assert_eq!(object.counter_count(CounterKind::PlusOnePlusOne), 1);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(angel))
    );
    assert!(batch.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(Ev::FaceChanged(changed)) if changed.object_id == angel && !changed.face_down
    )));
    let zone_view = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("turn-face-up batch carries a zone view");
    assert!(
        !zone_view.battlefields_unchanged,
        "face-down state changes must invalidate the battlefield snapshot"
    );
    let published = zone_view.per_player[0]
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == angel)
        .expect("turned permanent is republished");
    assert!(!published.face_down);
    assert_eq!((published.power, published.toughness), (5, 5));
}

#[test]
fn manifest_dread_completes_with_one_or_zero_cards_without_a_choice() {
    for (seed, remaining) in [(98_007, 1usize), (98_008, 0usize)] {
        let decks = Some(vec![
            deck_with("forest", &["manifest_dread"]),
            deck_with("swamp", &[]),
        ]);
        let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
        advance_to_main1_from_game_start(&mut engine);
        ensure_card_in_hand(&mut engine, 0, "manifest_dread");
        let kept = (remaining == 1).then(|| inject_library_card(&mut engine, 0, "serra_angel"));
        let library: Vec<_> = engine.state.players[0].library.iter().copied().collect();
        for object_id in library {
            if Some(object_id) != kept {
                engine.state.players[0]
                    .library
                    .retain(|candidate| *candidate != object_id);
                engine.state.players[0].graveyard.push(object_id);
                engine
                    .state
                    .objects
                    .get_mut(&object_id)
                    .expect("library card")
                    .zone = Zone::Graveyard;
            }
        }
        if let Some(object_id) = kept {
            engine.state.players[0]
                .library
                .retain(|candidate| *candidate != object_id);
            engine.state.players[0].library.push_front(object_id);
        }
        give_mana(
            &mut engine,
            0,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "manifest_dread");
        engine
            .apply_command(0, &cast_spell(slot, vec![]))
            .expect("cast");
        engine.apply_command(0, &pass()).expect("caster pass");
        let completed = engine.apply_command(1, &pass()).expect("resolve");

        assert!(find_resolution_choice(&completed).is_none());
        assert!(engine.state.pending_resolution.is_none());
        if let Some(object_id) = kept {
            assert_eq!(engine.state.objects[&object_id].zone, Zone::Battlefield);
            assert!(engine.state.objects[&object_id].face_down);
        }
    }
}

#[test]
fn leaving_the_battlefield_resets_face_down_and_changes_generation() {
    let decks = Some(vec![
        deck_with("swamp", &["murder", "serra_angel"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(98_009, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let angel = relocate_to_battlefield(&mut engine, 0, "serra_angel", false);
    engine
        .state
        .objects
        .get_mut(&angel)
        .expect("angel")
        .face_down = true;
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&angel)
        .copied()
        .unwrap_or(0);
    ensure_card_in_hand(&mut engine, 0, "murder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "murder");
    engine
        .apply_command(0, &cast_spell(slot, target_object(angel)))
        .expect("cast Murder");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&angel].zone, Zone::Graveyard);
    assert!(!engine.state.objects[&angel].face_down);
    assert!(engine.state.zone_change_generation[&angel] > old_generation);
}

#[test]
fn stale_or_noncreature_turn_face_up_action_is_not_legal() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(98_003, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let bolt = relocate_to_battlefield(&mut engine, 0, "lightning_bolt", false);
    engine.state.objects.get_mut(&bolt).expect("bolt").face_down = true;
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .permanent_actions
        .is_empty());
    let err = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::TurnFaceUp(TurnFaceUp {
                    object_id: bolt,
                    expected_zone_change_generation: 999,
                    flex_payments: vec![],
                    restricted_mana: vec![],
                })),
            },
        )
        .expect_err("stale command");
    assert!(err.to_string().contains("stale"));
    assert!(engine.state.objects[&bolt].face_down);
}

#[test]
fn manifest_dread_privately_chooses_one_top_card_and_moves_both_exact_objects() {
    let decks = Some(vec![
        deck_with("forest", &["manifest_dread"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(98_004, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "manifest_dread");
    let top = seat_on_top(&mut engine, 0, &["grizzly_bears", "lightning_bolt"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "manifest_dread");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Manifest Dread");
    engine.apply_command(0, &pass()).expect("caster pass");
    let parked = engine.apply_command(1, &pass()).expect("resolve to choice");
    let choice = find_resolution_choice(&parked).expect("private top-two choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::ManifestDread);
    assert_eq!(choice.candidate_object_ids, top);
    assert_eq!((choice.min, choice.max), (1, 1));

    let completed = engine
        .apply_command(0, &submit_resolution_choice(vec![top[1]]))
        .expect("choose second card to manifest");
    assert_eq!(
        engine.state.objects[&top[1]].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.objects[&top[1]].face_down);
    assert_eq!(
        engine.state.objects[&top[0]].zone,
        tricerules_core::Zone::Graveyard
    );
    let moved: Vec<_> = completed
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::PermanentMoved(moved))
                if moved.object_id == top[0] || moved.object_id == top[1] =>
            {
                Some(moved)
            }
            _ => None,
        })
        .collect();
    assert_eq!(moved.len(), 2);
    assert_eq!(moved[0].object_id, top[1]);
    assert_eq!(moved[0].source_library_position, Some(1));
    assert!(moved[0].face_down);
    assert_eq!(moved[1].object_id, top[0]);
    assert_eq!(moved[1].source_library_position, Some(0));
}

#[test]
fn unable_to_scream_applies_layers_and_prohibits_turn_face_up_until_it_leaves() {
    let decks = Some(vec![
        deck_with("island", &["unable_to_scream", "serra_angel"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(98_005, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "unable_to_scream");
    let angel = relocate_to_battlefield(&mut engine, 0, "serra_angel", false);
    engine
        .state
        .objects
        .get_mut(&angel)
        .expect("angel")
        .face_down = true;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "unable_to_scream");
    engine
        .apply_command(0, &cast_spell(slot, target_object(angel)))
        .expect("cast Unable to Scream");
    resolve_entire_stack_two_player(&mut engine);

    let aura = battlefield_object_for_card(&engine, 0, "unable_to_scream");
    let toy = engine.characteristics(angel).expect("enchanted manifest");
    assert!(toy.has_type("Creature"));
    assert!(toy.has_type("Artifact"));
    assert!(toy.has_type("Toy"));
    assert_eq!((toy.power, toy.toughness), (Some(0), Some(2)));
    assert!(toy.keywords.is_empty());
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .permanent_actions
        .is_empty());

    engine.state.objects.get_mut(&aura).expect("aura").zone = tricerules_core::Zone::Graveyard;
    let restored = engine
        .characteristics(angel)
        .expect("manifest after aura leaves");
    assert_eq!((restored.power, restored.toughness), (Some(2), Some(2)));
    assert!(!restored.has_type("Artifact"));
    assert!(!restored.has_type("Toy"));
    assert_eq!(
        engine.initial_response_batch().legal_by_player[&0]
            .permanent_actions
            .len(),
        1
    );
}

#[test]
fn turn_inside_out_watches_the_exact_generation_once() {
    let decks = Some(vec![
        deck_with("mountain", &["turn_inside_out", "murder", "grizzly_bears"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(98_006, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    ensure_card_in_hand(&mut engine, 0, "turn_inside_out");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let pump_slot = hand_index_for_card(&engine, 0, "turn_inside_out");
    engine
        .apply_command(0, &cast_spell(pump_slot, target_object(bear)))
        .expect("cast Turn Inside Out");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(5));
    assert_eq!(engine.state.active_delayed_triggers.len(), 1);

    let top = seat_on_top(&mut engine, 0, &["serra_angel", "lightning_bolt"]);
    ensure_card_in_hand(&mut engine, 0, "murder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder_slot = hand_index_for_card(&engine, 0, "murder");
    engine
        .apply_command(0, &cast_spell(murder_slot, target_object(bear)))
        .expect("cast Murder");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.active_delayed_triggers.len(),
        0,
        "consumed once"
    );
    let parked = {
        engine
            .apply_command(0, &pass())
            .expect("trigger controller pass");
        engine
            .apply_command(1, &pass())
            .expect("resolve delayed trigger")
    };
    let choice = find_resolution_choice(&parked).expect("manifest-dread choice");
    assert_eq!(choice.candidate_object_ids, top);
}

#[test]
fn bashful_beastie_and_innocuous_rat_death_triggers_manifest_dread() {
    for (seed, creature_id, basic) in [
        (98_010, "bashful_beastie", "forest"),
        (98_011, "innocuous_rat", "swamp"),
    ] {
        let decks = Some(vec![
            deck_with(basic, &[creature_id, "murder"]),
            deck_with("plains", &[]),
        ]);
        let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
        advance_to_main1_from_game_start(&mut engine);
        let creature = relocate_to_battlefield(&mut engine, 0, creature_id, false);
        let top = seat_on_top(&mut engine, 0, &["serra_angel", "lightning_bolt"]);
        ensure_card_in_hand(&mut engine, 0, "murder");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "murder");
        engine
            .apply_command(0, &cast_spell(slot, target_object(creature)))
            .expect("cast Murder");
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &pass())
            .expect("trigger controller pass");
        let parked = engine
            .apply_command(1, &pass())
            .expect("resolve death trigger");

        assert_eq!(
            find_resolution_choice(&parked)
                .expect("manifest choice")
                .candidate_object_ids,
            top
        );
    }
}

#[test]
fn twist_reality_supports_both_counter_and_manifest_modes() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        deck_with("island", &["twist_reality"]),
    ]);
    let mut engine = GameEngine::new(98_012, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "lightning_bolt");
    ensure_card_in_hand(&mut engine, 1, "twist_reality");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    let bolt_slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt_slot, target_player(1)))
        .expect("cast Bolt");
    let bolt = engine.state.stack.last().expect("Bolt on stack").id;
    engine.apply_command(0, &pass()).expect("caster pass");
    let twist_slot = hand_index_for_card(&engine, 1, "twist_reality");
    engine
        .apply_command(
            1,
            &cast_modal_spell(twist_slot, vec![(0, target_object(bolt))]),
        )
        .expect("cast counter mode");
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.players[0].graveyard.contains(&bolt));

    let decks = Some(vec![
        deck_with("island", &["twist_reality"]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(98_013, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "twist_reality");
    let top = seat_on_top(&mut engine, 0, &["serra_angel", "lightning_bolt"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "twist_reality");
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, vec![])]))
        .expect("cast manifest mode");
    engine.apply_command(0, &pass()).expect("caster pass");
    let parked = engine
        .apply_command(1, &pass())
        .expect("resolve manifest mode");
    assert_eq!(
        find_resolution_choice(&parked)
            .expect("manifest choice")
            .candidate_object_ids,
        top
    );
}
