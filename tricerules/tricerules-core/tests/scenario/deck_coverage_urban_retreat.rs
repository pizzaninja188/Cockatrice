//! Urban Retreat's hand-activated, exact-source land entry.

use super::helpers::*;
use tricerules_cards::primitives::{EntersTappedAffected, StaticAbilityDef};
use tricerules_cards::{
    AbilityCost, AbilitySourceZone, CardRegistry, Layout, ManaAmount, ManaCost, SpellEffectKind,
};
use tricerules_proto::ruled::v1 as rv1;

#[test]
fn urban_retreat_registers_its_complete_printed_identity() {
    let card = CardRegistry::global()
        .get("urban_retreat")
        .expect("complete Urban Retreat definition");
    assert_eq!(card.id, "urban_retreat");
    assert_eq!(card.name, "Urban Retreat");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.types, vec!["Land".to_string()]);
    assert_eq!(face.activated_abilities.len(), 2);
    let mana = &face.activated_abilities[0];
    assert!(!mana.intrinsic_land_mana);
    assert_eq!(mana.costs, vec![AbilityCost::Tap]);
    assert_eq!(
        mana.effect,
        vec![SpellEffectKind::ProduceMana {
            options: vec![
                ManaAmount {
                    g: 1,
                    ..Default::default()
                },
                ManaAmount {
                    w: 1,
                    ..Default::default()
                },
                ManaAmount {
                    u: 1,
                    ..Default::default()
                },
            ],
            commander_color_identity: false,
            restriction: None,
            conditional: None,
        }]
    );
    let put_onto_battlefield = &face.activated_abilities[1];
    assert_eq!(put_onto_battlefield.source_zone, AbilitySourceZone::Hand);
    assert!(put_onto_battlefield.requires_sorcery_speed());
    assert_eq!(
        put_onto_battlefield.costs,
        vec![
            AbilityCost::Mana(ManaCost::parse("{2}").unwrap()),
            AbilityCost::ReturnTappedCreature,
        ]
    );
    assert_eq!(
        put_onto_battlefield.effect,
        vec![SpellEffectKind::PutAbilitySourceOntoBattlefield]
    );
    assert!(matches!(
        &face.static_abilities[0].definition,
        StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            ..
        }
    ));
}

#[test]
fn urban_retreat_stages_from_hand_before_tapping_its_mana_creature_cost_candidate() {
    let decks = Some(vec![
        deck_with("forest", &["llanowar_elves", "eumidian_terrabotanist"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(60_810_001, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "urban_retreat");
    let mana_creature = move_ready_to_battlefield(&mut engine, 0, "llanowar_elves");
    let landfall_observer = move_ready_to_battlefield(&mut engine, 0, "eumidian_terrabotanist");
    // A creature you control can be owned by another player; paying this cost returns it there.
    engine.state.objects.get_mut(&mana_creature).unwrap().owner = 1;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    let batch = engine.initial_response_batch();
    let legal = &batch.legal_by_player[&0];
    let action = legal
        .zone_ability_actions
        .iter()
        .find(|action| action.object_id == source && action.ability_index == 1)
        .expect("Urban Retreat hand activation");
    assert_eq!(action.source_zone(), rv1::AbilitySourceZone::Hand);
    assert!(action.ability.as_ref().unwrap().activatable);
    let choices = &legal.cost_choices_by_ability[&((u64::from(source) << 32) | 1)];
    assert!(choices.non_mana_costs_payable);
    assert!(choices.choices[0].candidate_objects.is_empty());

    let began = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
                    source_object_id: source,
                    expected_zone_change_generation: action.zone_change_generation,
                    ability_index: 1,
                    source_zone: rv1::AbilitySourceZone::Hand as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("a hand activation may enter its payment window");

    let pending = engine.state.pending_ability_activation.clone().unwrap();
    assert_eq!(pending.stage(), rv1::AbilityActivationStage::Payment);
    assert_eq!(pending.source_zone(), rv1::AbilitySourceZone::Hand);
    assert!(pending.return_tapped_creature_candidates.is_empty());
    assert!(began.legal_by_player[&0]
        .pending_ability_activation
        .as_ref()
        .and_then(|pending| pending.payment_preview.as_ref())
        .is_some_and(|preview| preview.valid));
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Hand
    );
    assert!(!engine.state.objects[&mana_creature].tapped);

    let initial_reveal = active_reveals(&began, source);
    assert_eq!(
        initial_reveal.len(),
        1,
        "the hand source is publicly revealed once"
    );

    let mana_generation = engine.state.zone_change_generation[&mana_creature];
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
                    source_object_id: mana_creature,
                    source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: mana_generation,
                    ability_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("mana ability remains available during the staged activation");

    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    assert!(
        pending.revision > 1,
        "candidate refresh retires old choices"
    );
    assert_eq!(pending.return_tapped_creature_candidates.len(), 1);
    let selected = pending.return_tapped_creature_candidates[0];
    let pending_transaction_id = pending.transaction_id;
    let pending_revision = pending.revision;
    assert_eq!(selected.object_id, mana_creature);
    assert_eq!(selected.zone_change_generation, mana_generation);
    assert!(engine.state.objects[&mana_creature].tapped);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);

    let refreshed_views = engine.initial_response_batch();
    let actor_pending = refreshed_views.legal_by_player[&0]
        .pending_ability_activation
        .as_ref()
        .unwrap();
    let opponent_pending = refreshed_views.legal_by_player[&1]
        .pending_ability_activation
        .as_ref()
        .unwrap();
    assert_eq!(actor_pending.return_tapped_creature_candidates.len(), 1);
    assert!(opponent_pending
        .return_tapped_creature_candidates
        .is_empty());
    let actor_reveal = active_reveals(&refreshed_views, source);
    let opponent_reveal = active_reveals(&refreshed_views, source);
    assert_eq!(actor_reveal.len(), 1);
    assert_eq!(opponent_reveal.len(), 1);
    assert_eq!(actor_reveal[0].reveal_id, initial_reveal[0].reveal_id);
    assert_eq!(opponent_reveal[0].reveal_id, initial_reveal[0].reveal_id);

    let committed = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitAbilityActivation(rv1::CommitAbilityActivation {
                    transaction_id: pending_transaction_id,
                    expected_revision: pending_revision,
                    return_tapped_creature: Some(selected),
                    ..Default::default()
                })),
            },
        )
        .expect("the current exact tapped creature pays the nonmana cost");
    assert_eq!(
        engine.state.objects[&mana_creature].zone,
        tricerules_core::Zone::Hand
    );
    assert!(engine.state.players[1].hand.contains(&mana_creature));
    assert!(!engine.state.players[0].battlefield.contains(&mana_creature));
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(source));
    let committed_reveal = active_reveals(&committed, source);
    assert_eq!(committed_reveal.len(), 1);
    assert_eq!(committed_reveal[0].reveal_id, initial_reveal[0].reveal_id);

    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the land entry triggered the battlefield observer"
    );
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(
        engine.state.objects[&source].tapped,
        "Urban Retreat enters tapped"
    );
    assert_eq!(engine.state.players[0].life, 20);
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    assert_eq!(engine.state.stack.len(), 0);
    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(
        engine.state.objects[&landfall_observer].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.lands_played_this_turn, 0,
        "putting the land onto the battlefield is not a land play"
    );
    let resolved = engine.initial_response_batch();
    assert!(active_reveals(&resolved, source).is_empty());
}

#[test]
fn urban_retreat_mana_ability_produces_each_printed_color_option() {
    let decks = Some(vec![
        deck_with("forest", &["urban_retreat"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(60_810_010, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let land = move_ready_to_battlefield(&mut engine, 0, "urban_retreat");
    assert!(engine.state.objects[&land].tapped, "the land enters tapped");

    for (option_index, color) in [(0, "green"), (1, "white"), (2, "blue")] {
        // Stand in for the ordinary untap step so this focused check can exercise every mode.
        engine.state.objects.get_mut(&land).unwrap().tapped = false;
        let generation = engine.state.zone_change_generation[&land];
        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
                        source_object_id: land,
                        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                        expected_zone_change_generation: generation,
                        ability_index: 0,
                        mana_option_index: option_index,
                        ..Default::default()
                    })),
                },
            )
            .unwrap_or_else(|error| panic!("activate Urban Retreat for {color}: {error}"));
        assert!(engine.state.objects[&land].tapped);
        assert_eq!(
            engine.state.stack.len(),
            0,
            "mana abilities resolve immediately"
        );
        match color {
            "green" => assert_eq!(engine.state.players[0].mana_pool.green, 1),
            "white" => assert_eq!(engine.state.players[0].mana_pool.white, 1),
            "blue" => assert_eq!(engine.state.players[0].mana_pool.blue, 1),
            _ => unreachable!(),
        }
    }
}

#[test]
fn urban_retreat_requires_the_staged_hand_activation_transaction() {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(60_810_010, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "urban_retreat");
    let creature = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&creature).unwrap().tapped = true;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or_default();
    let creature_generation = engine
        .state
        .zone_change_generation
        .get(&creature)
        .copied()
        .unwrap_or_default();

    let direct = engine.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
                source_object_id: source,
                source_zone: rv1::AbilitySourceZone::Hand as i32,
                expected_zone_change_generation: source_generation,
                ability_index: 1,
                cost_selections: vec![rv1::CostSelection {
                    cost_index: 1,
                    selection: Some(rv1::cost_selection::Selection::BattlefieldObjects(
                        rv1::CostObjectRefs {
                            objects: vec![rv1::CostObjectRef {
                                object_id: creature,
                                zone_change_generation: creature_generation,
                            }],
                        },
                    )),
                }],
                ..Default::default()
            })),
        },
    );
    assert!(
        direct.is_err(),
        "this hand ability must be announced through its staged mana window"
    );
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(
        engine.state.objects[&creature].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
}

#[test]
fn urban_retreat_accepts_a_creature_tapped_by_springleaf_drum_during_payment() {
    let decks = Some(vec![
        deck_with("forest", &["springleaf_drum", "grizzly_bears"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(60_810_009, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "urban_retreat");
    let drum = move_ready_to_battlefield(&mut engine, 0, "springleaf_drum");
    let creature = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    let batch = engine.initial_response_batch();
    let legal = &batch.legal_by_player[&0];
    let action = legal
        .zone_ability_actions
        .iter()
        .find(|action| action.object_id == source && action.ability_index == 1)
        .expect("Urban Retreat hand activation");
    assert!(
        action.ability.as_ref().unwrap().activatable,
        "Springleaf Drum can tap the otherwise untapped controlled creature"
    );

    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
                    source_object_id: source,
                    expected_zone_change_generation: action.zone_change_generation,
                    ability_index: 1,
                    source_zone: rv1::AbilitySourceZone::Hand as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("the staged hand activation starts before Drum taps its cost candidate");

    let drum_generation = engine.state.zone_change_generation[&drum];
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
                    source_object_id: drum,
                    source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: drum_generation,
                    ability_index: 0,
                    mana_option_index: 4,
                    cost_selections: vec![rv1::CostSelection {
                        cost_index: 1,
                        selection: Some(rv1::cost_selection::Selection::BattlefieldObjects(
                            rv1::CostObjectRefs {
                                objects: vec![rv1::CostObjectRef {
                                    object_id: creature,
                                    zone_change_generation: engine.state.zone_change_generation
                                        [&creature],
                                }],
                            },
                        )),
                    }],
                    ..Default::default()
                })),
            },
        )
        .expect("Springleaf Drum taps the creature and adds one green mana");

    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    assert_eq!(pending.return_tapped_creature_candidates.len(), 1);
    let selected = pending.return_tapped_creature_candidates[0];
    assert_eq!(selected.object_id, creature);
    assert!(engine.state.objects[&creature].tapped);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    let committed = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitAbilityActivation(rv1::CommitAbilityActivation {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    return_tapped_creature: Some(selected),
                    ..Default::default()
                })),
            },
        )
        .expect("the exact creature tapped by Drum pays Urban Retreat's cost");
    assert_eq!(
        engine.state.objects[&creature].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(active_reveals(&committed, source).len(), 1);

    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.objects[&source].tapped);
}

#[test]
fn urban_retreat_return_cost_triggers_leaves_ability_above_the_land_activation() {
    let decks = Some(vec![
        deck_with("forest", &["featherbrained_filcher"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(60_810_005, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "urban_retreat");
    let creature = move_ready_to_battlefield(&mut engine, 0, "featherbrained_filcher");
    engine.state.objects.get_mut(&creature).unwrap().owner = 1;
    engine.state.objects.get_mut(&creature).unwrap().tapped = true;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
                    source_object_id: source,
                    expected_zone_change_generation: source_generation,
                    ability_index: 1,
                    source_zone: rv1::AbilitySourceZone::Hand as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("begin Urban Retreat's hand activation");
    let pending = engine.state.pending_ability_activation.clone().unwrap();
    assert_eq!(pending.return_tapped_creature_candidates.len(), 1);
    assert_eq!(
        pending.return_tapped_creature_candidates[0].object_id,
        creature
    );

    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitAbilityActivation(rv1::CommitAbilityActivation {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    return_tapped_creature: Some(pending.return_tapped_creature_candidates[0]),
                    ..Default::default()
                })),
            },
        )
        .expect("return the tapped creature as an activation cost");
    assert!(engine.state.players[1].hand.contains(&creature));
    assert_eq!(engine.state.stack.len(), 2);
    let activation_index = engine
        .state
        .stack
        .iter()
        .position(|item| !item.is_triggered && item.source_permanent_id == Some(source))
        .expect("Urban Retreat activation remains on the stack");
    let trigger_index = engine
        .state
        .stack
        .iter()
        .position(|item| item.is_triggered && item.source_permanent_id == Some(creature))
        .expect("the returned creature's leaves trigger is on the stack");
    assert!(
        trigger_index > activation_index,
        "the leaves trigger is above the activation"
    );

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the land activation remains below the trigger"
    );
    assert!(engine.state.objects.values().any(
        |object| object.card_id == "food" && object.zone == tricerules_core::Zone::Battlefield
    ));
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.objects[&source].tapped);
}

#[test]
fn urban_retreat_hand_activation_is_limited_to_the_active_players_main_phase() {
    let decks = Some(vec![
        deck_with("island", &[]),
        deck_with("forest", &["llanowar_elves"]),
    ]);
    let mut engine = GameEngine::new(60_810_006, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 1, "urban_retreat");
    let creature = move_ready_to_battlefield(&mut engine, 1, "llanowar_elves");
    engine.state.objects.get_mut(&creature).unwrap().tapped = true;
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &pass())
        .expect("active player passes priority");

    let legal = engine.initial_response_batch();
    let action = legal.legal_by_player[&1]
        .zone_ability_actions
        .iter()
        .find(|action| action.object_id == source && action.ability_index == 1)
        .expect("the hand ability remains visible as a disabled action");
    assert!(!action.ability.as_ref().unwrap().activatable);

    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let rejected = engine.apply_command(
        1,
        &RuledCommand {
            cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
                source_object_id: source,
                expected_zone_change_generation: source_generation,
                ability_index: 1,
                source_zone: rv1::AbilitySourceZone::Hand as i32,
                ..Default::default()
            })),
        },
    );
    assert!(rejected.is_err(), "the nonactive player cannot activate it");
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Hand
    );
}

#[test]
fn urban_retreat_rejects_missing_and_stale_tapped_creature_receipts() {
    let decks = Some(vec![
        deck_with("forest", &["llanowar_elves"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(60_810_002, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "urban_retreat");
    let mana_creature = move_ready_to_battlefield(&mut engine, 0, "llanowar_elves");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    let action = engine.initial_response_batch().legal_by_player[&0]
        .zone_ability_actions
        .iter()
        .find(|action| action.object_id == source && action.ability_index == 1)
        .unwrap()
        .clone();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
                    source_object_id: source,
                    expected_zone_change_generation: action.zone_change_generation,
                    ability_index: 1,
                    source_zone: rv1::AbilitySourceZone::Hand as i32,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    let mana_generation = engine.state.zone_change_generation[&mana_creature];
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
                    source_object_id: mana_creature,
                    source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: mana_generation,
                    ability_index: 0,
                    ..Default::default()
                })),
            },
        )
        .unwrap();

    let pending = engine.state.pending_ability_activation.clone().unwrap();
    let missing = engine.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CommitAbilityActivation(rv1::CommitAbilityActivation {
                transaction_id: pending.transaction_id,
                expected_revision: pending.revision,
                ..Default::default()
            })),
        },
    );
    assert!(
        missing.is_err(),
        "the deferred return cost is mandatory at commit"
    );
    assert!(engine.state.objects[&mana_creature].tapped);

    let stale = rv1::CostObjectRef {
        object_id: mana_creature,
        zone_change_generation: engine.state.zone_change_generation[&mana_creature] + 1,
    };
    let stale_result = engine.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CommitAbilityActivation(rv1::CommitAbilityActivation {
                transaction_id: pending.transaction_id,
                expected_revision: pending.revision,
                return_tapped_creature: Some(stale),
                ..Default::default()
            })),
        },
    );
    assert!(
        stale_result.is_err(),
        "a later incarnation cannot pay the offered cost"
    );
    assert!(engine.state.objects[&mana_creature].tapped);
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Hand
    );

    let valid = pending.return_tapped_creature_candidates[0];
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitAbilityActivation(rv1::CommitAbilityActivation {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    return_tapped_creature: Some(valid),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    assert_eq!(
        engine.state.objects[&mana_creature].zone,
        tricerules_core::Zone::Hand
    );
}

#[test]
fn urban_retreat_cancel_ends_its_temporary_public_reveal() {
    let decks = Some(vec![
        deck_with("forest", &["llanowar_elves"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(60_810_003, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "urban_retreat");
    let _potential_return_candidate = move_ready_to_battlefield(&mut engine, 0, "llanowar_elves");
    let generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let began = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
                    source_object_id: source,
                    expected_zone_change_generation: generation,
                    ability_index: 1,
                    source_zone: rv1::AbilitySourceZone::Hand as i32,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    let revealed = active_reveals(&began, source);
    assert_eq!(revealed.len(), 1);
    let first_reveal_id = revealed[0].reveal_id.clone();

    let pending = engine.state.pending_ability_activation.clone().unwrap();
    let canceled = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CancelAbilityActivation(rv1::CancelAbilityActivation {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                })),
            },
        )
        .unwrap();
    assert!(engine.state.pending_ability_activation.is_none());
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Hand
    );
    assert!(active_reveals(&canceled, source).is_empty());

    let reannounced = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
                    source_object_id: source,
                    expected_zone_change_generation: generation,
                    ability_index: 1,
                    source_zone: rv1::AbilitySourceZone::Hand as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("the same hand incarnation may be announced again");
    let second_reveal = active_reveals(&reannounced, source);
    assert_eq!(second_reveal.len(), 1);
    assert_ne!(
        second_reveal[0].reveal_id, first_reveal_id,
        "each activation transaction needs a new reveal identity"
    );
}

#[test]
fn urban_retreat_does_not_move_a_new_hand_incarnation_when_its_ability_resolves() {
    let decks = Some(vec![
        deck_with("forest", &["llanowar_elves"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(60_810_004, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "urban_retreat");
    let mana_creature = move_ready_to_battlefield(&mut engine, 0, "llanowar_elves");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let mana_generation = engine.state.zone_change_generation[&mana_creature];
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
                    source_object_id: mana_creature,
                    source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: mana_generation,
                    ability_index: 0,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    let source_action = engine.initial_response_batch().legal_by_player[&0]
        .zone_ability_actions
        .iter()
        .find(|action| action.object_id == source && action.ability_index == 1)
        .unwrap()
        .clone();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
                    source_object_id: source,
                    expected_zone_change_generation: source_action.zone_change_generation,
                    ability_index: 1,
                    source_zone: rv1::AbilitySourceZone::Hand as i32,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    let pending = engine.state.pending_ability_activation.clone().unwrap();
    let selected = pending.return_tapped_creature_candidates[0];
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitAbilityActivation(rv1::CommitAbilityActivation {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    return_tapped_creature: Some(selected),
                    ..Default::default()
                })),
            },
        )
        .unwrap();

    let original_generation = engine.state.stack.last().unwrap().source_zone_change;
    let actor = engine.state.player_idx(0).unwrap();
    engine.state.players[actor]
        .hand
        .retain(|object_id| *object_id != source);
    engine.state.players[actor].exile.push(source);
    engine.state.objects.get_mut(&source).unwrap().zone = tricerules_core::Zone::Exile;
    engine
        .state
        .zone_change_generation
        .insert(source, original_generation + 1);
    engine.state.players[actor]
        .exile
        .retain(|object_id| *object_id != source);
    engine.state.players[actor].hand.push(source);
    engine.state.objects.get_mut(&source).unwrap().zone = tricerules_core::Zone::Hand;
    engine
        .state
        .zone_change_generation
        .insert(source, original_generation + 2);

    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Hand
    );
    assert!(engine.state.players[actor].hand.contains(&source));
}

fn active_reveals(batch: &rv1::RuledEventBatch, source: u32) -> Vec<rv1::CardsRevealed> {
    batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::ActivePublicRevealSnapshot(snapshot)) => {
                Some(snapshot.reveals.clone())
            }
            _ => None,
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|reveal| reveal.cards.iter().any(|card| card.object_id == source))
        .collect()
}
