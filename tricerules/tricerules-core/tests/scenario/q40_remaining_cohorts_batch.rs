//! Actual-card scenarios for queue-v40 shrink, drain, enchantment removal, attack-pump and
//! dies-trigger cohorts.
//!
//! Expectations are printed Oracle values. CR 603.6a/603.3d/115.1 cover entry triggers and their
//! targets; CR 700.4/603.6c cover dies triggers; CR 508.1m/603.2c cover attack triggers;
//! CR 119.3 covers life loss/gain; CR 121.1 covers drawing; and CR 611.2a/514.2 ends pumps.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind};

fn choose_permanent_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(
            tricerules_proto::ruled::v1::ruled_command::Cmd::ChooseTriggerTarget(
                ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![TargetRef {
                        object_id,
                        group_index: 0,
                        kind: TargetRefKind::Permanent as i32,
                        ..Default::default()
                    }],
                },
            ),
        ),
    }
}

fn choose_player_target(player_index: usize, e: &GameEngine) -> RuledCommand {
    RuledCommand {
        cmd: Some(
            tricerules_proto::ruled::v1::ruled_command::Cmd::ChooseTriggerTarget(
                ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![TargetRef {
                        object_id: e.state.players[player_index].id as u32,
                        group_index: 0,
                        kind: TargetRefKind::Player as i32,
                        ..Default::default()
                    }],
                },
            ),
        ),
    }
}

fn engine(seed: u64, card: &str, basic: &str) -> GameEngine {
    let decks = Some(vec![deck_with(basic, &[card]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn combat_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_declare_attackers(&mut e);
    e
}

fn resolve_targeted_entry(e: &mut GameEngine, card: &str, target: u32) {
    move_ready_to_battlefield(e, 0, card);
    answer_trigger_order_in_engine_order(e);
    semantic::accepted(e, 0, &choose_permanent_target(target));
    resolve_entire_stack_two_player(e);
}

fn kill_with_murder(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    give_mana(
        e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(source)))
        .expect("cast Murder");
    pass_both_players(e);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
}

fn advance_one_turn(e: &mut GameEngine) {
    let turn = e.state.turn;
    for _ in 0..120 {
        let (actor, command) = match e.state.cleanup_discard_player {
            Some(player) => {
                let index = e.state.player_idx(player).expect("cleanup player");
                let excess = e.state.players[index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (e.state.priority_player_id(), pass()),
        };
        e.apply_scenario_command(actor, &command)
            .expect("pass through the end of the turn");
        if e.state.turn != turn {
            return;
        }
    }
    panic!("turn did not advance");
}

#[test]
fn q40_etb_shrink_canary_simplest() {
    let mut e = engine(40_040_001, "blister_beetle", "swamp");
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    resolve_targeted_entry(&mut e, "blister_beetle", target);
    assert_eq!(e.effective_power(target), Some(1));
    assert_eq!(e.effective_toughness(target), Some(1));
}

#[test]
fn q40_etb_shrink_canary_hardest() {
    let mut e = engine(40_040_002, "keening_banshee", "swamp");
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
    resolve_targeted_entry(&mut e, "keening_banshee", target);
    assert_eq!(e.effective_power(target), Some(1));
    assert_eq!(e.effective_toughness(target), Some(1));
}

#[test]
fn q40_etb_shrink_cards_apply_the_printed_penalty_until_end_of_turn() {
    let cases = [
        ("blister_beetle", -1, -1),
        ("farbog_boneflinger", -2, -2),
        ("keening_banshee", -2, -2),
    ];
    for (index, (card, dp, dt)) in cases.into_iter().enumerate() {
        let mut e = engine(40_041_000 + index as u64, card, "swamp");
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 4, 4);
        resolve_targeted_entry(&mut e, card, target);
        assert_eq!(e.effective_power(target), Some((4 + dp) as u32), "{card}");
        assert_eq!(
            e.effective_toughness(target),
            Some((4 + dt) as u32),
            "{card}"
        );
        advance_one_turn(&mut e);
        assert_eq!(e.effective_power(target), Some(4), "{card}: pump expires");
        assert_eq!(
            e.effective_toughness(target),
            Some(4),
            "{card}: pump expires"
        );
    }
}

#[test]
fn q40_etb_shrink_cards_must_choose_a_creature_when_none_other_exists() {
    for (index, card) in ["blister_beetle", "farbog_boneflinger", "keening_banshee"]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(40_042_000 + index as u64, card, "swamp");
        move_ready_to_battlefield(&mut e, 0, card);
        answer_trigger_order_in_engine_order(&mut e);
        let source = battlefield_object_for_card(&e, 0, card);
        semantic::accepted(&mut e, 0, &choose_permanent_target(source));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{card}: the mandatory self-target is lethal"
        );
    }
}

#[test]
fn q40_etb_drain_cards_apply_printed_life_changes() {
    let cases = [
        ("highway_robber", 2),
        ("dakmor_ghoul", 2),
        ("vampire_sovereign", 3),
    ];
    for (index, (card, amount)) in cases.into_iter().enumerate() {
        let mut e = engine(40_050_000 + index as u64, card, "swamp");
        let opponent_life = e.state.players[1].life;
        let library = e.state.players[1].library.len();
        move_ready_to_battlefield(&mut e, 0, card);
        answer_trigger_order_in_engine_order(&mut e);
        let choose_opponent = choose_player_target(1, &e);
        semantic::accepted(&mut e, 0, &choose_opponent);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life,
            20 + amount,
            "{card}: controller gains"
        );
        assert_eq!(
            e.state.players[1].life,
            opponent_life - amount,
            "{card}: target opponent loses"
        );
        assert_eq!(e.state.players[1].library.len(), library, "{card}: no mill");
    }
}

#[test]
fn q40_etb_destroy_enchantment_cards_destroy_the_chosen_aura() {
    for (index, card) in ["monk_realist", "aven_cloudchaser", "cloudchaser_eagle"]
        .into_iter()
        .enumerate()
    {
        let mut e = if index % 2 == 0 {
            engine(40_060_000 + index as u64, card, "plains")
        } else {
            engine(40_060_000 + index as u64, card, "island")
        };
        let aura = inject_permanent_on_battlefield(&mut e, 1, "clachan_festival");
        resolve_targeted_entry(&mut e, card, aura);
        assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard, "{card}");
        assert!(
            e.state.pending_triggers.is_empty(),
            "{card}: the trigger resolved"
        );
    }
}

#[test]
fn q40_etb_destroy_enchantment_does_nothing_without_a_legal_target() {
    let mut e = engine(40_061_001, "monk_realist", "plains");
    move_ready_to_battlefield(&mut e, 0, "monk_realist");
    answer_trigger_order_in_engine_order(&mut e);
    assert!(
        e.state.stack.is_empty(),
        "no trigger without a legal target"
    );
    assert!(e.state.pending_triggers.is_empty());
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_object_for_card(&e, 0, "monk_realist"),
        e.state.players[0]
            .battlefield
            .iter()
            .copied()
            .find(|oid| e.state.objects[oid].card_id == "monk_realist")
            .unwrap()
    );
}

#[test]
fn q40_attack_self_pump_cards_pump_only_the_attacking_source() {
    let cases = [
        ("charging_griffin", 1, 1),
        ("ravenous_skirge", 2, 0),
        ("hungry_spriggan", 3, 3),
    ];
    for (index, (card, dp, dt)) in cases.into_iter().enumerate() {
        let mut e = combat_engine(40_070_000 + index as u64, card);
        let source = move_ready_to_battlefield(&mut e, 0, card);
        let base_power = e.effective_power(source).unwrap() as i32;
        let base_toughness = e.effective_toughness(source).unwrap() as i32;
        e.apply_scenario_command(0, &declare_attackers(vec![source]))
            .expect("declare the source as an attacker");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some((base_power + dp) as u32),
            "{card}"
        );
        assert_eq!(
            e.effective_toughness(source),
            Some((base_toughness + dt) as u32),
            "{card}"
        );

        let mut other = combat_engine(40_070_500 + index as u64, card);
        let unboosted = move_ready_to_battlefield(&mut other, 0, card);
        let other_power = other.effective_power(unboosted).unwrap();
        let other_toughness = other.effective_toughness(unboosted).unwrap();
        let bystander = inject_creature_on_battlefield(&mut other, 0, "grizzly_bears");
        other
            .apply_scenario_command(0, &declare_attackers(vec![bystander]))
            .expect("declare another attacker");
        resolve_entire_stack_two_player(&mut other);
        assert_eq!(
            other.effective_power(unboosted),
            Some(other_power),
            "{card}"
        );
        assert_eq!(
            other.effective_toughness(unboosted),
            Some(other_toughness),
            "{card}: the source did not attack"
        );
    }
}

#[test]
fn q40_dies_return_self_cards_return_to_their_owners_hand() {
    for (index, card) in [
        "mortus_strider",
        "immortal_phoenix",
        "shivan_phoenix",
        "weatherseed_treefolk",
        "endless_cockroaches",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(40_080_000 + index as u64, card, "swamp");
        let source = inject_creature_with_stats(&mut e, 0, card, 2, 2);
        kill_with_murder(&mut e, source);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&source].zone, Zone::Hand, "{card}");
        assert!(e.state.players[0].hand.contains(&source), "{card}");
    }
}

#[test]
fn q40_dies_return_self_does_not_return_a_card_that_left_the_graveyard() {
    let mut e = engine(40_081_001, "mortus_strider", "swamp");
    let source = inject_creature_with_stats(&mut e, 0, "mortus_strider", 1, 1);
    kill_with_murder(&mut e, source);
    e.state.players[0].graveyard.retain(|oid| *oid != source);
    e.state.players[0].exile.push(source);
    e.state.objects.get_mut(&source).unwrap().zone = Zone::Exile;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Exile);
    assert!(!e.state.players[0].hand.contains(&source));
}

#[test]
fn q40_dies_draw_cards_draw_one_and_leave_the_opponent_alone() {
    for (index, card) in ["riptide_crab", "malcators_watcher", "silverback_shaman"]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(40_090_000 + index as u64, card, "forest");
        let source = inject_creature_with_stats(&mut e, 0, card, 2, 2);
        let hand_before = e.state.players[0].hand.len();
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        kill_with_murder(&mut e, source);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[0].hand.len(), hand_before + 1, "{card}");
        assert_eq!(e.state.players[1].hand.len(), opponent_hand, "{card}");
        assert_eq!(e.state.players[1].library.len(), opponent_library, "{card}");
    }
}
