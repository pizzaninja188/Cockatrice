//! Actual-card semantics for five pinned Standard modal spells.
//! Oracle and rulings checked 2026-09-22; CR 115.6, 608.2b, 611.2c, 614.1, 700.2c.

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

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

#[test]
fn issue_misc42_rydias_return() {
    let mut pump = engine(842_001);
    let own = inject_creature_on_battlefield(&mut pump, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut pump, 1, "grizzly_bears");
    cast_and_resolve(&mut pump, "rydias_return", 0, vec![]);
    assert_eq!(pump.effective_power(own), Some(5));
    assert_eq!(pump.effective_toughness(own), Some(5));
    assert_eq!(pump.effective_power(opposing), Some(2));
    let late = inject_creature_on_battlefield(&mut pump, 0, "grizzly_bears");
    assert_eq!(pump.effective_power(late), Some(2));

    let mut return_cards = engine(842_002);
    let land = inject_graveyard_card(&mut return_cards, 0, "forest");
    let artifact = inject_graveyard_card(&mut return_cards, 0, "howling_mine");
    cast_and_resolve(
        &mut return_cards,
        "rydias_return",
        1,
        vec![target_object(land)[0], target_object(artifact)[0]],
    );
    assert_eq!(return_cards.state.objects[&land].zone, Zone::Hand);
    assert_eq!(return_cards.state.objects[&artifact].zone, Zone::Hand);

    let mut zero = engine(842_003);
    let untouched = inject_graveyard_card(&mut zero, 0, "forest");
    cast_and_resolve(&mut zero, "rydias_return", 1, vec![]);
    assert_eq!(zero.state.objects[&untouched].zone, Zone::Graveyard);

    let mut invalid = engine(842_004);
    let sorcery = inject_graveyard_card(&mut invalid, 0, "divination");
    inject_card_into_hand(&mut invalid, 0, "rydias_return");
    let slot = hand_index_for_card(&invalid, 0, "rydias_return");
    assert!(invalid
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_object(sorcery))])
        )
        .is_err());
}

#[test]
fn issue_misc42_splatter_technique() {
    let mut draw = engine(842_010);
    let before = draw.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut draw, "splatter_technique", 0, vec![]);
    assert_eq!(
        draw.state.turn_history.current.player(0).cards_drawn,
        before + 4
    );

    let mut damage = engine(842_011);
    let own = inject_creature_on_battlefield(&mut damage, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut damage, 1, "grizzly_bears");
    let walker = inject_permanent_on_battlefield(&mut damage, 1, "jace_beleren");
    damage
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    let artifact = inject_permanent_on_battlefield(&mut damage, 1, "howling_mine");
    cast_and_resolve(&mut damage, "splatter_technique", 1, vec![]);
    assert_eq!(damage.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(damage.state.objects[&opposing].zone, Zone::Graveyard);
    assert_eq!(damage.state.objects[&walker].zone, Zone::Graveyard);
    assert_eq!(damage.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(damage.state.players[0].life, 20);
    assert_eq!(damage.state.players[1].life, 20);
}

#[test]
fn issue_misc42_damage_then_exile_modes() {
    for (seed, card, damage) in [(842_020, "suplex", 3), (842_021, "agate_assault", 4)] {
        let mut e = engine(seed);
        let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
        cast_and_resolve(&mut e, card, 0, target_object(creature));
        assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield, "{card}");
        assert_eq!(e.state.objects[&creature].damage, damage, "{card}");
        e.state.objects.get_mut(&creature).unwrap().damage = 6;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&creature].zone, Zone::Exile, "{card}");

        let mut artifact_mode = engine(seed + 10);
        let artifact = inject_permanent_on_battlefield(&mut artifact_mode, 1, "howling_mine");
        cast_and_resolve(&mut artifact_mode, card, 1, target_object(artifact));
        assert_eq!(
            artifact_mode.state.objects[&artifact].zone,
            Zone::Exile,
            "{card}"
        );

        let mut illegal = engine(seed + 20);
        let land = inject_permanent_on_battlefield(&mut illegal, 1, "forest");
        inject_card_into_hand(&mut illegal, 0, card);
        let slot = hand_index_for_card(&illegal, 0, card);
        assert!(
            illegal
                .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(land))]))
                .is_err(),
            "{card}"
        );
    }
}

#[test]
fn issue_misc42_school_daze() {
    let mut homework = engine(842_030);
    let before = homework.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut homework, "school_daze", 0, vec![]);
    assert_eq!(
        homework.state.turn_history.current.player(0).cards_drawn,
        before + 3
    );

    let mut crime = engine(842_031);
    inject_card_into_hand(&mut crime, 0, "grizzly_bears");
    let bear_slot = hand_index_for_card(&crime, 0, "grizzly_bears");
    let bear = crime.state.players[0].hand[bear_slot];
    semantic::accepted(&mut crime, 0, &cast_spell(bear_slot, vec![]));
    let spell = crime.state.stack.last().expect("bear spell").id;
    let before = crime.state.turn_history.current.player(0).cards_drawn;
    cast_and_resolve(&mut crime, "school_daze", 1, stack_target(spell));
    assert_eq!(crime.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(
        crime.state.turn_history.current.player(0).cards_drawn,
        before + 1
    );

    let mut invalid = engine(842_032);
    inject_card_into_hand(&mut invalid, 0, "school_daze");
    let slot = hand_index_for_card(&invalid, 0, "school_daze");
    assert!(invalid
        .apply_command(0, &cast_modal_spell(slot, vec![(1, vec![])]))
        .is_err());

    let mut fizzled = engine(842_033);
    inject_card_into_hand(&mut fizzled, 0, "grizzly_bears");
    let bear_slot = hand_index_for_card(&fizzled, 0, "grizzly_bears");
    semantic::accepted(&mut fizzled, 0, &cast_spell(bear_slot, vec![]));
    let bear_spell = fizzled.state.stack.last().expect("bear spell").id;
    cast_only(&mut fizzled, "school_daze", 1, stack_target(bear_spell));
    let before = fizzled.state.turn_history.current.player(0).cards_drawn;
    inject_card_into_hand(&mut fizzled, 0, "counterspell");
    grant_pool(&mut fizzled, 0);
    let counter_slot = hand_index_for_card(&fizzled, 0, "counterspell");
    semantic::accepted(
        &mut fizzled,
        0,
        &cast_spell(counter_slot, stack_target(bear_spell)),
    );
    resolve_entire_stack_two_player(&mut fizzled);
    assert_eq!(
        fizzled.state.turn_history.current.player(0).cards_drawn,
        before,
        "School Daze does not draw when its only target is countered first"
    );
}
