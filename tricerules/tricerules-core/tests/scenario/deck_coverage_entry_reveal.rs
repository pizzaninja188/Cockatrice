use super::helpers::*;
use tricerules_cards::primitives::ContinuousEffectKind;
use tricerules_cards::primitives::{EntersTappedAffected, EntryCost, StaticAbilityDef};
use tricerules_cards::{AbilityCost, AbilityPresentation, ZoneCardFilter};
use tricerules_cards::{
    ControllerReference, EffectDuration, PermanentTypeFilter, TypeLineAddition,
};
use tricerules_core::{AffectedScope, ContinuousEffect, GameEngine, Zone};
use tricerules_proto::ruled::v1::{self as rv1, ruled_command::Cmd};

fn setup(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("island", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn illegal(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = format!("{:?}", engine.state);
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(
        format!("{:?}", engine.state),
        before,
        "rejection preserves the complete parked state"
    );
}

fn play(engine: &mut GameEngine, card: &str) -> (u32, RuledEventBatch) {
    let oid = inject_card_into_hand(engine, 0, card);
    let command = play_land(hand_index_for_card(engine, 0, card));
    (oid, engine.apply_command(0, &command).unwrap())
}

#[test]
fn entry_reveal_pair_game_trail_and_bosk_complete_definitions_are_registered() {
    let registry = tricerules_cards::CardRegistry::global();
    for (card, name) in [
        ("game_trail", "Game Trail"),
        ("murmuring_bosk", "Murmuring Bosk"),
    ] {
        let definition = registry
            .get(card)
            .expect("complete entry-reveal card definition");
        assert_eq!(definition.name, name);
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert!(face.supertypes.is_empty());
        assert_eq!(face.mana_cost.to_string(), "");
        assert_eq!(
            face.types,
            if card == "game_trail" {
                vec!["Land"]
            } else {
                vec!["Land", "Forest"]
            }
        );
        assert_eq!(face.static_abilities.len(), 1);
        let entry = &face.static_abilities[0];
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![if card == "game_trail" { 1 } else { 2 }])
        );
        let StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: None,
            unless_cost: Some(EntryCost::RevealFromHand { filter }),
        } = &entry.definition
        else {
            panic!("complete entry reveal");
        };
        let expected = if card == "game_trail" {
            ZoneCardFilter {
                any_of: Some(vec![
                    ZoneCardFilter {
                        required_subtypes: vec!["Mountain".into()],
                        ..Default::default()
                    },
                    ZoneCardFilter {
                        required_subtypes: vec!["Forest".into()],
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }
        } else {
            ZoneCardFilter {
                required_subtypes: vec!["Treefolk".into()],
                ..Default::default()
            }
        };
        assert_eq!(filter, &expected);
        assert_eq!(
            face.activated_abilities.len(),
            if card == "game_trail" { 1 } else { 2 }
        );
        for ability in &face.activated_abilities {
            assert_eq!(ability.costs, vec![AbilityCost::Tap]);
        }
        if card == "murmuring_bosk" {
            assert!(face.activated_abilities[0].intrinsic_land_mana);
        }
    }
}

#[test]
fn entry_reveal_pair_printed_subtype_filters_accept_nonbasic_or_and_noncreature_changeling() {
    for (card, candidate, eligible) in [
        ("game_trail", "forest", true),
        ("game_trail", "mountain", true),
        ("game_trail", "canopy_vista", true),
        ("game_trail", "murmuring_bosk", true),
        ("game_trail", "game_trail", false),
        ("game_trail", "karplusan_forest", false),
        ("murmuring_bosk", "firdoch_core", true),
        ("murmuring_bosk", "blighted_blackthorn", true),
        ("murmuring_bosk", "grizzly_bears", false),
    ] {
        let mut engine = setup(614_130);
        let source = inject_card_into_hand(&mut engine, 0, card);
        let candidate = inject_card_into_hand(&mut engine, 0, candidate);
        let command = play_land(
            engine.state.players[0]
                .hand
                .iter()
                .position(|oid| *oid == source)
                .unwrap(),
        );
        let offered = engine.apply_command(0, &command).unwrap();
        if eligible {
            let choice = find_resolution_choice(&offered).expect("qualifying printed hand card");
            assert_eq!(choice.candidate_object_ids, vec![candidate]);
            let generation = semantic::generation(&engine, candidate);
            let batch = engine
                .apply_command(0, &submit_resolution_choice(vec![candidate]))
                .unwrap();
            let revealed: Vec<_> = batch
                .events
                .iter()
                .filter_map(|event| match &event.ev {
                    Some(Ev::CardsRevealed(reveal)) => Some(reveal),
                    _ => None,
                })
                .collect();
            assert_eq!(revealed.len(), 1);
            assert_eq!(revealed[0].cards.len(), 1);
            assert_eq!(revealed[0].cards[0].object_id, candidate);
            assert_eq!(revealed[0].cards[0].zone_change_generation, generation);
            assert_eq!(semantic::generation(&engine, candidate), generation);
            assert!(!engine.state.objects[&source].tapped);
        } else {
            let choice = find_resolution_choice(&offered)
                .expect("empty eligibility still offers a private choice");
            assert_eq!((choice.min, choice.max), (0, 1));
            assert!(choice.candidate_object_ids.is_empty());
            engine
                .apply_command(0, &submit_resolution_choice(vec![]))
                .unwrap();
            assert!(engine.state.objects[&source].tapped);
        }
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&candidate].zone, Zone::Hand);
    }
}

#[test]
fn entry_reveal_pair_decline_and_empty_hand_match_enter_tapped_without_revealing() {
    for card in ["game_trail", "murmuring_bosk"] {
        for candidate in [
            None,
            Some(if card == "game_trail" {
                "forest"
            } else {
                "firdoch_core"
            }),
        ] {
            let mut engine = setup(614_140);
            if let Some(candidate) = candidate {
                inject_card_into_hand(&mut engine, 0, candidate);
            }
            let (source, offered) = play(&mut engine, card);
            let choice = find_resolution_choice(&offered)
                .expect("prompt existence must not reveal eligibility");
            assert_eq!((choice.min, choice.max), (0, 1));
            assert_eq!(
                choice.candidate_object_ids.len(),
                usize::from(candidate.is_some())
            );
            assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
            let final_batch = engine
                .apply_command(0, &submit_resolution_choice(vec![]))
                .unwrap();
            assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
            assert!(engine.state.objects[&source].tapped);
            assert!(final_batch
                .events
                .iter()
                .all(|event| !matches!(event.ev, Some(Ev::CardsRevealed(_)))));
        }
    }
}

#[test]
fn entry_reveal_pair_wrong_actor_identity_and_extra_payloads_reject_atomically_then_retry() {
    let mut engine = setup(614_150);
    let forest = inject_card_into_hand(&mut engine, 0, "forest");
    let other = inject_card_into_hand(&mut engine, 1, "forest");
    let bad_card = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let (source, _) = play(&mut engine, "game_trail");
    let valid = submit_resolution_choice(vec![forest]);
    illegal(&mut engine, 1, &valid);
    for selected in [
        vec![other],
        vec![bad_card],
        vec![source],
        vec![forest, forest],
        vec![u32::MAX],
    ] {
        illegal(&mut engine, 0, &submit_resolution_choice(selected));
    }
    for variant in 0..9 {
        let mut command = valid.clone();
        let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() else {
            unreachable!()
        };
        match variant {
            0 => answer.decision = rv1::ResolutionChoiceDecision::Decline as i32,
            1 => answer.selected_branch_index = 1,
            2 => answer.payment = Some(Default::default()),
            3 => answer.restricted_mana.push(Default::default()),
            4 => answer.cast_spell = Some(Default::default()),
            5 => answer.spell_cast_announcement = Some(Default::default()),
            6 => answer.chosen_combat_defender = Some(Default::default()),
            7 => answer.chosen_player_ids.push(0),
            _ => answer.decision = i32::MAX,
        }
        illegal(&mut engine, 0, &command);
    }
    engine.apply_command(0, &valid).unwrap();
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Hand);
}

#[test]
fn entry_reveal_pair_stale_generations_and_copy_revision_reject_without_clearing_choice() {
    for stale in 0..3 {
        let mut engine = setup(614_160);
        let forest = inject_card_into_hand(&mut engine, 0, "forest");
        let (source, _) = play(&mut engine, "game_trail");
        if stale < 2 {
            let oid = if stale == 0 { forest } else { source };
            let generation = semantic::generation(&engine, oid);
            // Simulate an external departure/return: the exact proposed/candidate incarnation is stale.
            engine
                .state
                .zone_change_generation
                .insert(oid, generation + 2);
        } else {
            engine.state.objects.get_mut(&source).unwrap().copy_revision += 1;
        }
        illegal(&mut engine, 0, &submit_resolution_choice(vec![forest]));
        assert!(engine.state.pending_resolution.is_some());
        assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
        assert_eq!(engine.state.objects[&forest].zone, Zone::Hand);
    }
}

#[test]
fn entry_reveal_pair_stale_life_branch_cannot_answer_a_reveal_cost() {
    let mut engine = setup(614_161);
    let (source, _) = play(&mut engine, "watery_grave");
    // Corrupt the parked effect's definition without changing its index: branch and hand
    // continuations must remain separate even when stale state now names the new cost kind.
    engine.state.objects.get_mut(&source).unwrap().card_id = "game_trail".into();
    let mut answer = submit_resolution_choice(vec![]);
    let Some(Cmd::SubmitResolutionChoice(choice)) = answer.cmd.as_mut() else {
        unreachable!()
    };
    choice.decision = rv1::ResolutionChoiceDecision::SelectBranch as i32;
    illegal(&mut engine, 0, &answer);
}

#[test]
fn entry_reveal_pair_forced_tapping_is_never_cleared_in_either_replacement_order() {
    for own_first in [false, true] {
        let mut engine = setup(614_170);
        inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
        let forest = inject_card_into_hand(&mut engine, 0, "forest");
        let (source, batch) = play(&mut engine, "game_trail");
        let order = find_resolution_choice(&batch).unwrap();
        assert_eq!(order.choice_kind(), rv1::ChoiceKind::ReplacementEffect);
        let index = order
            .candidate_names
            .iter()
            .position(|name| {
                name.starts_with(if own_first {
                    "Game Trail"
                } else {
                    "Orb of Dreams"
                })
            })
            .unwrap();
        engine
            .apply_command(
                0,
                &submit_resolution_choice(vec![order.candidate_object_ids[index]]),
            )
            .unwrap();
        engine
            .apply_command(0, &submit_resolution_choice(vec![forest]))
            .unwrap();
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert!(engine.state.objects[&source].tapped);
    }
}

#[test]
fn entry_reveal_pair_bosk_and_trail_mana_options_are_immediate_and_damage_only_bosk_white_black() {
    for (card, ability, option, expected, life) in [
        ("game_trail", 0, 0, [0, 0, 0, 1, 0, 0], 20),
        ("game_trail", 0, 1, [0, 0, 0, 0, 1, 0], 20),
        ("murmuring_bosk", 0, 0, [0, 0, 0, 0, 1, 0], 20),
        ("murmuring_bosk", 1, 0, [1, 0, 0, 0, 0, 0], 19),
        ("murmuring_bosk", 1, 1, [0, 0, 1, 0, 0, 0], 19),
    ] {
        let mut engine = setup(614_180);
        inject_card_into_hand(
            &mut engine,
            0,
            if card == "game_trail" {
                "forest"
            } else {
                "firdoch_core"
            },
        );
        let (source, offered) = play(&mut engine, card);
        let selected = find_resolution_choice(&offered)
            .unwrap()
            .candidate_object_ids[0];
        engine
            .apply_command(0, &submit_resolution_choice(vec![selected]))
            .unwrap();
        let mut command = activate_ability_for(&engine, source, ability, vec![]);
        let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
            unreachable!()
        };
        activation.mana_option_index = option;
        illegal(&mut engine, 1, &command);
        let mut invalid = command.clone();
        let Some(Cmd::ActivateAbility(activation)) = invalid.cmd.as_mut() else {
            unreachable!()
        };
        activation.mana_option_index = 99;
        illegal(&mut engine, 0, &invalid);
        let batch = engine.apply_command(0, &command).unwrap();
        let pool = &engine.state.players[0].mana_pool;
        assert_eq!(
            [
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ],
            expected
        );
        assert_eq!(engine.state.players[0].life, life);
        assert_eq!(engine.state.players[1].life, 20);
        assert!(engine.state.objects[&source].tapped);
        assert!(engine.state.stack.is_empty());
        illegal(&mut engine, 0, &command);
        if life == 19 {
            assert_eq!(batch.legal_by_player[&0].undoable_mana_abilities, 0);
            illegal(&mut engine, 0, &undo_mana_ability());
        }
    }
}

#[test]
fn entry_reveal_pair_bosk_colored_mana_damage_uses_prevention_pipeline() {
    let mut engine = setup(614_190);
    let treefolk = inject_card_into_hand(&mut engine, 0, "firdoch_core");
    let (source, _) = play(&mut engine, "murmuring_bosk");
    engine
        .apply_command(0, &submit_resolution_choice(vec![treefolk]))
        .unwrap();
    engine.state.add_damage_prevention_shield(0, 1);
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 1, vec![]))
        .unwrap();
    assert_eq!(engine.state.players[0].mana_pool.white, 1);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.remaining_damage_prevention(0), 0);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn entry_reveal_pair_game_trail_reveals_a_forest_without_moving_it() {
    let mut engine = GameEngine::new(
        614_121,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_card_into_hand(&mut engine, 0, "game_trail");
    let forest = inject_card_into_hand(&mut engine, 0, "forest");
    let played = engine
        .apply_command(0, &play_land(hand_index_for_card(&engine, 0, "game_trail")))
        .unwrap();
    let choice = find_resolution_choice(&played).expect("private entry reveal offer");
    assert_eq!(
        choice.choice_kind(),
        tricerules_proto::ruled::v1::ChoiceKind::HandCards
    );
    assert_eq!(choice.candidate_object_ids, vec![forest]);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.public_reveal.is_none());
    engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .unwrap();
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&land].tapped);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Hand);
}

#[test]
fn entry_reveal_pair_bosk_accepts_literal_noncreature_kindred_treefolk() {
    let registry = tricerules_cards::CardRegistry::global();
    let StaticAbilityDef::EntersTapped {
        unless_cost: Some(EntryCost::RevealFromHand { filter }),
        ..
    } = &registry
        .get("murmuring_bosk")
        .unwrap()
        .primary_face()
        .static_abilities[0]
        .definition
    else {
        unreachable!()
    };
    // Definition-only predicate fixture; no synthetic card is admitted to the registry.
    let mut literal = registry.get("eyeblights_ending").unwrap().clone();
    literal.faces[0].types = vec!["Kindred".into(), "Instant".into(), "Treefolk".into()];
    literal.faces[0].characteristic_defining_abilities.clear();
    assert!(!literal.primary_face().is_creature);
    assert!(literal.matches_zone_card_filter(filter));
    literal.faces[0].types.retain(|value| value != "Treefolk");
    assert!(!literal.matches_zone_card_filter(filter));
}

#[test]
fn entry_reveal_pair_actual_clone_rechecks_copied_entry_and_does_not_copy_animation() {
    for (card, matching) in [("game_trail", "forest"), ("murmuring_bosk", "firdoch_core")] {
        let mut engine = setup(614_200);
        let candidate = inject_card_into_hand(&mut engine, 0, matching);
        let generation = semantic::generation(&engine, candidate);
        let (land, _) = play(&mut engine, card);
        engine
            .apply_command(0, &submit_resolution_choice(vec![]))
            .unwrap();
        for kind in [
            ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                land_types: vec![],
                card_types: vec![PermanentTypeFilter::Creature],
                creature_types: vec!["Shapeshifter".into()],
            }),
            ContinuousEffectKind::Layer7bSetPt {
                power: 2,
                toughness: 2,
            },
        ] {
            engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(land),
                kind,
                condition: None,
                duration: EffectDuration::Indefinite,
                timestamp: engine.state.command_index,
            });
        }
        let clone = inject_card_into_hand(&mut engine, 0, "clone");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        );
        engine
            .apply_command(
                0,
                &cast_spell(hand_index_for_card(&engine, 0, "clone"), vec![]),
            )
            .unwrap();
        pass_both_players(&mut engine);
        let copy = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(copy.presentation.choice_kind, ChoiceKind::CopySource);
        assert!(copy.presentation.candidates.contains(&land));
        let offered = engine
            .apply_command(0, &submit_resolution_choice(vec![land]))
            .unwrap();
        let reveal = find_resolution_choice(&offered).unwrap();
        assert_eq!(reveal.choice_kind(), ChoiceKind::HandCards);
        assert_eq!(reveal.source_object_id, clone);
        assert_eq!(reveal.candidate_object_ids, vec![candidate]);
        engine
            .apply_command(0, &submit_resolution_choice(vec![candidate]))
            .unwrap();
        assert_eq!(engine.state.objects[&clone].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&clone].copy_revision, 1);
        assert!(!engine.state.objects[&clone].tapped);
        assert!(!engine.characteristics(clone).unwrap().has_type("Creature"));
        assert!(engine.state.objects[&land].tapped);
        assert_eq!(engine.state.objects[&candidate].zone, Zone::Hand);
        assert_eq!(semantic::generation(&engine, candidate), generation);
    }
}

#[test]
fn entry_reveal_pair_real_forest_search_parks_bosk_before_entry_and_shuffle() {
    for spell in ["natures_lore", "three_visits"] {
        for reveal in [false, true] {
            let mut engine = setup(614_210);
            let bosk = inject_library_card(&mut engine, 0, "murmuring_bosk");
            let candidate = inject_card_into_hand(&mut engine, 0, "firdoch_core");
            let candidate_generation = semantic::generation(&engine, candidate);
            inject_card_into_hand(&mut engine, 0, spell);
            give_mana(
                &mut engine,
                0,
                ManaGift {
                    g: 1,
                    c: 1,
                    ..Default::default()
                },
            );
            engine
                .apply_command(
                    0,
                    &cast_spell(hand_index_for_card(&engine, 0, spell), vec![]),
                )
                .unwrap();
            engine.apply_command(0, &pass()).unwrap();
            let offered = engine.apply_command(1, &pass()).unwrap();
            let search = find_resolution_choice(&offered).unwrap();
            assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
            assert!(search.candidate_object_ids.contains(&bosk));
            let library = engine.state.players[0].library.clone();
            let generation = semantic::generation(&engine, bosk);
            let parked = engine
                .apply_command(0, &submit_resolution_choice(vec![bosk]))
                .unwrap();
            assert_eq!(
                find_resolution_choice(&parked).unwrap().choice_kind(),
                ChoiceKind::HandCards
            );
            assert_eq!(engine.state.objects[&bosk].zone, Zone::Library);
            assert_eq!(semantic::generation(&engine, bosk), generation);
            assert_eq!(engine.state.players[0].library, library);
            assert!(!parked.events.iter().any(|event| matches!(&event.ev,
                Some(Ev::PermanentMoved(moved)) if moved.object_id == bosk)));
            assert!(!parked.events.iter().any(|event| matches!(&event.ev,
                Some(Ev::Log(log)) if log.text.contains("shuffles their library"))));
            let completion = engine
                .apply_command(
                    0,
                    &submit_resolution_choice(if reveal { vec![candidate] } else { vec![] }),
                )
                .unwrap();
            assert_eq!(engine.state.objects[&bosk].zone, Zone::Battlefield);
            assert_eq!(semantic::generation(&engine, bosk), generation + 1);
            assert_eq!(engine.state.objects[&bosk].tapped, !reveal);
            assert_eq!(engine.state.objects[&candidate].zone, Zone::Hand);
            assert_eq!(
                semantic::generation(&engine, candidate),
                candidate_generation
            );
            let movement = completion.events.iter().position(|event| matches!(&event.ev,
                Some(Ev::PermanentMoved(moved)) if moved.object_id == bosk && moved.destination == rv1::permanent_moved::Destination::Battlefield as i32)).unwrap();
            let shuffle = completion
                .events
                .iter()
                .position(|event| {
                    matches!(&event.ev,
                Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
                })
                .unwrap();
            assert!(movement < shuffle);
            assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
        }
    }
}

#[test]
fn entry_reveal_pair_serialized_accepted_commands_and_batches_replay_exactly() {
    use prost::Message;
    for (card, matching) in [("game_trail", "forest"), ("murmuring_bosk", "firdoch_core")] {
        for reveal in [false, true] {
            let fresh = || {
                let mut engine = setup(614_220);
                let candidate = inject_card_into_hand(&mut engine, 0, matching);
                let source = inject_card_into_hand(&mut engine, 0, card);
                (engine, candidate, source)
            };
            let (mut engine, candidate, source) = fresh();
            let mut recorded = vec![];
            let command = play_land(
                engine.state.players[0]
                    .hand
                    .iter()
                    .position(|oid| *oid == source)
                    .unwrap(),
            );
            let batch = engine.apply_command(0, &command).unwrap();
            recorded.push((command, batch));
            let command = submit_resolution_choice(if reveal { vec![candidate] } else { vec![] });
            let index = engine.state.command_index;
            illegal(&mut engine, 1, &command);
            assert_eq!(engine.state.command_index, index);
            let batch = engine.apply_command(0, &command).unwrap();
            recorded.push((command, batch));
            let (mut replay, ..) = fresh();
            for (command, expected) in recorded {
                let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
                assert_eq!(replay.apply_command(0, &decoded).unwrap(), expected);
            }
            assert_eq!(
                replay.diagnostic_snapshot().unwrap(),
                engine.diagnostic_snapshot().unwrap()
            );
            assert!(engine.state.pending_resolution.is_none());
        }
    }
}

#[test]
fn entry_reveal_pair_four_nonconsecutive_seats_use_chooser_hand_and_current_mana_controller() {
    let mut engine = GameEngine::new(
        614_230,
        &[10, 20, 30, 40],
        20,
        Some(vec![vec!["island".into(); 30]; 4]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    for _ in 0..120 {
        if engine.state.active_player_id() == 40
            && engine.state.turn_step == tricerules_core::TurnStep::Main1
        {
            break;
        }
        if engine.state.cleanup_discard_player.is_some() {
            resolve_cleanup_discards_if_any(&mut engine);
        } else {
            let actor = engine.state.priority_player_id();
            engine.apply_command(actor, &pass()).unwrap();
        }
    }
    assert_eq!(engine.state.active_player_id(), 40);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    let candidates: Vec<_> = (0..4)
        .map(|index| inject_card_into_hand(&mut engine, index, "firdoch_core"))
        .collect();
    let source = inject_card_into_hand(&mut engine, 3, "murmuring_bosk");
    let slot = engine.state.players[3]
        .hand
        .iter()
        .position(|oid| *oid == source)
        .unwrap();
    let offered = engine.apply_command(40, &play_land(slot)).unwrap();
    let choice = find_resolution_choice(&offered).unwrap();
    assert_eq!(choice.deciding_player_id, 40);
    assert_eq!(choice.candidate_object_ids, vec![candidates[3]]);
    let answer = submit_resolution_choice(vec![candidates[3]]);
    for actor in [10, 20, 30] {
        illegal(&mut engine, actor, &answer);
    }
    engine.apply_command(40, &answer).unwrap();
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(30),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    for actor in [40, 10, 20] {
        engine.apply_command(actor, &pass()).unwrap();
    }
    let activation = activate_ability_for(&engine, source, 1, vec![]);
    engine.apply_command(30, &activation).unwrap();
    assert_eq!(engine.state.objects[&source].owner, 40);
    assert_eq!(engine.characteristics(source).unwrap().controller, 30);
    assert_eq!(engine.state.players[2].mana_pool.white, 1);
    assert_eq!(engine.state.players[2].life, 19);
    assert_eq!(engine.state.players[3].life, 20);
}
