//! Actual-card semantics for five pinned Standard combat spells.
//! Pinned Oracle and published rulings reviewed 2026-09-22.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast_and_resolve(e: &mut GameEngine, player: usize, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, player, card);
    grant_pool(e, player);
    let slot = hand_index_for_card(e, player, card);
    semantic::accepted(e, player as i32, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

fn remove_target_before_resolution(e: &mut GameEngine, target: u32) {
    let controller = e.state.objects[&target].controller as usize;
    let owner = e.state.objects[&target].owner as usize;
    e.state.players[controller]
        .battlefield
        .retain(|id| *id != target);
    e.state.players[owner].graveyard.push(target);
    e.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(target).or_default() += 1;
}

#[test]
fn issue_misc38_crash_through() {
    let mut e = engine(838_001);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let drawn_before = e.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut e, 0, "crash_through", vec![]);
    assert!(e.effective_has_keyword(own, Keyword::Trample));
    assert!(!e.effective_has_keyword(opposing, Keyword::Trample));
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        drawn_before + 1
    );
    let later = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(
        !e.effective_has_keyword(later, Keyword::Trample),
        "one-shot effect excludes later entrants"
    );

    let mut empty = engine(838_002);
    let before = empty.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut empty, 0, "crash_through", vec![]);
    assert_eq!(
        empty.state.turn_history.current.player(0).cards_drawn,
        before + 1
    );
}

#[test]
fn issue_misc38_efflorescence() {
    let mut plain = engine(838_010);
    let bear = inject_creature_on_battlefield(&mut plain, 0, "grizzly_bears");
    cast_and_resolve(&mut plain, 0, "efflorescence", target_object(bear));
    assert_eq!(
        plain.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert!(!plain.effective_has_keyword(bear, Keyword::Trample));
    assert!(!plain.effective_has_keyword(bear, Keyword::Indestructible));

    let mut opponent_gained = engine(838_013);
    let bear = inject_creature_on_battlefield(&mut opponent_gained, 0, "grizzly_bears");
    semantic::accepted(&mut opponent_gained, 0, &pass());
    cast_and_resolve(&mut opponent_gained, 1, "life_goes_on", vec![]);
    assert_eq!(
        opponent_gained
            .state
            .turn_history
            .current
            .player(0)
            .life_gained,
        0
    );
    assert!(
        opponent_gained
            .state
            .turn_history
            .current
            .player(1)
            .life_gained
            > 0
    );
    cast_and_resolve(
        &mut opponent_gained,
        0,
        "efflorescence",
        target_object(bear),
    );
    assert_eq!(
        opponent_gained.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert!(!opponent_gained.effective_has_keyword(bear, Keyword::Trample));
    assert!(!opponent_gained.effective_has_keyword(bear, Keyword::Indestructible));

    let mut gained = engine(838_011);
    let bear = inject_creature_on_battlefield(&mut gained, 0, "grizzly_bears");
    cast_and_resolve(&mut gained, 0, "life_goes_on", vec![]);
    assert!(gained.state.turn_history.current.player(0).life_gained > 0);
    cast_and_resolve(&mut gained, 0, "efflorescence", target_object(bear));
    assert_eq!(
        gained.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert!(gained.effective_has_keyword(bear, Keyword::Trample));
    assert!(gained.effective_has_keyword(bear, Keyword::Indestructible));

    let mut in_response = engine(838_012);
    let bear = inject_creature_on_battlefield(&mut in_response, 0, "grizzly_bears");
    inject_card_into_hand(&mut in_response, 0, "efflorescence");
    let slot = hand_index_for_card(&in_response, 0, "efflorescence");
    semantic::accepted(&mut in_response, 0, &cast_spell(slot, target_object(bear)));
    inject_card_into_hand(&mut in_response, 0, "life_goes_on");
    let slot = hand_index_for_card(&in_response, 0, "life_goes_on");
    semantic::accepted(&mut in_response, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut in_response);
    assert!(
        in_response.effective_has_keyword(bear, Keyword::Trample),
        "life gain after casting still qualifies at resolution"
    );
    assert!(in_response.effective_has_keyword(bear, Keyword::Indestructible));
}

#[test]
fn issue_misc38_might_of_the_meek() {
    for (seed, mouse_controller, expected_power) in [
        (838_020, None, 2),
        (838_021, Some(0), 3),
        (838_022, Some(1), 2),
    ] {
        let mut e = engine(seed);
        if let Some(player) = mouse_controller {
            inject_creature_on_battlefield(&mut e, player, "armory_mice");
        }
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let drawn_before = e.state.turn_history.current.player(0).cards_drawn;
        cast_and_resolve(&mut e, 0, "might_of_the_meek", target_object(target));
        assert_eq!(e.effective_power(target), Some(expected_power));
        assert!(e.effective_has_keyword(target, Keyword::Trample));
        assert_eq!(
            e.state.turn_history.current.player(0).cards_drawn,
            drawn_before + 1
        );
    }
    let mut late_mouse = engine(838_024);
    let target = inject_creature_on_battlefield(&mut late_mouse, 1, "grizzly_bears");
    inject_card_into_hand(&mut late_mouse, 0, "might_of_the_meek");
    let slot = hand_index_for_card(&late_mouse, 0, "might_of_the_meek");
    semantic::accepted(&mut late_mouse, 0, &cast_spell(slot, target_object(target)));
    inject_creature_on_battlefield(&mut late_mouse, 0, "armory_mice");
    resolve_entire_stack_two_player(&mut late_mouse);
    assert_eq!(
        late_mouse.effective_power(target),
        Some(3),
        "Mouse is checked at resolution"
    );
    let mut fizzled = engine(838_023);
    let target = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    inject_card_into_hand(&mut fizzled, 0, "might_of_the_meek");
    let before = fizzled.state.turn_history.current.player(0).cards_drawn;
    let slot = hand_index_for_card(&fizzled, 0, "might_of_the_meek");
    semantic::accepted(&mut fizzled, 0, &cast_spell(slot, target_object(target)));
    remove_target_before_resolution(&mut fizzled, target);
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(
        fizzled.state.turn_history.current.player(0).cards_drawn,
        before
    );
}

#[test]
fn issue_misc38_pedal_to_the_metal() {
    for (seed, x) in [(838_030, 0), (838_031, 3)] {
        let mut e = engine(seed);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, "pedal_to_the_metal");
        let slot = hand_index_for_card(&e, 0, "pedal_to_the_metal");
        semantic::accepted(&mut e, 0, &cast_spell_x(slot, target_object(target), x));
        assert_eq!(e.state.stack.last().unwrap().chosen_x, x);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(target), Some(2 + x));
        assert_eq!(e.effective_toughness(target), Some(2));
        assert!(e.effective_has_keyword(target, Keyword::FirstStrike));
    }
}

#[test]
fn issue_misc38_take_the_fall() {
    for (seed, rogue_controller, expected_power) in [
        (838_040, None, 5),
        (838_041, Some(0), 2),
        (838_042, Some(1), 5),
    ] {
        let mut e = engine(seed);
        if let Some(player) = rogue_controller {
            inject_creature_on_battlefield(&mut e, player, "audacious_thief");
        }
        let target = inject_creature_with_stats(&mut e, 1, "craw_wurm", 6, 4);
        let before = e.state.turn_history.current.player(0).cards_drawn;
        cast_and_resolve(&mut e, 0, "take_the_fall", target_object(target));
        assert_eq!(e.effective_power(target), Some(expected_power));
        assert_eq!(e.effective_toughness(target), Some(4));
        assert_eq!(
            e.state.turn_history.current.player(0).cards_drawn,
            before + 1
        );
    }
    for (seed, outlaw) in [
        (838_044, "hired_blade"),
        (838_045, "prickly_pair"),
        (838_046, "corsair_captain"),
        (838_047, "audacious_thief"),
        (838_048, "flamecache_gecko"),
    ] {
        let mut e = engine(seed);
        inject_creature_on_battlefield(&mut e, 0, outlaw);
        let target = inject_creature_with_stats(&mut e, 1, "craw_wurm", 6, 4);
        cast_and_resolve(&mut e, 0, "take_the_fall", target_object(target));
        assert_eq!(
            e.effective_power(target),
            Some(2),
            "{outlaw} must count as an outlaw"
        );
    }
    let mut late_outlaw = engine(838_049);
    let target = inject_creature_with_stats(&mut late_outlaw, 1, "craw_wurm", 6, 4);
    inject_card_into_hand(&mut late_outlaw, 0, "take_the_fall");
    let slot = hand_index_for_card(&late_outlaw, 0, "take_the_fall");
    semantic::accepted(
        &mut late_outlaw,
        0,
        &cast_spell(slot, target_object(target)),
    );
    inject_creature_on_battlefield(&mut late_outlaw, 0, "audacious_thief");
    resolve_entire_stack_two_player(&mut late_outlaw);
    assert_eq!(
        late_outlaw.effective_power(target),
        Some(2),
        "outlaw is checked at resolution"
    );
    let mut fizzled = engine(838_043);
    let target = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    inject_card_into_hand(&mut fizzled, 0, "take_the_fall");
    let before = fizzled.state.turn_history.current.player(0).cards_drawn;
    let slot = hand_index_for_card(&fizzled, 0, "take_the_fall");
    semantic::accepted(&mut fizzled, 0, &cast_spell(slot, target_object(target)));
    remove_target_before_resolution(&mut fizzled, target);
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(
        fizzled.state.turn_history.current.player(0).cards_drawn,
        before
    );
}
