//! Issue #425 focused scenarios for the six retained `Choose one or both —` identities.
//!
//! Oracle and the current Comprehensive Rules were verified 2026-09-19 against the pinned
//! Scryfall snapshot. CR 700.2 (mode announcement at cast time), CR 120 (damage), CR 614.1a
//! (exile-if-would-die replacement), CR 701.23 (search), CR 701.26 (tap), CR 400.7 (zone
//! changes and typed graveyard returns), CR 107.1b/613.4c (doubling), CR 701.12 (fight), and
//! CR 111.1/701.6 (artifact tokens and destruction) govern the exercised behavior.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision, TargetRef, TargetRefKind};

fn modal_engine(seed: u64) -> GameEngine {
    // Explicit land-only decks keep the default grizzly_bears/library fixtures out of the
    // opening hand so injected fixture names stay unique.
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn prepare_spell(engine: &mut GameEngine, card_id: &str) -> usize {
    inject_card_into_hand(engine, 0, card_id);
    grant_pool(engine, 0);
    hand_index_for_card(engine, 0, card_id)
}

fn permanent_target(object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

fn graveyard_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }]
}

#[test]
fn issue_425_amazing_acrobatics_counters_a_spell_and_taps_one_or_two_creatures() {
    let mut counter_engine = modal_engine(425_001);
    let bolt_slot = prepare_spell(&mut counter_engine, "lightning_bolt");
    counter_engine
        .apply_command(0, &cast_spell(bolt_slot, target_player(1)))
        .expect("cast the bolt");
    let bolt = counter_engine.state.stack.last().expect("bolt on stack").id;
    let slot = prepare_spell(&mut counter_engine, "amazing_acrobatics");
    counter_engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_object(bolt))]))
        .expect("counter the bolt");
    resolve_entire_stack_two_player(&mut counter_engine);
    assert!(
        counter_engine.state.players[0].graveyard.contains(&bolt),
        "the countered bolt is in its owner's graveyard"
    );

    let mut tap_engine = modal_engine(425_002);
    let first = inject_creature_on_battlefield(&mut tap_engine, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut tap_engine, 1, "storm_crow");
    let slot = prepare_spell(&mut tap_engine, "amazing_acrobatics");
    tap_engine
        .apply_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(
                    1,
                    vec![permanent_target(first, 0), permanent_target(second, 0)],
                )],
            ),
        )
        .expect("tap both printed targets");
    resolve_entire_stack_two_player(&mut tap_engine);
    assert!(tap_engine.state.objects[&first].tapped);
    assert!(tap_engine.state.objects[&second].tapped);
}

#[test]
fn issue_425_amazing_acrobatics_rejects_three_tap_targets() {
    let mut engine = modal_engine(425_003);
    let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");
    let third = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let slot = prepare_spell(&mut engine, "amazing_acrobatics");
    assert!(
        engine
            .apply_command(
                0,
                &cast_modal_spell(
                    slot,
                    vec![(
                        1,
                        vec![
                            permanent_target(first, 0),
                            permanent_target(second, 0),
                            permanent_target(third, 0),
                        ],
                    )],
                ),
            )
            .is_err(),
        "the printed group is capped at two creatures"
    );
}

#[test]
fn issue_425_avengers_disassembled_sweeps_and_lets_the_land_controller_search() {
    let mut sweep_engine = modal_engine(425_010);
    let ours = inject_creature_on_battlefield(&mut sweep_engine, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut sweep_engine, 1, "grizzly_bears");
    let survivor = inject_creature_on_battlefield(&mut sweep_engine, 1, "storm_crow");
    sweep_engine
        .state
        .objects
        .get_mut(&survivor)
        .expect("survivor")
        .toughness = Some(5);
    let slot = prepare_spell(&mut sweep_engine, "avengers_disassembled");
    sweep_engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, vec![])]))
        .expect("cast the sweep mode");
    resolve_entire_stack_two_player(&mut sweep_engine);
    assert_eq!(sweep_engine.state.objects[&ours].zone, Zone::Graveyard);
    assert_eq!(sweep_engine.state.objects[&theirs].zone, Zone::Graveyard);
    assert_eq!(sweep_engine.state.objects[&survivor].damage, 3);
    assert_eq!(
        sweep_engine.state.objects[&survivor].zone,
        Zone::Battlefield
    );

    let mut land_engine = modal_engine(425_011);
    let land = inject_permanent_on_battlefield(&mut land_engine, 1, "island");
    let forest = inject_library_card(&mut land_engine, 1, "forest");
    let slot = prepare_spell(&mut land_engine, "avengers_disassembled");
    land_engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(land))]))
        .expect("cast the land mode");
    land_engine
        .apply_command(0, &pass())
        .expect("caster passes");
    let parked = land_engine
        .apply_command(1, &pass())
        .expect("land destroyed and its controller is offered the search");
    assert_eq!(land_engine.state.objects[&land].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&parked).expect("optional search branch");
    assert_eq!(
        choice.deciding_player_id, 1,
        "the land's controller chooses"
    );
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((choice.min, choice.max), (0, 1));

    let searching = land_engine
        .apply_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("the controller elects to search");
    let choice = find_resolution_choice(&searching).expect("private basic-land search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!(choice.deciding_player_id, 1);
    assert!(choice.candidate_object_ids.contains(&forest));
    land_engine
        .apply_command(1, &submit_resolution_choice(vec![forest]))
        .expect("find the basic land");
    let found = land_engine.state.objects[&forest].clone();
    assert_eq!(found.zone, Zone::Battlefield);
    assert_eq!(found.controller, 1, "the searching player controls it");
    assert!(found.tapped, "the printed basic enters tapped");
    assert!(land_engine.state.pending_resolution.is_none());
}

#[test]
fn issue_425_decoy_ploy_returns_only_printed_graveyard_subtypes() {
    let mut engine = modal_engine(425_020);
    let villain = inject_graveyard_card(&mut engine, 0, "common_crook");
    let hero = inject_graveyard_card(&mut engine, 0, "amateur_hero");
    let plain = inject_graveyard_card(&mut engine, 0, "grizzly_bears");

    let slot = prepare_spell(&mut engine, "decoy_ploy");
    assert!(
        engine
            .apply_command(
                0,
                &cast_modal_spell(slot, vec![(0, graveyard_target(plain))])
            )
            .is_err(),
        "a card without the Villain subtype is not a legal target"
    );
    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(0, graveyard_target(villain))]),
        )
        .expect("return the Villain card");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&villain].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&villain));

    let slot = prepare_spell(&mut engine, "decoy_ploy");
    assert!(
        engine
            .apply_command(
                0,
                &cast_modal_spell(slot, vec![(1, graveyard_target(plain))])
            )
            .is_err(),
        "a card without the Hero subtype is not a legal target"
    );
    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, graveyard_target(hero))]),
        )
        .expect("return the Hero card");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&hero].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&hero));
}

#[test]
fn issue_425_epic_fight_doubles_both_characteristics_and_fights() {
    let mut double_engine = modal_engine(425_030);
    let target = inject_creature_on_battlefield(&mut double_engine, 0, "grizzly_bears");
    let slot = prepare_spell(&mut double_engine, "epic_fight");
    double_engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_object(target))]))
        .expect("cast the doubling mode");
    resolve_entire_stack_two_player(&mut double_engine);
    assert_eq!(double_engine.effective_power(target), Some(4));
    assert_eq!(double_engine.effective_toughness(target), Some(4));

    let mut fight_engine = modal_engine(425_031);
    let ours = inject_creature_on_battlefield(&mut fight_engine, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut fight_engine, 1, "grizzly_bears");
    let slot = prepare_spell(&mut fight_engine, "epic_fight");
    fight_engine
        .apply_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(
                    1,
                    vec![permanent_target(ours, 0), permanent_target(theirs, 1)],
                )],
            ),
        )
        .expect("cast the fight mode");
    resolve_entire_stack_two_player(&mut fight_engine);
    assert_eq!(fight_engine.state.objects[&ours].zone, Zone::Graveyard);
    assert_eq!(fight_engine.state.objects[&theirs].zone, Zone::Graveyard);
}

#[test]
fn issue_425_pinecone_strike_exiles_lethal_and_destroys_artifact_tokens() {
    let mut lethal_engine = modal_engine(425_040);
    let lethal = inject_creature_on_battlefield(&mut lethal_engine, 1, "grizzly_bears");
    let slot = prepare_spell(&mut lethal_engine, "pinecone_strike");
    lethal_engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_object(lethal))]))
        .expect("cast the damage mode");
    resolve_entire_stack_two_player(&mut lethal_engine);
    assert_eq!(
        lethal_engine.state.objects[&lethal].zone,
        Zone::Exile,
        "CR 614.1a: the lethal creature is exiled instead of dying"
    );

    let mut token_engine = modal_engine(425_041);
    let token = inject_permanent_on_battlefield(&mut token_engine, 1, "clue");
    let sword = inject_permanent_on_battlefield(&mut token_engine, 1, "short_sword");
    let slot = prepare_spell(&mut token_engine, "pinecone_strike");
    assert!(
        token_engine
            .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(sword))]),)
            .is_err(),
        "a non-token artifact is not a legal artifact-token target"
    );
    token_engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(token))]))
        .expect("destroy the Clue token");
    resolve_entire_stack_two_player(&mut token_engine);
    assert!(
        !token_engine.state.objects.contains_key(&token),
        "CR 111.7: the destroyed token ceases to exist"
    );
    assert_eq!(token_engine.state.objects[&sword].zone, Zone::Battlefield);
}

#[test]
fn issue_425_scour_for_scrap_searches_and_returns_artifacts() {
    let mut search_engine = modal_engine(425_050);
    let sword = inject_library_card(&mut search_engine, 0, "short_sword");
    let bear = inject_library_card(&mut search_engine, 0, "grizzly_bears");
    let slot = prepare_spell(&mut search_engine, "scour_for_scrap");
    search_engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, vec![])]))
        .expect("cast the search mode");
    search_engine
        .apply_command(0, &pass())
        .expect("caster passes");
    let parked = search_engine
        .apply_command(1, &pass())
        .expect("the search parks for the caster");
    let choice = find_resolution_choice(&parked).expect("library search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!(choice.deciding_player_id, 0);
    assert!(choice.candidate_object_ids.contains(&sword));
    assert!(
        !choice.candidate_object_ids.contains(&bear),
        "only artifact cards are candidates"
    );
    let completion = search_engine
        .apply_command(0, &submit_resolution_choice(vec![sword]))
        .expect("find the artifact");
    let reveal = completion
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .expect("the printed search reveals the found artifact");
    assert_eq!(reveal.cards[0].object_id, sword);
    assert_eq!(search_engine.state.objects[&sword].zone, Zone::Hand);

    let mut return_engine = modal_engine(425_051);
    let artifact = inject_graveyard_card(&mut return_engine, 0, "short_sword");
    let creature = inject_graveyard_card(&mut return_engine, 0, "grizzly_bears");
    let slot = prepare_spell(&mut return_engine, "scour_for_scrap");
    assert!(
        return_engine
            .apply_command(
                0,
                &cast_modal_spell(slot, vec![(1, graveyard_target(creature))]),
            )
            .is_err(),
        "a non-artifact card in the graveyard is not a legal target"
    );
    return_engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, graveyard_target(artifact))]),
        )
        .expect("return the artifact card");
    resolve_entire_stack_two_player(&mut return_engine);
    assert_eq!(return_engine.state.objects[&artifact].zone, Zone::Hand);
    assert!(return_engine.state.players[0].hand.contains(&artifact));
}

#[test]
fn issue_425_choose_both_modes_resolve_in_combination() {
    let mut decoy_engine = modal_engine(425_060);
    let villain = inject_graveyard_card(&mut decoy_engine, 0, "common_crook");
    let hero = inject_graveyard_card(&mut decoy_engine, 0, "amateur_hero");
    let slot = prepare_spell(&mut decoy_engine, "decoy_ploy");
    decoy_engine
        .apply_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(0, graveyard_target(villain)), (1, graveyard_target(hero))],
            ),
        )
        .expect("cast both Decoy Ploy modes");
    resolve_entire_stack_two_player(&mut decoy_engine);
    assert!(decoy_engine.state.players[0].hand.contains(&villain));
    assert!(decoy_engine.state.players[0].hand.contains(&hero));

    let mut avengers_engine = modal_engine(425_061);
    let ours = inject_creature_on_battlefield(&mut avengers_engine, 0, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut avengers_engine, 1, "island");
    let forest = inject_library_card(&mut avengers_engine, 1, "forest");
    let slot = prepare_spell(&mut avengers_engine, "avengers_disassembled");
    avengers_engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(0, vec![]), (1, target_object(land))]),
        )
        .expect("cast both Avengers Disassembled modes");
    avengers_engine
        .apply_command(0, &pass())
        .expect("caster passes");
    let parked = avengers_engine
        .apply_command(1, &pass())
        .expect("sweep resolves and the search parks");
    // Resolution is suspended at the search, so lethal state-based actions have not run yet;
    // check the applied damage now and the departure after the search completes.
    assert_eq!(avengers_engine.state.objects[&ours].damage, 3);
    assert_eq!(avengers_engine.state.objects[&land].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&parked).expect("optional search branch");
    assert_eq!(choice.deciding_player_id, 1);
    let searching = avengers_engine
        .apply_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("the land's controller searches");
    let choice = find_resolution_choice(&searching).expect("library search");
    assert!(choice.candidate_object_ids.contains(&forest));
    avengers_engine
        .apply_command(1, &submit_resolution_choice(vec![forest]))
        .expect("find the basic land");
    assert_eq!(
        avengers_engine.state.objects[&ours].zone,
        Zone::Graveyard,
        "the lethally damaged creature dies once resolution completes"
    );
    assert_eq!(
        avengers_engine.state.objects[&forest].zone,
        Zone::Battlefield
    );
    assert!(avengers_engine.state.objects[&forest].tapped);
}
