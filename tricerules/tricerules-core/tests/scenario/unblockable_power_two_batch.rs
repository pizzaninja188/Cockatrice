//! Actual-card scenarios for the reviewed power-two unblockable cohort.
//!
//! Expectations are printed Oracle values. CR 115.1 makes the creature a target;
//! CR 509.1a-b check evasion as blockers are declared; CR 608.2b rejects a target
//! whose power is above two on resolution; CR 611.2a keeps the restriction for
//! the turn. The pinned Pathmaker Initiate and Goblin Tunneler rulings restate
//! the resolution and post-resolution cases.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::EngineError;
use tricerules_proto::ruled::v1::BlockPair;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "crafty_pathmage",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 1),
        },
        Case {
            id: "dwarven_nomad",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 1),
        },
        Case {
            id: "dwarven_warriors",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 1),
        },
        Case {
            id: "goblin_tunneler",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
        },
        Case {
            id: "pathmaker_initiate",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 1),
        },
    ]
}

fn canaries() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|case| matches!(case.id, "crafty_pathmage" | "goblin_tunneler"))
        .collect()
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "giant_growth"]),
        deck_with("island", &[]),
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

fn combat_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "giant_growth"]),
        deck_with("island", &["grizzly_bears"]),
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
    advance_to_declare_attackers(&mut e);
    e
}

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let hand = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
    let creature = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (Some(case.body.0), Some(case.body.1)),
        "{}: printed body",
        case.id
    );
    creature
}

fn activate_unblockable(e: &mut GameEngine, source: u32, target: u32, id: &str) {
    apply_ability(e, 0, source, 0, target_object(target))
        .unwrap_or_else(|error| panic!("{id}: activate the tap ability: {error:?}"));
    assert!(e.state.objects[&source].tapped, "{id}: tap cost paid");
    resolve_entire_stack_two_player(e);
    assert!(e.state.stack.is_empty(), "{id}: the ability resolved");
}

#[test]
fn unblockable_power_two_batch_canary_cards_make_a_power_two_creature_unblockable() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = combat_engine(930_000 + index as u64, case.id);
        let helper = move_ready_to_battlefield(&mut e, 0, case.id);
        let attacker = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        activate_unblockable(&mut e, helper, attacker, case.id);
        pass_both_players(&mut e);
        let blocker = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        assert!(
            matches!(
                e.apply_scenario_command(
                    1,
                    &declare_blockers(vec![BlockPair {
                        attacker_id: attacker,
                        blocker_id: blocker
                    }]),
                ),
                Err(EngineError::Illegal(_))
            ),
            "{}: the power-two attacker cannot be blocked",
            case.id
        );
    }
}

#[test]
fn unblockable_power_two_batch_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(931_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}",
            case.id
        );
        assert!(!e.state.objects[&creature].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn unblockable_power_two_batch_cards_reject_a_target_above_power_two() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(932_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let big = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 3, 3);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(big)).is_err(),
            "{}: a power-three creature is not a legal target",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: rejected activation does not tap",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn unblockable_power_two_batch_cards_keep_the_restriction_after_a_later_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = combat_engine(933_000 + index as u64, case.id);
        let helper = move_ready_to_battlefield(&mut e, 0, case.id);
        let attacker = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        activate_unblockable(&mut e, helper, attacker, case.id);
        ensure_in_hand(&mut e, 0, "giant_growth");
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "giant_growth");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(attacker)))
            .expect("pump the unblockable attacker");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(attacker), Some(5), "{}: pumped", case.id);
        pass_both_players(&mut e);
        let blocker = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        assert!(
            matches!(
                e.apply_scenario_command(
                    1,
                    &declare_blockers(vec![BlockPair {
                        attacker_id: attacker,
                        blocker_id: blocker
                    }]),
                ),
                Err(EngineError::Illegal(_))
            ),
            "{}: the ruling keeps the creature unblockable after a later pump",
            case.id
        );
    }
}

#[test]
fn unblockable_power_two_batch_cards_do_not_undo_an_existing_block() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = combat_engine(934_000 + index as u64, case.id);
        let helper = move_ready_to_battlefield(&mut e, 0, case.id);
        let attacker = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let blocker = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        pass_both_players(&mut e);
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare the block before the ability");
        activate_unblockable(&mut e, helper, attacker, case.id);
        assert_eq!(
            e.state.combat.as_ref().expect("combat").blockers[&attacker],
            [blocker],
            "{}: the existing block is not undone",
            case.id
        );
    }
}

#[test]
fn unblockable_power_two_batch_cards_reject_activation_when_the_source_is_tapped() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(935_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        activate_unblockable(&mut e, source, target, case.id);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(target)).is_err(),
            "{}: a tapped source cannot pay the tap cost",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
