//! Actual-card semantics for five pinned Standard spells.
//! Oracle and rulings checked 2026-09-22; CR 601.2f, 608.2b-c, 702.34.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{CastMethod, CastSpell, ChoiceKind};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn cast_regular(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
}

fn reduction(e: &mut GameEngine, card: &str) -> u32 {
    let slot = hand_index_for_card(e, 0, card) as u32;
    e.initial_response_batch().legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == slot)
        .expect("cast action")
        .generic_cost_reduction
}

#[test]
fn issue_misc44_mental_modulation() {
    let mut e = engine(844_001);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "howling_mine");
    inject_card_into_hand(&mut e, 0, "mental_modulation");
    assert_eq!(reduction(&mut e, "mental_modulation"), 1);
    let before = e.state.turn_history.current.player(0).cards_drawn;
    let slot = hand_index_for_card(&e, 0, "mental_modulation");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(artifact)));
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&artifact].tapped);
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        before + 1
    );

    let mut creature = engine(844_002);
    let bear = inject_creature_on_battlefield(&mut creature, 1, "grizzly_bears");
    cast_regular(&mut creature, "mental_modulation", target_object(bear));
    resolve_entire_stack_two_player(&mut creature);
    assert!(creature.state.objects[&bear].tapped);

    let mut fizzled = engine(844_003);
    let target = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    let before = fizzled.state.turn_history.current.player(0).cards_drawn;
    cast_regular(&mut fizzled, "mental_modulation", target_object(target));
    cast_regular(&mut fizzled, "unsummon", target_object(target));
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(fizzled.state.objects[&target].zone, Zone::Hand);
    assert_eq!(
        fizzled.state.turn_history.current.player(0).cards_drawn,
        before
    );
}

#[test]
fn issue_misc44_incinerating_blast() {
    let mut e = engine(844_010);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let discard = inject_card_into_hand(&mut e, 0, "forest");
    let before = e.state.turn_history.current.player(0).cards_drawn;
    cast_regular(&mut e, "incinerating_blast", target_object(target));
    semantic::accepted(&mut e, 0, &pass());
    let parked = semantic::accepted(&mut e, 1, &pass());
    let choice = find_resolution_choice(&parked).expect("optional discard choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (0, 1));
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![discard]));
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&discard].zone, Zone::Graveyard);
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        before + 1
    );

    let mut fizzled = engine(844_011);
    let target = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    let discard = inject_card_into_hand(&mut fizzled, 0, "forest");
    let before = fizzled.state.turn_history.current.player(0).cards_drawn;
    cast_regular(&mut fizzled, "incinerating_blast", target_object(target));
    cast_regular(&mut fizzled, "unsummon", target_object(target));
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(fizzled.state.objects[&target].zone, Zone::Hand);
    assert_eq!(fizzled.state.objects[&discard].zone, Zone::Hand);
    assert_eq!(
        fizzled.state.turn_history.current.player(0).cards_drawn,
        before
    );
}

#[test]
fn issue_misc44_slick_sequence() {
    let mut e = engine(844_020);
    cast_regular(&mut e, "ornithopter", vec![]);
    resolve_entire_stack_two_player(&mut e);
    let before = e.state.turn_history.current.player(0).cards_drawn;
    cast_regular(&mut e, "slick_sequence", target_player(1));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 18);
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        before + 1
    );

    let mut first = engine(844_021);
    let before = first.state.turn_history.current.player(0).cards_drawn;
    cast_regular(&mut first, "slick_sequence", target_player(1));
    resolve_entire_stack_two_player(&mut first);
    assert_eq!(
        first.state.turn_history.current.player(0).cards_drawn,
        before
    );
}

#[test]
fn issue_misc44_visions_of_villainy() {
    let mut e = engine(844_030);
    inject_card_into_hand(&mut e, 0, "visions_of_villainy");
    assert_eq!(reduction(&mut e, "visions_of_villainy"), 0);
    inject_permanent_on_battlefield(&mut e, 0, "firdoch_core");
    assert_eq!(reduction(&mut e, "visions_of_villainy"), 1);
    let before = e.state.turn_history.current.player(0).cards_drawn;
    let slot = hand_index_for_card(&e, 0, "visions_of_villainy");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 18);
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        before + 2
    );
}

#[test]
fn issue_misc44_duel_tactics() {
    let mut e = engine(844_040);
    let first = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "duel_tactics");
    let slot = hand_index_for_card(&e, 0, "duel_tactics");
    let spell = e.state.players[0].hand[slot];
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(first)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&first].damage, 1);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 1, first),
        vec!["Can't block"]
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    let second = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    grant_pool(&mut e, 0);
    let generation = e.state.zone_change_generation[&spell];
    semantic::accepted(
        &mut e,
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                source: Some(graveyard_cast_source(spell, generation)),
                cast_method: CastMethod::Flashback as i32,
                targets: target_object(second),
                ..Default::default()
            })),
        },
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&second].damage, 1);
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
}
