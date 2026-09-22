//! Actual-card semantics for five pinned Standard spells.
//! Oracle and rulings checked 2026-09-22; CR 115.6, 608.2b-c, 700.2c, 701.6a.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn cast_modal(e: &mut GameEngine, card: &str, mode: u32, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_modal_spell(slot, vec![(mode, targets)]));
}

fn cast_regular(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
}

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

#[test]
fn issue_misc43_deadly_plot() {
    let mut creature = engine(843_001);
    let bear = inject_creature_on_battlefield(&mut creature, 1, "grizzly_bears");
    cast_modal(&mut creature, "deadly_plot", 0, target_object(bear));
    resolve_entire_stack_two_player(&mut creature);
    assert_eq!(creature.state.objects[&bear].zone, Zone::Graveyard);

    let mut walker = engine(843_002);
    let jace = inject_permanent_on_battlefield(&mut walker, 1, "jace_beleren");
    walker
        .state
        .objects
        .get_mut(&jace)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    cast_modal(&mut walker, "deadly_plot", 0, target_object(jace));
    resolve_entire_stack_two_player(&mut walker);
    assert_eq!(walker.state.objects[&jace].zone, Zone::Graveyard);

    let mut grave = engine(843_003);
    let zombie = inject_graveyard_card(&mut grave, 0, "walking_corpse");
    cast_modal(&mut grave, "deadly_plot", 1, target_object(zombie));
    resolve_entire_stack_two_player(&mut grave);
    assert_eq!(grave.state.objects[&zombie].zone, Zone::Battlefield);
    assert!(grave.state.objects[&zombie].tapped);

    let mut invalid = engine(843_004);
    let ordinary = inject_graveyard_card(&mut invalid, 0, "grizzly_bears");
    inject_card_into_hand(&mut invalid, 0, "deadly_plot");
    let slot = hand_index_for_card(&invalid, 0, "deadly_plot");
    assert!(invalid
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_object(ordinary))])
        )
        .is_err());
}

#[test]
fn issue_misc43_break_down_the_door() {
    for (seed, mode, card) in [(843_010, 0, "howling_mine"), (843_011, 1, "impact_tremors")] {
        let mut e = engine(seed);
        let target = inject_permanent_on_battlefield(&mut e, 1, card);
        cast_modal(&mut e, "break_down_the_door", mode, target_object(target));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{card}");
    }

    let mut invalid = engine(843_012);
    let creature = inject_creature_on_battlefield(&mut invalid, 1, "grizzly_bears");
    inject_card_into_hand(&mut invalid, 0, "break_down_the_door");
    let slot = hand_index_for_card(&invalid, 0, "break_down_the_door");
    assert!(invalid
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(0, target_object(creature))])
        )
        .is_err());

    let mut manifest = engine(843_013);
    let land = inject_library_card(&mut manifest, 0, "forest");
    let creature = inject_library_card(&mut manifest, 0, "grizzly_bears");
    manifest.state.players[0]
        .library
        .retain(|id| *id != land && *id != creature);
    manifest.state.players[0].library.push_front(creature);
    manifest.state.players[0].library.push_front(land);
    cast_modal(&mut manifest, "break_down_the_door", 2, vec![]);
    semantic::accepted(&mut manifest, 0, &pass());
    let parked = semantic::accepted(&mut manifest, 1, &pass());
    let choice = find_resolution_choice(&parked).expect("manifest dread choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::ManifestDread);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, vec![land, creature]);
    semantic::accepted(&mut manifest, 0, &submit_resolution_choice(vec![land]));
    assert_eq!(manifest.state.objects[&land].zone, Zone::Battlefield);
    assert!(manifest.state.objects[&land].face_down);
    assert_eq!(manifest.effective_power(land), Some(2));
    assert_eq!(manifest.state.objects[&creature].zone, Zone::Graveyard);
}

#[test]
fn issue_misc43_spectral_interference() {
    let mut e = engine(843_020);
    inject_card_into_hand(&mut e, 0, "howling_mine");
    let artifact_slot = hand_index_for_card(&e, 0, "howling_mine");
    let artifact = e.state.players[0].hand[artifact_slot];
    semantic::accepted(&mut e, 0, &cast_spell(artifact_slot, vec![]));
    let spell = e.state.stack.last().expect("artifact spell").id;
    cast_regular(&mut e, "spectral_interference", stack_target(spell));
    semantic::accepted(&mut e, 0, &pass());
    let parked = semantic::accepted(&mut e, 1, &pass());
    let choice = find_resolution_choice(&parked).expect("soft-counter payment");
    assert_eq!(choice.choice_kind(), ChoiceKind::ManaPayment);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.generic_mana_cost, 4);
    submit_mana_resolution_decision(&mut e, 0, ResolutionChoiceDecision::Decline)
        .expect("decline soft-counter payment");
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);

    let mut creature = engine(843_022);
    inject_card_into_hand(&mut creature, 0, "grizzly_bears");
    let creature_slot = hand_index_for_card(&creature, 0, "grizzly_bears");
    let creature_card = creature.state.players[0].hand[creature_slot];
    semantic::accepted(&mut creature, 0, &cast_spell(creature_slot, vec![]));
    let creature_spell = creature.state.stack.last().expect("creature spell").id;
    cast_regular(
        &mut creature,
        "spectral_interference",
        stack_target(creature_spell),
    );
    semantic::accepted(&mut creature, 0, &pass());
    let parked = semantic::accepted(&mut creature, 1, &pass());
    let choice = find_resolution_choice(&parked).expect("creature spell payment");
    assert_eq!(choice.choice_kind(), ChoiceKind::ManaPayment);
    submit_mana_resolution_decision(&mut creature, 0, ResolutionChoiceDecision::Decline)
        .expect("decline creature spell payment");
    assert_eq!(creature.state.objects[&creature_card].zone, Zone::Graveyard);

    let mut invalid = engine(843_021);
    cast_regular(&mut invalid, "divination", vec![]);
    let sorcery_spell = invalid.state.stack.last().expect("sorcery spell").id;
    inject_card_into_hand(&mut invalid, 0, "spectral_interference");
    let slot = hand_index_for_card(&invalid, 0, "spectral_interference");
    assert!(invalid
        .apply_command(0, &cast_spell(slot, stack_target(sorcery_spell)))
        .is_err());
}

#[test]
fn issue_misc43_calamitous_tide() {
    let mut e = engine(843_030);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let discard = inject_card_into_hand(&mut e, 0, "forest");
    let before = e.state.turn_history.current.player(0).cards_drawn;
    cast_regular(
        &mut e,
        "calamitous_tide",
        vec![target_object(own)[0], target_object(opposing)[0]],
    );
    semantic::complete(&mut e, 8, |_| {
        Some((0, submit_resolution_choice(vec![discard])))
    })
    .require_exercised();
    assert_eq!(e.state.objects[&own].zone, Zone::Hand);
    assert_eq!(e.state.objects[&opposing].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&own));
    assert!(e.state.players[1].hand.contains(&opposing));
    assert!(!e.state.players[0].hand.contains(&opposing));
    assert!(!e.state.players[1].hand.contains(&own));
    assert_eq!(e.state.objects[&discard].zone, Zone::Graveyard);
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        before + 2
    );

    let mut zero = engine(843_031);
    let discard = inject_card_into_hand(&mut zero, 0, "forest");
    let before = zero.state.turn_history.current.player(0).cards_drawn;
    cast_regular(&mut zero, "calamitous_tide", vec![]);
    semantic::complete(&mut zero, 8, |_| {
        Some((0, submit_resolution_choice(vec![discard])))
    })
    .require_exercised();
    assert_eq!(
        zero.state.turn_history.current.player(0).cards_drawn,
        before + 2
    );

    let mut fizzled = engine(843_032);
    let creature = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    let before = fizzled.state.turn_history.current.player(0).cards_drawn;
    cast_regular(&mut fizzled, "calamitous_tide", target_object(creature));
    cast_regular(&mut fizzled, "unsummon", target_object(creature));
    semantic::complete(&mut fizzled, 8, |_| None).require_exercised();
    assert_eq!(fizzled.state.objects[&creature].zone, Zone::Hand);
    assert_eq!(
        fizzled.state.turn_history.current.player(0).cards_drawn,
        before
    );
}

#[test]
fn issue_misc43_ozais_cruelty() {
    let mut e = engine(843_040);
    let first = inject_card_into_hand(&mut e, 1, "grizzly_bears");
    let second = inject_card_into_hand(&mut e, 1, "forest");
    let kept = inject_card_into_hand(&mut e, 1, "island");
    cast_regular(&mut e, "ozais_cruelty", target_player(1));
    semantic::accepted(&mut e, 0, &pass());
    let parked = semantic::accepted(&mut e, 1, &pass());
    let choice = find_resolution_choice(&parked).expect("affected player's discard choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(e.state.players[1].life, 18);
    semantic::accepted(&mut e, 1, &submit_resolution_choice(vec![first, second]));
    assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
    assert!(e.state.players[1].hand.contains(&kept));
    assert_eq!(e.state.players[0].life, 20);
}
