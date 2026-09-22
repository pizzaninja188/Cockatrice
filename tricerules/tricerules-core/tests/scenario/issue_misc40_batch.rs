//! Actual-card semantics for six pinned Standard targeted and modal spells.
//! Exact pinned Oracle records and published rulings reviewed 2026-09-22.

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

fn group_one(oid: u32) -> TargetRef {
    let mut target = target_object(oid).remove(0);
    target.group_index = 1;
    target
}

fn cast_only(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
}

fn cast_and_resolve(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    cast_only(e, card, targets);
    resolve_entire_stack_two_player(e);
}

fn remove_target(e: &mut GameEngine, oid: u32) {
    let controller = e.state.objects[&oid].controller as usize;
    let owner = e.state.objects[&oid].owner as usize;
    e.state.players[controller]
        .battlefield
        .retain(|id| *id != oid);
    e.state.players[owner].graveyard.push(oid);
    e.state.objects.get_mut(&oid).unwrap().zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(oid).or_default() += 1;
}

#[test]
fn issue_misc40_mabels_mettle() {
    let mut e = engine(840_001);
    let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_resolve(
        &mut e,
        "mabels_mettle",
        vec![target_object(first)[0], group_one(second)],
    );
    assert_eq!(e.effective_power(first), Some(4));
    assert_eq!(e.effective_toughness(first), Some(4));
    assert_eq!(e.effective_power(second), Some(3));

    let mut optional = engine(840_002);
    let only = inject_creature_on_battlefield(&mut optional, 0, "grizzly_bears");
    cast_and_resolve(&mut optional, "mabels_mettle", target_object(only));
    assert_eq!(optional.effective_power(only), Some(4));

    let mut invalid = engine(840_003);
    let first = inject_creature_on_battlefield(&mut invalid, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut invalid, 1, "grizzly_bears");
    inject_card_into_hand(&mut invalid, 0, "mabels_mettle");
    let slot = hand_index_for_card(&invalid, 0, "mabels_mettle");
    assert!(invalid
        .apply_command(
            0,
            &cast_spell(slot, vec![target_object(first)[0], group_one(first)])
        )
        .is_err());
    semantic::accepted(
        &mut invalid,
        0,
        &cast_spell(slot, vec![target_object(first)[0], group_one(second)]),
    );
    remove_target(&mut invalid, first);
    resolve_entire_stack_two_player(&mut invalid);
    assert_eq!(invalid.effective_power(second), Some(3));
}

#[test]
fn issue_misc40_skulduggery() {
    let mut e = engine(840_010);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let foe = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "skulduggery");
    let slot = hand_index_for_card(&e, 0, "skulduggery");
    assert!(e
        .apply_command(0, &cast_spell(slot, target_object(own)))
        .is_err());
    assert!(e
        .apply_command(
            0,
            &cast_spell(slot, vec![target_object(own)[0], group_one(own)])
        )
        .is_err());
    semantic::accepted(
        &mut e,
        0,
        &cast_spell(slot, vec![target_object(own)[0], group_one(foe)]),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(own), Some(3));
    assert_eq!(e.effective_power(foe), Some(1));
    assert_eq!(e.effective_toughness(foe), Some(1));

    let mut partial = engine(840_011);
    let own = inject_creature_on_battlefield(&mut partial, 0, "grizzly_bears");
    let foe = inject_creature_on_battlefield(&mut partial, 1, "grizzly_bears");
    cast_only(
        &mut partial,
        "skulduggery",
        vec![target_object(own)[0], group_one(foe)],
    );
    remove_target(&mut partial, own);
    resolve_entire_stack_two_player(&mut partial);
    assert_eq!(partial.effective_power(foe), Some(1));
}

#[test]
fn issue_misc40_combat_tutorial() {
    let mut e = engine(840_030);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let foe = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "combat_tutorial");
    let slot = hand_index_for_card(&e, 0, "combat_tutorial");
    assert!(e
        .apply_command(
            0,
            &cast_spell(slot, vec![target_player(1)[0], group_one(foe)])
        )
        .is_err());
    let before = e.state.turn_history.current.player(1).cards_drawn;
    semantic::accepted(
        &mut e,
        0,
        &cast_spell(slot, vec![target_player(1)[0], group_one(own)]),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.turn_history.current.player(1).cards_drawn,
        before + 2
    );
    assert_eq!(
        e.state.objects[&own].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        e.state.objects[&foe].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    let mut optional = engine(840_031);
    let before = optional.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut optional, "combat_tutorial", target_player(0));
    assert_eq!(
        optional.state.turn_history.current.player(0).cards_drawn,
        before + 2
    );
}

#[test]
fn issue_misc40_cost_of_brilliance() {
    let mut e = engine(840_040);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let before = e.state.turn_history.current.player(1).cards_drawn;
    cast_and_resolve(
        &mut e,
        "cost_of_brilliance",
        vec![target_player(1)[0], group_one(creature)],
    );
    assert_eq!(
        e.state.turn_history.current.player(1).cards_drawn,
        before + 2
    );
    assert_eq!(e.state.players[1].life, 18);
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(
        e.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        1
    );

    let mut optional = engine(840_041);
    let before = optional.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut optional, "cost_of_brilliance", target_player(0));
    assert_eq!(
        optional.state.turn_history.current.player(0).cards_drawn,
        before + 2
    );
    assert_eq!(optional.state.players[0].life, 18);
}

#[test]
fn issue_misc40_mouser_attack() {
    let mut token = engine(840_050);
    inject_card_into_hand(&mut token, 0, "mouser_attack!");
    let slot = hand_index_for_card(&token, 0, "mouser_attack!");
    semantic::accepted(&mut token, 0, &cast_modal_spell(slot, vec![(0, vec![])]));
    resolve_entire_stack_two_player(&mut token);
    assert_eq!(battlefield_token_oids(&token, 0, "robot_c_1_1").len(), 1);

    let mut pump = engine(840_051);
    let creature = inject_creature_on_battlefield(&mut pump, 1, "grizzly_bears");
    inject_card_into_hand(&mut pump, 0, "mouser_attack!");
    let slot = hand_index_for_card(&pump, 0, "mouser_attack!");
    assert!(pump
        .apply_command(0, &cast_modal_spell(slot, vec![(1, vec![])]))
        .is_err());
    semantic::accepted(
        &mut pump,
        0,
        &cast_modal_spell(slot, vec![(1, target_object(creature))]),
    );
    resolve_entire_stack_two_player(&mut pump);
    assert_eq!(pump.effective_power(creature), Some(5));
    assert_eq!(pump.effective_toughness(creature), Some(2));
    assert!(pump.effective_has_keyword(creature, Keyword::FirstStrike));
    assert!(battlefield_token_oids(&pump, 0, "robot_c_1_1").is_empty());
}

#[test]
fn issue_misc40_rat_out() {
    let mut no_target = engine(840_060);
    cast_and_resolve(&mut no_target, "rat_out", vec![]);
    assert_eq!(
        battlefield_token_oids(&no_target, 0, "rat_b_1_1_cant_block").len(),
        1
    );

    let mut targeted = engine(840_061);
    let creature = inject_creature_on_battlefield(&mut targeted, 1, "grizzly_bears");
    cast_and_resolve(&mut targeted, "rat_out", target_object(creature));
    assert_eq!(targeted.effective_power(creature), Some(1));
    assert_eq!(targeted.effective_toughness(creature), Some(1));
    assert_eq!(
        battlefield_token_oids(&targeted, 0, "rat_b_1_1_cant_block").len(),
        1
    );

    let mut fizzled = engine(840_062);
    let creature = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    cast_only(&mut fizzled, "rat_out", target_object(creature));
    remove_target(&mut fizzled, creature);
    resolve_entire_stack_two_player(&mut fizzled);
    assert!(battlefield_token_oids(&fizzled, 0, "rat_b_1_1_cant_block").is_empty());
}
