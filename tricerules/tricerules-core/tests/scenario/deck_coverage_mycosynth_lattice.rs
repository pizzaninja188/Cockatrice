//! Actual Mycosynth Lattice scenarios: CR 611.3 and 613 govern its static effects.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::{Color, ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const LATTICE: &str = "mycosynth_lattice";

fn game(seed: u64, players: &[i32]) -> GameEngine {
    assert!(
        tricerules_cards::registry::global().get(LATTICE).is_some(),
        "Mycosynth Lattice must be registered"
    );
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        players,
        20,
        None,
        true,
    )
    .expect("create game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn has_type(engine: &GameEngine, object: u32, card_type: &str) -> bool {
    engine.characteristics(object).unwrap().has_type(card_type)
}

fn last_zone_view(batch: &RuledEventBatch) -> &rv1::ZoneViewSync {
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .next_back()
        .expect("zone view")
}

fn enter_lattice(engine: &mut GameEngine) -> u32 {
    let in_hand = inject_card_into_hand(engine, 0, LATTICE);
    let entered = move_ready_to_battlefield(engine, 0, LATTICE);
    assert_eq!(entered, in_hand);
    entered
}

fn move_lattice_to_graveyard(engine: &mut GameEngine) {
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: engine.state.players[0].id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Mycosynth Lattice".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move Lattice through the normal zone-change path");
}

#[test]
fn lattice_paid_cast_adds_artifact_to_every_players_permanents_until_it_leaves() {
    let mut engine = game(26_102_701, &[4, 9, 17, 27]);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "coral_merfolk");
    let other_creature = inject_creature_on_battlefield(&mut engine, 1, "coral_merfolk");
    let token = inject_creature_on_battlefield(&mut engine, 2, "soldier_w_1_1");
    let face_down = inject_permanent_on_battlefield(&mut engine, 3, "sol_ring");
    engine.state.objects.get_mut(&face_down).unwrap().face_down = true;
    let forge_in_hand = inject_card_into_hand(&mut engine, 0, "darksteel_forge");
    let forge = move_ready_to_battlefield(&mut engine, 0, "darksteel_forge");
    assert_eq!(forge, forge_in_hand);
    let hand = inject_card_into_hand(&mut engine, 2, "coral_merfolk");
    for object in [land, own_creature, other_creature, token, face_down, hand] {
        assert!(!has_type(&engine, object, "Artifact"));
    }
    assert!(!engine
        .characteristics(own_creature)
        .unwrap()
        .has_keyword(Keyword::Indestructible));

    let lattice = inject_card_into_hand(&mut engine, 0, LATTICE);
    let slot = hand_index_for_card(&engine, 0, LATTICE);
    give_mana(
        &mut engine,
        4,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(4, &cast_spell(slot, vec![]))
        .expect_err("the six-mana artifact cannot be cast with five mana");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 5);
    give_mana(
        &mut engine,
        4,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(4, &cast_spell(slot, vec![]))
        .expect("cast Mycosynth Lattice for six mana");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&lattice].zone, Zone::Battlefield);
    for object in [
        lattice,
        forge,
        land,
        own_creature,
        other_creature,
        token,
        face_down,
    ] {
        assert!(has_type(&engine, object, "Artifact"), "object {object}");
    }
    assert!(has_type(&engine, land, "Land"));
    assert!(has_type(&engine, own_creature, "Creature"));
    assert!(!has_type(&engine, hand, "Artifact"));
    assert!(engine
        .characteristics(own_creature)
        .unwrap()
        .has_keyword(Keyword::Indestructible));
    assert!(!engine
        .characteristics(other_creature)
        .unwrap()
        .has_keyword(Keyword::Indestructible));

    engine
        .apply_scenario_command(
            4,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 4,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Mycosynth Lattice".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move Lattice out of the battlefield");
    assert_eq!(engine.state.objects[&lattice].zone, Zone::Graveyard);
    for object in [land, own_creature, other_creature, token, face_down, hand] {
        assert!(!has_type(&engine, object, "Artifact"), "object {object}");
    }
    assert!(has_type(&engine, forge, "Artifact"));
    assert!(!engine
        .characteristics(own_creature)
        .unwrap()
        .has_keyword(Keyword::Indestructible));
}

#[test]
fn lattice_makes_a_nonartifact_land_enter_as_an_artifact_for_quicksmith() {
    let mut engine = game(26_102_702, &[0, 1]);
    let lattice_in_hand = inject_card_into_hand(&mut engine, 0, LATTICE);
    let lattice = move_ready_to_battlefield(&mut engine, 0, LATTICE);
    assert_eq!(lattice, lattice_in_hand);
    let watcher = inject_creature_on_battlefield(&mut engine, 0, "quicksmith_genius");
    assert!(has_type(&engine, watcher, "Artifact"));
    let land = inject_card_into_hand(&mut engine, 0, "island");
    assert!(!has_type(&engine, land, "Artifact"));
    let slot = hand_index_for_card(&engine, 0, "island");
    engine
        .apply_scenario_command(0, &play_land(slot))
        .expect("play Island");
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert!(has_type(&engine, land, "Artifact"));
    assert_eq!(
        engine.state.pending_triggers.len() + engine.state.stack.len(),
        1,
        "the land entered as an artifact and triggered Quicksmith Genius once"
    );
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("decline Quicksmith's optional discard");
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn lattice_makes_cards_and_permanents_colorless_until_its_incarnation_leaves() {
    let mut engine = game(26_102_703, &[0, 1]);
    let permanent = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    let hand = inject_card_into_hand(&mut engine, 1, "coral_merfolk");
    let library = inject_library_card(&mut engine, 1, "grizzly_bears");
    let graveyard = inject_graveyard_card(&mut engine, 1, "hill_giant");
    for object in [permanent, token, hand, library, graveyard] {
        assert!(!engine.characteristics(object).unwrap().colors.is_empty());
    }
    let lattice = enter_lattice(&mut engine);
    let generation = engine.state.zone_change_generation[&lattice];
    for object in [permanent, token, hand, library, graveyard] {
        assert!(
            engine.characteristics(object).unwrap().colors.is_empty(),
            "object {object}"
        );
    }
    assert!(has_type(&engine, permanent, "Artifact"));
    assert!(!has_type(&engine, hand, "Artifact"));

    // Internal pre-SBA fixture: an object that is a token outside the battlefield is not a card.
    let departed_token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != departed_token);
    engine.state.players[1].graveyard.push(departed_token);
    engine.state.objects.get_mut(&departed_token).unwrap().zone = Zone::Graveyard;
    assert_eq!(
        engine.characteristics(departed_token).unwrap().colors,
        [Color::White]
    );

    move_lattice_to_graveyard(&mut engine);
    assert_eq!(
        engine.state.zone_change_generation[&lattice],
        generation + 1
    );
    for object in [permanent, token, hand, library, graveyard] {
        assert!(
            !engine.characteristics(object).unwrap().colors.is_empty(),
            "object {object}"
        );
    }
    let returned = move_ready_to_battlefield(&mut engine, 0, LATTICE);
    assert_eq!(returned, lattice);
    assert_eq!(
        engine.state.zone_change_generation[&lattice],
        generation + 2
    );
    for object in [permanent, token, hand, library, graveyard] {
        assert!(
            engine.characteristics(object).unwrap().colors.is_empty(),
            "object {object}"
        );
    }
}

#[test]
fn lattice_makes_physical_and_copied_spells_illegal_for_flashfreeze() {
    for with_lattice in [false, true] {
        let mut engine = game(26_102_704 + u64::from(with_lattice), &[0, 1]);
        if with_lattice {
            enter_lattice(&mut engine);
        }
        inject_card_into_hand(&mut engine, 0, "hill_giant");
        inject_card_into_hand(&mut engine, 1, "flashfreeze");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
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
        let slot = hand_index_for_card(&engine, 0, "hill_giant");
        engine
            .apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap();
        let original = engine.state.stack.last().unwrap().id;
        // The real spell is copied in the same way as the existing stack-filter fixture.
        let mut copy = engine.state.stack.last().unwrap().clone();
        copy.id = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        copy.is_copy = true;
        let copied = copy.id;
        engine.state.stack.push(copy);
        assert!(!engine.state.objects.contains_key(&copied));
        engine.apply_scenario_command(0, &pass()).unwrap();
        let slot = hand_index_for_card(&engine, 1, "flashfreeze");
        let batch = engine.initial_response_batch();
        let candidates =
            &batch.legal_by_player[&1].valid_targets_by_hand_slot[&((slot as u32) << 8)].groups[0]
                .valid_stack_ids;
        for target in [original, copied] {
            assert_eq!(
                candidates.contains(&target),
                !with_lattice,
                "spell {target}"
            );
        }
    }
}

#[test]
fn lattice_changes_proposed_spell_color_and_target_legality_at_resolution() {
    let mut engine = game(26_102_706, &[0, 1]);
    let knight = inject_creature_on_battlefield(&mut engine, 1, "white_knight");
    inject_card_into_hand(&mut engine, 0, "murder");
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
        .apply_scenario_command(0, &cast_spell(slot, target_object(knight)))
        .expect_err("protection from black rejects a black Murder");
    enter_lattice(&mut engine);
    let slot = hand_index_for_card(&engine, 0, "murder");
    let offers = engine.initial_response_batch();
    assert!(
        offers.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)].groups[0]
            .valid_permanent_ids
            .contains(&knight)
    );
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(knight)))
        .expect("colorless Murder may target the protected creature");
    let spell = engine.state.stack.last().unwrap().id;
    move_lattice_to_graveyard(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&knight].zone, Zone::Battlefield);
}

#[test]
fn lattice_color_and_type_effects_survive_later_layer_six_ability_removal() {
    let mut engine = game(26_102_707, &[0, 1]);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let lattice = enter_lattice(&mut engine);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(lattice),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
    assert!(has_type(&engine, creature, "Artifact"));
    assert!(engine.characteristics(creature).unwrap().colors.is_empty());
}

#[test]
fn lattice_loses_its_type_and_color_effects_to_song_of_the_dryads() {
    let mut engine = game(26_102_708, &[0, 1]);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let lattice = enter_lattice(&mut engine);
    assert!(engine.characteristics(creature).unwrap().colors.is_empty());
    inject_card_into_hand(&mut engine, 0, "song_of_the_dryads");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "song_of_the_dryads");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(lattice)))
        .unwrap();
    while !engine.state.stack.is_empty() {
        pass_priority_round(&mut engine);
    }
    assert!(has_type(&engine, lattice, "Forest"));
    assert!(!has_type(&engine, creature, "Artifact"));
    assert_eq!(
        engine.characteristics(creature).unwrap().colors,
        [Color::Red]
    );
}

#[test]
fn lattice_and_kenriths_transformation_obey_layer_five_timestamp() {
    let mut engine = game(26_102_709, &[0, 1]);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let lattice = enter_lattice(&mut engine);
    inject_card_into_hand(&mut engine, 0, "kenriths_transformation");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "kenriths_transformation");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .unwrap();
    while !engine.state.stack.is_empty() {
        pass_priority_round(&mut engine);
    }
    assert_eq!(
        engine.characteristics(creature).unwrap().colors,
        [Color::Green]
    );
    move_lattice_to_graveyard(&mut engine);
    let returned = move_ready_to_battlefield(&mut engine, 0, LATTICE);
    assert_eq!(returned, lattice);
    assert!(engine.characteristics(creature).unwrap().colors.is_empty());
}

#[test]
fn lattice_allows_a_colorless_filtered_library_top_cast_with_a_synthetic_permission() {
    let mut engine = game(26_102_710, &[0, 1]);
    let top = inject_library_card(&mut engine, 0, "grizzly_bears");
    engine.state.players[0]
        .library
        .retain(|object| *object != top);
    engine.state.players[0].library.push_front(top);
    // This is a synthetic permission, not a claim that Mystic Forge is implemented.
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Player(0),
        kind: ContinuousEffectKind::LibraryTopPermission {
            look_at_top: true,
            cast_filter: Some(tricerules_cards::primitives::ZoneCardFilter {
                is_colorless: Some(true),
                ..Default::default()
            }),
            play_lands: false,
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: 0,
    });
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let before = engine.initial_response_batch();
    assert!(before.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .all(|action| action.object_id != top || action.source_zone != 2));

    enter_lattice(&mut engine);
    let offered = engine.initial_response_batch();
    assert!(offered.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .any(|action| action.object_id == top && action.source_zone == 2));
    let view = last_zone_view(&offered);
    let owner = view
        .per_player
        .iter()
        .find(|view| view.player_id == 0)
        .unwrap();
    let opponent = view
        .per_player
        .iter()
        .find(|view| view.player_id == 1)
        .unwrap();
    assert_eq!(
        owner
            .private_library_top
            .as_ref()
            .map(|card| card.object_id),
        Some(top)
    );
    assert!(opponent.private_library_top.is_none());
    let generation = engine
        .state
        .zone_change_generation
        .get(&top)
        .copied()
        .unwrap_or(0);
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(rv1::BeginSpellCast {
                    announcement: Some(rv1::SpellCastAnnouncement {
                        source: Some(rv1::CastSource {
                            location: Some(rv1::cast_source::Location::LibraryObjectId(top)),
                            expected_zone_change_generation: Some(generation),
                        }),
                        face_index: 0,
                        cast_method: rv1::CastMethod::Normal as i32,
                        ..Default::default()
                    }),
                })),
            },
        )
        .expect("begin casting the actual colorless library-top card");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .unwrap()
        .transaction_id;
    let payment_state_revision = engine.state.command_index;
    let payment_source_generation = engine
        .state
        .zone_change_generation
        .get(&top)
        .copied()
        .unwrap_or(0);
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                    transaction_id,
                    payment: Some(rv1::PaymentSelection {
                        expected_state_revision: payment_state_revision,
                        source: Some(rv1::CostObjectRef {
                            object_id: top,
                            zone_change_generation: payment_source_generation,
                        }),
                        mana: Some(rv1::PaymentMana {
                            g: 1,
                            c: 1,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
        )
        .expect("commit the cast using the card's unchanged mana cost");
    assert_eq!(engine.state.objects[&top].zone, Zone::Stack);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&top].zone, Zone::Battlefield);
    assert!(engine.characteristics(top).unwrap().colors.is_empty());
    assert!(has_type(&engine, top, "Artifact"));
}

#[test]
fn lattice_does_not_replace_rings_activated_source_departure_lki() {
    for lattice_at_departure in [false, true] {
        let mut engine = game(26_102_711 + u64::from(lattice_at_departure), &[0, 1]);
        inject_permanent_on_battlefield(&mut engine, 0, "rings_of_brighthearth");
        let source = inject_creature_on_battlefield(&mut engine, 0, "royal_assassin");
        let legal = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let protected = inject_creature_on_battlefield(&mut engine, 1, "white_knight");
        engine.state.objects.get_mut(&legal).unwrap().tapped = true;
        engine.state.objects.get_mut(&protected).unwrap().tapped = true;
        if lattice_at_departure {
            enter_lattice(&mut engine);
        }
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let original_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        apply_ability(&mut engine, 0, source, 0, target_object(legal))
            .expect("activate Royal Assassin targeting the unprotected creature");
        engine.enable_dev_commands();
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 0,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: "Royal Assassin".into(),
                            zone: DevZone::Graveyard as i32,
                            ready: false,
                        })),
                    })),
                },
            )
            .expect("move the actual source through the normal zone-change path");
        assert_eq!(
            move_ready_to_battlefield(&mut engine, 0, "royal_assassin"),
            source
        );
        assert_eq!(
            engine.state.zone_change_generation[&source],
            original_generation + 2
        );
        if lattice_at_departure {
            move_lattice_to_graveyard(&mut engine);
            assert_eq!(
                engine.characteristics(source).unwrap().colors,
                [Color::Black]
            );
        } else {
            enter_lattice(&mut engine);
            assert!(engine.characteristics(source).unwrap().colors.is_empty());
        }

        let batch = pass_both_players_with_batch(&mut engine);
        let branch = find_resolution_choice(&batch).expect("Rings offers its optional copy");
        assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
        let payment = engine
            .apply_scenario_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
            )
            .expect("choose to pay for the copy");
        assert_eq!(
            find_resolution_choice(&payment).unwrap().choice_kind(),
            ChoiceKind::ManaPayment
        );
        let copy_offer =
            submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
                .expect("pay {2} for the copy");
        let choice = find_resolution_choice(&copy_offer).expect("copy retarget offer");
        assert_eq!(choice.candidate_object_ids.contains(&protected), lattice_at_departure,
            "White Knight protection uses the old source's departure colors, not its returned incarnation");
        let pushed = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .unwrap();
        let copy = pushed
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::StackPushed(push)) if push.is_copy => Some(push),
                _ => None,
            })
            .expect("Rings publishes the copy");
        assert!(
            copy.ability_source.is_none(),
            "the returned incarnation is not the old source"
        );
    }
}
