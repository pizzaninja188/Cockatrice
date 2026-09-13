//! Issue #265 — fail-closed generated `Choose one —` spells.
//!
//! Oracle and rulings were checked 2026-09-12. CR 115.8, 601.2b-d, 608.2b-c, and 700.2
//! govern cast-time mode and target choices, target revalidation, and printed resolution order.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn modal_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn fund_modal_spell(engine: &mut GameEngine) {
    give_mana(
        engine,
        0,
        ManaGift {
            w: 4,
            u: 4,
            r: 4,
            g: 4,
            c: 4,
            ..Default::default()
        },
    );
}

fn cast_generated_mode(
    engine: &mut GameEngine,
    card_id: &str,
    mode_index: u32,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) {
    inject_card_into_hand(engine, 0, card_id);
    fund_modal_spell(engine);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(mode_index, targets)]))
        .unwrap_or_else(|error| panic!("cast {card_id} mode {mode_index}: {error:?}"));
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

#[test]
fn generated_abrade_modes_keep_their_own_targets_and_results() {
    let mut damage = modal_engine(265_001);
    let creature = inject_creature_on_battlefield(&mut damage, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut damage, 1, "short_sword");
    cast_generated_mode(&mut damage, "abrade", 0, target_object(creature));
    resolve_entire_stack_two_player(&mut damage);
    assert_eq!(damage.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(damage.state.objects[&artifact].zone, Zone::Battlefield);

    let mut destroy = modal_engine(265_002);
    let creature = inject_creature_on_battlefield(&mut destroy, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut destroy, 1, "short_sword");
    cast_generated_mode(&mut destroy, "abrade", 1, target_object(artifact));
    resolve_entire_stack_two_player(&mut destroy);
    assert_eq!(destroy.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(destroy.state.objects[&creature].zone, Zone::Battlefield);
}

#[test]
fn generated_team_modes_affect_only_the_casters_creatures() {
    let mut family = modal_engine(265_003);
    let own = inject_creature_on_battlefield(&mut family, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut family, 1, "grizzly_bears");
    cast_generated_mode(&mut family, "family_reunion", 0, vec![]);
    resolve_entire_stack_two_player(&mut family);
    assert_eq!(family.effective_power(own), Some(3));
    assert_eq!(family.effective_power(opponent), Some(2));

    cast_generated_mode(&mut family, "family_reunion", 1, vec![]);
    resolve_entire_stack_two_player(&mut family);
    assert!(family.effective_has_keyword(own, Keyword::Hexproof));
    assert!(!family.effective_has_keyword(opponent, Keyword::Hexproof));

    let mut goblins = modal_engine(265_004);
    cast_generated_mode(&mut goblins, "goblin_surprise", 1, vec![]);
    resolve_entire_stack_two_player(&mut goblins);
    assert_eq!(
        goblins.state.players[0]
            .battlefield
            .iter()
            .filter(|object_id| goblins.state.objects[object_id].card_id == "goblin_r_1_1")
            .count(),
        2
    );
}

#[test]
fn generated_sarkhan_modes_enforce_only_the_chosen_modes_filter() {
    let mut engine = modal_engine(265_005);
    let ordinary = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "sarkhans_resolve");
    fund_modal_spell(&mut engine);
    let slot = hand_index_for_card(&engine, 0, "sarkhans_resolve");
    assert!(engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_object(ordinary))]),
        )
        .is_err());

    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(0, target_object(ordinary))]),
        )
        .expect("pump mode does not inherit the flying restriction");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(ordinary), Some(5));
}

#[test]
fn generated_spellgyre_surveils_privately_before_drawing_two() {
    let mut engine = modal_engine(265_006);
    let first = inject_library_card(&mut engine, 0, "grizzly_bears");
    let second = inject_library_card(&mut engine, 0, "storm_crow");
    let third = inject_library_card(&mut engine, 0, "island");
    engine.state.players[0]
        .library
        .retain(|object_id| ![first, second, third].contains(object_id));
    for object_id in [third, second, first] {
        engine.state.players[0].library.push_front(object_id);
    }

    cast_generated_mode(&mut engine, "spellgyre", 1, vec![]);
    let hand_before_resolution = engine.state.players[0].hand.len();
    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("Surveil 2 choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, vec![first, second]);

    engine
        .apply_command(0, &submit_resolution_choice(vec![first]))
        .expect("put one card into the graveyard, then draw two");
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_resolution + 2
    );
}
