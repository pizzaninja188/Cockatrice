//! Actual-card semantics for three pinned Standard graveyard spells.
//! Oracle and rulings checked 2026-09-22; CR 115.3, 400.7, 608.2b-c.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{TargetRef, TargetRefKind};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn target(object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
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
fn issue_misc36_true_ancestry_semantics() {
    let mut zero = engine(836_001);
    let untouched = inject_graveyard_card(&mut zero, 0, "grizzly_bears");
    cast(&mut zero, "true_ancestry", vec![]);
    resolve_entire_stack_two_player(&mut zero);
    assert_eq!(zero.state.objects[&untouched].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&zero, 0, "clue").len(), 1);
    assert!(battlefield_token_oids(&zero, 1, "clue").is_empty());

    let mut chosen = engine(836_002);
    let land = inject_graveyard_card(&mut chosen, 0, "forest");
    let creature = inject_graveyard_card(&mut chosen, 0, "grizzly_bears");
    let instant = inject_graveyard_card(&mut chosen, 0, "lightning_bolt");
    let opposing = inject_graveyard_card(&mut chosen, 1, "forest");
    inject_card_into_hand(&mut chosen, 0, "true_ancestry");
    let slot = hand_index_for_card(&chosen, 0, "true_ancestry");
    let legal = &chosen.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    let offered = &legal.groups[0].valid_graveyard_ids;
    assert!(offered.contains(&land) && offered.contains(&creature));
    assert!(!offered.contains(&instant) && !offered.contains(&opposing));
    for invalid in [instant, opposing] {
        assert!(chosen
            .apply_command(0, &cast_spell(slot, vec![target(invalid, 0)]))
            .is_err());
        assert!(chosen.state.stack.is_empty());
    }
    semantic::accepted(&mut chosen, 0, &cast_spell(slot, vec![target(land, 0)]));
    resolve_entire_stack_two_player(&mut chosen);
    assert_eq!(chosen.state.objects[&land].zone, Zone::Hand);
    assert_eq!(chosen.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&chosen, 0, "clue").len(), 1);

    let mut fizzle = engine(836_003);
    let victim = inject_graveyard_card(&mut fizzle, 0, "grizzly_bears");
    cast(&mut fizzle, "true_ancestry", vec![target(victim, 0)]);
    exile_graveyard_card(&mut fizzle, victim);
    resolve_entire_stack_two_player(&mut fizzle);
    assert!(battlefield_token_oids(&fizzle, 0, "clue").is_empty());
}

#[test]
fn issue_misc36_pull_through_the_weft_semantics() {
    let mut e = engine(836_010);
    let creature = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let artifact = inject_graveyard_card(&mut e, 0, "bonesplitter");
    let land_a = inject_graveyard_card(&mut e, 0, "forest");
    let land_b = inject_graveyard_card(&mut e, 0, "mountain");
    let instant = inject_graveyard_card(&mut e, 0, "lightning_bolt");
    let opposing = inject_graveyard_card(&mut e, 1, "forest");
    inject_card_into_hand(&mut e, 0, "pull_through_the_weft");
    let slot = hand_index_for_card(&e, 0, "pull_through_the_weft");
    let legal = &e.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    let first = &legal.groups[0].valid_graveyard_ids;
    let second = &legal.groups[1].valid_graveyard_ids;
    assert!(first.contains(&creature) && first.contains(&artifact));
    assert!(!first.contains(&land_a) && !first.contains(&instant));
    assert!(second.contains(&land_a) && second.contains(&land_b));
    assert!(!second.contains(&creature) && !second.contains(&opposing));
    assert!(e
        .apply_command(0, &cast_spell(slot, vec![target(land_a, 0)]))
        .is_err());
    assert!(e.state.stack.is_empty());
    semantic::accepted(
        &mut e,
        0,
        &cast_spell(
            slot,
            vec![
                target(creature, 0),
                target(artifact, 0),
                target(land_a, 1),
                target(land_b, 1),
            ],
        ),
    );
    resolve_entire_stack_two_player(&mut e);
    for id in [creature, artifact] {
        assert_eq!(e.state.objects[&id].zone, Zone::Hand);
    }
    for id in [land_a, land_b] {
        assert_eq!(e.state.objects[&id].zone, Zone::Battlefield);
        assert!(e.state.objects[&id].tapped);
    }
    assert_eq!(e.state.objects[&instant].zone, Zone::Graveyard);

    let mut zero = engine(836_011);
    let untouched = inject_graveyard_card(&mut zero, 0, "forest");
    cast(&mut zero, "pull_through_the_weft", vec![]);
    resolve_entire_stack_two_player(&mut zero);
    assert_eq!(zero.state.objects[&untouched].zone, Zone::Graveyard);
}

#[test]
fn issue_misc36_badlands_revival_semantics() {
    let mut distinct = engine(836_020);
    let creature = inject_graveyard_card(&mut distinct, 0, "grizzly_bears");
    let land = inject_graveyard_card(&mut distinct, 0, "forest");
    let instant = inject_graveyard_card(&mut distinct, 0, "lightning_bolt");
    inject_card_into_hand(&mut distinct, 0, "badlands_revival");
    let slot = hand_index_for_card(&distinct, 0, "badlands_revival");
    let legal = &distinct.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    assert!(legal.groups[0].valid_graveyard_ids.contains(&creature));
    assert!(!legal.groups[0].valid_graveyard_ids.contains(&land));
    assert!(legal.groups[1].valid_graveyard_ids.contains(&creature));
    assert!(legal.groups[1].valid_graveyard_ids.contains(&land));
    assert!(!legal.groups[1].valid_graveyard_ids.contains(&instant));
    semantic::accepted(
        &mut distinct,
        0,
        &cast_spell(slot, vec![target(creature, 0), target(land, 1)]),
    );
    resolve_entire_stack_two_player(&mut distinct);
    assert_eq!(distinct.state.objects[&creature].zone, Zone::Battlefield);
    assert!(!distinct.state.objects[&creature].tapped);
    assert_eq!(distinct.state.objects[&land].zone, Zone::Hand);

    // CR 115.3 allows the same card for the two distinct target clauses. The first
    // instruction moves it; the second no longer finds that card in the graveyard.
    let mut same = engine(836_021);
    let both = inject_graveyard_card(&mut same, 0, "grizzly_bears");
    cast(
        &mut same,
        "badlands_revival",
        vec![target(both, 0), target(both, 1)],
    );
    resolve_entire_stack_two_player(&mut same);
    assert_eq!(same.state.objects[&both].zone, Zone::Battlefield);
    assert!(!same.state.players[0].hand.contains(&both));

    let mut zero = engine(836_022);
    let untouched = inject_graveyard_card(&mut zero, 0, "grizzly_bears");
    cast(&mut zero, "badlands_revival", vec![]);
    resolve_entire_stack_two_player(&mut zero);
    assert_eq!(zero.state.objects[&untouched].zone, Zone::Graveyard);

    let mut mixed = engine(836_023);
    let gone = inject_graveyard_card(&mut mixed, 0, "grizzly_bears");
    let surviving = inject_graveyard_card(&mut mixed, 0, "forest");
    cast(
        &mut mixed,
        "badlands_revival",
        vec![target(gone, 0), target(surviving, 1)],
    );
    exile_graveyard_card(&mut mixed, gone);
    resolve_entire_stack_two_player(&mut mixed);
    assert_eq!(mixed.state.objects[&surviving].zone, Zone::Hand);
    assert_eq!(mixed.state.objects[&gone].zone, Zone::Exile);
}
