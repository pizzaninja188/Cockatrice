//! Issue #96 — CR 701.25 surveil and bounded top-library partitions.

use crate::helpers::*;
use tricerules_cards::primitives::{
    CastTriggerPlayer, ContinuousEffectKind, EffectDuration, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::CardRegistry;
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{permanent_moved, ChoiceKind};

fn black_deck_with(card: &str) -> Option<Vec<Vec<String>>> {
    Some(vec![deck_with("swamp", &[card]), vec!["forest".into(); 20]])
}

fn black_mana(amount: u32) -> ManaGift {
    ManaGift {
        b: 1,
        c: amount.saturating_sub(1),
        ..Default::default()
    }
}

/// Put `card_ids` on top of `player`'s library, first entry on top, and return their OIDs.
fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect();
    e.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        e.state.players[player].library.push_front(oid);
    }
    oids
}

fn advance_to_main2(e: &mut GameEngine) {
    for _ in 0..20 {
        let actor = e.state.priority_player_id();
        e.apply_command(actor, &pass())
            .expect("pass through combat");
        if e.state.turn_step == TurnStep::Main2 {
            return;
        }
    }
    panic!("combat did not reach main2");
}

fn cast_creature_and_resolve(e: &mut GameEngine, card_id: &str, mana: ManaGift) {
    ensure_in_hand(e, 0, card_id);
    give_mana(e, 0, mana);
    let index = hand_index_for_card(e, 0, card_id);
    e.apply_command(0, &cast_spell(index, vec![]))
        .expect("cast creature");
    pass_both_players(e);
}

fn resolve_top_stack(e: &mut GameEngine) -> RuledEventBatch {
    let first = e.state.priority_player_id();
    let second = 1 - first;
    e.apply_command(first, &pass())
        .expect("first pass on stack item");
    e.apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

#[test]
fn issue_96_cruel_truths_moves_the_chosen_card_then_resumes_its_tail() {
    let mut e = GameEngine::new(96_001, &[0, 1], 20, black_deck_with("cruel_truths"), true)
        .expect("new engine");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "cruel_truths");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow", "hill_giant"]);
    let (to_graveyard, kept, next) = (top[0], top[1], top[2]);
    let moved_generation = e
        .state
        .zone_change_generation
        .get(&to_graveyard)
        .copied()
        .unwrap_or(0);
    let kept_generation = e
        .state
        .zone_change_generation
        .get(&kept)
        .copied()
        .unwrap_or(0);

    let batch = cast_instant_and_resolve(&mut e, 0, "cruel_truths", black_mana(4));
    let choice = find_resolution_choice(&batch).expect("surveil choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert!(choice.ordered, "graveyard order is significant");
    assert_eq!(choice.candidate_object_ids, vec![to_graveyard, kept]);

    let completion = e
        .apply_command(0, &submit_resolution_choice(vec![to_graveyard]))
        .expect("put one surveilled card into the graveyard");

    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.objects[&to_graveyard].zone, Zone::Graveyard);
    assert!(e.state.players[0].graveyard.contains(&to_graveyard));
    assert_eq!(
        e.state.zone_change_generation[&to_graveyard],
        moved_generation + 1,
        "library-to-graveyard creates a new zone object"
    );
    assert_eq!(
        e.state
            .zone_change_generation
            .get(&kept)
            .copied()
            .unwrap_or(0),
        kept_generation,
        "the card retained on top never changes zones"
    );
    assert!(e.state.players[0].hand.contains(&kept));
    assert!(e.state.players[0].hand.contains(&next));
    assert_eq!(e.state.players[0].life, 18);
    let moved = permanents_moved_in(&completion)
        .into_iter()
        .find(|moved| moved.object_id == to_graveyard)
        .expect("surveilled card publishes its zone move");
    assert_eq!(moved.destination(), permanent_moved::Destination::Graveyard);
    assert_eq!(
        moved.source_library_position,
        Some(0),
        "the relay must bind the exact private candidate instead of a same-named card"
    );
}

#[test]
fn issue_96_surveillance_waits_for_top_order_before_later_effects() {
    let mut e = GameEngine::new(96_002, &[0, 1], 20, black_deck_with("cruel_truths"), true)
        .expect("new engine");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "cruel_truths");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow", "hill_giant"]);
    let hand_before = e.state.players[0].hand.len();

    cast_instant_and_resolve(&mut e, 0, "cruel_truths", black_mana(4));
    let ordering = e
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both cards on top");
    let order_choice = find_resolution_choice(&ordering).expect("top ordering choice");
    assert_eq!(order_choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((order_choice.min, order_choice.max), (2, 2));
    assert_eq!(order_choice.candidate_object_ids, top[..2]);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1,
        "the spell left hand, but its draw tail has not run"
    );
    assert_eq!(e.state.players[0].life, 20);

    e.apply_command(0, &submit_resolution_choice(vec![top[1], top[0]]))
        .expect("order the retained cards");

    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.players[0].life, 18);
    let hand = &e.state.players[0].hand;
    assert_eq!(&hand[hand.len() - 2..], &[top[0], top[1]]);
}

#[test]
fn issue_96_surveil_rejects_illegal_submissions_atomically() {
    let mut e = GameEngine::new(96_003, &[0, 1], 20, black_deck_with("cruel_truths"), true)
        .expect("new engine");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "cruel_truths");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow", "hill_giant"]);
    let (a, b, below_window) = (top[0], top[1], top[2]);

    cast_instant_and_resolve(&mut e, 0, "cruel_truths", black_mana(4));
    let library_before: Vec<u32> = e.state.players[0].library.iter().copied().collect();

    for (label, player, choice) in [
        ("wrong player", 1, vec![a]),
        ("card below the surveilled window", 0, vec![below_window]),
        (
            "more cards than were looked at",
            0,
            vec![a, b, below_window],
        ),
        ("the same card twice", 0, vec![a, a]),
    ] {
        assert!(
            e.apply_command(player, &submit_resolution_choice(choice))
                .is_err(),
            "{label} must be rejected"
        );
        assert!(
            e.state.pending_resolution.is_some(),
            "{label}: the choice remains outstanding"
        );
        assert!(e.state.players[0].graveyard.is_empty());
        assert_eq!(
            e.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            library_before,
            "{label}: the library is untouched"
        );
    }

    let completion = e
        .apply_command(0, &submit_resolution_choice(vec![a, b]))
        .expect("valid submission after rejected attempts");
    assert!(e.state.pending_resolution.is_none());
    assert!(e.state.players[0].graveyard.contains(&a));
    assert!(e.state.players[0].graveyard.contains(&b));
    assert!(!e.state.players[0].graveyard.contains(&below_window));
    let indexed_moves: Vec<_> = permanents_moved_in(&completion)
        .into_iter()
        .filter(|moved| moved.object_id == a || moved.object_id == b)
        .map(|moved| (moved.object_id, moved.source_library_position))
        .collect();
    assert_eq!(
        indexed_moves,
        vec![(a, Some(0)), (b, Some(0))],
        "each sequential relay move uses its position after earlier selected cards leave"
    );
}

#[test]
fn issue_96_surveil_trigger_fires_only_after_the_complete_action() {
    let mut e = GameEngine::new(96_004, &[0, 1], 20, black_deck_with("cruel_truths"), true)
        .expect("new engine");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "cruel_truths");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
    let observer = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let mut ability = CardRegistry::global()
        .get("audacious_thief")
        .expect("Audacious Thief definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::WheneverPlayerSurveils {
        player: CastTriggerPlayer::Controller,
    };
    e.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(observer),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: e.state.command_index,
    });

    cast_instant_and_resolve(&mut e, 0, "cruel_truths", black_mana(4));
    let ordering = e
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both surveilled cards on top");
    assert!(find_resolution_choice(&ordering).is_some());
    assert!(
        e.state
            .stack
            .iter()
            .all(|item| item.source_permanent_id != Some(observer)),
        "surveil has not happened until the retained cards are ordered"
    );
    assert!(e.state.pending_triggers.is_empty());
    assert!(e.state.staged_trigger_groups.is_empty());

    e.apply_command(0, &submit_resolution_choice(vec![top[1], top[0]]))
        .expect("finish the surveil action");
    assert!(e
        .state
        .stack
        .iter()
        .any(|item| item.source_permanent_id == Some(observer) && item.is_triggered));
}

#[test]
fn issue_96_gutless_plunderer_requires_raid_and_handles_a_short_library() {
    let mut no_raid = GameEngine::new(
        96_005,
        &[0, 1],
        20,
        black_deck_with("gutless_plunderer"),
        true,
    )
    .expect("new engine");
    advance_to_main1_from_game_start(&mut no_raid);
    cast_creature_and_resolve(&mut no_raid, "gutless_plunderer", black_mana(3));
    assert!(
        no_raid.state.stack.is_empty(),
        "raid did not trigger in main 1"
    );
    assert!(no_raid.state.pending_resolution.is_none());

    let mut raid = GameEngine::new(
        96_006,
        &[0, 1],
        20,
        black_deck_with("gutless_plunderer"),
        true,
    )
    .expect("new engine");
    advance_to_declare_attackers(&mut raid);
    let attacker = battlefield_object_for_card(&raid, 0, "grizzly_bears");
    raid.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare an attacker for raid");
    advance_to_main2(&mut raid);
    ensure_in_hand(&mut raid, 0, "gutless_plunderer");
    raid.state.players[0].library.clear();
    let top = seat_on_top(&mut raid, 0, &["storm_crow", "hill_giant"]);

    cast_creature_and_resolve(&mut raid, "gutless_plunderer", black_mana(3));
    let trigger = raid.state.stack.last().expect("raid ETB trigger");
    assert!(trigger.is_triggered);
    let resolution = resolve_top_stack(&mut raid);
    let pending = find_resolution_choice(&resolution).expect("short-library look choice");
    assert_eq!(pending.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((pending.min, pending.max), (1, 2));
    assert_eq!(pending.candidate_object_ids, top);

    raid.apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("put one card in the graveyard and retain one on top");
    assert!(raid.state.players[0].graveyard.contains(&top[0]));
    assert_eq!(raid.state.players[0].library.front(), Some(&top[1]));
}

#[test]
fn issue_96_wary_creatures_surveil_on_entry() {
    for (offset, card_id) in ["wary_thespian", "wary_watchdog"].into_iter().enumerate() {
        let mut e = GameEngine::new(
            96_010 + offset as u64,
            &[0, 1],
            20,
            black_deck_with(card_id),
            true,
        )
        .expect("new engine");
        advance_to_main1_from_game_start(&mut e);
        let top = seat_on_top(&mut e, 0, &["storm_crow"])[0];
        cast_creature_and_resolve(
            &mut e,
            card_id,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        assert!(e.state.stack.last().is_some_and(|item| item.is_triggered));
        let resolution = resolve_top_stack(&mut e);
        let choice = find_resolution_choice(&resolution).expect("entry surveil choice");
        assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
        assert_eq!(choice.candidate_object_ids, vec![top]);
        e.apply_command(0, &submit_resolution_choice(vec![top]))
            .expect("put the surveilled card into the graveyard");
        assert!(e.state.players[0].graveyard.contains(&top));
    }
}

#[test]
fn issue_96_surveillance_creatures_surveil_when_they_attack() {
    for (offset, card_id) in ["appendage_amalgam", "fear_of_surveillance"]
        .into_iter()
        .enumerate()
    {
        let mut e =
            GameEngine::new(96_020 + offset as u64, &[0, 1], 20, None, true).expect("new engine");
        advance_to_declare_attackers(&mut e);
        let source = inject_creature_on_battlefield(&mut e, 0, card_id);
        let top = seat_on_top(&mut e, 0, &["storm_crow"])[0];
        e.apply_command(0, &declare_attackers(vec![source]))
            .expect("declare the surveil creature as an attacker");
        assert!(e
            .state
            .stack
            .last()
            .is_some_and(|item| { item.is_triggered && item.source_permanent_id == Some(source) }));
        let resolution = resolve_top_stack(&mut e);
        let choice = find_resolution_choice(&resolution).expect("attack surveil choice");
        assert_eq!(choice.candidate_object_ids, vec![top]);
        e.apply_command(0, &submit_resolution_choice(vec![]))
            .expect("keep the surveilled card on top");
        assert_eq!(e.state.players[0].library.front(), Some(&top));
    }
}

#[test]
fn issue_96_registers_the_complete_surveil_card_cohort() {
    let registry = CardRegistry::global();
    for card_id in [
        "wary_thespian",
        "wary_watchdog",
        "appendage_amalgam",
        "fear_of_surveillance",
        "cruel_truths",
        "gutless_plunderer",
    ] {
        assert!(registry.get(card_id).is_some(), "missing {card_id}");
    }

    let cruel = registry
        .get("cruel_truths")
        .expect("Cruel Truths")
        .primary_face();
    assert!(matches!(
        cruel.spell_effect.first(),
        Some(SpellEffectKind::LibraryPartition { count: 2, .. })
    ));
    let plunderer = registry
        .get("gutless_plunderer")
        .expect("Gutless Plunderer")
        .primary_face();
    assert!(matches!(
        plunderer.triggered_abilities[0].effect.first(),
        Some(SpellEffectKind::LibraryPartition { count: 3, .. })
    ));
}
