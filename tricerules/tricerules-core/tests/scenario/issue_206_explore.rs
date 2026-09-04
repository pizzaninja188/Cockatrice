//! Issue #206 — CR 701.44 Explore resolution and public library reveal choices.

use crate::helpers::*;
use tricerules_cards::primitives::{
    CastTriggerPlayer, ContinuousEffectKind, EffectDuration, EffectSubject, SpellEffectKind,
    TriggerCondition,
};
use tricerules_cards::{CardRegistry, CounterKind};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{permanent_moved, ChoiceKind, ResolutionRevealAudience};

fn put_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object_id = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].library.push_front(object_id);
    object_id
}

fn grant_source_explore(engine: &mut GameEngine, source: u32) {
    let mut ability = CardRegistry::global()
        .get("acrobatic_cheerleader")
        .expect("phase-trigger fixture")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::AtBeginningOfCombat {
        player: CastTriggerPlayer::Controller,
    };
    ability.effect = vec![SpellEffectKind::Explore {
        subject: EffectSubject::Source,
    }];
    ability.modal = None;
    ability.targeting = None;
    ability.may = false;
    ability.intervening_if = None;
    ability.triggers_only_once = false;
    ability.max_triggers_per_turn = None;
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

fn begin_source_explore(seed: u64, top_card: Option<&str>) -> (GameEngine, u32, Option<u32>) {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let top = top_card.map(|card_id| put_on_top(&mut engine, 0, card_id));

    grant_source_explore(&mut engine, source);

    engine
        .apply_command(0, &primitive_yield())
        .expect("enter beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::BeginCombat);
    assert!(engine
        .state
        .stack
        .last()
        .is_some_and(|item| item.is_triggered));
    (engine, source, top)
}

#[test]
fn issue_206_nonland_explore_counters_then_offers_a_public_optional_graveyard_move() {
    let (mut engine, source, top) = begin_source_explore(206_001, Some("storm_crow"));
    let top = top.expect("top card");
    let below = inject_library_card(&mut engine, 0, "hill_giant");

    let first = engine.state.priority_player_id();
    let second = 1 - first;
    engine.apply_command(first, &pass()).expect("first pass");
    let batch = engine
        .apply_command(second, &pass())
        .expect("resolve Explore trigger");
    let choice = find_resolution_choice(&batch).expect("Explore choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.candidate_object_ids, vec![top]);
    assert_eq!(choice.candidate_selectable, vec![true]);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(
        choice.reveal_audience(),
        ResolutionRevealAudience::AllParticipants
    );
    assert_eq!(choice.revealed_zone_owner_player_id, Some(0));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1
    );

    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![below]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.players[0].library.front(), Some(&top));

    let revealed_generation = engine
        .state
        .zone_change_generation
        .get(&top)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(top, revealed_generation + 1);
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .is_err());
    assert!(
        engine.state.pending_resolution.is_some(),
        "a stale revealed generation keeps the choice outstanding"
    );
    engine
        .state
        .zone_change_generation
        .insert(top, revealed_generation);

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![top]))
        .expect("put the revealed nonland into the graveyard");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&top].zone, Zone::Graveyard);
    let moved = permanents_moved_in(&completion)
        .into_iter()
        .find(|moved| moved.object_id == top)
        .expect("revealed card move");
    assert_eq!(moved.destination(), permanent_moved::Destination::Graveyard);
    assert_eq!(moved.source_library_position, Some(0));
}

#[test]
fn issue_206_land_explore_reveals_and_moves_the_land_to_hand_without_a_counter() {
    let (mut engine, source, top) = begin_source_explore(206_002, Some("forest"));
    let top = top.expect("top card");
    pass_both_players(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&top].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&top));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn issue_206_empty_library_still_completes_explore_and_places_the_counter() {
    let (mut engine, source, _) = begin_source_explore(206_003, None);
    engine.state.players[0].library.clear();
    pass_both_players(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn issue_206_departed_source_uses_last_known_controller_and_cannot_receive_a_counter() {
    let decks = Some(vec![
        deck_with("forest", &["storm_crow"]),
        deck_with("swamp", &["murder"]),
    ]);
    let mut engine = GameEngine::new(206_005, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_source_explore(&mut engine, source);
    let top = put_on_top(&mut engine, 0, "storm_crow");
    ensure_in_hand(&mut engine, 1, "murder");
    engine
        .apply_command(0, &primitive_yield())
        .expect("put Explore trigger on the stack");
    engine
        .apply_command(0, &pass())
        .expect("yield to responder");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder_slot = hand_index_for_card(&engine, 1, "murder");
    engine
        .apply_command(1, &cast_spell(murder_slot, target_object(source)))
        .expect("destroy Explore source in response");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);

    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("LKI controller gets the Explore choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.candidates, [top]);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "a departed permanent cannot receive the Explore counter"
    );
}

#[test]
fn issue_206_spyglass_siren_creates_a_map_whose_atomic_activation_explores() {
    let decks = Some(vec![
        deck_with("island", &["spyglass_siren", "storm_crow"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine = GameEngine::new(206_004, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "spyglass_siren");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "spyglass_siren");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Spyglass Siren");
    pass_both_players(&mut engine);
    let siren = battlefield_object_for_card(&engine, 0, "spyglass_siren");
    assert!(engine
        .state
        .stack
        .last()
        .is_some_and(|item| item.is_triggered));
    pass_both_players(&mut engine);
    let maps = battlefield_token_oids(&engine, 0, "map");
    let [map] = maps.as_slice() else {
        panic!("Spyglass Siren should create one Map");
    };

    let top = put_on_top(&mut engine, 0, "storm_crow");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, *map, 0, target_object(siren))
        .expect("activate Map at sorcery speed");
    assert!(
        battlefield_token_oids(&engine, 0, "map").is_empty(),
        "tap and sacrifice are committed as activation costs"
    );
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("revealed nonland choice");
    assert_eq!(pending.presentation.candidates, [top]);
    assert_eq!(
        engine.state.objects[&siren].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("leave the revealed card on top");
    assert_eq!(engine.state.players[0].library.front(), Some(&top));
}
