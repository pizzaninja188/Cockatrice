//! Actual-card semantics for five pinned Standard removal spells.
//! Oracle and rulings checked 2026-09-22; CR 115.3, 400.7, 608.2b-c.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast(e: &mut GameEngine, card: &str, target: u32) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, target_object(target)));
}

fn illegal_at_resolution(e: &mut GameEngine, target: u32) {
    let controller = e.state.objects[&target].controller as usize;
    e.state.players[controller]
        .battlefield
        .retain(|id| *id != target);
    let owner = e.state.objects[&target].owner as usize;
    e.state.players[owner].graveyard.push(target);
    e.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(target).or_default() += 1;
}

#[test]
fn issue_misc37_harsh_annotation() {
    let mut e = engine(837_001);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, "harsh_annotation", bear);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    let tokens = battlefield_token_oids(&e, 1, "inkling_wb_1_1_flying");
    assert_eq!(tokens.len(), 1);
    assert!(e.effective_has_keyword(tokens[0], Keyword::Flying));
    assert!(battlefield_token_oids(&e, 0, "inkling_wb_1_1_flying").is_empty());

    let mut indestructible = engine(837_002);
    let myr = inject_creature_on_battlefield(&mut indestructible, 1, "darksteel_myr");
    cast(&mut indestructible, "harsh_annotation", myr);
    resolve_entire_stack_two_player(&mut indestructible);
    assert_eq!(indestructible.state.objects[&myr].zone, Zone::Battlefield);
    assert_eq!(
        battlefield_token_oids(&indestructible, 1, "inkling_wb_1_1_flying").len(),
        1
    );

    let mut fizzled = engine(837_003);
    let bear = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    cast(&mut fizzled, "harsh_annotation", bear);
    illegal_at_resolution(&mut fizzled, bear);
    resolve_entire_stack_two_player(&mut fizzled);
    assert!(battlefield_token_oids(&fizzled, 1, "inkling_wb_1_1_flying").is_empty());
}

#[test]
fn issue_misc37_zukos_exile() {
    for (seed, card) in [
        (837_010, "grizzly_bears"),
        (837_011, "swiftfoot_boots"),
        (837_012, "glorious_anthem"),
    ] {
        let mut e = engine(seed);
        let target = inject_permanent_on_battlefield(&mut e, 1, card);
        cast(&mut e, "zukos_exile", target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{card}");
        assert_eq!(battlefield_token_oids(&e, 1, "clue").len(), 1, "{card}");
        assert!(battlefield_token_oids(&e, 0, "clue").is_empty());
    }
    let mut bad = engine(837_013);
    let land = inject_permanent_on_battlefield(&mut bad, 1, "forest");
    inject_card_into_hand(&mut bad, 0, "zukos_exile");
    let slot = hand_index_for_card(&bad, 0, "zukos_exile");
    assert!(bad
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .is_err());

    let mut fizzled = engine(837_014);
    let bear = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    cast(&mut fizzled, "zukos_exile", bear);
    illegal_at_resolution(&mut fizzled, bear);
    resolve_entire_stack_two_player(&mut fizzled);
    assert!(battlefield_token_oids(&fizzled, 1, "clue").is_empty());
}

#[test]
fn issue_misc37_shattered_wings() {
    for (seed, card) in [
        (837_020, "swiftfoot_boots"),
        (837_021, "glorious_anthem"),
        (837_022, "serra_angel"),
    ] {
        let mut e = engine(seed);
        let target = inject_permanent_on_battlefield(&mut e, 1, card);
        let top = *e.state.players[0].library.front().expect("caster top card");
        let next = *e.state.players[0]
            .library
            .get(1)
            .expect("caster second card");
        let caster_library_len = e.state.players[0].library.len();
        let opposing_library = e.state.players[1].library.clone();
        cast(&mut e, "shattered_wings", target);
        semantic::accepted(&mut e, 0, &pass());
        let batch = e.apply_command(1, &pass()).expect("surveil choice");
        let choice = find_resolution_choice(&batch).expect("surveil 1");
        assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.candidate_object_ids, vec![top]);
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard, "{card}");
        semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![top]));
        assert!(e.state.pending_resolution.is_none());
        assert_eq!(e.state.objects[&top].zone, Zone::Graveyard);
        assert!(e.state.players[0].graveyard.contains(&top));
        assert_eq!(e.state.players[0].library.len(), caster_library_len - 1);
        assert_eq!(e.state.players[0].library.front(), Some(&next));
        assert_eq!(e.state.players[1].library, opposing_library);
    }
    let mut bad = engine(837_023);
    let bear = inject_creature_on_battlefield(&mut bad, 1, "grizzly_bears");
    inject_card_into_hand(&mut bad, 0, "shattered_wings");
    let slot = hand_index_for_card(&bad, 0, "shattered_wings");
    assert!(bad
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .is_err());
    let mut fizzled = engine(837_024);
    let artifact = inject_permanent_on_battlefield(&mut fizzled, 1, "swiftfoot_boots");
    cast(&mut fizzled, "shattered_wings", artifact);
    illegal_at_resolution(&mut fizzled, artifact);
    resolve_entire_stack_two_player(&mut fizzled);
    assert!(fizzled.state.pending_resolution.is_none());
}

#[test]
fn issue_misc37_devout_decree() {
    for (seed, card) in [
        (837_030, "goblin_piker"),
        (837_031, "kaito,_bane_of_nightmares"),
    ] {
        let mut e = engine(seed);
        let target = inject_permanent_on_battlefield(&mut e, 1, card);
        if card == "kaito,_bane_of_nightmares" {
            e.state
                .objects
                .get_mut(&target)
                .unwrap()
                .set_counter(CounterKind::Loyalty, 4);
        }
        let top = *e.state.players[0].library.front().expect("caster top card");
        let next = *e.state.players[0]
            .library
            .get(1)
            .expect("caster second card");
        let caster_library_len = e.state.players[0].library.len();
        let opposing_library = e.state.players[1].library.clone();
        cast(&mut e, "devout_decree", target);
        semantic::accepted(&mut e, 0, &pass());
        let batch = e.apply_command(1, &pass()).expect("scry choice");
        let choice = find_resolution_choice(&batch).unwrap_or_else(|| {
            panic!(
                "scry 1 after {card}; target zone {:?}; pending {:?}",
                e.state.objects[&target].zone,
                e.state
                    .pending_resolution
                    .as_ref()
                    .map(|p| p.presentation.choice_kind)
            )
        });
        assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.candidate_object_ids, vec![top]);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{card}");
        semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![top]));
        assert!(e.state.pending_resolution.is_none());
        assert_eq!(e.state.players[0].library.len(), caster_library_len);
        assert_eq!(e.state.players[0].library.front(), Some(&next));
        assert_eq!(e.state.players[0].library.back(), Some(&top));
        assert_eq!(e.state.players[1].library, opposing_library);
    }
    let mut bad = engine(837_032);
    let green = inject_creature_on_battlefield(&mut bad, 1, "grizzly_bears");
    inject_card_into_hand(&mut bad, 0, "devout_decree");
    let slot = hand_index_for_card(&bad, 0, "devout_decree");
    assert!(bad
        .apply_command(0, &cast_spell(slot, target_object(green)))
        .is_err());
    let mut fizzled = engine(837_033);
    let red = inject_creature_on_battlefield(&mut fizzled, 1, "goblin_piker");
    cast(&mut fizzled, "devout_decree", red);
    illegal_at_resolution(&mut fizzled, red);
    resolve_entire_stack_two_player(&mut fizzled);
    assert!(fizzled.state.pending_resolution.is_none());
}

#[test]
fn issue_misc37_inevitable_defeat() {
    let mut e = engine(837_040);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "swiftfoot_boots");
    inject_card_into_hand(&mut e, 0, "inevitable_defeat");
    let spell = e.state.players[0].hand[hand_index_for_card(&e, 0, "inevitable_defeat")];
    let slot = hand_index_for_card(&e, 0, "inevitable_defeat");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(artifact)));
    inject_card_into_hand(&mut e, 1, "counterspell");
    semantic::accepted(&mut e, 0, &pass());
    let counter_slot = hand_index_for_card(&e, 1, "counterspell");
    semantic::accepted(&mut e, 1, &cast_spell(counter_slot, target_object(spell)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Exile);
    assert_eq!(e.state.players[0].life, 23);
    assert_eq!(e.state.players[1].life, 17);

    let mut bad = engine(837_041);
    let land = inject_permanent_on_battlefield(&mut bad, 1, "forest");
    inject_card_into_hand(&mut bad, 0, "inevitable_defeat");
    let slot = hand_index_for_card(&bad, 0, "inevitable_defeat");
    assert!(bad
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .is_err());
    let mut fizzled = engine(837_042);
    let bear = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    cast(&mut fizzled, "inevitable_defeat", bear);
    illegal_at_resolution(&mut fizzled, bear);
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(fizzled.state.players[0].life, 20);
    assert_eq!(fizzled.state.players[1].life, 20);
}
