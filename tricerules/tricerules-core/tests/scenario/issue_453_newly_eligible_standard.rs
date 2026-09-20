//! Issue #453 — the 24 newly eligible pinned-Standard identities through the command boundary.
//!
//! Every expectation is the reviewed printed Oracle behavior. Exact Scryfall records and
//! `rulings_uri` responses were fetched 2026-09-19 against pinned snapshot
//! `27bf3214-1271-490b-bdfe-c0be6c23d02e`; none changed the emitted mechanics. Governing CR
//! concepts: 701.8 (destroy), 120.2b (damage source), 702.51 (Convoke), 702.21 (Ward), 701.42
//! (surveil), 702.29 (cycling/basic landcycling), 614.1d (enters tapped), 603.6c/700.4 (dies
//! triggers), 702.108 (prowess), 715 (Adventure), 303.4 (Aura attach), 113.6 (graveyard
//! abilities), and 111.10b (Food).

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    self as rv1, AbilitySourceZone, ChoiceKind, ResolutionChoiceDecision,
};

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn land_engine(seed: u64) -> GameEngine {
    main1_engine(seed, &[], &[])
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    engine
        .apply_command(first, &pass())
        .expect("first priority pass");
    let second = engine.state.priority_player_id();
    engine
        .apply_command(second, &pass())
        .expect("second priority pass resolves the stack item")
}

fn hand_ability(engine: &GameEngine, source: u32, ability_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            ..Default::default()
        })),
    }
}

fn zone_ability(
    engine: &GameEngine,
    source: u32,
    zone: AbilitySourceZone,
    ability_index: u32,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: zone as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            ..Default::default()
        })),
    }
}

/// Put `card_ids` on top of `player`'s library, first entry on top, and return their OIDs.
fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    engine.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        engine.state.players[player].library.push_front(oid);
    }
    oids
}

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn grouped_targets(pairs: &[(u32, u32)]) -> Vec<TargetRef> {
    pairs
        .iter()
        .map(|&(object_id, group_index)| TargetRef {
            object_id,
            group_index,
            ..Default::default()
        })
        .collect()
}

fn object_ref(engine: &GameEngine, oid: u32) -> rv1::CostObjectRef {
    rv1::CostObjectRef {
        object_id: oid,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0),
    }
}

fn convoke_cast(
    engine: &GameEngine,
    slot: usize,
    targets: Vec<TargetRef>,
    contributions: &[(u32, rv1::ObjectPaymentKind)],
    mana: rv1::PaymentMana,
) -> RuledCommand {
    let source = engine.state.players[0].hand[slot];
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(hand_cast_source(slot)),
            targets,
            payment: Some(rv1::PaymentSelection {
                source: Some(object_ref(engine, source)),
                expected_state_revision: engine.state.command_index,
                convoke: contributions
                    .iter()
                    .map(|&(oid, kind)| rv1::ObjectPaymentContribution {
                        object: Some(object_ref(engine, oid)),
                        kind: kind as i32,
                    })
                    .collect(),
                mana: Some(mana),
                ..Default::default()
            }),
            ..Default::default()
        })),
    }
}

#[test]
fn issue_453_bilbos_deadly_slice_destroys_only_creatures() {
    let mut engine = land_engine(453_001);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let source = inject_card_into_hand(&mut engine, 0, "bilbos_deadly_slice");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "bilbos_deadly_slice");
    let generation = semantic::generation(&engine, source);

    for illegal in [target_player(1), target_object(artifact)] {
        let before = engine.state.command_index;
        engine
            .apply_command(0, &cast_spell(slot, illegal.clone()))
            .expect_err("destroy target creature rejects players and artifacts");
        assert_eq!(engine.state.command_index, before);
    }

    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));
    semantic::complete(&mut engine, 8, |_| None).require_exercised();
    semantic::assert_object(
        &engine,
        source,
        "bilbos_deadly_slice",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
}

#[test]
fn issue_453_vote_out_convoke_taps_a_creature_without_mana_for_the_generic() {
    let mut engine = land_engine(453_002);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let source = inject_card_into_hand(&mut engine, 0, "vote_out");
    let slot = hand_index_for_card(&engine, 0, "vote_out");

    // CR 601.2h: without mana or an offered Convoke contribution the whole cost is illegal.
    let before = engine.state.command_index;
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect_err("Vote Out is uncastable with no mana and no contributions");
    assert_eq!(engine.state.command_index, before);
    assert_eq!(engine.state.players[0].hand[slot], source);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let command = convoke_cast(
        &engine,
        slot,
        target_object(target),
        &[(bear, rv1::ObjectPaymentKind::Generic)],
        rv1::PaymentMana {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &command)
        .expect("one creature pays one generic and the pool pays {2}{B}");
    assert!(
        engine.state.objects[&bear].tapped,
        "CR 702.51a: tapping is the cost"
    );
    assert_eq!(engine.state.players[0].mana_pool.black, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn issue_453_protective_response_destroys_only_attacking_or_blocking_creatures() {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("plains", &[])]);
    let mut engine = GameEngine::new(453_003, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut engine);
    let attacker = battlefield_object_for_card(&engine, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let source = inject_card_into_hand(&mut engine, 1, "protective_response");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "protective_response");
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    engine
        .apply_command(0, &pass())
        .expect("active player passes; the defender receives priority");

    let before = engine.state.command_index;
    engine
        .apply_command(1, &cast_spell(slot, target_object(bystander)))
        .expect_err("an untapped noncombat creature is not an attacking or blocking creature");
    assert_eq!(engine.state.command_index, before);

    give_mana(
        &mut engine,
        1,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(1, &cast_spell(slot, target_object(attacker)))
        .expect("the attacking creature is a legal target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&attacker].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&bystander].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn issue_453_magnificent_end_reduces_only_for_a_tapped_creature_target() {
    let mut engine = land_engine(453_004);
    let tapped = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 5, 5);
    let untapped = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 5, 5);
    engine
        .state
        .objects
        .get_mut(&tapped)
        .expect("tapped creature")
        .tapped = true;
    let source = inject_card_into_hand(&mut engine, 0, "magnificent_end");
    let slot = hand_index_for_card(&engine, 0, "magnificent_end");
    let generation = semantic::generation(&engine, source);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );

    // The published legality attributes the three-generic reduction to the tapped target only.
    let batch = engine.initial_response_batch();
    let published = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    let application = published
        .targeted_cost_reduction_applications
        .first()
        .expect("target-dependent reduction");
    assert_eq!(application.generic_mana, 3);
    assert!(application
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == tapped));
    assert!(!application
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == untapped));

    let before = engine.state.command_index;
    engine
        .apply_command(0, &cast_spell(slot, target_object(untapped)))
        .expect_err("two mana does not cover the full {4}{W} on an untapped target");
    assert_eq!(engine.state.command_index, before);

    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(tapped)));
    semantic::complete(&mut engine, 8, |_| None).require_exercised();
    semantic::assert_object(
        &engine,
        source,
        "magnificent_end",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
    assert_eq!(engine.state.objects[&tapped].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&untapped].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn issue_453_huatli_final_strike_pumps_then_deals_the_pumped_power() {
    let mut engine = land_engine(453_005);
    let own = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    let opposing = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 3, 3);
    let source = inject_card_into_hand(&mut engine, 0, "huatlis_final_strike");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "huatlis_final_strike");
    let generation = semantic::generation(&engine, source);

    for illegal in [
        grouped_targets(&[(opposing, 0), (opposing, 1)]),
        grouped_targets(&[(own, 0), (own, 1)]),
    ] {
        let before = engine.state.command_index;
        engine
            .apply_command(0, &cast_spell(slot, illegal))
            .expect_err("each printed target group keeps its own controller scope");
        assert_eq!(engine.state.command_index, before);
    }

    semantic::accepted(
        &mut engine,
        0,
        &cast_spell(slot, grouped_targets(&[(own, 0), (opposing, 1)])),
    );
    semantic::complete(&mut engine, 8, |_| None).require_exercised();
    assert_eq!(engine.effective_power(own), Some(3));
    assert_eq!(engine.effective_toughness(own), Some(2));
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&own].zone, Zone::Battlefield);
    semantic::assert_object(
        &engine,
        source,
        "huatlis_final_strike",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
}

#[test]
fn issue_453_surveil_lands_enter_tapped_and_partition_the_library() {
    for (index, card_id) in ["raucous_theater", "thundering_falls"]
        .into_iter()
        .enumerate()
    {
        let mut engine = land_engine(453_010 + index as u64);
        let top = seat_on_top(&mut engine, 0, &["grizzly_bears", "storm_crow"]);
        let source = inject_card_into_hand(&mut engine, 0, card_id);
        let slot = hand_index_for_card(&engine, 0, card_id);

        let played = engine
            .apply_command(0, &play_land(slot))
            .expect("play the surveil land");
        let land = battlefield_object_for_card(&engine, 0, card_id);
        assert!(
            engine.state.objects[&land].tapped,
            "{card_id} enters tapped"
        );
        assert!(played.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::StackPushed(stack)) if stack.is_triggered
        )));

        let resolution = resolve_top_stack(&mut engine);
        let choice = find_resolution_choice(&resolution).expect("surveil 1 choice");
        assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.candidate_object_ids, vec![top[0]]);
        engine
            .apply_command(0, &submit_resolution_choice(vec![top[0]]))
            .expect("put the surveilled card into the graveyard");

        assert_eq!(engine.state.objects[&top[0]].zone, Zone::Graveyard);
        assert!(engine.state.players[0].graveyard.contains(&top[0]));
        assert_eq!(engine.state.players[0].library.front(), Some(&top[1]));
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert_eq!(engine.state.players[0].life, 20);
    }
}

#[test]
fn issue_453_bloodfell_caves_gains_life_then_taps_for_black_or_red() {
    let mut engine = land_engine(453_020);
    let source = inject_card_into_hand(&mut engine, 0, "bloodfell_caves");
    let slot = hand_index_for_card(&engine, 0, "bloodfell_caves");
    engine
        .apply_command(0, &play_land(slot))
        .expect("play Bloodfell Caves");
    let land = battlefield_object_for_card(&engine, 0, "bloodfell_caves");
    assert!(engine.state.objects[&land].tapped);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].life, 21, "ETB gains 1 life");

    // Untap through the fixture so the printed mana ability can be activated as a real command.
    engine.state.objects.get_mut(&land).expect("land").tapped = false;
    engine
        .apply_command(0, &activate_ability_for(&engine, land, 0, vec![]))
        .expect("activate {B} or {R}");
    assert_eq!(engine.state.players[0].mana_pool.black, 1);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert!(engine.state.objects[&land].tapped);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
}

#[test]
fn issue_453_stony_voiced_goblins_makes_each_opponent_discard() {
    let mut engine = land_engine(453_030);
    let source = inject_card_into_hand(&mut engine, 0, "stony-voiced_goblins");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "stony-voiced_goblins");
    let own_hand_before = engine.state.players[0].hand.len();
    let opposing_hand_before = engine.state.players[1].hand.clone();

    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Stony-Voiced Goblins");
    engine.apply_command(0, &pass()).expect("caster pass");
    engine.apply_command(1, &pass()).expect("spell resolves");
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine
        .state
        .stack
        .last()
        .is_some_and(|item| item.is_triggered));

    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("discard choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.min, 1);
    assert_eq!(choice.candidate_object_ids, opposing_hand_before);
    let discarded = choice.candidate_object_ids[0];
    engine
        .apply_command(1, &submit_resolution_choice(vec![discarded]))
        .expect("opponent discards");

    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[1].hand.len(),
        opposing_hand_before.len() - 1
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        own_hand_before - 1,
        "only the controller's cast left their hand"
    );
}

#[test]
fn issue_453_gingerbread_hunter_makes_food_and_puny_snack_shrinks() {
    let mut engine = land_engine(453_040);
    let source = inject_card_into_hand(&mut engine, 0, "gingerbread_hunter_puny_snack");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "gingerbread_hunter_puny_snack");
    engine
        .apply_command(0, &cast_spell_face(slot, vec![], 0))
        .expect("cast Gingerbread Hunter");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);
    assert!(engine
        .state
        .objects
        .values()
        .any(|object| { object.card_id == "food" && object.zone == Zone::Battlefield }));

    // Cast the Adventure face of a fresh copy; CR 715.3d exiles it instead of the graveyard.
    let mut adventure = land_engine(453_041);
    let target = inject_creature_with_stats(&mut adventure, 1, "hill_giant", 5, 5);
    let source = inject_card_into_hand(&mut adventure, 0, "gingerbread_hunter_puny_snack");
    grant_pool(&mut adventure, 0);
    let slot = hand_index_for_card(&adventure, 0, "gingerbread_hunter_puny_snack");
    adventure
        .apply_command(0, &cast_spell_face(slot, target_object(target), 1))
        .expect("cast Puny Snack");
    resolve_entire_stack_two_player(&mut adventure);
    assert_eq!(adventure.effective_power(target), Some(3));
    assert_eq!(adventure.effective_toughness(target), Some(3));
    assert_eq!(adventure.state.objects[&source].zone, Zone::Exile);
    assert!(!adventure.state.players[0].graveyard.contains(&source));
}

#[test]
fn issue_453_minecart_daredevil_ride_the_rails_pumps() {
    let mut engine = land_engine(453_042);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let source = inject_card_into_hand(&mut engine, 0, "minecart_daredevil_ride_the_rails");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "minecart_daredevil_ride_the_rails");
    engine
        .apply_command(0, &cast_spell_face(slot, target_object(target), 1))
        .expect("cast Ride the Rails");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(target), Some(4));
    assert_eq!(engine.effective_toughness(target), Some(3));
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
}

#[test]
fn issue_453_smaug_spew_flame_deals_five() {
    let mut engine = land_engine(453_043);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 5, 5);
    let bystander = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    let source = inject_card_into_hand(&mut engine, 0, "smaug,_the_great_calamity_spew_flame");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "smaug,_the_great_calamity_spew_flame");
    engine
        .apply_command(0, &cast_spell_face(slot, target_object(target), 1))
        .expect("cast Spew Flame");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&bystander].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
}

#[test]
fn issue_453_al_bhed_salvagers_drains_once_per_permanent() {
    let mut engine = land_engine(453_050);
    inject_creature_with_stats(&mut engine, 0, "al_bhed_salvagers", 2, 3);
    let first = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    let second = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    for doomed in [first, second] {
        engine
            .state
            .objects
            .get_mut(&doomed)
            .expect("creature")
            .damage = 2;
    }

    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("lethal-damage state-based action");
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    answer_trigger_order_in_engine_order(&mut engine);
    let mut answered = 0;
    while !engine.state.pending_triggers.is_empty() {
        engine
            .apply_command(0, &choose_trigger_targets(target_player(1)))
            .expect("choose target opponent");
        answered += 1;
    }
    assert_eq!(
        answered, 2,
        "CR 700.4/603.6c: each simultaneous death triggers separately"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        (engine.state.players[0].life, engine.state.players[1].life),
        (22, 18)
    );

    // A dying noncreature artifact you control is inside the union filter.
    let mut artifact_engine = land_engine(453_051);
    inject_creature_with_stats(&mut artifact_engine, 0, "al_bhed_salvagers", 2, 3);
    let prism = inject_permanent_on_battlefield(&mut artifact_engine, 0, "prophetic_prism");
    inject_card_into_hand(&mut artifact_engine, 0, "disenchant");
    give_mana(
        &mut artifact_engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let disenchant = hand_index_for_card(&artifact_engine, 0, "disenchant");
    artifact_engine
        .apply_command(0, &cast_spell(disenchant, target_object(prism)))
        .expect("cast Disenchant at the controlled artifact");
    artifact_engine
        .apply_command(0, &pass())
        .expect("caster passes");
    artifact_engine
        .apply_command(1, &pass())
        .expect("Disenchant resolves");
    assert_eq!(artifact_engine.state.objects[&prism].zone, Zone::Graveyard);
    assert_eq!(artifact_engine.state.pending_triggers.len(), 1);
    artifact_engine
        .apply_command(0, &choose_trigger_targets(target_player(1)))
        .expect("choose target opponent");
    resolve_entire_stack_two_player(&mut artifact_engine);
    assert_eq!(
        (
            artifact_engine.state.players[0].life,
            artifact_engine.state.players[1].life
        ),
        (21, 19)
    );
}

#[test]
fn issue_453_lightshell_duo_prowess_surveils_and_expires() {
    let mut engine = land_engine(453_060);
    let top = seat_on_top(
        &mut engine,
        0,
        &["hill_giant", "storm_crow", "grizzly_bears"],
    );
    let duo = inject_card_into_hand(&mut engine, 0, "lightshell_duo");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "lightshell_duo");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Lightshell Duo");
    pass_both_players(&mut engine);
    let resolution = resolve_top_stack(&mut engine);
    let surveil = find_resolution_choice(&resolution).expect("surveil 2 after the ETB");
    assert_eq!(surveil.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((surveil.min, surveil.max), (0, 2));
    assert_eq!(surveil.candidate_object_ids, top[..2]);
    engine
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("put one surveilled card into the graveyard");
    assert_eq!(engine.state.objects[&top[0]].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].library.front(), Some(&top[1]));

    // CR 702.108a: one trigger for the controller's noncreature spell.
    let artifact = inject_card_into_hand(&mut engine, 0, "bonesplitter");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "bonesplitter");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast a noncreature spell");
    assert_eq!(engine.state.stack.len(), 2);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.is_triggered && item.source_permanent_id == Some(duo)));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(duo), Some(4));
    assert_eq!(engine.effective_toughness(duo), Some(5));

    // A creature spell does not trigger Prowess, and cleanup ends the pump.
    let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast a creature spell");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "no Prowess for a creature spell"
    );
    resolve_entire_stack_two_player(&mut engine);
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(duo), Some(3));
    assert_eq!(engine.effective_toughness(duo), Some(4));
}

#[test]
fn issue_453_basic_landcycling_searches_from_hand() {
    for (index, (card_id, mana)) in [
        (
            "kree_sentinel",
            ManaGift {
                c: 2,
                ..Default::default()
            },
        ),
        (
            "savage_land_dinosaur",
            ManaGift {
                c: 2,
                ..Default::default()
            },
        ),
        (
            "topiary_panther",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        // A nonbasic-only library proves the search predicate filters on card type.
        let decks = Some(vec![
            vec!["grizzly_bears".to_string(); 20],
            vec!["forest".to_string(); 20],
        ]);
        let mut engine =
            GameEngine::new(453_070 + index as u64, &[0, 1], 20, decks, true).expect("new game");
        advance_to_main1_from_game_start(&mut engine);
        let source = inject_card_into_hand(&mut engine, 0, card_id);
        let mountain = inject_library_card(&mut engine, 0, "mountain");
        let forest = inject_library_card(&mut engine, 0, "forest");

        let batch = engine.initial_response_batch();
        assert!(
            batch.legal_by_player[&0]
                .zone_ability_actions
                .iter()
                .any(|action| {
                    action.object_id == source && action.source_zone() == AbilitySourceZone::Hand
                }),
            "{card_id} publishes its hand ability"
        );
        engine
            .apply_command(0, &hand_ability(&engine, source, 0))
            .expect_err("the complete cost is checked before any discard");
        assert_eq!(engine.state.objects[&source].zone, Zone::Hand);

        give_mana(&mut engine, 0, mana);
        engine
            .apply_command(0, &hand_ability(&engine, source, 0))
            .expect("activate basic landcycling");
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        engine.apply_command(0, &pass()).expect("controller passes");
        let search = engine
            .apply_command(1, &pass())
            .expect("basic landcycling resolves to a private search");
        let choice = find_resolution_choice(&search).expect("private library search");
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(
            choice.candidate_object_ids,
            vec![mountain, forest],
            "only basic land cards are offered"
        );
        assert!(!choice.candidate_object_ids.iter().any(|oid| {
            engine
                .state
                .objects
                .get(oid)
                .is_some_and(|object| object.card_id == "grizzly_bears")
        }));

        let completed = engine
            .apply_command(0, &submit_resolution_choice(vec![mountain]))
            .expect("choose the basic land");
        assert_eq!(engine.state.objects[&mountain].zone, Zone::Hand);
        assert!(completed.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::CardsRevealed(reveal))
                if reveal.cards.len() == 1 && reveal.cards[0].object_id == mountain
        )));
        assert!(completed.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )));
    }
}

#[test]
fn issue_453_capital_city_cycles_to_draw() {
    let mut engine = land_engine(453_080);
    let source = inject_card_into_hand(&mut engine, 0, "capital_city");
    let drawn = seat_on_top(&mut engine, 0, &["grizzly_bears"])[0];

    engine
        .apply_command(0, &hand_ability(&engine, source, 2))
        .expect_err("cycling {2} needs mana");
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &hand_ability(&engine, source, 2))
        .expect("activate cycling");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&drawn));

    // The same card is a land: its first printed ability adds {C} when played normally.
    let mut land_engine = land_engine(453_081);
    let source = inject_card_into_hand(&mut land_engine, 0, "capital_city");
    let slot = hand_index_for_card(&land_engine, 0, "capital_city");
    land_engine
        .apply_command(0, &play_land(slot))
        .expect("play Capital City");
    let land = battlefield_object_for_card(&land_engine, 0, "capital_city");
    land_engine
        .apply_command(0, &activate_ability_for(&land_engine, land, 0, vec![]))
        .expect("tap for {C}");
    assert_eq!(land_engine.state.players[0].mana_pool.colorless, 1);
    assert_eq!(land_engine.state.objects[&source].zone, Zone::Battlefield);
}

#[test]
fn issue_453_bestial_bloodline_attaches_modifies_and_returns_from_graveyard() {
    let mut engine = land_engine(453_090);
    let bear = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    let source = inject_card_into_hand(&mut engine, 0, "bestial_bloodline");
    inject_card_into_hand(&mut engine, 0, "disenchant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "bestial_bloodline");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Bestial Bloodline");
    resolve_entire_stack_two_player(&mut engine);
    let aura = battlefield_object_for_card(&engine, 0, "bestial_bloodline");
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
    assert_eq!(engine.effective_power(bear), Some(4));
    assert_eq!(engine.effective_toughness(bear), Some(4));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let disenchant = hand_index_for_card(&engine, 0, "disenchant");
    engine
        .apply_command(0, &cast_spell(disenchant, target_object(aura)))
        .expect("disenchant the Aura");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(engine.effective_power(bear), Some(2));
    assert_eq!(engine.effective_toughness(bear), Some(2));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &zone_ability(&engine, aura, AbilitySourceZone::Graveyard, 0),
        )
        .expect("activate the graveyard return");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&aura));
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
}

#[test]
fn issue_453_archive_dragon_ward_counters_unless_the_caster_pays() {
    // Paying {2} preserves the targeting spell.
    let mut paid = land_engine(453_100);
    let dragon = inject_creature_with_stats(&mut paid, 0, "archive_dragon", 4, 6);
    inject_card_into_hand(&mut paid, 1, "unsummon");
    give_mana(
        &mut paid,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    paid.apply_command(0, &pass())
        .expect("active player passes priority to the opponent");
    let unsummon = hand_index_for_card(&paid, 1, "unsummon");
    paid.apply_command(1, &cast_spell(unsummon, target_object(dragon)))
        .expect("opponent casts Unsummon at the Dragon");
    assert_eq!(paid.state.stack.len(), 2, "Ward is above the spell");
    assert!(paid
        .state
        .stack
        .last()
        .is_some_and(|item| item.is_triggered));
    pass_both_players(&mut paid);
    let pending = paid
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward payment");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::ManaPayment);
    give_mana(
        &mut paid,
        1,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut paid, 1, ResolutionChoiceDecision::PayMana)
        .expect("pay Ward {2}");
    pass_both_players(&mut paid);
    assert_eq!(paid.state.objects[&dragon].zone, Zone::Hand);

    // Declining counters the spell and leaves the Dragon in place.
    let mut declined = land_engine(453_101);
    let dragon = inject_creature_with_stats(&mut declined, 0, "archive_dragon", 4, 6);
    let unsummon_card = inject_card_into_hand(&mut declined, 1, "unsummon");
    give_mana(
        &mut declined,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    declined
        .apply_command(0, &pass())
        .expect("active player passes priority to the opponent");
    let unsummon = hand_index_for_card(&declined, 1, "unsummon");
    declined
        .apply_command(1, &cast_spell(unsummon, target_object(dragon)))
        .expect("opponent casts Unsummon at the Dragon");
    pass_both_players(&mut declined);
    submit_mana_resolution_decision(&mut declined, 1, ResolutionChoiceDecision::Decline)
        .expect("decline Ward");
    assert_eq!(declined.state.objects[&unsummon_card].zone, Zone::Graveyard);
    assert_eq!(declined.state.objects[&dragon].zone, Zone::Battlefield);
    assert!(declined.state.stack.is_empty());
}

#[test]
fn issue_453_impact_tremors_damages_each_opponent_on_creature_entry() {
    let mut engine = land_engine(453_110);
    inject_card_into_hand(&mut engine, 0, "impact_tremors");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "impact_tremors");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Impact Tremors");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine
            .state
            .objects
            .values()
            .filter(|object| {
                object.card_id == "impact_tremors" && object.zone == Zone::Battlefield
            })
            .count(),
        1
    );

    let entered = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast a creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&entered].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(
        engine.state.players[1].life, 19,
        "each opponent takes exactly one damage per entering creature"
    );
}
