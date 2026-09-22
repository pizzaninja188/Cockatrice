//! Actual-card semantics for six pinned Standard combat spells.
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

fn cast_only(e: &mut GameEngine, player: usize, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, player, card);
    grant_pool(e, player);
    let slot = hand_index_for_card(e, player, card);
    semantic::accepted(e, player as i32, &cast_spell(slot, targets));
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
fn issue_misc39_impolite_entrance() {
    let mut e = engine(839_001);
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let before = e.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut e, 0, "impolite_entrance", target_object(opposing));
    assert!(e.effective_has_keyword(opposing, Keyword::Trample));
    assert!(e.effective_has_keyword(opposing, Keyword::Haste));
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        before + 1
    );

    let mut fizzled = engine(839_002);
    let target = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    let before = fizzled.state.turn_history.current.player(0).cards_drawn;
    cast_only(&mut fizzled, 0, "impolite_entrance", target_object(target));
    remove_target_before_resolution(&mut fizzled, target);
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(
        fizzled.state.turn_history.current.player(0).cards_drawn,
        before
    );
}

#[test]
fn issue_misc39_youre_not_alone() {
    for (seed, own_count, expected) in [(839_010, 2, 4), (839_011, 3, 6)] {
        let mut e = engine(seed);
        for _ in 0..own_count {
            inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        }
        let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_and_resolve(&mut e, 0, "youre_not_alone", target_object(opposing));
        assert_eq!(e.effective_power(opposing), Some(expected));
        assert_eq!(e.effective_toughness(opposing), Some(expected));
    }

    let mut late_third = engine(839_012);
    for _ in 0..2 {
        inject_creature_on_battlefield(&mut late_third, 0, "grizzly_bears");
    }
    let target = inject_creature_on_battlefield(&mut late_third, 1, "grizzly_bears");
    cast_only(&mut late_third, 0, "youre_not_alone", target_object(target));
    inject_creature_on_battlefield(&mut late_third, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut late_third);
    assert_eq!(late_third.effective_power(target), Some(6));
}

#[test]
fn issue_misc39_get_a_leg_up() {
    let mut own_target = engine(839_020);
    let target = inject_creature_on_battlefield(&mut own_target, 0, "grizzly_bears");
    cast_and_resolve(&mut own_target, 0, "get_a_leg_up", target_object(target));
    assert_eq!(
        own_target.effective_power(target),
        Some(3),
        "count own target"
    );
    assert_eq!(own_target.effective_toughness(target), Some(3));
    assert!(own_target.effective_has_keyword(target, Keyword::Reach));

    let mut late_count = engine(839_021);
    inject_creature_on_battlefield(&mut late_count, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut late_count, 1, "grizzly_bears");
    cast_only(&mut late_count, 0, "get_a_leg_up", target_object(opposing));
    inject_creature_on_battlefield(&mut late_count, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut late_count);
    assert_eq!(late_count.effective_power(opposing), Some(4));
    assert_eq!(late_count.effective_toughness(opposing), Some(4));
    assert!(late_count.effective_has_keyword(opposing, Keyword::Reach));
    inject_creature_on_battlefield(&mut late_count, 0, "grizzly_bears");
    assert_eq!(late_count.effective_power(opposing), Some(4), "fixed bonus");
}

#[test]
fn issue_misc39_thoughtweft_charge() {
    let mut none = engine(839_030);
    let target = inject_creature_on_battlefield(&mut none, 1, "grizzly_bears");
    let before = none.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut none, 0, "thoughtweft_charge", target_object(target));
    assert_eq!(none.effective_power(target), Some(5));
    assert_eq!(
        none.state.turn_history.current.player(0).cards_drawn,
        before
    );

    let mut opponent_only = engine(839_031);
    let target = inject_creature_on_battlefield(&mut opponent_only, 1, "grizzly_bears");
    semantic::accepted(&mut opponent_only, 0, &pass());
    cast_and_resolve(&mut opponent_only, 1, "sprout", vec![]);
    assert!(opponent_only
        .state
        .turn_history
        .current
        .permanents_entered
        .iter()
        .any(|fact| fact.controller == 1));
    let before = opponent_only
        .state
        .turn_history
        .current
        .player(0)
        .cards_drawn;
    cast_and_resolve(
        &mut opponent_only,
        0,
        "thoughtweft_charge",
        target_object(target),
    );
    assert_eq!(
        opponent_only
            .state
            .turn_history
            .current
            .player(0)
            .cards_drawn,
        before
    );

    let mut in_response = engine(839_032);
    let target = inject_creature_on_battlefield(&mut in_response, 1, "grizzly_bears");
    let before = in_response.state.turn_history.current.player(0).cards_drawn;
    cast_only(
        &mut in_response,
        0,
        "thoughtweft_charge",
        target_object(target),
    );
    cast_only(&mut in_response, 0, "sprout", vec![]);
    resolve_entire_stack_two_player(&mut in_response);
    assert_eq!(in_response.effective_power(target), Some(5));
    assert_eq!(
        in_response.state.turn_history.current.player(0).cards_drawn,
        before + 1
    );

    let mut fizzled = engine(839_033);
    let target = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    cast_and_resolve(&mut fizzled, 0, "sprout", vec![]);
    let before = fizzled.state.turn_history.current.player(0).cards_drawn;
    cast_only(&mut fizzled, 0, "thoughtweft_charge", target_object(target));
    remove_target_before_resolution(&mut fizzled, target);
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(
        fizzled.state.turn_history.current.player(0).cards_drawn,
        before
    );
}

#[test]
fn issue_misc39_dual_sun_technique() {
    let mut plain = engine(839_040);
    let target = inject_creature_on_battlefield(&mut plain, 0, "grizzly_bears");
    let before = plain.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut plain, 0, "dual-sun_technique", target_object(target));
    assert!(plain.effective_has_keyword(target, Keyword::DoubleStrike));
    assert_eq!(
        plain.state.turn_history.current.player(0).cards_drawn,
        before
    );

    let mut in_response = engine(839_041);
    let target = inject_creature_on_battlefield(&mut in_response, 0, "grizzly_bears");
    let before = in_response.state.turn_history.current.player(0).cards_drawn;
    cast_only(
        &mut in_response,
        0,
        "dual-sun_technique",
        target_object(target),
    );
    cast_only(&mut in_response, 0, "efflorescence", target_object(target));
    resolve_entire_stack_two_player(&mut in_response);
    assert_eq!(
        in_response.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert!(in_response.effective_has_keyword(target, Keyword::DoubleStrike));
    assert_eq!(
        in_response.state.turn_history.current.player(0).cards_drawn,
        before + 1
    );

    let mut removed = engine(839_042);
    let target = inject_creature_on_battlefield(&mut removed, 0, "grizzly_bears");
    removed
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    cast_only(&mut removed, 0, "dual-sun_technique", target_object(target));
    removed
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 0);
    let before = removed.state.turn_history.current.player(0).cards_drawn;
    resolve_entire_stack_two_player(&mut removed);
    assert_eq!(
        removed.state.turn_history.current.player(0).cards_drawn,
        before
    );

    let mut opponent = engine(839_043);
    let target = inject_creature_on_battlefield(&mut opponent, 1, "grizzly_bears");
    inject_card_into_hand(&mut opponent, 0, "dual-sun_technique");
    let slot = hand_index_for_card(&opponent, 0, "dual-sun_technique");
    assert!(opponent
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .is_err());
}

#[test]
fn issue_misc39_thwip() {
    let mut spider = engine(839_050);
    let target = inject_creature_with_stats(&mut spider, 1, "giant_spider", 2, 4);
    let before = spider.state.players[0].life;
    cast_and_resolve(&mut spider, 0, "thwip!", target_object(target));
    assert_eq!(spider.effective_power(target), Some(4));
    assert_eq!(spider.effective_toughness(target), Some(6));
    assert!(spider.effective_has_keyword(target, Keyword::Flying));
    assert_eq!(spider.state.players[0].life, before + 2);

    let mut changeling = engine(839_053);
    let target = inject_creature_with_stats(&mut changeling, 1, "changeling_wayfinder", 1, 2);
    assert!(changeling
        .characteristics(target)
        .unwrap()
        .has_type("Spider"));
    let before = changeling.state.players[0].life;
    cast_and_resolve(&mut changeling, 0, "thwip!", target_object(target));
    assert_eq!(changeling.state.players[0].life, before + 2);

    let mut non_spider = engine(839_051);
    let target = inject_creature_on_battlefield(&mut non_spider, 0, "grizzly_bears");
    let before = non_spider.state.players[0].life;
    cast_and_resolve(&mut non_spider, 0, "thwip!", target_object(target));
    assert_eq!(non_spider.effective_power(target), Some(4));
    assert!(non_spider.effective_has_keyword(target, Keyword::Flying));
    assert_eq!(non_spider.state.players[0].life, before);

    let mut fizzled = engine(839_052);
    let target = inject_creature_on_battlefield(&mut fizzled, 1, "giant_spider");
    let before = fizzled.state.players[0].life;
    cast_only(&mut fizzled, 0, "thwip!", target_object(target));
    remove_target_before_resolution(&mut fizzled, target);
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(fizzled.state.players[0].life, before);
}
