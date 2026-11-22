//! Actual-card scenarios for the queue-v22 "entry, target creature can't block
//! this turn" cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a makes the entry trigger;
//! CR 509.1b rejects a declaration of blockers that disobeys the restriction;
//! CR 611.2a/514.2 expire it at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    generic: u32,
    red: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "crossway_vampire",
            generic: 1,
            red: 2,
        },
        Case {
            id: "goblin_shortcutter",
            generic: 1,
            red: 1,
        },
        Case {
            id: "unstoppable_ogre",
            generic: 2,
            red: 1,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card, "grizzly_bears", "unsummon"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
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

fn choose_trigger_target(targets: Vec<tricerules_proto::ruled::v1::TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn cast_and_target(e: &mut GameEngine, case: &Case, target: u32) {
    give_mana(
        e,
        0,
        ManaGift {
            r: case.red,
            c: case.generic,
            ..Default::default()
        },
    );
    ensure_in_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the entry creature");
    pass_both_players(e);
    assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the target creature");
    resolve_entire_stack_two_player(e);
}

fn attack_and_reach_blockers(e: &mut GameEngine, attacker: u32) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_both_players(e);
}

#[test]
fn etb_cant_block_batch_canary_cards_publish_the_restriction() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "goblin_shortcutter" | "crossway_vampire"))
    {
        let mut e = engine(11_300_000, case.id);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_and_target(&mut e, &case, bear);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 1, bear).contains(&"Can't block".to_string()),
            "{}: the until-end-of-turn restriction is published",
            case.id
        );
    }
}

#[test]
fn etb_cant_block_batch_cards_reject_the_target_as_a_blocker() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_310_000 + index as u64, case.id);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let other = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_and_target(&mut e, &case, bear);
        attack_and_reach_blockers(&mut e, attacker);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.blocker_id == other),
            "{}: the unrestricted creature keeps the block step open",
            case.id
        );
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.blocker_id == bear),
            "{}: the restricted creature is not offered as a blocker",
            case.id
        );
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: bear,
                }]),
            )
            .is_err(),
            "{}: the engine rejects the forbidden block",
            case.id
        );
    }
}

#[test]
fn etb_cant_block_batch_cards_may_target_their_own_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_320_000 + index as u64, case.id);
        let own_bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast_and_target(&mut e, &case, own_bear);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, own_bear)
                .contains(&"Can't block".to_string()),
            "{}: the printed text allows any creature, including your own",
            case.id
        );
    }
}

#[test]
fn etb_cant_block_batch_cards_expire_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_330_000 + index as u64, case.id);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_and_target(&mut e, &case, bear);
        end_active_turn(&mut e, 0);
        assert!(
            !zone_view_rules_annotation_labels(&mut e, 1, bear)
                .contains(&"Can't block".to_string()),
            "{}: the restriction expired at cleanup",
            case.id
        );
    }
}

#[test]
fn etb_cant_block_batch_cards_fizzle_when_the_target_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_340_000 + index as u64, case.id);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: case.red,
                c: case.generic,
                ..Default::default()
            },
        );
        ensure_in_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the entry creature");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(bear)))
            .expect("choose the target");
        ensure_in_hand(&mut e, 0, "unsummon");
        give_mana(
            &mut e,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let unsummon = hand_index_for_card(&e, 0, "unsummon");
        e.apply_scenario_command(0, &cast_spell(unsummon, target_object(bear)))
            .expect("return the chosen creature in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&bear].zone,
            tricerules_core::Zone::Hand,
            "{}",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: the trigger left the stack",
            case.id
        );
        for seat in 0..2 {
            for oid in e.state.players[seat].battlefield.clone() {
                assert!(
                    !zone_view_rules_annotation_labels(&mut e, seat, oid)
                        .contains(&"Can't block".to_string()),
                    "{}: the fizzled trigger creates no restriction",
                    case.id
                );
            }
        }
    }
}
