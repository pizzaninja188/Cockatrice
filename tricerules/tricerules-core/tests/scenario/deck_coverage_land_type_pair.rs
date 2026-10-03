//! Paid exact-card commands for the additive and setting basic-land effects.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{AttachmentRecipient, Zone};

const YAVIMAYA: &str = "yavimaya,_cradle_of_growth";
const SONG: &str = "song_of_the_dryads";

fn setup(seed: u64, players: &[i32]) -> GameEngine {
    let deck = deck_with(
        "forest",
        &[
            YAVIMAYA,
            SONG,
            SONG,
            "disenchant",
            "disenchant",
            "sol_ring",
            "island",
            "gift_of_paradise",
            "clever_impersonator",
            "thorin_oakenshield",
            "kopala,_warden_of_waves",
            "coral_merfolk",
            "unsummon",
            "unsummon",
            "prodigal_sorcerer",
        ],
    );
    let mut engine =
        GameEngine::new(seed, players, 20, Some(vec![deck; players.len()]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn play_yavimaya(engine: &mut GameEngine) -> u32 {
    ensure_card_in_hand(engine, 0, YAVIMAYA);
    let slot = hand_index_for_card(engine, 0, YAVIMAYA);
    let object = engine.state.players[0].hand[slot];
    engine.apply_command(0, &play_land(slot)).unwrap();
    object
}

fn song(engine: &mut GameEngine, target: u32) -> u32 {
    ensure_card_in_hand(engine, 0, SONG);
    let slot = hand_index_for_card(engine, 0, SONG);
    let aura = engine.state.players[0].hand[slot];
    give_mana(
        engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    while !engine.state.stack.is_empty() {
        pass_priority_round(engine);
    }
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(target))
    );
    aura
}

fn disenchant(engine: &mut GameEngine, target: u32) {
    ensure_card_in_hand(engine, 0, "disenchant");
    give_mana(
        engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "disenchant");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    while !engine.state.stack.is_empty() {
        pass_priority_round(engine);
    }
}

#[test]
fn song_suppresses_printed_targeting_tax_in_offers_and_paid_execution_then_restores_it() {
    let mut engine = setup(26_100_215, &[0, 1]);
    let kopala = relocate_to_battlefield(&mut engine, 1, "kopala,_warden_of_waves", false);
    let merfolk = relocate_to_battlefield(&mut engine, 1, "coral_merfolk", false);
    let sorcerer = relocate_to_battlefield(&mut engine, 0, "prodigal_sorcerer", false);
    ensure_card_in_hand(&mut engine, 0, "unsummon");
    let assert_offered_tax = |engine: &mut GameEngine, expected: bool| {
        let slot = hand_index_for_card(engine, 0, "unsummon");
        let batch = engine.initial_response_batch();
        let spell = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
        let ability =
            &batch.legal_by_player[&0].valid_targets_by_ability[&(u64::from(sorcerer) << 32)];
        for offered in [spell, ability] {
            assert_eq!(!offered.targeting_cost_applications.is_empty(), expected);
            if expected {
                assert_eq!(offered.targeting_cost_applications[0].generic_mana, 2);
                assert!(offered.targeting_cost_applications[0]
                    .affected_targets
                    .iter()
                    .any(|target| target.object_id == merfolk));
            }
        }
    };
    assert_offered_tax(&mut engine, true);

    // Song itself must pay Kopala's still-present {2} tax before the source is transformed.
    ensure_card_in_hand(&mut engine, 0, SONG);
    let slot = hand_index_for_card(&engine, 0, SONG);
    let aura = engine.state.players[0].hand[slot];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            g: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(slot, target_object(kopala)))
        .unwrap();
    while !engine.state.stack.is_empty() {
        pass_priority_round(&mut engine);
    }
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(kopala))
    );
    assert_offered_tax(&mut engine, false);

    // The separate Merfolk is still a creature, so losing Kopala's own types cannot explain this.
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(slot, target_object(merfolk)))
        .expect("suppressed Kopala does not charge a targeting surcharge");
    while !engine.state.stack.is_empty() {
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.objects[&merfolk].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    let restored_merfolk = relocate_to_battlefield(&mut engine, 1, "coral_merfolk", false);
    assert_eq!(restored_merfolk, merfolk);
    disenchant(&mut engine, aura);
    ensure_card_in_hand(&mut engine, 0, "unsummon");
    assert_offered_tax(&mut engine, true);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let before = engine.diagnostic_snapshot().unwrap();
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(merfolk)))
        .expect_err("restored Kopala again requires two generic mana");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn yavimaya_paid_land_play_adds_forest_to_every_players_lands_and_preserves_other_abilities() {
    let mut engine = setup(26_100_201, &[0, 1, 2, 3]);
    let islands: Vec<_> = (0..4)
        .map(|p| inject_permanent_on_battlefield(&mut engine, p, "island"))
        .collect();
    let artifact = relocate_to_battlefield(&mut engine, 1, "sol_ring", false);
    let off_battlefield = relocate_to_hand(&mut engine, 2, "island");
    let yavimaya = play_yavimaya(&mut engine);
    for object in islands.iter().copied().chain([yavimaya]) {
        let current = engine.characteristics(object).unwrap();
        assert!(current.types.contains(&"Forest".to_owned()));
    }
    assert_eq!(
        engine.characteristics(yavimaya).unwrap().supertypes,
        ["Legendary"]
    );
    assert_eq!(
        engine.characteristics(yavimaya).unwrap().names,
        ["Yavimaya, Cradle of Growth"]
    );
    assert_eq!(
        engine.characteristics(islands[1]).unwrap().types,
        ["Land", "Island", "Forest"]
    );
    assert_eq!(
        engine.characteristics(artifact).unwrap().types,
        ["Artifact"]
    );
    assert_eq!(
        engine.characteristics(off_battlefield).unwrap().types,
        ["Land", "Island"]
    );
    let activation = activate_ability_for(&engine, yavimaya, 0, vec![]);
    let before = engine.state.command_index;
    engine
        .apply_command(1, &activation)
        .expect_err("wrong controller cannot use derived mana");
    assert_eq!(engine.state.command_index, before);
    engine.apply_command(0, &activation).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert!(engine.state.stack.is_empty());
    let mut island_activation = activate_ability_for(&engine, islands[0], 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = island_activation.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = 1;
    engine.apply_command(0, &island_activation).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.green, 2);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    engine.apply_command(0, &pass()).unwrap();
    let mut opponent_activation = activate_ability_for(&engine, islands[1], 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = opponent_activation.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = 1;
    engine.apply_command(1, &opponent_activation).unwrap();
    assert_eq!(engine.state.players[1].mana_pool.green, 1);
}

#[test]
fn song_paid_cast_sets_colorless_forest_preserves_identity_and_restores_when_destroyed() {
    for (seed, card) in [
        (26_100_202, "sol_ring"),
        (26_100_203, "zetalpa,_primal_dawn"),
        (26_100_204, "island"),
        (26_100_205, "jace,_wielder_of_mysteries"),
        (26_100_206, "garruks_uprising"),
    ] {
        let mut engine = setup(seed, &[0, 1]);
        let target = inject_permanent_on_battlefield(&mut engine, 0, card);
        if card == "jace,_wielder_of_mysteries" {
            engine.state.objects.get_mut(&target).unwrap().add_counters(
                tricerules_cards::CounterKind::Loyalty,
                4,
                engine.state.command_index,
            );
        }
        let original = engine.characteristics(target).unwrap();
        let aura = song(&mut engine, target);
        let current = engine.characteristics(target).unwrap();
        assert_eq!(current.names, original.names);
        assert_eq!(current.supertypes, original.supertypes);
        assert_eq!(current.types, ["Land", "Forest"]);
        assert!(current.colors.is_empty());
        assert!(current.keywords.is_empty());
        let authored_span = tricerules_cards::CardRegistry::global()
            .get(card)
            .unwrap()
            .primary_face()
            .activated_abilities
            .len() as u32;
        let derived_index = if card == "island" { 0 } else { authored_span };
        let command = activate_ability_for(&engine, target, derived_index, vec![]);
        engine.apply_command(0, &command).unwrap();
        assert_eq!(engine.state.players[0].mana_pool.green, 1);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        assert!(engine.state.stack.is_empty());
        disenchant(&mut engine, aura);
        let restored = engine.characteristics(target).unwrap();
        assert_eq!(restored.types, original.types);
        assert_eq!(restored.colors, original.colors);
        assert_eq!(restored.keywords, original.keywords);
        assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    }
}

#[test]
fn song_on_yavimaya_suppresses_its_global_static_and_restores_it_after_departure() {
    let mut engine = setup(26_100_207, &[0, 1]);
    let island = inject_permanent_on_battlefield(&mut engine, 1, "island");
    let yavimaya = play_yavimaya(&mut engine);
    assert!(engine
        .characteristics(island)
        .unwrap()
        .types
        .contains(&"Forest".to_owned()));
    let aura = song(&mut engine, yavimaya);
    assert_eq!(
        engine.characteristics(island).unwrap().types,
        ["Land", "Island"]
    );
    assert_eq!(
        engine.characteristics(yavimaya).unwrap().types,
        ["Land", "Forest"]
    );
    disenchant(&mut engine, aura);
    assert!(engine
        .characteristics(island)
        .unwrap()
        .types
        .contains(&"Forest".to_owned()));
}

#[test]
fn song_detaches_a_former_equipment_and_does_not_move_it_or_detach_song() {
    let mut engine = setup(26_100_208, &[0, 1]);
    let host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let equipment = inject_permanent_on_battlefield(&mut engine, 1, "skullclamp");
    engine
        .state
        .objects
        .get_mut(&equipment)
        .unwrap()
        .attached_to = Some(AttachmentRecipient::Object(host));
    let aura = song(&mut engine, equipment);
    assert_eq!(engine.state.objects[&equipment].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(equipment))
    );
    assert_eq!(engine.characteristics(host).unwrap().power, Some(2));
    disenchant(&mut engine, aura);
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
}

#[test]
fn song_rejects_unaffordable_and_player_targets_without_mutating_state() {
    let mut engine = setup(26_100_209, &[0, 1]);
    ensure_card_in_hand(&mut engine, 0, SONG);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let slot = hand_index_for_card(&engine, 0, SONG);
    let before = engine.state.command_index;
    let hand = engine.state.players[0].hand.clone();
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect_err("must pay 2G");
    assert_eq!(engine.state.command_index, before);
    assert_eq!(engine.state.players[0].hand, hand);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(slot, target_player(1)))
        .expect_err("Enchant permanent excludes players");
    assert_eq!(engine.state.command_index, before);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn song_public_projection_exposes_current_forest_type_and_retains_no_printed_flying() {
    let mut engine = setup(26_100_210, &[0, 1]);
    let target = inject_creature_on_battlefield(&mut engine, 1, "zetalpa,_primal_dawn");
    song(&mut engine, target);
    assert!(!engine.effective_has_keyword(target, Keyword::Flying));
    assert!(zone_view_rules_annotation_labels(&mut engine, 1, target)
        .contains(&"Basic land types: Forest".to_string()));
}

#[test]
fn song_fizzles_on_a_new_target_incarnation_and_rejects_its_stale_mana_action() {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
    let mut engine = setup(26_100_211, &[0, 1]);
    let target = inject_creature_on_battlefield(&mut engine, 0, "zetalpa,_primal_dawn");
    let aura = song(&mut engine, target);
    let stale_mana = activate_ability_for(&engine, target, 0, vec![]);
    disenchant(&mut engine, aura);
    ensure_card_in_hand(&mut engine, 0, SONG);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, SONG);
    let second_aura = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    let generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    engine.enable_dev_commands();
    for zone in [DevZone::Graveyard, DevZone::Battlefield] {
        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 0,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: "Zetalpa, Primal Dawn".into(),
                            zone: zone as i32,
                            ready: true,
                        })),
                    })),
                },
            )
            .unwrap();
    }
    assert_eq!(engine.state.zone_change_generation[&target], generation + 2);
    assert_eq!(
        engine.state.objects[&target].card_id,
        "zetalpa,_primal_dawn"
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    let before = engine.diagnostic_snapshot().unwrap();
    engine
        .apply_command(0, &stale_mana)
        .expect_err("generation-bound action cannot follow the physical card");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    while !engine.state.stack.is_empty() {
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.objects[&second_aura].zone, Zone::Graveyard);
    assert!(engine.characteristics(target).unwrap().has_type("Creature"));
    assert!(!engine.characteristics(target).unwrap().has_type("Land"));
    assert!(engine.effective_has_keyword(target, Keyword::Flying));
}

#[test]
fn song_paid_response_destroying_its_only_target_fizzles_without_a_phantom_land() {
    let mut engine = setup(26_100_214, &[0, 1]);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    ensure_card_in_hand(&mut engine, 0, SONG);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, SONG);
    let aura = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    disenchant(&mut engine, target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&aura].attached_to, None);
    assert_eq!(engine.characteristics(target).unwrap().types, ["Artifact"]);
    assert!(!engine.state.players[1].battlefield.contains(&target));
}

#[test]
fn song_retains_independent_mana_grants_in_either_order_and_restores_original_mana() {
    for gift_first in [false, true] {
        let mut engine = setup(26_100_215 + u64::from(gift_first), &[0, 1]);
        let island = inject_permanent_on_battlefield(&mut engine, 0, "island");
        let cast_gift = |engine: &mut GameEngine| {
            ensure_card_in_hand(engine, 0, "gift_of_paradise");
            give_mana(
                engine,
                0,
                ManaGift {
                    g: 1,
                    c: 2,
                    ..Default::default()
                },
            );
            let slot = hand_index_for_card(engine, 0, "gift_of_paradise");
            engine
                .apply_command(0, &cast_spell(slot, target_object(island)))
                .unwrap();
            resolve_entire_stack_two_player(engine);
        };
        if gift_first {
            cast_gift(&mut engine);
        }
        let aura = song(&mut engine, island);
        if !gift_first {
            cast_gift(&mut engine);
        }
        // Island's authored intrinsic marker already owns slot 0; the grant appends at 1.
        let mut grant = activate_ability_for(&engine, island, 1, vec![]);
        let Some(Cmd::ActivateAbility(ability)) = grant.cmd.as_mut() else {
            unreachable!()
        };
        ability.mana_option_index = 1;
        engine.apply_command(0, &grant).unwrap();
        assert_eq!(engine.state.players[0].mana_pool.blue, 2);
        engine.apply_command(0, &undo_mana_ability()).unwrap();
        let intrinsic = activate_ability_for(&engine, island, 0, vec![]);
        engine.apply_command(0, &intrinsic).unwrap();
        assert_eq!(engine.state.players[0].mana_pool.green, 1);
        engine.apply_command(0, &undo_mana_ability()).unwrap();
        disenchant(&mut engine, aura);
        let intrinsic = activate_ability_for(&engine, island, 0, vec![]);
        engine.apply_command(0, &intrinsic).unwrap();
        assert_eq!(engine.state.players[0].mana_pool.blue, 1);
        engine.apply_command(0, &undo_mana_ability()).unwrap();
        engine.apply_command(0, &grant).unwrap();
        assert_eq!(engine.state.players[0].mana_pool.blue, 2);
    }
}

#[test]
fn land_pair_serialized_paid_commands_replay_exact_public_batches_and_current_indices() {
    use prost::Message;
    fn fresh() -> GameEngine {
        let mut engine = setup(26_100_217, &[0, 1]);
        inject_permanent_on_battlefield(&mut engine, 1, "island");
        ensure_card_in_hand(&mut engine, 0, YAVIMAYA);
        ensure_card_in_hand(&mut engine, 0, SONG);
        ensure_card_in_hand(&mut engine, 0, "disenchant");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                g: 1,
                w: 1,
                c: 3,
                ..Default::default()
            },
        );
        engine
    }
    let mut engine = fresh();
    let mut recorded = Vec::new();
    let record = |engine: &mut GameEngine, recorded: &mut Vec<_>, actor, command: RuledCommand| {
        let batch = engine.apply_command(actor, &command).unwrap();
        recorded.push((actor, command, batch));
    };
    let slot = hand_index_for_card(&engine, 0, YAVIMAYA);
    let yavimaya = engine.state.players[0].hand[slot];
    record(&mut engine, &mut recorded, 0, play_land(slot));
    let slot = hand_index_for_card(&engine, 0, SONG);
    let aura = engine.state.players[0].hand[slot];
    record(
        &mut engine,
        &mut recorded,
        0,
        cast_spell(slot, target_object(yavimaya)),
    );
    while !engine.state.stack.is_empty() {
        let actor = engine.state.priority_player_id();
        record(&mut engine, &mut recorded, actor, pass());
    }
    let command = activate_ability_for(&engine, yavimaya, 0, vec![]);
    record(&mut engine, &mut recorded, 0, command);
    let slot = hand_index_for_card(&engine, 0, "disenchant");
    record(
        &mut engine,
        &mut recorded,
        0,
        cast_spell(slot, target_object(aura)),
    );
    while !engine.state.stack.is_empty() {
        let actor = engine.state.priority_player_id();
        record(&mut engine, &mut recorded, actor, pass());
    }
    let mut replay = fresh();
    for (actor, command, batch) in recorded {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), batch);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap()
    );
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert!(engine.characteristics(yavimaya).unwrap().has_type("Forest"));
    let island = battlefield_object_for_card(&engine, 1, "island");
    assert!(engine.characteristics(island).unwrap().has_type("Forest"));
}

#[test]
fn song_targeting_a_face_down_permanent_preserves_granted_trigger_without_revealing_identity() {
    let mut engine = setup(26_100_218, &[0, 1]);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    engine.state.objects.get_mut(&target).unwrap().face_down = true;
    move_ready_to_battlefield(&mut engine, 1, "thorin_oakenshield");
    engine.state.players[1].has_enduring_story = true;
    ensure_card_in_hand(&mut engine, 0, SONG);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, SONG);
    let activated = engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    assert_eq!(
        engine.state.stack.len(),
        2,
        "independent granted trigger stacks above Song"
    );
    let trigger_id = engine.state.stack.last().unwrap().id;
    assert!(engine.state.stack.last().unwrap().is_triggered);
    for event in &activated.events {
        if matches!(&event.ev, Some(Ev::Log(_)) | Some(Ev::StackPushed(_))) {
            let text = format!("{event:?}");
            assert!(
                !text.contains("Sol Ring") && !text.contains("sol_ring"),
                "{text}"
            );
        }
    }
    // The public event label remains captured if the permanent is revealed before resolution.
    engine.state.objects.get_mut(&target).unwrap().face_down = false;
    let mut events = Vec::new();
    for _ in 0..4 {
        if engine
            .state
            .stack
            .last()
            .is_none_or(|item| item.id != trigger_id)
            || engine.state.pending_resolution.is_some()
        {
            break;
        }
        let actor = engine.state.priority_player_id();
        events.extend(engine.apply_command(actor, &pass()).unwrap().events);
    }
    assert!(engine.state.pending_resolution.is_some());
    for event in &events {
        if matches!(
            &event.ev,
            Some(Ev::Log(_)) | Some(Ev::ResolutionChoiceRequired(_))
        ) {
            let text = format!("{event:?}");
            assert!(
                !text.contains("Sol Ring") && !text.contains("sol_ring"),
                "{text}"
            );
        }
    }
    submit_mana_resolution(
        &mut engine,
        0,
        SubmitResolutionChoice {
            decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::PayMana as i32,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(engine.state.pending_resolution.is_none());
    for _ in 0..4 {
        if engine.state.stack.is_empty() {
            break;
        }
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.characteristics(target).unwrap().types,
        ["Land", "Forest"]
    );
}

#[test]
fn yavimaya_preserves_printed_utility_abilities_and_song_keeps_an_independent_grant() {
    let mut engine = setup(26_100_212, &[0, 1]);
    let grove = inject_permanent_on_battlefield(&mut engine, 0, "waterlogged_grove");
    play_yavimaya(&mut engine);
    let before_life = engine.state.players[0].life;
    let command = activate_ability_for(&engine, grove, 0, vec![]);
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].life, before_life - 1);
    // Reset this fixture's tap state; the life payment makes its printed ability nonundoable.
    engine.state.objects.get_mut(&grove).unwrap().tapped = false;
    let life_after_printed = engine.state.players[0].life;
    let command = activate_ability_for(&engine, grove, 2, vec![]);
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].life, life_after_printed);
    engine.apply_command(0, &undo_mana_ability()).unwrap();
    ensure_card_in_hand(&mut engine, 0, "gift_of_paradise");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "gift_of_paradise");
    engine
        .apply_command(0, &cast_spell(slot, target_object(grove)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    song(&mut engine, grove);
    let before = engine.diagnostic_snapshot().unwrap();
    let printed = activate_ability_for(&engine, grove, 0, vec![]);
    engine
        .apply_command(0, &printed)
        .expect_err("Song removes printed utility mana");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    let mut granted = activate_ability_for(&engine, grove, 3, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = granted.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = 1;
    engine.apply_command(0, &granted).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.blue, 2);
}

#[test]
fn copied_song_uses_its_current_aura_recipient_and_detaches_a_transformed_aura() {
    let mut engine = setup(26_100_213, &[0, 1]);
    let first = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let original_song = song(&mut engine, first);
    let host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let pacifism = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
    engine.state.objects.get_mut(&pacifism).unwrap().attached_to =
        Some(AttachmentRecipient::Object(host));
    ensure_card_in_hand(&mut engine, 0, "clever_impersonator");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "clever_impersonator");
    let copy = engine.state.players[0].hand[slot];
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![original_song]))
        .unwrap();
    engine
        .apply_command(0, &submit_resolution_choice(vec![pacifism]))
        .unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&copy].attached_to,
        Some(AttachmentRecipient::Object(pacifism))
    );
    assert_eq!(engine.state.objects[&pacifism].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&pacifism].attached_to, None);
    assert_eq!(
        engine.characteristics(pacifism).unwrap().types,
        ["Land", "Forest"]
    );
    disenchant(&mut engine, copy);
    assert_eq!(
        engine.state.objects[&pacifism].zone,
        Zone::Graveyard,
        "restored unattached Aura goes to its owner's graveyard"
    );
    assert_eq!(
        engine.state.objects[&original_song].attached_to,
        Some(AttachmentRecipient::Object(first))
    );
    assert_eq!(
        engine.characteristics(first).unwrap().types,
        ["Land", "Forest"]
    );
}
