//! Issue #440 actual-card scenarios: removing every counter of every kind.
//!
//! Oracle and Scryfall rulings checked 2026-11-23 against the current Comprehensive Rules
//! (2026-11-06). Perfect Intimidation: CR 700.2 (choose one or both), 701.13a and 406 (the
//! opponent exiles, which is not a discard under 701.9a). Vampire Hexmage: CR 602.2 and 701.21a
//! (sacrifice cost), 122.1 (all counters, including 122.1b keyword and 122.1e loyalty counters),
//! 704.5i (a planeswalker with 0 loyalty), and 608.2b (an illegal target). Pinned Hexmage
//! rulings: any permanent may be targeted, even one without counters.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChoiceKind, TargetRef, TargetRefKind};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &[]),
        deck_with("plains", &["swords_to_plowshares"]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn permanent(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }]
}

fn opponent() -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: 1,
        group_index: 0,
        kind: TargetRefKind::Player as i32,
        ..Default::default()
    }]
}

fn put(e: &mut GameEngine, oid: u32, counters: &[(CounterKind, u32)]) {
    let object = e.state.objects.get_mut(&oid).expect("object");
    for &(kind, n) in counters {
        object.set_counter(kind, n);
    }
}

/// Casts Perfect Intimidation with `(mode index, targets)` pairs and passes once per player.
fn cast_intimidation(
    e: &mut GameEngine,
    modes: Vec<(u32, Vec<TargetRef>)>,
) -> Result<RuledEventBatch, tricerules_core::EngineError> {
    inject_card_into_hand(e, 0, "perfect_intimidation");
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, "perfect_intimidation");
    e.apply_scenario_command(0, &cast_modal_spell(slot, modes))?;
    e.apply_scenario_command(0, &pass()).expect("caster passes");
    Ok(e.apply_scenario_command(1, &pass())
        .expect("opponent passes"))
}

fn fresh_opponent_hand(e: &mut GameEngine, cards: &[&str]) -> Vec<u32> {
    e.state.players[1].hand.clear();
    cards
        .iter()
        .map(|card| inject_card_into_hand(e, 1, card))
        .collect()
}

#[test]
fn issue_440_perfect_intimidation_removes_every_counter_kind_from_the_target_creature() {
    let mut e = engine(440_001);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    put(
        &mut e,
        bear,
        &[
            (CounterKind::PlusOnePlusOne, 2),
            (CounterKind::Keyword(Keyword::Flying), 1),
            (CounterKind::Stun, 1),
        ],
    );
    let bystander = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    put(&mut e, bystander, &[(CounterKind::PlusOnePlusOne, 1)]);
    assert!(e.effective_has_keyword(bear, Keyword::Flying));

    cast_intimidation(&mut e, vec![(1, permanent(bear))]).expect("cast the counter mode");

    assert!(
        e.state.objects[&bear].counters.is_empty(),
        "every counter of every kind is removed: {:?}",
        e.state.objects[&bear].counters
    );
    let c = e.characteristics(bear).expect("bear");
    assert_eq!((c.power, c.toughness), (Some(2), Some(2)), "printed 2/2");
    assert!(!e.effective_has_keyword(bear, Keyword::Flying));
    assert_eq!(
        e.state.objects[&bystander].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "only the target loses counters"
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn issue_440_perfect_intimidation_counter_mode_is_a_no_op_on_a_counterless_creature() {
    let mut e = engine(440_002);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_intimidation(&mut e, vec![(1, permanent(bear))]).expect("a counterless target is legal");
    assert_eq!(e.state.objects[&bear].zone, Zone::Battlefield);
    assert!(e.state.objects[&bear].counters.is_empty());
    assert!(e.state.stack.is_empty());
}

#[test]
fn issue_440_perfect_intimidation_counter_mode_rejects_a_noncreature_target() {
    let mut e = engine(440_003);
    let saga = inject_creature_on_battlefield(&mut e, 1, "the_cave_of_two_lovers");
    put(&mut e, saga, &[(CounterKind::Lore, 1)]);
    assert!(
        cast_intimidation(&mut e, vec![(1, permanent(saga))]).is_err(),
        "the second mode targets a creature only"
    );
    assert_eq!(e.state.objects[&saga].counter_count(CounterKind::Lore), 1);
}

#[test]
fn issue_440_perfect_intimidation_opponent_privately_exiles_two_cards_of_their_choice() {
    let mut e = engine(440_004);
    let hand = fresh_opponent_hand(&mut e, &["grizzly_bears", "storm_crow", "hill_giant"]);
    let parked = cast_intimidation(&mut e, vec![(0, opponent())]).expect("cast the exile mode");

    let choice = find_resolution_choice(&parked).expect("opponent hand choice");
    assert_eq!(
        choice.choice_kind(),
        ChoiceKind::HandCards,
        "the affected opponent chooses from their own hand"
    );
    assert_eq!(choice.deciding_player_id, 1, "the opponent chooses");
    assert!(choice.public_reveal.is_none(), "the hand stays private");
    assert_eq!((choice.min, choice.max), (2, 2));
    assert!(
        e.apply_scenario_command(0, &submit_resolution_choice(vec![hand[0], hand[1]]))
            .is_err(),
        "the caster cannot answer the opponent's choice"
    );

    e.apply_scenario_command(1, &submit_resolution_choice(vec![hand[0], hand[2]]))
        .expect("the opponent exiles two");
    assert_eq!(e.state.objects[&hand[0]].zone, Zone::Exile);
    assert_eq!(e.state.objects[&hand[2]].zone, Zone::Exile);
    assert_eq!(e.state.objects[&hand[1]].zone, Zone::Hand);
    assert!(
        e.state.players[1].graveyard.is_empty(),
        "exiling is not discarding"
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn issue_440_perfect_intimidation_exiles_the_only_card_of_a_one_card_hand() {
    let mut e = engine(440_005);
    let hand = fresh_opponent_hand(&mut e, &["grizzly_bears"]);
    let parked = cast_intimidation(&mut e, vec![(0, opponent())]).expect("cast the exile mode");
    if let Some(choice) = find_resolution_choice(&parked) {
        assert_eq!(choice.max, 1, "only one card can be chosen");
        e.apply_scenario_command(1, &submit_resolution_choice(vec![hand[0]]))
            .expect("exile the only card");
    }
    assert_eq!(e.state.objects[&hand[0]].zone, Zone::Exile);
    assert!(e.state.players[1].hand.is_empty());
}

#[test]
fn issue_440_perfect_intimidation_resolves_both_modes_in_printed_order() {
    let mut e = engine(440_006);
    let hand = fresh_opponent_hand(&mut e, &["grizzly_bears", "storm_crow"]);
    let bear = inject_creature_on_battlefield(&mut e, 1, "hill_giant");
    put(&mut e, bear, &[(CounterKind::PlusOnePlusOne, 3)]);
    let parked = cast_intimidation(&mut e, vec![(0, opponent()), (1, permanent(bear))])
        .expect("cast both modes");

    find_resolution_choice(&parked).expect("the exile mode parks first");
    assert_eq!(
        e.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        3,
        "the counter mode has not resolved yet"
    );
    e.apply_scenario_command(1, &submit_resolution_choice(hand.clone()))
        .expect("the opponent exiles both");
    assert!(hand
        .iter()
        .all(|oid| e.state.objects[oid].zone == Zone::Exile));
    assert!(e.state.objects[&bear].counters.is_empty());
    assert!(e.state.stack.is_empty());
}

fn hexmage(e: &mut GameEngine) -> u32 {
    let hexmage = inject_creature_on_battlefield(e, 0, "vampire_hexmage");
    assert!(e.effective_has_keyword(hexmage, Keyword::FirstStrike));
    hexmage
}

#[test]
fn issue_440_vampire_hexmage_removes_all_loyalty_and_the_planeswalker_dies() {
    let mut e = engine(440_010);
    let source = hexmage(&mut e);
    let walker = inject_creature_on_battlefield(&mut e, 1, "ajani_steadfast");
    put(&mut e, walker, &[(CounterKind::Loyalty, 4)]);

    apply_ability(&mut e, 0, source, 0, permanent(walker)).expect("sacrifice to activate");
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Graveyard,
        "sacrifice is a cost"
    );
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.objects[&walker].zone,
        Zone::Graveyard,
        "a planeswalker with 0 loyalty is put into its owner's graveyard"
    );
}

#[test]
fn issue_440_vampire_hexmage_clears_a_noncreature_permanent_and_a_counterless_one() {
    let mut e = engine(440_011);
    let source = hexmage(&mut e);
    let saga = inject_creature_on_battlefield(&mut e, 1, "the_cave_of_two_lovers");
    put(&mut e, saga, &[(CounterKind::Lore, 2)]);
    apply_ability(&mut e, 0, source, 0, permanent(saga)).expect("target the Saga");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&saga].zone, Zone::Battlefield);
    assert!(e.state.objects[&saga].counters.is_empty());

    let second = hexmage(&mut e);
    let land = inject_creature_on_battlefield(&mut e, 1, "plains");
    apply_ability(&mut e, 0, second, 0, permanent(land))
        .expect("any permanent is a legal target, even one without counters");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
}

#[test]
fn issue_440_vampire_hexmage_rejects_a_player_target() {
    let mut e = engine(440_012);
    let source = hexmage(&mut e);
    assert!(apply_ability(&mut e, 0, source, 0, opponent()).is_err());
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "no cost is paid for a rejected activation"
    );
}

#[test]
fn issue_440_vampire_hexmage_does_nothing_to_a_target_that_left_the_battlefield() {
    let mut e = engine(440_013);
    let source = hexmage(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    put(&mut e, bear, &[(CounterKind::PlusOnePlusOne, 2)]);
    apply_ability(&mut e, 0, source, 0, permanent(bear)).expect("activate at the bear");
    e.apply_scenario_command(0, &pass())
        .expect("activator passes");
    give_mana(
        &mut e,
        1,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut e, 1, "swords_to_plowshares");
    let swords = hand_index_for_card(&e, 1, "swords_to_plowshares");
    e.apply_scenario_command(1, &cast_spell(swords, target_object(bear)))
        .expect("exile the target in response");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Exile);
    assert!(
        e.state.stack.is_empty(),
        "the ability resolves without a legal target"
    );
}
