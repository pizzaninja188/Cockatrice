//! Issue #358 — the five reviewed condition, cost, and graveyard Standard cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path. CR
//! 614.1c/122.6/119.3 govern the opponent-life-loss conditional entry counter; CR
//! 119.4/602.2b/602.5b/514.2 the pay-two-life once-per-turn self pump and its cleanup expiry; CR
//! 601.2f/700.4 the conditional {3} cost reduction; CR 113.6b/602.2/701.13/121.2 the graveyard
//! exile-self draw-then-lose-life; and CR 603.6c/603.10a/700.4/115.1 the self-inclusive
//! creature-or-artifact leaves-the-battlefield drain.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{AbilitySourceZone, RuledCommand};

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn zone_ability(
    engine: &GameEngine,
    source: u32,
    source_zone: AbilitySourceZone,
    ability_index: u32,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: source_zone as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            ..Default::default()
        })),
    }
}

#[track_caller]
fn hand_action_reduction(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(engine, player, card_id) as u32;
    let batch = engine.initial_response_batch();
    let actions = &batch.legal_by_player[&(player as i32)].hand_actions;
    actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| {
            panic!(
                "missing cast action for {card_id}; published: {:?}",
                actions
                    .iter()
                    .map(|action| (action.hand_index, action.card_name.as_str()))
                    .collect::<Vec<_>>()
            )
        })
        .generic_cost_reduction
}

#[test]
fn issue_358_frilled_sparkshooter_counts_only_an_opponents_life_loss() {
    // Nobody lost life: no counter.
    let mut none = main1_engine(358_001, &["frilled_sparkshooter"], &[]);
    let sparks = move_ready_to_battlefield(&mut none, 0, "frilled_sparkshooter");
    resolve_entire_stack_two_player(&mut none);
    assert_eq!(
        none.state.objects[&sparks].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    // An opponent lost life: one counter.
    let decks = Some(vec![
        deck_with("swamp", &["frilled_sparkshooter", "bump_in_the_night"]),
        deck_with("forest", &[]),
    ]);
    let mut opponent_loss = GameEngine::new(358_002, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut opponent_loss);
    ensure_in_hand(&mut opponent_loss, 0, "bump_in_the_night");
    give_mana(
        &mut opponent_loss,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let bump = hand_index_for_card(&opponent_loss, 0, "bump_in_the_night");
    opponent_loss
        .apply_command(0, &cast_spell(bump, target_player(1)))
        .expect("cast Bump in the Night at the opponent");
    resolve_entire_stack_two_player(&mut opponent_loss);
    assert_eq!(opponent_loss.state.players[1].life, 17);
    let sparks = move_ready_to_battlefield(&mut opponent_loss, 0, "frilled_sparkshooter");
    resolve_entire_stack_two_player(&mut opponent_loss);
    assert_eq!(
        opponent_loss.state.objects[&sparks].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "CR 614.1c/122.6: an opponent's earlier life loss adds the counter"
    );
    assert_eq!(opponent_loss.effective_power(sparks), Some(4));

    // Only the controller lost life: the opponent scope leaves the counter off.
    let decks = Some(vec![
        deck_with("swamp", &["frilled_sparkshooter", "decode_transmissions"]),
        deck_with("forest", &[]),
    ]);
    let mut controller_loss = GameEngine::new(358_003, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut controller_loss);
    ensure_in_hand(&mut controller_loss, 0, "decode_transmissions");
    give_mana(
        &mut controller_loss,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let decode = hand_index_for_card(&controller_loss, 0, "decode_transmissions");
    controller_loss
        .apply_command(0, &cast_spell(decode, vec![]))
        .expect("cast Decode Transmissions");
    resolve_entire_stack_two_player(&mut controller_loss);
    assert_eq!(controller_loss.state.players[0].life, 18);
    assert_eq!(controller_loss.state.players[1].life, 20);
    let sparks = move_ready_to_battlefield(&mut controller_loss, 0, "frilled_sparkshooter");
    resolve_entire_stack_two_player(&mut controller_loss);
    assert_eq!(
        controller_loss.state.objects[&sparks].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "CR 119.3: the controller's own life loss is not an opponent's"
    );
}

#[test]
fn issue_358_desolation_prowler_pays_life_once_per_turn_and_expires() {
    let mut engine = main1_engine(358_004, &["desolation_prowler"], &[]);
    let prowler = move_ready_to_battlefield(&mut engine, 0, "desolation_prowler");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(prowler), Some(2));

    apply_ability(&mut engine, 0, prowler, 0, vec![]).expect("pay two life");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 18,
        "CR 119.4: two life was paid as the activation cost"
    );
    assert_eq!(engine.effective_power(prowler), Some(4));
    assert_eq!(engine.effective_toughness(prowler), Some(4));

    assert!(
        apply_ability(&mut engine, 0, prowler, 0, vec![]).is_err(),
        "CR 602.5b: the printed once-each-turn restriction rejects a second activation"
    );
    assert_eq!(engine.state.players[0].life, 18);

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(prowler),
        Some(2),
        "CR 514.2: the pump expires at cleanup"
    );
}

#[test]
fn issue_358_dreaded_bat_cloud_reduces_only_after_a_creature_dies() {
    let mut engine = main1_engine(
        358_005,
        &["dreaded_bat-cloud", "murder", "grizzly_bears"],
        &[],
    );
    ensure_in_hand(&mut engine, 0, "dreaded_bat-cloud");
    let doomed = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);

    assert_eq!(
        hand_action_reduction(&mut engine, 0, "dreaded_bat-cloud"),
        0,
        "no creature has died this turn"
    );

    ensure_in_hand(&mut engine, 0, "murder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(&engine, 0, "murder");
    engine
        .apply_command(0, &cast_spell(murder, target_object(doomed)))
        .expect("cast Murder at the controlled creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&doomed].zone, Zone::Graveyard);
    assert_eq!(engine.state.turn_history.current.creatures_died, 1);

    assert_eq!(
        hand_action_reduction(&mut engine, 0, "dreaded_bat-cloud"),
        3,
        "CR 601.2f/700.4: a creature died this turn, so the generic cost drops by three"
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "dreaded_bat-cloud");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Dreaded Bat-Cloud for {1}{B}");
    resolve_entire_stack_two_player(&mut engine);
    battlefield_object_for_card(&engine, 0, "dreaded_bat-cloud");
}

#[test]
fn issue_358_faerie_dreamthief_exiles_itself_to_draw_and_lose() {
    let mut engine = main1_engine(358_006, &["faerie_dreamthief", "grizzly_bears"], &[]);
    let thief = inject_graveyard_card(&mut engine, 0, "faerie_dreamthief");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 2,
            ..Default::default()
        },
    );
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();

    engine
        .apply_command(
            0,
            &zone_ability(&engine, thief, AbilitySourceZone::Graveyard, 0),
        )
        .expect("activate the graveyard ability");
    assert_eq!(
        engine.state.objects[&thief].zone,
        Zone::Exile,
        "CR 701.13: exile-self is paid as the activation cost"
    );
    assert!(!engine.state.players[0].graveyard.contains(&thief));
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 19,
        "CR 119.3: the controller loses 1 life"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
}

#[test]
fn issue_358_susurian_voidborn_drains_when_a_controlled_creature_dies() {
    let mut engine = main1_engine(358_007, &["susurian_voidborn", "grizzly_bears"], &[]);
    inject_creature_with_stats(&mut engine, 0, "susurian_voidborn", 2, 2);
    let friendly = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);

    engine
        .state
        .objects
        .get_mut(&friendly)
        .expect("bears")
        .damage = 2;
    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("lethal-damage state-based action");
    assert_eq!(engine.state.objects[&friendly].zone, Zone::Graveyard);
    assert_eq!(engine.state.pending_triggers.len(), 1);

    engine
        .apply_command(0, &choose_trigger_targets(target_player(1)))
        .expect("choose target opponent");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        (engine.state.players[0].life, engine.state.players[1].life),
        (21, 19),
        "CR 603.6c/700.4: the controller drains the target opponent"
    );
}

#[test]
fn issue_358_susurian_voidborn_drains_when_the_source_dies() {
    let mut engine = main1_engine(358_008, &["susurian_voidborn"], &[]);
    let voidborn = inject_creature_with_stats(&mut engine, 0, "susurian_voidborn", 2, 2);

    engine
        .state
        .objects
        .get_mut(&voidborn)
        .expect("voidborn")
        .damage = 2;
    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("lethal-damage state-based action");
    assert_eq!(engine.state.objects[&voidborn].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "the observation is self-inclusive"
    );

    engine
        .apply_command(0, &choose_trigger_targets(target_player(1)))
        .expect("choose target opponent");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        (engine.state.players[0].life, engine.state.players[1].life),
        (21, 19)
    );
}

#[test]
fn issue_358_susurian_voidborn_drains_when_a_controlled_artifact_leaves() {
    let mut engine = main1_engine(358_009, &["susurian_voidborn", "disenchant"], &[]);
    inject_creature_with_stats(&mut engine, 0, "susurian_voidborn", 2, 2);
    let prism = inject_permanent_on_battlefield(&mut engine, 0, "prophetic_prism");
    ensure_in_hand(&mut engine, 0, "disenchant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "disenchant");
    engine
        .apply_command(0, &cast_spell(slot, target_object(prism)))
        .expect("cast Disenchant at the controlled artifact");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine
        .apply_command(1, &pass())
        .expect("opponent passes and Disenchant resolves");
    assert_eq!(engine.state.objects[&prism].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "CR 603.6c: an artifact you control leaving is observed through the union filter"
    );

    engine
        .apply_command(0, &choose_trigger_targets(target_player(1)))
        .expect("choose target opponent");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        (engine.state.players[0].life, engine.state.players[1].life),
        (21, 19)
    );
}

#[test]
fn issue_358_susurian_voidborn_observes_a_controlled_token_dying() {
    // CR 111.7/700.4/603.6c: a dying token is a creature put into the graveyard from the
    // battlefield, so the union observation fires before the token ceases to exist.
    let mut engine = main1_engine(358_012, &["susurian_voidborn"], &[]);
    inject_creature_with_stats(&mut engine, 0, "susurian_voidborn", 2, 2);
    let token = inject_creature_with_stats(&mut engine, 0, "soldier_w_1_1", 1, 1);
    assert!(
        engine.state.objects[&token].is_token(),
        "the fixture must be a real token"
    );
    engine
        .state
        .objects
        .get_mut(&token)
        .expect("soldier token")
        .damage = 1;
    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("lethal-damage state-based action");
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "CR 700.4: a materialized token dying is observed"
    );

    engine
        .apply_command(0, &choose_trigger_targets(target_player(1)))
        .expect("choose target opponent");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        (engine.state.players[0].life, engine.state.players[1].life),
        (21, 19)
    );
}

#[test]
fn issue_358_susurian_voidborn_excludes_nonqualifying_departures() {
    let mut engine = main1_engine(358_010, &["susurian_voidborn"], &["grizzly_bears"]);
    inject_creature_with_stats(&mut engine, 0, "susurian_voidborn", 2, 2);
    let opposing = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);

    engine
        .state
        .objects
        .get_mut(&opposing)
        .expect("bears")
        .damage = 2;
    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("lethal-damage state-based action");
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Graveyard);
    assert!(
        engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty(),
        "CR 115 / 603.6c: an opponent's creature is outside the controlled union"
    );
    assert_eq!(
        (engine.state.players[0].life, engine.state.players[1].life),
        (20, 20)
    );
}

#[test]
fn issue_358_scenarios_reach_main1() {
    let engine = main1_engine(358_011, &[], &[]);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
}
