//! Boseiju's real hand activation, counted discount, required targets and optional search.
use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    AbilitySourceZone, PaymentMana, PreviewPayment, ResolutionChoiceDecision,
};

const BOSEIJU: &str = "boseiju,_who_endures";

fn setup(seed: u64) -> GameEngine {
    let deck = deck_with(
        "forest",
        &[
            BOSEIJU,
            "sol_ring",
            "liquimetal_torque",
            "kopala,_warden_of_waves",
            "coral_merfolk",
            "ghalta,_primal_hunger",
            "thorin_oakenshield",
            "yavimaya,_cradle_of_growth",
            "song_of_the_dryads",
            "taiga",
            "disenchant",
        ],
    );
    let mut engine = GameEngine::new(seed, &[0, 1], 20, Some(vec![deck; 2]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn channel_source(engine: &mut GameEngine) -> u32 {
    ensure_card_in_hand(engine, 0, BOSEIJU);
    engine.state.players[0].hand[hand_index_for_card(engine, 0, BOSEIJU)]
}

fn preview(
    engine: &GameEngine,
    command: &RuledCommand,
) -> tricerules_proto::ruled::v1::PaymentPreview {
    let Some(Cmd::ActivateAbility(ability)) = &command.cmd else {
        unreachable!()
    };
    engine.preview_payment(
        0,
        &PreviewPayment {
            activate_ability: Some(ability.clone()),
            ..Default::default()
        },
    )
}

fn paid(engine: &GameEngine, command: &RuledCommand, generic: u32, green: u32) -> RuledCommand {
    let mut selection = preview(engine, command).selection.unwrap();
    selection.mana = Some(PaymentMana {
        c: generic,
        g: green,
        ..Default::default()
    });
    let mut command = command.clone();
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.payment = Some(selection);
    command
}

fn resolve(engine: &mut GameEngine) -> RuledEventBatch {
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap()
}

#[test]
fn boseiju_channel_counted_reduction_consumes_kopala_tax() {
    for count in 0..=3usize {
        let mut engine = setup(202_610_030 + count as u64);
        let torque = move_ready_to_battlefield(&mut engine, 0, "liquimetal_torque");
        move_ready_to_battlefield(&mut engine, 1, "kopala,_warden_of_waves");
        let target = move_ready_to_battlefield(&mut engine, 1, "coral_merfolk");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        apply_ability(&mut engine, 0, torque, 1, target_object(target)).unwrap();
        resolve(&mut engine);
        for card in [
            "ghalta,_primal_hunger",
            "thorin_oakenshield",
            "kopala,_warden_of_waves",
        ]
        .into_iter()
        .take(count)
        {
            move_ready_to_battlefield(&mut engine, 0, card);
        }
        // Legendary noncreatures and opponent legends never contribute to the discount.
        move_ready_to_battlefield(&mut engine, 0, "yavimaya,_cradle_of_growth");
        let source = channel_source(&mut engine);
        let command = channel(&engine, source, target);
        let before = format!("{:?}", engine.state);
        let quote = preview(&engine, &command);
        assert!(quote.valid, "{quote:?}");
        let generic = 3 - count as u32;
        assert_eq!(
            quote.total_cost,
            if generic == 0 {
                "{G}".into()
            } else {
                format!("{{{generic}}}{{G}}")
            }
        );
        assert_eq!(quote.remaining_cost, quote.total_cost);
        assert_eq!(
            format!("{:?}", engine.state),
            before,
            "preview must be read-only"
        );
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: generic,
                g: 1,
                ..Default::default()
            },
        );
        let underpaid = paid(
            &engine,
            &command,
            generic.saturating_sub(1),
            if generic == 0 { 0 } else { 1 },
        );
        let before = format!("{:?}", engine.state);
        assert!(engine.apply_command(0, &underpaid).is_err());
        assert_eq!(
            format!("{:?}", engine.state),
            before,
            "rejected payment is atomic"
        );
        let exact = paid(&engine, &command, generic, 1);
        engine.apply_command(0, &exact).unwrap();
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(engine.state.players[0].mana_pool.green, 0);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        assert_eq!(engine.state.stack.len(), 1);
    }
}

#[test]
fn boseiju_optional_search_uses_destroyed_targets_controller_and_all_basic_subtypes() {
    let mut engine = setup(202_610_040);
    let target = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    engine.state.players[0]
        .battlefield
        .retain(|id| *id != target);
    engine.state.players[1].battlefield.push(target);
    let object = engine.state.objects.get_mut(&target).unwrap();
    object.base_controller = 1;
    object.controller = 1;
    let eligible: Vec<_> = ["plains", "island", "swamp", "mountain", "forest", "taiga"]
        .into_iter()
        .map(|card| inject_library_card(&mut engine, 1, card))
        .collect();
    let ineligible = inject_library_card(&mut engine, 1, "wastes");
    let source = channel_source(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &channel(&engine, source, target))
        .unwrap();
    let batch = resolve(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&target));
    assert_eq!(
        find_resolution_choice(&batch).unwrap().deciding_player_id,
        1
    );
    let accept = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    let before = format!("{:?}", engine.state);
    assert!(engine.apply_command(0, &accept).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    let batch = engine.apply_command(1, &accept).unwrap();
    let choice = find_resolution_choice(&batch).unwrap();
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    for id in &eligible {
        assert!(choice.candidate_object_ids.contains(id));
    }
    assert!(!choice.candidate_object_ids.contains(&ineligible));
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![eligible[5]]))
        .is_err());
    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![ineligible]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    let completion = engine
        .apply_command(1, &submit_resolution_choice(vec![eligible[5]]))
        .unwrap();
    let found = &engine.state.objects[&eligible[5]];
    assert_eq!(found.zone, Zone::Battlefield);
    assert_eq!(found.controller, 1);
    assert!(!found.tapped);
    assert!(completion.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P1 shuffles their library.")
    ));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn boseiju_illegal_targets_missing_targets_and_stale_sources_preserve_resources() {
    let mut engine = setup(202_610_041);
    let source = channel_source(&mut engine);
    let own = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let basic = move_ready_to_battlefield(&mut engine, 1, "forest");
    let creature = move_ready_to_battlefield(&mut engine, 1, "coral_merfolk");
    let legal = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    let mut missing = channel(&engine, source, legal);
    if let Some(Cmd::ActivateAbility(ability)) = missing.cmd.as_mut() {
        ability.targets.clear();
    }
    let mut stale = channel(&engine, source, legal);
    if let Some(Cmd::ActivateAbility(ability)) = stale.cmd.as_mut() {
        ability.expected_zone_change_generation += 1;
    }
    let mut wrong_zone = channel(&engine, source, legal);
    if let Some(Cmd::ActivateAbility(ability)) = wrong_zone.cmd.as_mut() {
        ability.source_zone = AbilitySourceZone::Battlefield as i32;
    }
    for command in [
        channel(&engine, source, own),
        channel(&engine, source, basic),
        channel(&engine, source, creature),
        missing,
        stale,
        wrong_zone,
    ] {
        let before = format!("{:?}", engine.state);
        assert!(!preview(&engine, &command).valid);
        assert!(engine.apply_command(0, &command).is_err());
        assert_eq!(format!("{:?}", engine.state), before);
    }
}

#[test]
fn boseiju_decline_does_not_shuffle_but_accepted_failure_to_find_does() {
    for decline in [true, false] {
        let mut engine = setup(202_610_042);
        let target = inject_permanent_on_battlefield(&mut engine, 1, "darksteel_citadel");
        let source = channel_source(&mut engine);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                g: 1,
                ..Default::default()
            },
        );
        engine
            .apply_command(0, &channel(&engine, source, target))
            .unwrap();
        let batch = resolve(&mut engine);
        assert_eq!(
            engine.state.objects[&target].zone,
            Zone::Battlefield,
            "indestructible target remains legal"
        );
        assert_eq!(
            find_resolution_choice(&batch).unwrap().deciding_player_id,
            1
        );
        let before = engine.state.players[1].library.clone();
        let batch = engine
            .apply_command(
                1,
                &submit_resolution_decision(if decline {
                    ResolutionChoiceDecision::Decline
                } else {
                    ResolutionChoiceDecision::SelectBranch
                }),
            )
            .unwrap();
        if decline {
            assert_eq!(engine.state.players[1].library, before);
            assert!(!batch.events.iter().any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P1 shuffles their library.")));
        } else {
            assert!(find_resolution_choice(&batch).is_some());
            let batch = engine
                .apply_command(1, &submit_resolution_choice(vec![]))
                .unwrap();
            assert_eq!(batch.events.iter().filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P1 shuffles their library.")).count(), 1);
        }
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn boseiju_stale_private_search_keeps_the_choice_and_recovers() {
    let mut engine = setup(202_610_043);
    let target = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    let found = inject_library_card(&mut engine, 1, "taiga");
    let source = channel_source(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &channel(&engine, source, target))
        .unwrap();
    resolve(&mut engine);
    engine
        .apply_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .unwrap();
    *engine
        .state
        .zone_change_generation
        .entry(found)
        .or_default() += 1;
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![found]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    engine
        .apply_command(1, &submit_resolution_choice(vec![]))
        .unwrap();
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn boseiju_fizzle_after_paid_response_retains_discard_without_search() {
    let mut engine = setup(202_610_044);
    let target = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    let source = channel_source(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &channel(&engine, source, target))
        .unwrap();
    engine.apply_command(0, &pass()).unwrap();
    ensure_card_in_hand(&mut engine, 1, "disenchant");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 1,
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "disenchant");
    engine
        .apply_command(1, &cast_spell(slot, target_object(target)))
        .unwrap();
    let mut batches = Vec::new();
    while !engine.state.stack.is_empty() {
        let actor = engine.state.priority_player_id();
        batches.push(engine.apply_command(actor, &pass()).unwrap());
    }
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(batches
        .iter()
        .all(|batch| find_resolution_choice(batch).is_none()));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn boseiju_countered_channel_retains_paid_discard_without_search() {
    let mut engine = setup(202_610_045);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "marauding_brinefang");
    let torque = move_ready_to_battlefield(&mut engine, 0, "liquimetal_torque");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, torque, 1, target_object(target)).unwrap();
    resolve(&mut engine);
    submit_mana_resolution(
        &mut engine,
        0,
        SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::PayMana as i32,
            ..Default::default()
        },
    )
    .unwrap();
    resolve(&mut engine);
    assert!(engine.characteristics(target).unwrap().has_type("Artifact"));
    let source = channel_source(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &channel(&engine, source, target))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 2);
    resolve(&mut engine);
    let batch = engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .unwrap();
    assert!(batch
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::StackObjectCountered(_)))));
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

fn channel(engine: &GameEngine, source: u32, target: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 1, target_object(target));
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.source_zone = AbilitySourceZone::Hand as i32;
    command
}

#[test]
fn boseiju_channel_preview_keeps_colored_cost_and_discards_as_cost() {
    let mut engine = setup(202_610_020);
    ensure_card_in_hand(&mut engine, 0, BOSEIJU);
    let source = engine.state.players[0].hand[hand_index_for_card(&engine, 0, BOSEIJU)];
    let target = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    let command = channel(&engine, source, target);
    let Some(Cmd::ActivateAbility(ability)) = &command.cmd else {
        unreachable!()
    };
    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            activate_ability: Some(ability.clone()),
            ..Default::default()
        },
    );
    assert!(preview.valid, "{preview:?}");
    assert_eq!(preview.total_cost, "{1}{G}");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &command).unwrap();
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(engine.state.stack.len(), 1);
}

fn paid_cast(engine: &mut GameEngine, card: &str, targets: Vec<TargetRef>, mana: ManaGift) -> u32 {
    let object = inject_card_into_hand(engine, 0, card);
    give_mana(engine, 0, mana);
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == object)
        .unwrap();
    engine.apply_command(0, &cast_spell(slot, targets)).unwrap();
    resolve(engine);
    object
}

#[test]
fn boseiju_discount_uses_copied_and_token_legendary_characteristics() {
    let mut engine = setup(202_610_046);
    let source = channel_source(&mut engine);
    let target = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    let original = inject_permanent_on_battlefield(&mut engine, 1, "isamaru,_hound_of_konda");
    assert_eq!(
        preview(&engine, &channel(&engine, source, target)).total_cost,
        "{1}{G}"
    );
    let clone = paid_cast(
        &mut engine,
        "clone",
        vec![],
        ManaGift {
            c: 3,
            u: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![original]))
        .unwrap();
    assert!(engine.state.objects[&clone].copiable_values.is_some());
    let characteristics = engine.characteristics(clone).unwrap();
    assert!(characteristics
        .supertypes
        .iter()
        .any(|kind| kind == "Legendary"));
    assert!(characteristics.has_type("Creature"));
    assert_eq!(
        preview(&engine, &channel(&engine, source, target)).total_cost,
        "{G}"
    );

    let mut engine = setup(202_610_047);
    let source = channel_source(&mut engine);
    let target = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    let original = inject_permanent_on_battlefield(&mut engine, 0, "isamaru,_hound_of_konda");
    paid_cast(
        &mut engine,
        "cackling_counterpart",
        target_object(original),
        ManaGift {
            c: 1,
            u: 2,
            ..Default::default()
        },
    );
    let choice = &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation;
    assert_eq!(choice.choice_kind, ChoiceKind::LegendKeep);
    let token = *engine.state.players[0]
        .battlefield
        .iter()
        .find(|id| engine.state.objects[id].is_token())
        .unwrap();
    engine
        .apply_command(0, &submit_resolution_choice(vec![token]))
        .unwrap();
    assert_eq!(engine.state.objects[&original].zone, Zone::Graveyard);
    assert!(engine.state.objects[&token].is_token());
    let characteristics = engine.characteristics(token).unwrap();
    assert!(characteristics
        .supertypes
        .iter()
        .any(|kind| kind == "Legendary"));
    assert!(characteristics.has_type("Creature"));
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        preview(&engine, &channel(&engine, source, target)).total_cost,
        "{G}"
    );
}

#[test]
fn boseiju_discount_excludes_face_down_legend_and_tracks_song_removal() {
    let mut engine = setup(202_610_048);
    let source = channel_source(&mut engine);
    let target = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    let legend = inject_library_card(&mut engine, 0, "isamaru,_hound_of_konda");
    let forest = inject_library_card(&mut engine, 0, "forest");
    engine.state.players[0]
        .library
        .retain(|id| *id != legend && *id != forest);
    engine.state.players[0].library.push_front(forest);
    engine.state.players[0].library.push_front(legend);
    paid_cast(
        &mut engine,
        "manifest_dread",
        vec![],
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![legend]))
        .unwrap();
    assert!(engine.state.objects[&legend].face_down);
    assert_eq!(
        engine.state.objects[&legend].card_id,
        "isamaru,_hound_of_konda"
    );
    assert!(engine
        .characteristics(legend)
        .unwrap()
        .supertypes
        .is_empty());
    assert_eq!(
        preview(&engine, &channel(&engine, source, target)).total_cost,
        "{1}{G}"
    );

    let mut engine = setup(202_610_049);
    let source = channel_source(&mut engine);
    let target = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    let legend = inject_permanent_on_battlefield(&mut engine, 0, "isamaru,_hound_of_konda");
    assert_eq!(
        preview(&engine, &channel(&engine, source, target)).total_cost,
        "{G}"
    );
    let aura = paid_cast(
        &mut engine,
        "song_of_the_dryads",
        target_object(legend),
        ManaGift {
            c: 2,
            g: 1,
            ..Default::default()
        },
    );
    let characteristics = engine.characteristics(legend).unwrap();
    assert!(characteristics
        .supertypes
        .iter()
        .any(|kind| kind == "Legendary"));
    assert!(!characteristics.has_type("Creature"));
    assert!(characteristics.has_type("Land"));
    assert!(characteristics.has_type("Forest"));
    assert_eq!(
        preview(&engine, &channel(&engine, source, target)).total_cost,
        "{1}{G}"
    );
    paid_cast(
        &mut engine,
        "disenchant",
        target_object(aura),
        ManaGift {
            c: 1,
            w: 1,
            ..Default::default()
        },
    );
    assert!(engine.characteristics(legend).unwrap().has_type("Creature"));
    assert_eq!(
        preview(&engine, &channel(&engine, source, target)).total_cost,
        "{G}"
    );
}

#[test]
fn boseiju_serialized_paid_channel_and_private_search_replay_exactly() {
    use prost::Message;
    fn fresh() -> (GameEngine, u32, u32, u32) {
        let mut engine = setup(202_610_050);
        let source = channel_source(&mut engine);
        let target = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
        let found = inject_library_card(&mut engine, 1, "taiga");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                g: 1,
                ..Default::default()
            },
        );
        (engine, source, target, found)
    }
    let (mut engine, source, target, found) = fresh();
    let mut recorded = Vec::new();
    let activation = paid(&engine, &channel(&engine, source, target), 1, 1);
    for (actor, command) in [
        (0, activation),
        (0, pass()),
        (1, pass()),
        (
            1,
            submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        ),
        (1, submit_resolution_choice(vec![found])),
    ] {
        let batch = engine.apply_command(actor, &command).unwrap();
        recorded.push((actor, command, batch));
    }
    let (mut replay, ..) = fresh();
    for (actor, command, batch) in recorded {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), batch);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap()
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&found].zone, Zone::Battlefield);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn boseiju_printed_mana_ability_and_playing_land_are_independent_of_channel() {
    let mut engine = setup(202_610_051);
    let source = channel_source(&mut engine);
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == source)
        .unwrap();
    engine.apply_command(0, &play_land(slot)).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    let command = activate_ability_for(&engine, source, 0, vec![]);
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert!(engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());
    engine.apply_command(0, &undo_mana_ability()).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
    assert!(!engine.state.objects[&source].tapped);
}

#[test]
fn boseiju_target_union_accepts_enchantment_nonbasic_and_basic_artifact_land() {
    for card in [
        "sol_ring",
        "yavimaya,_cradle_of_growth",
        "darksteel_citadel",
        "ominous_seas",
    ] {
        let mut engine = setup(202_610_052);
        let source = channel_source(&mut engine);
        let target = inject_permanent_on_battlefield(&mut engine, 1, card);
        if card == "darksteel_citadel" {
            // Exercise the OR: Basic disqualifies only the land branch, not Artifact.
            engine
                .state
                .objects
                .get_mut(&target)
                .unwrap()
                .copiable_values = Some(tricerules_core::state::CopiableValues {
                source_card_id: card.into(),
                source_face_index: 0,
                display_name: "Basic artifact land fixture".into(),
                face: {
                    let mut face = tricerules_cards::CardRegistry::global()
                        .get(card)
                        .unwrap()
                        .primary_face()
                        .clone();
                    face.supertypes.push("Basic".into());
                    face
                },
                room_faces: None,
            });
        }
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                g: 1,
                ..Default::default()
            },
        );
        let command = channel(&engine, source, target);
        assert!(preview(&engine, &command).valid, "{card}");
        engine.apply_command(0, &command).unwrap();
        let batch = resolve(&mut engine);
        assert_eq!(
            find_resolution_choice(&batch).unwrap().deciding_player_id,
            1
        );
        engine
            .apply_command(
                1,
                &submit_resolution_decision(ResolutionChoiceDecision::Decline),
            )
            .unwrap();
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn boseiju_refresh_recounts_live_legends_and_sanitizes_excess_payment() {
    let mut engine = setup(202_610_053);
    let source = channel_source(&mut engine);
    let target = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    let old = paid(&engine, &channel(&engine, source, target), 1, 1);
    assert!(preview(&engine, &old).complete);
    inject_permanent_on_battlefield(&mut engine, 0, "isamaru,_hound_of_konda");
    let before = format!("{:?}", engine.state);
    let refreshed = preview(&engine, &old);
    assert!(refreshed.valid && refreshed.complete && refreshed.selection_changed);
    assert_eq!(refreshed.total_cost, "{G}");
    let mana = refreshed.selection.as_ref().unwrap().mana.as_ref().unwrap();
    assert_eq!((mana.c, mana.g), (0, 1));
    assert_eq!(format!("{:?}", engine.state), before);
    assert!(
        engine.apply_command(0, &old).is_err(),
        "excess old payment cannot execute"
    );
    assert_eq!(format!("{:?}", engine.state), before);
    let mut command = channel(&engine, source, target);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.payment = refreshed.selection;
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
}
