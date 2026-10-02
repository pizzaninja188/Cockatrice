use super::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, CounterKind, Keyword};
use tricerules_cards::{EffectDuration, PermanentTypeFilter, TypeLineAddition};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::FlexPipPayment;

fn setup(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_copy(engine: &mut GameEngine, card: &str) -> u32 {
    let oid = inject_card_into_hand(engine, 0, card);
    give_mana(
        engine,
        0,
        ManaGift {
            u: 3,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, card);
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(engine);
    oid
}

#[test]
fn phyrexian_metamorph_complete_registry_identity_exists() {
    let registry = tricerules_cards::CardRegistry::global();
    let definition = registry
        .get("phyrexian_metamorph")
        .expect("complete Phyrexian Metamorph definition");
    assert_eq!(definition.name, "Phyrexian Metamorph");
    assert_eq!(definition.face_count(), 1);
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{U/P}");
    assert_eq!(
        face.types,
        vec!["Artifact", "Creature", "Phyrexian", "Shapeshifter"]
    );
    assert_eq!(face.power, Some(0));
    assert_eq!(face.toughness, Some(0));
    assert_eq!(face.colors(), vec![tricerules_cards::Color::Blue]);
    assert!(face.supertypes.is_empty());
    assert_eq!(face.static_abilities.len(), 1);
    assert!(face.activated_abilities.is_empty() && face.triggered_abilities.is_empty());
    assert_eq!(
        face.static_abilities[0].presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![2])
    );
    let tricerules_cards::primitives::StaticAbilityDef::EntersAsCopy {
        filter,
        artifact_in_addition,
    } = &face.static_abilities[0].definition
    else {
        panic!("Metamorph entry copying");
    };
    assert!(*artifact_in_addition);
    assert_eq!(
        filter.kind,
        tricerules_cards::primitives::TargetKind::AnyPermanent
    );
    assert_eq!(
        filter.permanent_types,
        vec![PermanentTypeFilter::Artifact, PermanentTypeFilter::Creature]
    );
    for card in ["clone", "mirrormade", "sculpting_steel"] {
        assert!(matches!(
            &registry.get(card).unwrap().primary_face().static_abilities[0].definition,
            tricerules_cards::primitives::StaticAbilityDef::EntersAsCopy {
                artifact_in_addition: false,
                ..
            }
        ));
    }
}

#[test]
fn phyrexian_metamorph_artifact_exception_is_copiable() {
    let mut engine = setup(49601);
    let source = inject_creature_on_battlefield(&mut engine, 1, "serra_angel");
    engine.state.objects.get_mut(&source).unwrap().tapped = true;
    let metamorph = cast_copy(&mut engine, "phyrexian_metamorph");
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .unwrap();
    let characteristics = engine.characteristics(metamorph).unwrap();
    assert!(
        characteristics.has_type("Artifact"),
        "copy exception is part of the copied values"
    );
    assert!(characteristics.has_type("Creature"));
    assert_eq!(characteristics.names, vec!["Serra Angel"]);
    assert_eq!(characteristics.power, Some(4));
    assert!(!engine.state.objects[&metamorph].tapped);
    let clone = cast_copy(&mut engine, "clone");
    engine
        .apply_command(0, &submit_resolution_choice(vec![metamorph]))
        .unwrap();
    assert!(engine.characteristics(clone).unwrap().has_type("Artifact"));
    assert_eq!(engine.effective_power(clone), Some(4));
}

#[test]
fn phyrexian_metamorph_face_down_candidate_conceals_registry_identity() {
    let mut engine = setup(49602);
    let source = inject_creature_on_battlefield(&mut engine, 1, "serra_angel");
    engine.state.objects.get_mut(&source).unwrap().face_down = true;
    let metamorph = inject_card_into_hand(&mut engine, 0, "phyrexian_metamorph");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "phyrexian_metamorph");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    let batch = engine.apply_command(1, &pass()).unwrap();
    let choice = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .unwrap();
    let index = choice
        .candidate_object_ids
        .iter()
        .position(|oid| *oid == source)
        .unwrap();
    assert_eq!(choice.candidate_names[index], "Face-down creature");
    assert!(
        choice.candidate_card_ids[index].is_empty(),
        "public copy-source metadata must not expose a face-down card"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .unwrap();
    let copied = engine.characteristics(metamorph).unwrap();
    assert!(copied.has_type("Artifact") && copied.has_type("Creature"));
    assert_eq!(copied.power, Some(2));
    assert!(!copied.has_keyword(Keyword::Flying));
    assert!(!copied.has_name("Serra Angel"));
}

fn illegal(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = format!("{:?}", engine.state);
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}

#[test]
fn phyrexian_metamorph_blue_or_life_payment_and_unaffordable_rejection() {
    for life_payment in [false, true] {
        let mut engine = setup(49603);
        let source = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let metamorph = inject_card_into_hand(&mut engine, 0, "phyrexian_metamorph");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: u32::from(!life_payment),
                c: 3,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "phyrexian_metamorph");
        let command = if life_payment {
            cast_spell_flex(
                slot,
                vec![],
                vec![FlexPipPayment {
                    pip_index: 1,
                    pay_life: true,
                }],
            )
        } else {
            cast_spell(slot, vec![])
        };
        let batch = engine.apply_command(0, &command).unwrap();
        assert_eq!(
            engine.state.players[0].life,
            if life_payment { 18 } else { 20 }
        );
        if life_payment {
            assert!(life_changes_in(&batch)
                .iter()
                .any(|event| event.player_id == 0 && event.delta == -2 && event.new_total == 18));
        }
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        assert_eq!(engine.state.players[0].mana_pool.blue, 0);
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &submit_resolution_choice(vec![source]))
            .unwrap();
        assert!(engine.characteristics(metamorph).unwrap().is_artifact());
    }
    let mut engine = setup(49604);
    inject_card_into_hand(&mut engine, 0, "phyrexian_metamorph");
    engine.state.players[0].life = 1;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "phyrexian_metamorph");
    illegal(
        &mut engine,
        0,
        &cast_spell_flex(
            slot,
            vec![],
            vec![FlexPipPayment {
                pip_index: 1,
                pay_life: true,
            }],
        ),
    );
}

#[test]
fn phyrexian_metamorph_source_filter_is_optional_untargeted_artifact_or_creature() {
    let mut engine = setup(49605);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "slippery_bogle");
    let artifact = inject_creature_on_battlefield(&mut engine, 1, "sol_ring");
    let land = inject_creature_on_battlefield(&mut engine, 1, "forest");
    let enchantment = inject_creature_on_battlefield(&mut engine, 1, "omniscience");
    let metamorph = cast_copy(&mut engine, "phyrexian_metamorph");
    let prompt = &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation;
    assert_eq!(prompt.min, 0);
    assert_eq!(prompt.max, 1);
    assert!(prompt.candidates.contains(&creature) && prompt.candidates.contains(&artifact));
    for absent in [land, enchantment, metamorph] {
        assert!(!prompt.candidates.contains(&absent));
    }
    illegal(&mut engine, 1, &submit_resolution_choice(vec![creature]));
    illegal(&mut engine, 0, &submit_resolution_choice(vec![land]));
    illegal(
        &mut engine,
        0,
        &submit_resolution_choice(vec![creature, artifact]),
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![creature]))
        .unwrap();
    assert!(engine.effective_has_keyword(metamorph, Keyword::Hexproof));
    assert!(engine.characteristics(metamorph).unwrap().is_artifact());
}

#[test]
fn phyrexian_metamorph_noncreature_artifact_and_decline_preserve_exact_types() {
    let mut engine = setup(49606);
    let source = inject_creature_on_battlefield(&mut engine, 1, "sol_ring");
    let metamorph = cast_copy(&mut engine, "phyrexian_metamorph");
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .unwrap();
    let copied = engine.characteristics(metamorph).unwrap();
    assert!(copied.is_artifact() && !copied.is_creature());
    assert_eq!(
        copied
            .types
            .iter()
            .filter(|kind| *kind == "Artifact")
            .count(),
        1
    );
    assert_eq!(copied.names, vec!["Sol Ring"]);
    assert!(!engine.state.objects[&metamorph].is_token());
    let colorless_before = engine.state.players[0].mana_pool.colorless;
    let ability = activate_ability_for(&engine, metamorph, 0, vec![]);
    engine.apply_command(0, &ability).unwrap();
    assert_eq!(
        engine.state.players[0].mana_pool.colorless,
        colorless_before + 2
    );
    assert!(engine.state.objects[&metamorph].tapped);
    assert!(!engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());
    let declined = cast_copy(&mut engine, "phyrexian_metamorph");
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    assert_eq!(engine.state.objects[&declined].zone, Zone::Graveyard);
    assert!(engine.state.objects[&declined].copiable_values.is_none());
    let mut empty = setup(49607);
    let declined = cast_copy(&mut empty, "phyrexian_metamorph");
    assert!(empty.state.pending_resolution.is_none());
    assert_eq!(empty.state.objects[&declined].zone, Zone::Graveyard);
}

#[test]
fn phyrexian_metamorph_copies_tokens_without_becoming_token_and_excludes_counters() {
    let mut engine = setup(49608);
    let token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    engine
        .state
        .objects
        .get_mut(&token)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 4);
    let metamorph = cast_copy(&mut engine, "phyrexian_metamorph");
    engine
        .apply_command(0, &submit_resolution_choice(vec![token]))
        .unwrap();
    assert!(!engine.state.objects[&metamorph].is_token());
    assert_eq!(engine.effective_power(metamorph), Some(1));
    assert_eq!(
        engine.state.objects[&metamorph].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert!(engine.characteristics(metamorph).unwrap().is_artifact());
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
        .apply_command(0, &cast_spell(slot, target_object(metamorph)))
        .unwrap();
    engine.apply_command(0, &pass()).unwrap();
    let batch = engine.apply_command(1, &pass()).unwrap();
    let events = token_created_events(&batch);
    assert_eq!(events.len(), 1);
    let copied_token = events[0].object_id;
    assert!(engine.state.objects[&copied_token].is_token());
    assert!(engine.characteristics(copied_token).unwrap().is_artifact());
    assert_eq!(engine.effective_power(copied_token), Some(1));
}

#[test]
fn phyrexian_metamorph_later_clone_replacement_replaces_the_artifact_exception() {
    let mut engine = setup(49609);
    let clone = inject_creature_on_battlefield(&mut engine, 1, "clone");
    engine
        .state
        .objects
        .get_mut(&clone)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let metamorph = cast_copy(&mut engine, "phyrexian_metamorph");
    engine
        .apply_command(0, &submit_resolution_choice(vec![clone]))
        .unwrap();
    assert!(
        engine.state.pending_resolution.is_some(),
        "copied Clone entry replacement applies"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![bears]))
        .unwrap();
    let copied = engine.characteristics(metamorph).unwrap();
    assert!(
        !copied.is_artifact(),
        "later default copy replaces earlier modified values"
    );
    assert_eq!(copied.names, vec!["Grizzly Bears"]);
    assert_eq!(copied.power, Some(2));
}

#[test]
fn phyrexian_metamorph_copied_entry_replacements_and_draw_trigger_apply() {
    for card in ["diregraf_ghoul", "elvish_visionary"] {
        let mut engine = setup(49610);
        let source = inject_creature_on_battlefield(&mut engine, 1, card);
        let metamorph = cast_copy(&mut engine, "phyrexian_metamorph");
        let hand_before = engine.state.players[0].hand.len();
        engine
            .apply_command(0, &submit_resolution_choice(vec![source]))
            .unwrap();
        assert_eq!(
            engine.state.objects[&metamorph].tapped,
            card == "diregraf_ghoul"
        );
        assert!(engine.characteristics(metamorph).unwrap().is_artifact());
        if card == "elvish_visionary" {
            assert_eq!(engine.state.stack.len(), 1);
            pass_both_players(&mut engine);
            assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
            assert!(engine.state.stack.is_empty());
        }
    }
}

#[test]
fn phyrexian_metamorph_copied_sunburst_uses_its_own_committed_colors() {
    for life_payment in [false, true] {
        let mut engine = setup(49611);
        let prism = inject_creature_on_battlefield(&mut engine, 1, "pentad_prism");
        engine
            .state
            .objects
            .get_mut(&prism)
            .unwrap()
            .set_counter(CounterKind::Charge, 9);
        let metamorph = inject_card_into_hand(&mut engine, 0, "phyrexian_metamorph");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                w: 1,
                u: u32::from(!life_payment),
                b: 1,
                r: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "phyrexian_metamorph");
        let command = if life_payment {
            cast_spell_flex(
                slot,
                vec![],
                vec![FlexPipPayment {
                    pip_index: 1,
                    pay_life: true,
                }],
            )
        } else {
            cast_spell(slot, vec![])
        };
        engine.apply_command(0, &command).unwrap();
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &submit_resolution_choice(vec![prism]))
            .unwrap();
        assert_eq!(
            engine.state.objects[&metamorph].counter_count(CounterKind::Charge),
            if life_payment { 3 } else { 4 }
        );
        assert!(!engine.characteristics(metamorph).unwrap().is_creature());
    }
}

#[test]
fn phyrexian_metamorph_structural_animated_room_preserves_exception_on_owned_doors() {
    let mut engine = setup(49612);
    let room = inject_creature_on_battlefield(&mut engine, 1, "derelict_attic_widows_walk");
    engine.state.room_states.insert(
        room,
        tricerules_core::state::RoomState {
            unlocked: [true, true],
        },
    );
    // Structural continuous-effect fixture; no registered animated-Room producer is claimed.
    for kind in [
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            ..Default::default()
        }),
        ContinuousEffectKind::Layer7bSetPt {
            power: 4,
            toughness: 4,
        },
    ] {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(room),
            kind,
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: engine.state.command_index,
        });
    }
    assert!(engine.characteristics(room).unwrap().is_creature());
    let metamorph = cast_copy(&mut engine, "phyrexian_metamorph");
    engine
        .apply_command(0, &submit_resolution_choice(vec![room]))
        .unwrap();
    assert_eq!(
        engine.state.room_states[&metamorph].unlocked,
        [false, false]
    );
    let copied = engine.characteristics(metamorph).unwrap();
    assert!(copied.is_artifact() && copied.has_type("Enchantment") && copied.has_type("Room"));
    assert!(!copied.is_creature());
    for face in engine.state.objects[&metamorph]
        .copiable_values
        .as_ref()
        .unwrap()
        .room_faces
        .as_ref()
        .unwrap()
    {
        assert!(face.is_artifact && face.types.contains(&"Artifact".to_string()));
    }
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let generation = semantic::generation(&engine, metamorph);
    execute_permanent_action_with_payment(
        &mut engine,
        0,
        RuledCommand {
            cmd: Some(Cmd::ExecutePermanentAction(ExecutePermanentAction {
                kind: PermanentActionKind::UnlockRoomDoor as i32,
                object_id: metamorph,
                expected_zone_change_generation: generation,
                face_index: Some(0),
                ..Default::default()
            })),
        },
    )
    .unwrap();
    assert_eq!(engine.state.room_states[&metamorph].unlocked, [true, false]);
    assert!(engine.characteristics(metamorph).unwrap().is_artifact());
    pass_both_players(&mut engine);
    let steel = cast_copy(&mut engine, "sculpting_steel");
    engine
        .apply_command(0, &submit_resolution_choice(vec![metamorph]))
        .unwrap();
    assert_eq!(engine.state.room_states[&steel].unlocked, [false, false]);
    assert!(engine.characteristics(steel).unwrap().is_artifact());
}

#[test]
fn phyrexian_metamorph_bounce_clears_copy_and_recast_uses_a_new_source() {
    let mut engine = setup(49613);
    let bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let ring = inject_creature_on_battlefield(&mut engine, 1, "sol_ring");
    let metamorph = cast_copy(&mut engine, "phyrexian_metamorph");
    engine
        .apply_command(0, &submit_resolution_choice(vec![bears]))
        .unwrap();
    let generation = semantic::generation(&engine, metamorph);
    inject_card_into_hand(&mut engine, 0, "unsummon");
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(metamorph)))
        .unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&metamorph].zone, Zone::Hand);
    assert!(engine.state.objects[&metamorph].copiable_values.is_none());
    assert!(semantic::generation(&engine, metamorph) > generation);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "phyrexian_metamorph");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![ring]))
        .unwrap();
    assert_eq!(
        engine.state.objects[&metamorph].card_id,
        "phyrexian_metamorph"
    );
    let copied = engine.characteristics(metamorph).unwrap();
    assert_eq!(copied.names, vec!["Sol Ring"]);
    assert!(!copied.is_creature());
}

#[test]
fn phyrexian_metamorph_serialized_accepted_commands_replay_identically() {
    use prost::Message;
    fn fixture() -> (GameEngine, u32, usize) {
        let mut engine = setup(49614);
        let source = inject_creature_on_battlefield(&mut engine, 1, "serra_angel");
        inject_card_into_hand(&mut engine, 0, "phyrexian_metamorph");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "phyrexian_metamorph");
        (engine, source, slot)
    }
    let (mut engine, source, slot) = fixture();
    let (mut replay, replay_source, replay_slot) = fixture();
    assert_eq!((source, slot), (replay_source, replay_slot));
    for (actor, command) in [
        (0, cast_spell(slot, vec![])),
        (0, pass()),
        (1, pass()),
        (0, submit_resolution_choice(vec![source])),
    ] {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let actual = engine.apply_command(actor, &command).unwrap();
        let reproduced = replay.apply_command(actor, &decoded).unwrap();
        assert_eq!(actual.encode_to_vec(), reproduced.encode_to_vec());
        assert_eq!(
            engine.diagnostic_snapshot().unwrap(),
            replay.diagnostic_snapshot().unwrap()
        );
    }
}

#[test]
fn phyrexian_metamorph_four_nonconsecutive_seats_keep_copy_choice_with_its_controller() {
    let mut engine = GameEngine::new(
        49615,
        &[3, 7, 11, 19],
        20,
        Some(vec![deck_with("island", &[]); 4]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 3, "grizzly_bears");
    let metamorph = inject_card_into_hand(&mut engine, 0, "phyrexian_metamorph");
    give_mana(
        &mut engine,
        3,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == metamorph)
        .unwrap();
    engine.apply_command(3, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        3
    );
    for actor in [7, 11, 19] {
        illegal(&mut engine, actor, &submit_resolution_choice(vec![source]));
    }
    engine
        .apply_command(3, &submit_resolution_choice(vec![source]))
        .unwrap();
    assert_eq!(engine.state.objects[&metamorph].owner, 3);
    assert_eq!(engine.characteristics(metamorph).unwrap().controller, 3);
    assert!(engine.characteristics(metamorph).unwrap().is_artifact());
}
