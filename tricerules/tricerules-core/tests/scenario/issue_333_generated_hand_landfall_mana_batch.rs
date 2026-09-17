//! Issue #333 — the six reviewed hand, landfall, and mana Standard cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 701.9/701.20 govern the mandatory target-opponent public reveal and nonland discard and
//! its chooser-private/public boundary; CR 603.6a governs the Landfall and creature-enter
//! triggers and their controller scope; CR 605.1a/605.3b govern the {T} mana abilities and the
//! color choice made at activation; CR 613.4c governs the live count-scaled self +1/+0.

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

/// Activate `ability_index` of `permanent_id`, choosing `mana_option_index` for a mana ability.
fn activate_mana_option(
    engine: &GameEngine,
    permanent_id: u32,
    ability_index: u32,
    mana_option_index: u32,
) -> RuledCommand {
    let mut command = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: permanent_id,
            ability_index,
            mana_option_index,
            ..Default::default()
        })),
    };
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.expected_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&permanent_id)
        .copied()
        .unwrap_or(0);
    command
}

#[test]
fn issue_333_pilfer_publicly_reveals_and_discards_only_a_nonland() {
    let mut engine = main1_engine(333_001, &["pilfer"], &[]);
    let cleared: Vec<_> = engine.state.players[1].hand.drain(..).collect();
    engine.state.players[1].library.extend(cleared);
    let nonland = inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    let land = inject_card_into_hand(&mut engine, 1, "forest");
    relocate_to_hand(&mut engine, 0, "pilfer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "pilfer");
    engine
        .apply_command(0, &cast_spell(spell, target_player(1)))
        .expect("cast Pilfer");
    engine.apply_command(0, &pass()).expect("caster passes");
    let parked = engine
        .apply_command(1, &pass())
        .expect("opponent passes and Pilfer parks for the choice");

    let choice = find_resolution_choice(&parked).expect("opponent-hand choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::OpponentHand);
    assert_eq!(choice.deciding_player_id, 0, "the caster chooses");
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(
        choice
            .public_reveal
            .as_ref()
            .map(|reveal| reveal.zone_owner_player_id),
        Some(1),
        "CR 701.20: the revealed hand window is public"
    );
    let nonland_index = choice
        .candidate_object_ids
        .iter()
        .position(|object_id| *object_id == nonland)
        .expect("nonland candidate");
    let land_index = choice
        .candidate_object_ids
        .iter()
        .position(|object_id| *object_id == land)
        .expect("land remains visible");
    assert!(choice.candidate_selectable[nonland_index]);
    assert!(
        !choice.candidate_selectable[land_index],
        "the nonland filter publishes the land as visible but ineligible"
    );
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .expect("pending hand choice")
            .continuation,
        ResolutionContinuation::HandChoice { hand_choice, .. }
            if hand_choice.action == HandCardAction::Discard
    ));

    engine
        .apply_command(0, &submit_resolution_choice(vec![nonland]))
        .expect("choose the nonland card");
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
    assert!(engine.state.players[1].hand.contains(&land));
    assert_eq!(engine.state.players[1].hand.len(), 1);
}

#[test]
fn issue_333_pilfer_rejects_ineligible_and_stale_choices_atomically() {
    let mut engine = main1_engine(333_002, &["pilfer"], &[]);
    let cleared: Vec<_> = engine.state.players[1].hand.drain(..).collect();
    engine.state.players[1].library.extend(cleared);
    let nonland = inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    let land = inject_card_into_hand(&mut engine, 1, "forest");
    relocate_to_hand(&mut engine, 0, "pilfer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "pilfer");
    engine
        .apply_command(0, &cast_spell(spell, target_player(1)))
        .expect("cast Pilfer");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine
        .apply_command(1, &pass())
        .expect("resolution parks for the choice");

    let hand_before = engine.state.players[1].hand.clone();
    assert!(
        engine
            .apply_command(0, &submit_resolution_choice(vec![land]))
            .is_err(),
        "a land is not a legal nonland selection"
    );
    assert_eq!(engine.state.players[1].hand, hand_before);
    assert!(engine.state.pending_resolution.is_some());

    *engine
        .state
        .zone_change_generation
        .entry(nonland)
        .or_default() += 1;
    assert!(
        engine
            .apply_command(0, &submit_resolution_choice(vec![nonland]))
            .is_err(),
        "a stale generation must not satisfy the parked choice"
    );
    assert_eq!(engine.state.players[1].hand, hand_before);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Hand);
    assert!(engine.state.pending_resolution.is_some());
}

#[test]
fn issue_333_pilfer_requires_its_target_opponent() {
    let mut engine = main1_engine(333_003, &["pilfer"], &[]);
    relocate_to_hand(&mut engine, 0, "pilfer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "pilfer");
    assert!(
        engine.apply_command(0, &cast_spell(spell, vec![])).is_err(),
        "CR 601.2c: Pilfer cannot be cast without its mandatory opponent target"
    );
    assert_eq!(
        engine.state.objects[&engine.state.players[0].hand[spell]].zone,
        Zone::Hand
    );
}

#[test]
fn issue_333_pilfer_fizzles_when_its_target_opponent_has_left_the_game() {
    let mut engine = main1_engine(333_004, &["pilfer"], &[]);
    let cleared: Vec<_> = engine.state.players[1].hand.drain(..).collect();
    engine.state.players[1].library.extend(cleared);
    let nonland = inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    relocate_to_hand(&mut engine, 0, "pilfer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "pilfer");
    engine
        .apply_command(0, &cast_spell(spell, target_player(1)))
        .expect("cast Pilfer");

    engine.state.players[1].has_lost = true;
    let resolved = engine
        .apply_command(0, &pass())
        .expect("the spell resolves with an illegal player target");
    assert!(
        find_resolution_choice(&resolved).is_none(),
        "an illegal target fizzles without a hand choice"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Hand);
    assert_eq!(engine.state.players[1].hand.len(), 1);
}

#[test]
fn issue_333_eumidian_landfall_gains_one_life_for_controller_lands_only() {
    let mut engine = main1_engine(333_005, &["eumidian_terrabotanist"], &[]);
    move_ready_to_battlefield(&mut engine, 0, "eumidian_terrabotanist");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);

    // A controlled land entering triggers exactly once.
    move_ready_to_battlefield(&mut engine, 0, "forest");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "landfall trigger is on the stack"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(engine.state.players[1].life, 20);

    // Another player's land entering does not satisfy the controller scope.
    move_ready_to_battlefield(&mut engine, 1, "forest");
    assert!(
        engine.state.stack.is_empty(),
        "CR 603.6a: landfall is controller-scoped"
    );
    assert_eq!(engine.state.players[0].life, 21);

    // Two controlled lands entering before resolution collect two separate triggers.
    move_ready_to_battlefield(&mut engine, 0, "forest");
    move_ready_to_battlefield(&mut engine, 0, "forest");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "each land entry is a separate landfall event"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 23);
}

#[test]
fn issue_333_loporrit_scout_pumps_itself_only_for_another_controlled_creature() {
    let mut engine = main1_engine(
        333_006,
        &["loporrit_scout", "grizzly_bears"],
        &["grizzly_bears"],
    );
    let scout = move_ready_to_battlefield(&mut engine, 0, "loporrit_scout");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        0,
        "the source's own entry is excluded by \"another\""
    );
    assert_eq!(engine.effective_power(scout), Some(3));

    move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(scout), Some(4));
    assert_eq!(engine.effective_toughness(scout), Some(3));

    move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    assert!(
        engine.state.stack.is_empty(),
        "an opponent's creature does not satisfy \"you control\""
    );
    assert_eq!(engine.effective_power(scout), Some(4));

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(scout),
        Some(3),
        "CR 514.2: the pump expires at cleanup"
    );
    assert_eq!(engine.effective_toughness(scout), Some(2));
}

#[test]
fn issue_333_two_and_three_mana_abilities_produce_one_chosen_color() {
    let mut engine = main1_engine(333_007, &["transdimensional_bovine"], &[]);
    let bovine = move_ready_to_battlefield(&mut engine, 0, "transdimensional_bovine");
    let blue_two = activate_mana_option(&engine, bovine, 0, 1);
    engine.apply_command(0, &blue_two).expect("tap for {U}{U}");
    assert_eq!(engine.state.players[0].mana_pool.blue, 2);
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
    assert!(engine.state.objects[&bovine].tapped);
    let blue_again = activate_mana_option(&engine, bovine, 0, 1);
    assert!(
        engine.apply_command(0, &blue_again).is_err(),
        "an already-tapped source cannot pay the tap cost again"
    );

    let mut engine = main1_engine(333_008, &["gilded_lotus"], &[]);
    let lotus = move_ready_to_battlefield(&mut engine, 0, "gilded_lotus");
    let red_three = activate_mana_option(&engine, lotus, 0, 3);
    engine
        .apply_command(0, &red_three)
        .expect("tap for {R}{R}{R}");
    assert_eq!(engine.state.players[0].mana_pool.red, 3);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert!(engine.state.objects[&lotus].tapped);

    let bad_option = activate_mana_option(&engine, lotus, 0, 5);
    assert!(
        engine.apply_command(0, &bad_option).is_err(),
        "a fourth color option index is illegal"
    );
}

#[test]
fn issue_333_guidelight_synergist_tracks_artifacts_entering_and_leaving() {
    let mut engine = main1_engine(
        333_009,
        &["guidelight_synergist", "gilded_lotus"],
        &["gilded_lotus"],
    );
    let synergist = move_ready_to_battlefield(&mut engine, 0, "guidelight_synergist");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(synergist),
        Some(1),
        "the artifact creature counts itself"
    );
    assert_eq!(engine.effective_toughness(synergist), Some(4));

    let own_artifact = move_ready_to_battlefield(&mut engine, 0, "gilded_lotus");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(synergist), Some(2));

    move_ready_to_battlefield(&mut engine, 1, "gilded_lotus");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(synergist),
        Some(2),
        "only the controller's artifacts count"
    );

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != own_artifact);
    engine.state.players[0].graveyard.push(own_artifact);
    engine
        .state
        .objects
        .get_mut(&own_artifact)
        .expect("departed artifact")
        .zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(own_artifact)
        .or_default() += 1;
    assert_eq!(
        engine.effective_power(synergist),
        Some(1),
        "the live count re-evaluates as artifacts leave"
    );
}

#[test]
fn issue_333_scenarios_reach_main1() {
    let engine = main1_engine(333_010, &[], &[]);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
}
