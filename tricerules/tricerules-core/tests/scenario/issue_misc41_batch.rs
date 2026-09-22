//! Actual-card modal semantics for six pinned Standard spells.
//! Exact Oracle and rulings reviewed 2026-09-22; see direct-RON maps.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn cast_only(e: &mut GameEngine, card: &str, mode: u32, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_modal_spell(slot, vec![(mode, targets)]));
}

fn cast_and_resolve(e: &mut GameEngine, card: &str, mode: u32, targets: Vec<TargetRef>) {
    cast_only(e, card, mode, targets);
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_misc41_slagstorm() {
    let mut creatures = engine(841_001);
    let own = inject_creature_on_battlefield(&mut creatures, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut creatures, 1, "grizzly_bears");
    cast_and_resolve(&mut creatures, "slagstorm", 0, vec![]);
    assert_eq!(creatures.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(creatures.state.objects[&opposing].zone, Zone::Graveyard);
    assert_eq!(creatures.state.players[0].life, 20);
    assert_eq!(creatures.state.players[1].life, 20);

    let mut players = engine(841_002);
    let own = inject_creature_on_battlefield(&mut players, 0, "grizzly_bears");
    cast_and_resolve(&mut players, "slagstorm", 1, vec![]);
    assert_eq!(players.state.players[0].life, 17);
    assert_eq!(players.state.players[1].life, 17);
    assert_eq!(players.state.objects[&own].zone, Zone::Battlefield);
}

#[test]
fn issue_misc41_split_up() {
    for (seed, mode, destroy_tapped) in [(841_010, 0, true), (841_011, 1, false)] {
        let mut e = engine(seed);
        let tapped = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let untapped = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&tapped).unwrap().tapped = true;
        cast_and_resolve(&mut e, "split_up", mode, vec![]);
        assert_eq!(
            e.state.objects[&tapped].zone == Zone::Graveyard,
            destroy_tapped
        );
        assert_eq!(
            e.state.objects[&untapped].zone == Zone::Graveyard,
            !destroy_tapped
        );
    }
}

#[test]
fn issue_misc41_thorins_last_stand() {
    let mut pump = engine(841_020);
    let own = inject_creature_on_battlefield(&mut pump, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut pump, 1, "grizzly_bears");
    cast_and_resolve(&mut pump, "thorins_last_stand", 0, vec![]);
    assert_eq!(pump.effective_power(own), Some(4));
    assert_eq!(pump.effective_toughness(own), Some(3));
    assert_eq!(pump.effective_power(opposing), Some(2));
    let late = inject_creature_on_battlefield(&mut pump, 0, "grizzly_bears");
    assert_eq!(
        pump.effective_power(late),
        Some(2),
        "one-shot set is fixed at resolution"
    );

    let mut destroy = engine(841_021);
    let artifact = inject_permanent_on_battlefield(&mut destroy, 1, "howling_mine");
    cast_and_resolve(
        &mut destroy,
        "thorins_last_stand",
        1,
        target_object(artifact),
    );
    assert_eq!(destroy.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(destroy.state.players[0].life, 22);

    let mut enchantment = engine(841_022);
    let aura = inject_permanent_on_battlefield(&mut enchantment, 1, "impact_tremors");
    cast_and_resolve(
        &mut enchantment,
        "thorins_last_stand",
        1,
        target_object(aura),
    );
    assert_eq!(enchantment.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(enchantment.state.players[0].life, 22);

    let mut fizzled = engine(841_023);
    let artifact = inject_permanent_on_battlefield(&mut fizzled, 1, "howling_mine");
    cast_only(
        &mut fizzled,
        "thorins_last_stand",
        1,
        target_object(artifact),
    );
    fizzled.state.players[1]
        .battlefield
        .retain(|id| *id != artifact);
    fizzled.state.players[1].graveyard.push(artifact);
    fizzled.state.objects.get_mut(&artifact).unwrap().zone = Zone::Graveyard;
    *fizzled
        .state
        .zone_change_generation
        .entry(artifact)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(fizzled.state.players[0].life, 20);
}

#[test]
fn issue_misc41_battle_menu() {
    let mut token = engine(841_030);
    cast_and_resolve(&mut token, "battle_menu", 0, vec![]);
    let knights = battlefield_token_oids(&token, 0, "knight_w_2_2");
    assert_eq!(knights.len(), 1);
    assert_eq!(token.effective_power(knights[0]), Some(2));
    assert_eq!(token.effective_toughness(knights[0]), Some(2));

    let mut pump = engine(841_031);
    let creature = inject_creature_on_battlefield(&mut pump, 1, "grizzly_bears");
    cast_and_resolve(&mut pump, "battle_menu", 1, target_object(creature));
    assert_eq!(pump.effective_power(creature), Some(2));
    assert_eq!(pump.effective_toughness(creature), Some(6));
    assert!(battlefield_token_oids(&pump, 0, "knight_w_2_2").is_empty());

    let mut destroy = engine(841_032);
    let strong = inject_creature_with_stats(&mut destroy, 1, "grizzly_bears", 4, 4);
    let weak = inject_creature_on_battlefield(&mut destroy, 0, "grizzly_bears");
    inject_card_into_hand(&mut destroy, 0, "battle_menu");
    let slot = hand_index_for_card(&destroy, 0, "battle_menu");
    assert!(destroy
        .apply_command(0, &cast_modal_spell(slot, vec![(2, target_object(weak))]))
        .is_err());
    semantic::accepted(
        &mut destroy,
        0,
        &cast_modal_spell(slot, vec![(2, target_object(strong))]),
    );
    resolve_entire_stack_two_player(&mut destroy);
    assert_eq!(destroy.state.objects[&strong].zone, Zone::Graveyard);
    assert_eq!(destroy.state.objects[&weak].zone, Zone::Battlefield);

    let mut gain = engine(841_033);
    cast_and_resolve(&mut gain, "battle_menu", 3, vec![]);
    assert_eq!(gain.state.players[0].life, 24);
    assert!(battlefield_token_oids(&gain, 0, "knight_w_2_2").is_empty());
}

#[test]
fn issue_misc41_silverquill_charm() {
    let mut counters = engine(841_040);
    let creature = inject_creature_on_battlefield(&mut counters, 1, "grizzly_bears");
    cast_and_resolve(
        &mut counters,
        "silverquill_charm",
        0,
        target_object(creature),
    );
    assert_eq!(
        counters.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        2
    );

    let mut exile = engine(841_041);
    let weak = inject_creature_on_battlefield(&mut exile, 1, "grizzly_bears");
    let strong = inject_creature_with_stats(&mut exile, 1, "grizzly_bears", 3, 3);
    inject_card_into_hand(&mut exile, 0, "silverquill_charm");
    let slot = hand_index_for_card(&exile, 0, "silverquill_charm");
    assert!(exile
        .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(strong))]))
        .is_err());
    semantic::accepted(
        &mut exile,
        0,
        &cast_modal_spell(slot, vec![(1, target_object(weak))]),
    );
    resolve_entire_stack_two_player(&mut exile);
    assert_eq!(exile.state.objects[&weak].zone, Zone::Exile);
    assert_eq!(exile.state.objects[&strong].zone, Zone::Battlefield);

    let mut life = engine(841_042);
    cast_and_resolve(&mut life, "silverquill_charm", 2, vec![]);
    assert_eq!(life.state.players[0].life, 23);
    assert_eq!(life.state.players[1].life, 17);
}

#[test]
fn issue_misc41_glorious_decay() {
    let mut artifact = engine(841_050);
    let target = inject_permanent_on_battlefield(&mut artifact, 1, "howling_mine");
    cast_and_resolve(&mut artifact, "glorious_decay", 0, target_object(target));
    assert_eq!(artifact.state.objects[&target].zone, Zone::Graveyard);

    let mut flying = engine(841_051);
    let angel = inject_creature_on_battlefield(&mut flying, 1, "serra_angel");
    let bear = inject_creature_on_battlefield(&mut flying, 0, "grizzly_bears");
    inject_card_into_hand(&mut flying, 0, "glorious_decay");
    let slot = hand_index_for_card(&flying, 0, "glorious_decay");
    assert!(flying
        .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(bear))]))
        .is_err());
    semantic::accepted(
        &mut flying,
        0,
        &cast_modal_spell(slot, vec![(1, target_object(angel))]),
    );
    resolve_entire_stack_two_player(&mut flying);
    assert_eq!(flying.state.objects[&angel].zone, Zone::Graveyard);
    assert_eq!(flying.state.objects[&bear].zone, Zone::Battlefield);

    let mut grave = engine(841_052);
    let card = inject_graveyard_card(&mut grave, 1, "grizzly_bears");
    let before = grave.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut grave, "glorious_decay", 2, target_object(card));
    assert_eq!(grave.state.objects[&card].zone, Zone::Exile);
    assert_eq!(
        grave.state.turn_history.current.player(0).cards_drawn,
        before + 1
    );

    let mut fizzled = engine(841_053);
    let card = inject_graveyard_card(&mut fizzled, 1, "grizzly_bears");
    let before = fizzled.state.turn_history.current.player(0).cards_drawn;
    cast_only(&mut fizzled, "glorious_decay", 2, target_object(card));
    fizzled.state.players[1].graveyard.retain(|id| *id != card);
    fizzled.state.players[1].exile.push(card);
    fizzled.state.objects.get_mut(&card).unwrap().zone = Zone::Exile;
    *fizzled
        .state
        .zone_change_generation
        .entry(card)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(
        fizzled.state.turn_history.current.player(0).cards_drawn,
        before
    );
}
