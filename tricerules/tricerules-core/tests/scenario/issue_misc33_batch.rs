//! Actual-card semantics for seven pinned Standard damage spells.
//! Pinned Oracle and current Scryfall rulings reviewed 2026-09-22.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn cast(e: &mut GameEngine, card: &str, target: u32) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, target_object(target)));
}

fn cast_and_resolve(e: &mut GameEngine, card: &str, target: u32) {
    cast(e, card, target);
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_misc33_feed_the_flames_semantics() {
    let mut e = engine(833_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
    cast_and_resolve(&mut e, "feed_the_flames", target);
    assert_eq!(e.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&target].damage, 5);
    e.state.objects.get_mut(&target).unwrap().damage = 6;
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Exile);
    assert_eq!(e.state.turn_history.current.creatures_died, 0);
}

#[test]
fn issue_misc33_elspeths_smite_semantics() {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(833_010, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    grant_pool(&mut e, 0);
    let attacker = battlefield_object_for_card(&e, 0, "grizzly_bears");
    let idle = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "elspeths_smite");
    let slot = hand_index_for_card(&e, 0, "elspeths_smite");
    assert!(e
        .apply_command(0, &cast_spell(slot, target_object(idle)))
        .is_err());
    semantic::accepted(&mut e, 0, &declare_attackers(vec![attacker]));
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "elspeths_smite");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(attacker)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&attacker].zone, Zone::Exile);
    assert_eq!(e.state.objects[&idle].zone, Zone::Battlefield);
}

#[test]
fn issue_misc33_obliterating_bolt_semantics() {
    let mut e = engine(833_020);
    let walker = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
    e.state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    cast_and_resolve(&mut e, "obliterating_bolt", walker);
    assert_eq!(e.state.objects[&walker].zone, Zone::Exile);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    inject_card_into_hand(&mut e, 0, "obliterating_bolt");
    let slot = hand_index_for_card(&e, 0, "obliterating_bolt");
    assert!(e
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .is_err());
}

#[test]
fn issue_misc33_narsets_rebuke_semantics() {
    let mut e = engine(833_030);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
    e.state.players[0].mana_pool.clear();
    e.state.players[0].mana_pool.red = 1;
    e.state.players[0].mana_pool.colorless = 4;
    cast(&mut e, "narsets_rebuke", target);
    assert_eq!(e.state.players[0].mana_pool.red, 0);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].damage, 5);
    let pool = &e.state.players[0].mana_pool;
    assert_eq!((pool.blue, pool.red, pool.white), (1, 1, 1));
    e.state.objects.get_mut(&target).unwrap().damage = 6;
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Exile);

    let mut fizzled = engine(833_031);
    let target = inject_creature_with_stats(&mut fizzled, 1, "grizzly_bears", 6, 6);
    fizzled.state.players[0].mana_pool.clear();
    fizzled.state.players[0].mana_pool.red = 1;
    fizzled.state.players[0].mana_pool.colorless = 4;
    cast(&mut fizzled, "narsets_rebuke", target);
    fizzled.state.players[1]
        .battlefield
        .retain(|oid| *oid != target);
    fizzled.state.players[1].graveyard.push(target);
    fizzled.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *fizzled
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzled);
    let pool = &fizzled.state.players[0].mana_pool;
    assert_eq!(
        (pool.blue, pool.red, pool.white),
        (0, 0, 0),
        "illegal sole target counters the spell before its mana effect"
    );
}

#[test]
fn issue_misc33_invasive_maneuvers_semantics() {
    let mut ordinary = engine(833_040);
    let target = inject_creature_with_stats(&mut ordinary, 1, "grizzly_bears", 6, 6);
    cast_and_resolve(&mut ordinary, "invasive_maneuvers", target);
    assert_eq!(ordinary.state.objects[&target].damage, 3);

    let mut with_ship = engine(833_041);
    let target = inject_creature_with_stats(&mut with_ship, 1, "grizzly_bears", 6, 6);
    cast(&mut with_ship, "invasive_maneuvers", target);
    let ship = inject_permanent_on_battlefield(&mut with_ship, 0, "uthros_scanship");
    assert_eq!(
        with_ship.effective_power(ship),
        None,
        "Spacecraft need not be a creature"
    );
    resolve_entire_stack_two_player(&mut with_ship);
    assert_eq!(with_ship.state.objects[&target].damage, 5);
}

#[test]
fn issue_misc33_rumbling_rockslide_semantics() {
    let mut e = engine(833_050);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
    cast(&mut e, "rumbling_rockslide", target);
    for _ in 0..3 {
        inject_permanent_on_battlefield(&mut e, 0, "forest");
    }
    inject_permanent_on_battlefield(&mut e, 1, "forest");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].damage, 3);
}

#[test]
fn issue_misc33_galvanize_semantics() {
    let mut ordinary = engine(833_060);
    let target = inject_creature_with_stats(&mut ordinary, 1, "grizzly_bears", 6, 6);
    cast_and_resolve(&mut ordinary, "galvanize", target);
    assert_eq!(ordinary.state.objects[&target].damage, 3);

    let mut after_draws = engine(833_061);
    let target = inject_creature_with_stats(&mut after_draws, 1, "grizzly_bears", 6, 6);
    cast(&mut after_draws, "galvanize", target);
    inject_card_into_hand(&mut after_draws, 0, "quick_study");
    let slot = hand_index_for_card(&after_draws, 0, "quick_study");
    semantic::accepted(&mut after_draws, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut after_draws);
    assert_eq!(
        after_draws.state.turn_history.current.player(0).cards_drawn,
        2
    );
    assert_eq!(after_draws.state.objects[&target].damage, 5);
}
