//! Reviewed ETB/dies creature scenarios: Shinra Reinforcements, Sunshower Druid, Vault Plunderer,
//! Fierce Empath, Prickly Pair, Head of the Homestead, Agents of HYDRA and Hero in Training.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21. Every expectation is the
//! reviewed printed Oracle behavior. Governance: CR 603.6a (entry triggers), 603.7/700.4 (dies
//! triggers), 121.1 (draw), 701.13 (mill), 119.3 (life gain/loss), 122.1 (counters), 111.10 (token
//! definitions), 701.18/701.23 (search), and 608.2c (printed instruction order).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ResolutionChoiceDecision, TargetRefKind};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

fn choose_trigger_player(player_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id: player_id,
                group_index: 0,
                kind: TargetRefKind::Player as i32,
                ..Default::default()
            }],
        })),
    }
}

fn submit_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(
            tricerules_proto::ruled::v1::SubmitResolutionChoice {
                chosen_object_ids: Vec::new(),
                selected_branch_index: index,
                decision: ResolutionChoiceDecision::SelectBranch as i32,
                ..Default::default()
            },
        )),
    }
}

fn etb_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast_creature(e: &mut GameEngine, card: &str) -> u32 {
    let source = inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, vec![]));
    pass_both_players(e);
    source
}

#[test]
fn issue_etb_shinra_reinforcements() {
    let mut e = etb_engine(710_001);
    let life_before = e.state.players[0].life;
    let library_before = e.state.players[0].library.len();
    let graveyard_before = e.state.players[0].graveyard.len();
    cast_creature(&mut e, "shinra_reinforcements");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].library.len(), library_before - 3);
    assert_eq!(e.state.players[0].graveyard.len(), graveyard_before + 3);
    assert_eq!(e.state.players[0].life, life_before + 3);
}

#[test]
fn issue_etb_sunshower_druid() {
    let mut e = etb_engine(710_002);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let life_before = e.state.players[0].life;
    cast_creature(&mut e, "sunshower_druid");
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "entry trigger waits for a target"
    );
    e.apply_command(0, &choose_trigger_target(target))
        .expect("choose the target creature");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target]
            .counters
            .get(&tricerules_cards::CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert_eq!(e.effective_power(target), Some(3));
    assert_eq!(e.state.players[0].life, life_before + 1);

    // If the target is illegal at resolution the trigger doesn't resolve and no life is gained.
    let mut fizzle = etb_engine(710_010);
    let target = inject_creature_with_stats(&mut fizzle, 0, "grizzly_bears", 2, 2);
    let life_before = fizzle.state.players[0].life;
    cast_creature(&mut fizzle, "sunshower_druid");
    fizzle
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose the target creature");
    fizzle.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != target);
    fizzle.state.players[0].hand.push(target);
    fizzle.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;
    *fizzle
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(fizzle.state.players[0].life, life_before);
}

#[test]
fn issue_etb_vault_plunderer() {
    let mut e = etb_engine(710_003);
    let hand_before = e.state.players[1].hand.len();
    let life_before = e.state.players[1].life;
    cast_creature(&mut e, "vault_plunderer");
    assert_eq!(e.state.pending_triggers.len(), 1);
    e.apply_command(0, &choose_trigger_player(1))
        .expect("target the opponent player");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].hand.len(), hand_before + 1);
    assert_eq!(e.state.players[1].life, life_before - 1);

    // "Target player" also allows the caster.
    let mut own = etb_engine(710_012);
    let hand_before = own.state.players[0].hand.len();
    let life_before = own.state.players[0].life;
    cast_creature(&mut own, "vault_plunderer");
    own.apply_command(0, &choose_trigger_player(0))
        .expect("target the caster");
    resolve_entire_stack_two_player(&mut own);
    assert_eq!(own.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(own.state.players[0].life, life_before - 1);
}

#[test]
fn issue_etb_fierce_empath() {
    let mut e = etb_engine(710_004);
    let big = inject_library_card(&mut e, 0, "craw_wurm");
    let small = inject_library_card(&mut e, 0, "grizzly_bears");
    cast_creature(&mut e, "fierce_empath");
    e.apply_command(0, &pass()).expect("trigger resolves");
    let batch = e.apply_command(1, &pass()).expect("park for the search");
    let prompt = find_resolution_choice(&batch).expect("optional search prompt");
    assert_eq!(prompt.choice_kind(), ChoiceKind::ResolutionBranch);
    let batch = semantic::accepted(&mut e, 0, &submit_branch(0));
    let choice = find_resolution_choice(&batch).expect("library search");
    assert!(choice.candidate_object_ids.contains(&big));
    assert!(!choice.candidate_object_ids.contains(&small));
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![big]));
    assert!(e.state.players[0].hand.contains(&big));

    // Declining the optional search moves nothing.
    let mut decline = etb_engine(710_011);
    let big = inject_library_card(&mut decline, 0, "craw_wurm");
    let hand_before = decline.state.players[0].hand.len();
    let library_before = decline.state.players[0].library.len();
    cast_creature(&mut decline, "fierce_empath");
    decline.apply_command(0, &pass()).expect("trigger resolves");
    let batch = decline
        .apply_command(1, &pass())
        .expect("park for the search");
    let prompt = find_resolution_choice(&batch).expect("optional search prompt");
    assert_eq!(prompt.choice_kind(), ChoiceKind::ResolutionBranch);
    semantic::accepted(
        &mut decline,
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    );
    assert!(!decline.state.players[0].hand.contains(&big));
    assert_eq!(decline.state.players[0].library.len(), library_before);
    assert_eq!(decline.state.players[0].hand.len(), hand_before);
}

#[test]
fn issue_etb_prickly_pair() {
    let mut e = etb_engine(710_005);
    cast_creature(&mut e, "prickly_pair");
    resolve_entire_stack_two_player(&mut e);
    let mercenary = battlefield_object_for_card(&e, 0, "mercenary_r_1_1");
    assert_eq!(e.state.objects[&mercenary].card_id, "mercenary_r_1_1");
}

#[test]
fn issue_etb_head_of_the_homestead() {
    let mut e = etb_engine(710_006);
    cast_creature(&mut e, "head_of_the_homestead");
    resolve_entire_stack_two_player(&mut e);
    let rabbits: Vec<u32> = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| e.state.objects[object_id].card_id == "rabbit_w_1_1")
        .collect();
    assert_eq!(rabbits.len(), 2);
    for rabbit in rabbits {
        let characteristics = e.characteristics(rabbit).expect("token characteristics");
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(1), Some(1))
        );
        assert_eq!(characteristics.colors, vec![tricerules_cards::Color::White]);
    }
}

#[test]
fn issue_etb_agents_of_hydra() {
    let mut e = etb_engine(710_007);
    let hydra = cast_creature(&mut e, "agents_of_hydra");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&hydra].zone, Zone::Battlefield);
    e.state.objects.get_mut(&hydra).expect("source").damage = 99;
    e.apply_command(0, &pass())
        .expect("state-based actions destroy the source");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&hydra].zone, Zone::Graveyard);
    let villain = battlefield_object_for_card(&e, 0, "villain_b_2_1_menace");
    assert_eq!(e.state.objects[&villain].card_id, "villain_b_2_1_menace");
}

#[test]
fn issue_etb_hero_in_training() {
    // No other Hero: draw a card, no life gain.
    let mut alone = etb_engine(710_008);
    let hand_before = alone.state.players[0].hand.len();
    let life_before = alone.state.players[0].life;
    cast_creature(&mut alone, "hero_in_training");
    resolve_entire_stack_two_player(&mut alone);
    assert_eq!(alone.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(alone.state.players[0].life, life_before);

    // Another Hero: draw a card and gain two life.
    let mut pair = etb_engine(710_009);
    inject_creature_on_battlefield(&mut pair, 0, "hero_in_training");
    let hand_before = pair.state.players[0].hand.len();
    let life_before = pair.state.players[0].life;
    cast_creature(&mut pair, "hero_in_training");
    resolve_entire_stack_two_player(&mut pair);
    assert_eq!(pair.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(pair.state.players[0].life, life_before + 2);
}
