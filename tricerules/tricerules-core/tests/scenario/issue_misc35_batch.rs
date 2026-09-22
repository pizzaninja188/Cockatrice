//! Actual-card semantics for four pinned Standard graveyard spells.
//! Oracle and rulings checked 2026-09-22; CR 115.1, 202.3e, 400.7, 608.2b-c.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{TargetRef, TargetRefKind};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }
}

fn cast(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
}

fn exile_graveyard_card(e: &mut GameEngine, object_id: u32) {
    e.state.players[0].graveyard.retain(|id| *id != object_id);
    e.state.players[0].exile.push(object_id);
    e.state.objects.get_mut(&object_id).unwrap().zone = Zone::Exile;
    *e.state.zone_change_generation.entry(object_id).or_default() += 1;
}

#[test]
fn issue_misc35_helping_hand_semantics() {
    let mut e = engine(835_001);
    let bear = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let giant = inject_graveyard_card(&mut e, 0, "hill_giant");
    let opposing = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "helping_hand");
    let slot = hand_index_for_card(&e, 0, "helping_hand");
    let legal = &e.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    assert_eq!(legal.groups[0].valid_graveyard_ids, vec![bear]);
    for invalid in [giant, opposing] {
        assert!(e
            .apply_command(0, &cast_spell(slot, vec![target(invalid)]))
            .is_err());
        assert_eq!(e.state.stack.len(), 0);
    }
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![target(bear)]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Battlefield);
    assert!(e.state.objects[&bear].tapped);
    assert_eq!(e.state.objects[&giant].zone, Zone::Graveyard);
}

#[test]
fn issue_misc35_hazels_nocturne_semantics() {
    let mut zero = engine(835_010);
    cast(&mut zero, "hazels_nocturne", vec![]);
    resolve_entire_stack_two_player(&mut zero);
    assert_eq!(
        (zero.state.players[0].life, zero.state.players[1].life),
        (22, 18)
    );

    let mut two = engine(835_011);
    let first = inject_graveyard_card(&mut two, 0, "grizzly_bears");
    let second = inject_graveyard_card(&mut two, 0, "storm_crow");
    cast(
        &mut two,
        "hazels_nocturne",
        vec![target(first), target(second)],
    );
    resolve_entire_stack_two_player(&mut two);
    assert_eq!(
        (
            two.state.objects[&first].zone,
            two.state.objects[&second].zone
        ),
        (Zone::Hand, Zone::Hand)
    );
    assert_eq!(
        (two.state.players[0].life, two.state.players[1].life),
        (22, 18)
    );

    let mut fizzle = engine(835_012);
    let victim = inject_graveyard_card(&mut fizzle, 0, "grizzly_bears");
    cast(&mut fizzle, "hazels_nocturne", vec![target(victim)]);
    exile_graveyard_card(&mut fizzle, victim);
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(
        (fizzle.state.players[0].life, fizzle.state.players[1].life),
        (20, 20)
    );
}

#[test]
fn issue_misc35_pull_from_the_grave_semantics() {
    let mut zero = engine(835_020);
    cast(&mut zero, "pull_from_the_grave", vec![]);
    resolve_entire_stack_two_player(&mut zero);
    assert_eq!(zero.state.players[0].life, 22);

    let mut two = engine(835_021);
    let first = inject_graveyard_card(&mut two, 0, "grizzly_bears");
    let second = inject_graveyard_card(&mut two, 0, "storm_crow");
    cast(
        &mut two,
        "pull_from_the_grave",
        vec![target(first), target(second)],
    );
    resolve_entire_stack_two_player(&mut two);
    assert_eq!(
        (
            two.state.objects[&first].zone,
            two.state.objects[&second].zone
        ),
        (Zone::Hand, Zone::Hand)
    );
    assert_eq!(two.state.players[0].life, 22);

    let mut fizzle = engine(835_022);
    let victim = inject_graveyard_card(&mut fizzle, 0, "grizzly_bears");
    cast(&mut fizzle, "pull_from_the_grave", vec![target(victim)]);
    exile_graveyard_card(&mut fizzle, victim);
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(fizzle.state.players[0].life, 20);
}

#[test]
fn issue_misc35_mourners_surprise_semantics() {
    let mut zero = engine(835_030);
    cast(&mut zero, "mourners_surprise", vec![]);
    resolve_entire_stack_two_player(&mut zero);
    assert_eq!(battlefield_token_oids(&zero, 0, "mercenary_r_1_1").len(), 1);
    assert!(battlefield_token_oids(&zero, 1, "mercenary_r_1_1").is_empty());

    let mut targetted = engine(835_031);
    let creature = inject_graveyard_card(&mut targetted, 0, "grizzly_bears");
    cast(&mut targetted, "mourners_surprise", vec![target(creature)]);
    resolve_entire_stack_two_player(&mut targetted);
    assert_eq!(targetted.state.objects[&creature].zone, Zone::Hand);
    assert_eq!(
        battlefield_token_oids(&targetted, 0, "mercenary_r_1_1").len(),
        1
    );

    let mut fizzle = engine(835_032);
    let victim = inject_graveyard_card(&mut fizzle, 0, "grizzly_bears");
    cast(&mut fizzle, "mourners_surprise", vec![target(victim)]);
    exile_graveyard_card(&mut fizzle, victim);
    resolve_entire_stack_two_player(&mut fizzle);
    assert!(battlefield_token_oids(&fizzle, 0, "mercenary_r_1_1").is_empty());
}
