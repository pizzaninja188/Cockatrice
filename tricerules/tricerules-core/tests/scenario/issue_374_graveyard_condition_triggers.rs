//! Issue #374 focused scenarios for the five retained graveyard-condition trigger identities.
//!
//! Public graveyard state (CR 404.2) gates each trigger through the shipped `GraveyardAggregate`
//! condition, and CR 603.4 rechecks the intervening-if both when the trigger is created and when
//! it resolves. The scenarios drive the generated definitions through the authoritative command
//! path and pin the `0↔1` Elf/Lesson boundaries, the four-distinct-card-type Delirium boundary,
//! the four-permanent-card Descend boundary, the controller-owner scope, and the printed order of
//! Dawnhand Eulogist's mill-then-branch instruction.

use super::helpers::*;
use tricerules_cards::{CardRegistry, CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};

fn engine_with(seed: u64, own: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", own), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn stats(engine: &GameEngine, oid: u32) -> (u32, u32) {
    let characteristics = engine.characteristics(oid).expect("characteristics");
    (
        characteristics.power.expect("power"),
        characteristics.toughness.expect("toughness"),
    )
}

fn counters(engine: &GameEngine, oid: u32) -> u32 {
    engine.state.objects[&oid].counter_count(CounterKind::MinusOneMinusOne)
}

fn advance_to_end_step(engine: &mut GameEngine, active: i32) {
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 1 to beginning of combat");
    engine
        .apply_command(active, &primitive_yield())
        .expect("beginning of combat advance");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_command(active, &primitive_yield())
            .expect("declare no attackers");
    }
    engine
        .apply_command(active, &primitive_yield())
        .expect("end combat to main 2");
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 2 to end step");
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
}

fn move_graveyard_object_to_exile(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|id| *id != object_id);
    engine.state.players[player].exile.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("graveyard object")
        .zone = Zone::Exile;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object_id = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].library.push_front(object_id);
    object_id
}

/// Declare exactly `attacker` while the engine is already in the declare-attackers step.
fn declare_sole_attacker(engine: &mut GameEngine, attacker: u32) {
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    let assignment = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|assignment| assignment.attacker_object_id == attacker)
        .cloned()
        .expect("the attacker is legal");
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .expect("declare the attacker");
    resolve_entire_stack_two_player(engine);
}

/// Kill `target` with Lightning Bolt so the committed battlefield-to-graveyard move runs through
/// the state-based-action funnel and emits the CR 603.6c dies event.
fn kill_with_lightning_bolt(engine: &mut GameEngine, target: u32) {
    ensure_in_hand(engine, 0, "lightning_bolt");
    give_mana(
        engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Lightning Bolt");
    resolve_entire_stack_two_player(engine);
}

#[test]
fn issue_374_creakwood_enters_with_three_counters_and_removes_one_at_the_end_step() {
    let mut engine = engine_with(374_001, &["creakwood_safewright"]);
    let creakwood = move_ready_to_battlefield(&mut engine, 0, "creakwood_safewright");
    assert_eq!(
        counters(&engine, creakwood),
        3,
        "the entry replacement places exactly three -1/-1 counters"
    );
    assert_eq!(stats(&engine, creakwood), (2, 2));

    // Without an Elf card in the graveyard the end-step trigger never gets created.
    advance_to_end_step(&mut engine, 0);
    assert!(
        engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty(),
        "no Elf card means no end-step trigger"
    );
    assert_eq!(counters(&engine, creakwood), 3);

    // One Elf card enables the trigger and exactly one counter is removed.
    let mut enabled = engine_with(374_002, &["creakwood_safewright"]);
    let creakwood = move_ready_to_battlefield(&mut enabled, 0, "creakwood_safewright");
    inject_graveyard_card(&mut enabled, 0, "llanowar_visionary");
    advance_to_end_step(&mut enabled, 0);
    assert_eq!(
        enabled.state.stack.len(),
        1,
        "the end-step trigger resolves"
    );
    resolve_entire_stack_two_player(&mut enabled);
    assert_eq!(counters(&enabled, creakwood), 2);
    assert_eq!(stats(&enabled, creakwood), (3, 3));

    // With the Elf card present but no -1/-1 counter on the source, the conjunction fails.
    let mut uncounted = engine_with(374_003, &["creakwood_safewright"]);
    let creakwood = move_ready_to_battlefield(&mut uncounted, 0, "creakwood_safewright");
    inject_graveyard_card(&mut uncounted, 0, "llanowar_visionary");
    uncounted
        .state
        .objects
        .get_mut(&creakwood)
        .expect("source")
        .counters
        .remove(&CounterKind::MinusOneMinusOne);
    advance_to_end_step(&mut uncounted, 0);
    assert!(
        uncounted.state.stack.is_empty() && uncounted.state.pending_triggers.is_empty(),
        "no counter on the source means no end-step trigger"
    );
}

#[test]
fn issue_374_creakwood_intervening_if_rechecks_the_graveyard_on_resolution() {
    let mut engine = engine_with(374_010, &["creakwood_safewright"]);
    let creakwood = move_ready_to_battlefield(&mut engine, 0, "creakwood_safewright");
    let elf = inject_graveyard_card(&mut engine, 0, "llanowar_visionary");
    advance_to_end_step(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1, "the trigger is on the stack");

    // Emptying the Elf card before resolution fails the recheck even though the trigger was legal
    // when it was created.
    move_graveyard_object_to_exile(&mut engine, 0, elf);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        counters(&engine, creakwood),
        3,
        "the recheck stops the counter removal"
    );
}

#[test]
fn issue_374_dawnhand_mills_first_then_drains_when_the_mill_leaves_an_elf_card() {
    // The milled Elf card satisfies the branch even though the graveyard was empty before entry.
    let mut engine = engine_with(374_020, &["dawnhand_eulogist"]);
    seat_on_top(&mut engine, 0, "forest");
    seat_on_top(&mut engine, 0, "llanowar_visionary");
    seat_on_top(&mut engine, 0, "forest");
    let before = engine.state.players[0].life;
    let opponent_before = engine.state.players[1].life;
    move_ready_to_battlefield(&mut engine, 0, "dawnhand_eulogist");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        count_card_id_in_graveyard(&engine, 0, "llanowar_visionary"),
        1,
        "the printed mill happens first"
    );
    assert_eq!(engine.state.players[0].life, before + 2);
    assert_eq!(engine.state.players[1].life, opponent_before - 2);

    // A mill of non-Elf cards leaves the branch unsatisfied and no life changes.
    let mut empty = engine_with(374_021, &["dawnhand_eulogist"]);
    for _ in 0..3 {
        seat_on_top(&mut empty, 0, "forest");
    }
    let before = empty.state.players[0].life;
    let opponent_before = empty.state.players[1].life;
    move_ready_to_battlefield(&mut empty, 0, "dawnhand_eulogist");
    resolve_entire_stack_two_player(&mut empty);
    assert_eq!(
        count_card_id_in_graveyard(&empty, 0, "forest"),
        3,
        "the mill still happens"
    );
    assert_eq!(empty.state.players[0].life, before);
    assert_eq!(empty.state.players[1].life, opponent_before);

    // An opponent's Elf card is outside the controller-owner scope.
    let mut foreign = engine_with(374_022, &["dawnhand_eulogist"]);
    inject_graveyard_card(&mut foreign, 1, "llanowar_visionary");
    for _ in 0..3 {
        seat_on_top(&mut foreign, 0, "forest");
    }
    let before = foreign.state.players[0].life;
    let opponent_before = foreign.state.players[1].life;
    move_ready_to_battlefield(&mut foreign, 0, "dawnhand_eulogist");
    resolve_entire_stack_two_player(&mut foreign);
    assert_eq!(foreign.state.players[0].life, before);
    assert_eq!(foreign.state.players[1].life, opponent_before);
}

#[test]
fn issue_374_hand_that_feeds_attack_pump_needs_four_distinct_card_types() {
    // Three card types are below the Delirium boundary.
    let decks = Some(vec![
        deck_with("forest", &["hand_that_feeds"]),
        deck_with("forest", &[]),
    ]);
    let mut below = GameEngine::new(374_030, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut below);
    let hand = relocate_to_battlefield(&mut below, 0, "hand_that_feeds", false);
    inject_graveyard_card(&mut below, 0, "grizzly_bears");
    inject_graveyard_card(&mut below, 0, "lightning_bolt");
    inject_graveyard_card(&mut below, 0, "unsummon");
    declare_sole_attacker(&mut below, hand);
    assert_eq!(
        stats(&below, hand),
        (2, 2),
        "three card types are below the Delirium boundary"
    );
    assert!(!below.effective_has_keyword(hand, Keyword::Menace));

    // A fourth distinct card type turns the pump and menace on.
    let decks = Some(vec![
        deck_with("forest", &["hand_that_feeds"]),
        deck_with("forest", &[]),
    ]);
    let mut at = GameEngine::new(374_031, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut at);
    let hand = relocate_to_battlefield(&mut at, 0, "hand_that_feeds", false);
    inject_graveyard_card(&mut at, 0, "grizzly_bears");
    inject_graveyard_card(&mut at, 0, "lightning_bolt");
    inject_graveyard_card(&mut at, 0, "pyroclasm");
    inject_graveyard_card(&mut at, 0, "forest");
    declare_sole_attacker(&mut at, hand);
    assert_eq!(stats(&at, hand), (4, 2));
    assert!(at.effective_has_keyword(hand, Keyword::Menace));

    // Instant and sorcery cards alone are only two card types.
    let decks = Some(vec![
        deck_with("forest", &["hand_that_feeds"]),
        deck_with("forest", &[]),
    ]);
    let mut spells = GameEngine::new(374_032, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut spells);
    let hand = relocate_to_battlefield(&mut spells, 0, "hand_that_feeds", false);
    inject_graveyard_card(&mut spells, 0, "lightning_bolt");
    inject_graveyard_card(&mut spells, 0, "pyroclasm");
    declare_sole_attacker(&mut spells, hand);
    assert_eq!(stats(&spells, hand), (2, 2));
    assert!(!spells.effective_has_keyword(hand, Keyword::Menace));
}

#[test]
fn issue_374_stinging_cave_crawler_attack_needs_four_permanent_cards() {
    // Three permanent cards plus an instant is below the Descend boundary.
    let decks = Some(vec![
        deck_with("forest", &["stinging_cave_crawler"]),
        deck_with("forest", &[]),
    ]);
    let mut below = GameEngine::new(374_040, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut below);
    let crawler = relocate_to_battlefield(&mut below, 0, "stinging_cave_crawler", false);
    for _ in 0..3 {
        inject_graveyard_card(&mut below, 0, "grizzly_bears");
    }
    inject_graveyard_card(&mut below, 0, "lightning_bolt");
    let hand_before = below.state.players[0].hand.len();
    let life_before = below.state.players[0].life;
    declare_sole_attacker(&mut below, crawler);
    assert_eq!(below.state.players[0].hand.len(), hand_before);
    assert_eq!(below.state.players[0].life, life_before);

    // Four permanent cards enable the draw and the one-life loss; the instant still does not
    // count toward the permanent-card total.
    let decks = Some(vec![
        deck_with("forest", &["stinging_cave_crawler"]),
        deck_with("forest", &[]),
    ]);
    let mut at = GameEngine::new(374_041, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut at);
    let crawler = relocate_to_battlefield(&mut at, 0, "stinging_cave_crawler", false);
    for _ in 0..4 {
        inject_graveyard_card(&mut at, 0, "grizzly_bears");
    }
    inject_graveyard_card(&mut at, 0, "lightning_bolt");
    let hand_before = at.state.players[0].hand.len();
    let life_before = at.state.players[0].life;
    declare_sole_attacker(&mut at, crawler);
    assert_eq!(at.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(at.state.players[0].life, life_before - 1);

    // Four instant and sorcery cards are not permanent cards.
    let decks = Some(vec![
        deck_with("forest", &["stinging_cave_crawler"]),
        deck_with("forest", &[]),
    ]);
    let mut spells = GameEngine::new(374_042, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut spells);
    let crawler = relocate_to_battlefield(&mut spells, 0, "stinging_cave_crawler", false);
    for _ in 0..2 {
        inject_graveyard_card(&mut spells, 0, "lightning_bolt");
        inject_graveyard_card(&mut spells, 0, "unsummon");
    }
    let hand_before = spells.state.players[0].hand.len();
    let life_before = spells.state.players[0].life;
    declare_sole_attacker(&mut spells, crawler);
    assert_eq!(spells.state.players[0].hand.len(), hand_before);
    assert_eq!(spells.state.players[0].life, life_before);
}

#[test]
fn issue_374_walltop_sentries_dies_with_a_lesson_card_gains_two() {
    let mut engine = engine_with(374_050, &["walltop_sentries", "lightning_bolt"]);
    let walltop = move_ready_to_battlefield(&mut engine, 0, "walltop_sentries");
    inject_graveyard_card(&mut engine, 0, "combustion_technique");
    let before = engine.state.players[0].life;
    kill_with_lightning_bolt(&mut engine, walltop);
    assert_eq!(engine.state.objects[&walltop].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, before + 2);

    // Non-Lesson cards never satisfy the printed predicate.
    let mut other = engine_with(374_051, &["walltop_sentries", "lightning_bolt"]);
    let walltop = move_ready_to_battlefield(&mut other, 0, "walltop_sentries");
    inject_graveyard_card(&mut other, 0, "grizzly_bears");
    inject_graveyard_card(&mut other, 0, "forest");
    inject_graveyard_card(&mut other, 0, "ornithopter");
    let before = other.state.players[0].life;
    kill_with_lightning_bolt(&mut other, walltop);
    assert_eq!(other.state.players[0].life, before);

    // An opponent's Lesson card is outside the controller-owner scope.
    let mut foreign = engine_with(374_052, &["walltop_sentries", "lightning_bolt"]);
    let walltop = move_ready_to_battlefield(&mut foreign, 0, "walltop_sentries");
    inject_graveyard_card(&mut foreign, 1, "combustion_technique");
    let before = foreign.state.players[0].life;
    kill_with_lightning_bolt(&mut foreign, walltop);
    assert_eq!(
        foreign.state.players[0].life, before,
        "the opponent's Lesson card does not satisfy the controller's graveyard gate"
    );
}

#[test]
fn issue_374_blocked_identities_are_not_registered() {
    let registry = CardRegistry::global();
    for name in [
        "Fear of Burning Alive",
        "Fear of Missing Out",
        "Osseous Sticktwister",
        "Starving Revenant",
        "Trystan, Callous Cultivator // Trystan, Penitent Culler",
        "Winter, Misanthropic Guide",
    ] {
        assert!(
            registry.id_for_name(name).is_none(),
            "{name} must stay fail-closed until its narrower blocker lands"
        );
    }
}
