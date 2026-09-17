//! Issue #289 — discard-batch trigger counts.
//!
//! Oracle checked 2026-09-16 for Scrounging Skyray and Marauding Mako: "Whenever you discard one
//! or more cards, put that many +1/+1 counters on this creature." CR 603.2c makes a trigger fire
//! once per event while one event may contain many occurrences, so a per-card observer (Megrim,
//! Waste Not) still fires per card while "one or more cards" fires once with the committed count.
//! CR 701.9 and CR 514.1 place cost, effect, and cleanup discards under the same semantic action
//! boundary; CR 608.2h fixes the count when the trigger resolves from the event-time value.

use crate::helpers::*;
use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, ContinuousEffectKind, CounterKind, EffectDuration, EffectSubject,
    SpellEffectKind, TriggerCondition,
};
use tricerules_cards::CardRegistry;
use tricerules_core::{AffectedScope, ContinuousEffect, GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, AbilitySourceZone, ActivateAbility, DevCommand, DevMoveCard,
    DevZone, RuledCommand,
};

fn dev_move(player: i32, card_name: &str, zone: DevZone) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: card_name.into(),
                zone: zone as i32,
                ..Default::default()
            })),
        })),
    }
}

fn grant_discard_batch_counter(engine: &mut GameEngine, source: u32) {
    let mut ability = CardRegistry::global()
        .get("ajanis_pridemate")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::WheneverPlayerDiscardsOneOrMoreCards {
        player: CastTriggerPlayer::Controller,
    };
    ability.effect = vec![SpellEffectKind::PutCounters {
        counter: CounterKind::PlusOnePlusOne,
        count: Amount::EventCount,
        subject: EffectSubject::Source,
    }];
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

fn discard_batch_setup(seed: u64, source_player: usize, source_card: &str) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["mind_rot", "mind_rot", "megrim"]),
            deck_with(
                "forest",
                &["grizzly_bears", "grizzly_bears", "library_of_leng"],
            ),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, source_player, source_card);
    grant_discard_batch_counter(&mut engine, source);
    (engine, source)
}

fn mind_rot_setup(seed: u64) -> (GameEngine, u32) {
    // The granted ability is controller-relative ("whenever you discard"), so the observing
    // creature belongs to the player Mind Rot makes discard.
    discard_batch_setup(seed, 1, "grizzly_bears")
}

fn clear_hand(engine: &mut GameEngine, player: usize) {
    let hand = std::mem::take(&mut engine.state.players[player].hand);
    for oid in hand {
        engine
            .state
            .objects
            .get_mut(&oid)
            .expect("hand object")
            .zone = Zone::Library;
        engine.state.players[player].library.push_back(oid);
    }
}

fn cast_mind_rot(engine: &mut GameEngine) {
    ensure_in_hand(engine, 0, "mind_rot");
    give_mana(
        engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "mind_rot");
    engine
        .apply_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Mind Rot");
    engine.apply_command(0, &pass()).expect("controller passes");
    engine.apply_command(1, &pass()).expect("target passes");
}

fn counters(engine: &GameEngine, source: u32) -> u32 {
    engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne)
}

#[test]
fn issue_289_simultaneous_multi_card_discard_triggers_once_with_the_committed_count() {
    let (mut engine, source) = mind_rot_setup(289_001);
    clear_hand(&mut engine, 1);
    let first = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    let second = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    cast_mind_rot(&mut engine);
    engine
        .apply_command(1, &submit_resolution_choice(vec![first, second]))
        .expect("discard both cards");
    assert_eq!(
        counters(&engine, source),
        0,
        "trigger is still on the stack"
    );
    assert_eq!(engine.state.stack.len(), 1, "one event, one trigger");
    assert_eq!(engine.state.stack[0].trigger_context.event_count, Some(2));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, source), 2);
    assert_eq!(engine.characteristics(source).unwrap().power, Some(4));
}

#[test]
fn issue_289_one_card_discard_carries_count_one() {
    let (mut engine, source) = mind_rot_setup(289_002);
    clear_hand(&mut engine, 1);
    let only = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    cast_mind_rot(&mut engine);
    engine
        .apply_command(1, &submit_resolution_choice(vec![only]))
        .expect("discard the only card");
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, source), 1);
}

#[test]
fn issue_289_partial_discard_counts_only_the_committed_cards() {
    let (mut engine, source) = mind_rot_setup(289_003);
    clear_hand(&mut engine, 1);
    let only = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    assert_eq!(engine.state.players[1].hand.len(), 1);
    cast_mind_rot(&mut engine);
    engine
        .apply_command(1, &submit_resolution_choice(vec![only]))
        .expect("discard what exists");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, source), 1, "count is the committed set");
}

#[test]
fn issue_289_separate_discard_actions_remain_separate_events() {
    let (mut engine, source) = mind_rot_setup(289_004);
    clear_hand(&mut engine, 1);
    let first = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    cast_mind_rot(&mut engine);
    engine
        .apply_command(1, &submit_resolution_choice(vec![first]))
        .expect("first discard");
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, source), 1);

    let second = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    cast_mind_rot(&mut engine);
    engine
        .apply_command(1, &submit_resolution_choice(vec![second]))
        .expect("second discard");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "a separate action is a separate event"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, source), 2);
}

#[test]
fn issue_289_per_card_observers_keep_every_occurrence_alongside_the_batch_trigger() {
    let (mut engine, source) = mind_rot_setup(289_005);
    let megrim = relocate_to_battlefield(&mut engine, 0, "megrim", false);
    assert_eq!(engine.state.objects[&megrim].zone, Zone::Battlefield);
    clear_hand(&mut engine, 1);
    let first = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    let second = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    cast_mind_rot(&mut engine);
    engine
        .apply_command(1, &submit_resolution_choice(vec![first, second]))
        .expect("discard both cards");
    let ordering = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("the active player orders its simultaneous triggers first");
    assert_eq!(
        ordering.candidates.len(),
        2,
        "Megrim still sees one occurrence per discarded card"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 16, "Megrim per-card damage");
    assert_eq!(counters(&engine, source), 2, "batch count once");
}

#[test]
fn issue_289_cleanup_discard_is_one_event() {
    let (mut engine, source) = discard_batch_setup(289_006, 0, "grizzly_bears");
    while engine.state.players[0].hand.len() < 9 {
        relocate_to_hand(&mut engine, 0, "swamp");
    }
    engine.state.turn_step = tricerules_core::TurnStep::Cleanup;
    engine.state.cleanup_discard_player = Some(0);
    let excess = engine.state.players[0].hand.len() - 7;
    engine
        .apply_command(0, &discard_cleanup_batch((0..excess as u32).collect()))
        .expect("cleanup discard");
    assert_eq!(counters(&engine, source), 0);
    assert!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_triggered)
            .count()
            == 1,
        "one cleanup action is one discard event"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, source), excess as u32);
}

#[test]
fn issue_289_replaced_destinations_still_count_committed_discards() {
    let (mut engine, source) = mind_rot_setup(289_007);
    relocate_to_battlefield(&mut engine, 1, "library_of_leng", false);
    clear_hand(&mut engine, 1);
    let first = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    let second = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    cast_mind_rot(&mut engine);
    engine
        .apply_command(1, &submit_resolution_choice(vec![first, second]))
        .expect("choose both cards");
    // Opaque destination choices: 0 graveyard, 1 library.
    engine
        .apply_command(1, &submit_resolution_choice(vec![1]))
        .expect("first replacement destination");
    engine
        .apply_command(1, &submit_resolution_choice(vec![1]))
        .expect("second replacement destination");
    let ordered = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("top-first order choice")
        .presentation
        .candidates
        .clone();
    engine
        .apply_command(1, &submit_resolution_choice(ordered))
        .expect("finish the ordered replacement");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        counters(&engine, source),
        2,
        "hidden replacement destinations do not lose the semantic discard"
    );
    assert_eq!(engine.state.players[1].graveyard.len(), 0);
}

#[test]
fn issue_289_counters_do_not_land_on_a_new_incarnation() {
    let (mut engine, source) = discard_batch_setup(289_008, 1, "silvercoat_lion");
    clear_hand(&mut engine, 1);
    let first = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    let second = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    cast_mind_rot(&mut engine);
    engine
        .apply_command(1, &submit_resolution_choice(vec![first, second]))
        .expect("discard both cards");
    engine.enable_dev_commands();
    engine
        .apply_command(1, &dev_move(1, "Silvercoat Lion", DevZone::Graveyard))
        .expect("source leaves the battlefield");
    engine
        .apply_command(1, &dev_move(1, "Silvercoat Lion", DevZone::Battlefield))
        .expect("source returns as a new object");
    assert_ne!(
        engine.state.objects[&source].zone,
        Zone::Graveyard,
        "new incarnation is on the battlefield"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        counters(&engine, source),
        0,
        "CR 400.7: the old trigger never touches the returned object"
    );
}

fn hand_ability(engine: &GameEngine, source: u32) -> RuledCommand {
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
            ability_index: 0,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_289_generated_skyray_cycling_is_one_discard_event() {
    let decks = Some(vec![
        deck_with("island", &["scrounging_skyray", "scrounging_skyray"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(289_009, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let on_battlefield = move_ready_to_battlefield(&mut engine, 0, "scrounging_skyray");
    let in_hand = relocate_to_hand(&mut engine, 0, "scrounging_skyray");
    let drawn = inject_library_card(&mut engine, 0, "grizzly_bears");
    engine.state.players[0].library.retain(|oid| *oid != drawn);
    engine.state.players[0].library.push_front(drawn);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_command(0, &hand_ability(&engine, in_hand))
        .expect("activate Cycling");
    assert_eq!(engine.state.objects[&in_hand].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "the Cycling ability is on the stack below its trigger"
    );
    let cycling = engine
        .state
        .stack
        .iter()
        .find(|item| !item.is_triggered)
        .expect("Cycling ability on the stack");
    assert_eq!(
        cycling
            .activated_ability
            .as_ref()
            .map(|ability| ability.ability_id.as_str()),
        Some("activated_01")
    );
    let triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect::<Vec<_>>();
    assert_eq!(triggers.len(), 1, "one cost discard is one event");
    assert_eq!(triggers[0].trigger_context.event_count, Some(1));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, on_battlefield), 1);
    assert!(
        engine.state.players[0].hand.contains(&drawn),
        "resolving Cycling draws a card (CR 702.29a)"
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before,
        "one card discarded, one card drawn"
    );
}

#[test]
fn issue_289_ward_discard_cost_is_one_event() {
    let decks = Some(vec![
        deck_with("island", &["unsummon", "grizzly_bears"]),
        deck_with("swamp", &["spectral_snatcher"]),
    ]);
    let mut engine = GameEngine::new(289_011, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let observer = inject_creature_on_battlefield(&mut engine, 0, "silvercoat_lion");
    grant_discard_batch_counter(&mut engine, observer);
    let snatcher = relocate_to_battlefield(&mut engine, 1, "spectral_snatcher", false);
    ensure_in_hand(&mut engine, 0, "unsummon");
    let discard = relocate_to_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(snatcher)))
        .expect("cast Unsummon at Spectral Snatcher");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![discard]))
        .expect("pay Ward by discarding");
    assert_eq!(engine.state.objects[&discard].zone, Zone::Graveyard);
    let triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect::<Vec<_>>();
    assert_eq!(triggers.len(), 1, "a Ward discard cost is one event");
    assert_eq!(triggers[0].trigger_context.event_count, Some(1));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, observer), 1);
}

#[test]
fn issue_289_ward_discard_exiles_a_madness_card_and_still_counts_it() {
    let decks = Some(vec![
        deck_with("island", &["unsummon", "fiery_temper"]),
        deck_with("swamp", &["spectral_snatcher"]),
    ]);
    let mut engine = GameEngine::new(289_015, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let observer = inject_creature_on_battlefield(&mut engine, 0, "silvercoat_lion");
    grant_discard_batch_counter(&mut engine, observer);
    let snatcher = relocate_to_battlefield(&mut engine, 1, "spectral_snatcher", false);
    ensure_in_hand(&mut engine, 0, "unsummon");
    let madness_card = relocate_to_hand(&mut engine, 0, "fiery_temper");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(snatcher)))
        .expect("cast Unsummon at Spectral Snatcher");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![madness_card]))
        .expect("pay Ward by discarding the madness card");
    assert_eq!(
        engine.state.objects[&madness_card].zone,
        Zone::Exile,
        "CR 702.35a: a discarded madness card is exiled"
    );
    let ordering = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("the madness and discard triggers await ordering");
    assert_eq!(ordering.candidates.len(), 2);
    assert!(
        ordering
            .candidates
            .iter()
            .any(|trigger| trigger.trigger_context.event_count == Some(1)),
        "the batch observer still counts the cost discard"
    );
    assert!(
        ordering.candidates.iter().any(|trigger| matches!(
            trigger.ability.effect.as_slice(),
            [SpellEffectKind::CastMadness { .. }]
        )),
        "the madness trigger was staged"
    );
}

#[test]
fn issue_289_generated_mako_groups_a_two_card_instruction() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &[
                "marauding_mako",
                "mind_rot",
                "grizzly_bears",
                "grizzly_bears",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(289_010, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let mako = move_ready_to_battlefield(&mut engine, 0, "marauding_mako");
    clear_hand(&mut engine, 0);
    let first = relocate_to_hand(&mut engine, 0, "grizzly_bears");
    let second = relocate_to_hand(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "mind_rot");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "mind_rot");
    engine
        .apply_command(0, &cast_spell(slot, target_player(0)))
        .expect("cast Mind Rot");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine.apply_command(1, &pass()).expect("target passes");
    engine
        .apply_command(0, &submit_resolution_choice(vec![first, second]))
        .expect("discard both cards");
    let triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect::<Vec<_>>();
    assert_eq!(triggers.len(), 1, "one instruction is one event");
    assert_eq!(triggers[0].trigger_context.event_count, Some(2));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, mako), 2);
}

#[test]
fn issue_289_impossible_discard_fires_no_event_and_counts_nothing() {
    let (mut engine, source) = mind_rot_setup(289_016);
    clear_hand(&mut engine, 1);
    cast_mind_rot(&mut engine);
    assert!(
        engine.state.pending_resolution.is_none(),
        "no choice with an empty hand"
    );
    assert!(
        engine.state.stack.is_empty(),
        "no discard event and therefore no trigger"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, source), 0);
}

#[test]
fn issue_289_whole_hand_discard_is_one_event_with_every_card_counted() {
    let decks = Some(vec![
        deck_with("mountain", &["hearth_elemental_stoke_genius"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(289_012, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let observer = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_discard_batch_counter(&mut engine, observer);
    clear_hand(&mut engine, 0);
    for _ in 0..5 {
        relocate_to_hand(&mut engine, 0, "mountain");
    }
    ensure_in_hand(&mut engine, 0, "hearth_elemental_stoke_genius");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "hearth_elemental_stoke_genius");
    engine
        .apply_command(0, &cast_spell_face(slot, vec![], 1))
        .expect("cast Stoke Genius");
    pass_both_players(&mut engine);
    let triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect::<Vec<_>>();
    assert_eq!(
        triggers.len(),
        1,
        "discarding the whole hand is one instruction"
    );
    assert_eq!(triggers[0].trigger_context.event_count, Some(5));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, observer), 5);
}

#[test]
fn issue_289_simultaneous_multiplayer_discard_fires_once_per_player() {
    let decks = Some(vec![
        deck_with("swamp", &["fanatic_of_the_harrowing"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(289_013, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let first_observer = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second_observer = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    grant_discard_batch_counter(&mut engine, first_observer);
    grant_discard_batch_counter(&mut engine, second_observer);
    let first_card = relocate_to_hand(&mut engine, 0, "swamp");
    let second_card = relocate_to_hand(&mut engine, 1, "forest");
    ensure_in_hand(&mut engine, 0, "fanatic_of_the_harrowing");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "fanatic_of_the_harrowing");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Fanatic of the Harrowing");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is on the stack");

    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_command(1, &pass())
        .expect("opponent passes to the first private choice");
    let caster_hand = engine.state.players[0].hand.len();
    engine
        .apply_command(0, &submit_resolution_choice(vec![first_card]))
        .expect("record the active player's hidden choice");
    assert!(
        engine.state.stack.is_empty(),
        "no discard trigger before the action commits"
    );
    assert_eq!(engine.state.objects[&first_card].zone, Zone::Hand);
    engine
        .apply_command(1, &submit_resolution_choice(vec![second_card]))
        .expect("commit both discards");

    let triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect::<Vec<_>>();
    assert_eq!(
        triggers.len(),
        2,
        "each player's simultaneous discard is its own event"
    );
    assert_eq!(triggers[0].trigger_context.event_count, Some(1));
    assert_eq!(triggers[1].trigger_context.event_count, Some(1));
    assert_eq!(triggers[0].controller, 0, "active player's trigger first");
    assert_eq!(triggers[1].controller, 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, first_observer), 1);
    assert_eq!(counters(&engine, second_observer), 1);
    assert_eq!(engine.state.objects[&first_card].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[0].hand.len(),
        caster_hand,
        "the caster discarded one card and drew one card"
    );
}

#[test]
fn issue_289_random_multi_card_discard_is_one_event() {
    let decks = Some(vec![
        deck_with("swamp", &["hymn_to_tourach"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(289_014, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let observer = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    grant_discard_batch_counter(&mut engine, observer);
    clear_hand(&mut engine, 1);
    for _ in 0..3 {
        relocate_to_hand(&mut engine, 1, "forest");
    }
    ensure_in_hand(&mut engine, 0, "hymn_to_tourach");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "hymn_to_tourach");
    engine
        .apply_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Hymn to Tourach");
    pass_both_players(&mut engine);
    let triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect::<Vec<_>>();
    assert_eq!(
        triggers.len(),
        1,
        "both randomly chosen cards are one event"
    );
    assert_eq!(triggers[0].trigger_context.event_count, Some(2));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, observer), 2);
}
