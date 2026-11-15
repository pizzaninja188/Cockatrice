//! Actual-card scenarios for the reviewed blue flying-grant cohort.
//!
//! Expectations are printed Oracle values. CR 702.9a-b make flying an evasion
//! ability, CR 602.2 activates the printed `{U}` ability, CR 611.2a applies the
//! grant until end of turn, and CR 514.2 ends it during cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "gust-skimmer",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
        },
        Case {
            id: "killer_whale",
            cast: ManaGift {
                u: 2,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 5,
        },
        Case {
            id: "dukhara_peafowl",
            cast: ManaGift {
                c: 4,
                ..Default::default()
            },
            power: 2,
            toughness: 4,
        },
        Case {
            id: "manta_riders",
            cast: ManaGift {
                u: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "whiptongue_frog",
            cast: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 3,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("forest", &["grizzly_bears", "storm_crow", "coral_merfolk"]),
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
        deck_with("island", &[card]),
        deck_with("forest", &["grizzly_bears", "storm_crow", "coral_merfolk"]),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
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

fn cast_creature(e: &mut GameEngine, id: &str, mana: ManaGift) -> u32 {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    semantic::accepted(e, 0, &cast_spell(hand_slot, vec![]));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cast cost");
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{id}: resolves onto the battlefield"
    );
    slot
}

fn activate_flying(e: &mut GameEngine, creature: u32, id: &str) {
    give_mana(
        e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    apply_ability(e, 0, creature, 0, vec![]).expect("activate flying");
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cost consumed");
}

#[test]
fn blue_gain_flying_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_370_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        assert_eq!(e.effective_power(creature), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: no printed flying",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn blue_gain_flying_batch_cards_gain_flying_only_after_activation() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_371_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: ground creature before activation",
            case.id
        );
        activate_flying(&mut e, creature, case.id);
        assert!(
            e.effective_has_keyword(creature, Keyword::Flying),
            "{}: flying after activation",
            case.id
        );
        assert!(
            !e.state.objects[&creature].tapped,
            "{}: the grant does not tap",
            case.id
        );
    }
}

#[test]
fn blue_gain_flying_batch_cards_allow_a_ground_blocker_only_before_activation() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut before = combat_engine(9_372_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut before, 0, case.id);
        let ground = inject_creature_on_battlefield(&mut before, 1, "coral_merfolk");
        before
            .apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the ground attacker");
        before
            .apply_scenario_command(0, &pass())
            .expect("active pass declare attackers");
        before
            .apply_scenario_command(1, &pass())
            .expect("defender reaches blockers");
        before
            .apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: ground,
                }]),
            )
            .expect("a ground creature blocks before flying is granted");
        give_mana(
            &mut before,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        apply_ability(&mut before, 0, attacker, 0, vec![])
            .expect("activate flying after the block");
        resolve_entire_stack_two_player(&mut before);
        assert_eq!(
            before.state.combat.as_ref().expect("combat").blockers[&attacker],
            [ground],
            "{}: gaining flying after being blocked does not unblock",
            case.id
        );

        let mut after = combat_engine(9_373_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut after, 0, case.id);
        let ground = inject_creature_on_battlefield(&mut after, 1, "coral_merfolk");
        let flyer = inject_creature_on_battlefield(&mut after, 1, "storm_crow");
        after
            .apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        activate_flying(&mut after, attacker, case.id);
        after
            .apply_scenario_command(0, &pass())
            .expect("active pass declare attackers");
        after
            .apply_scenario_command(1, &pass())
            .expect("defender reaches blockers");
        let rejected = after.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: ground,
            }]),
        );
        assert!(
            matches!(rejected, Err(tricerules_core::EngineError::Illegal(_))),
            "{}: a ground creature cannot block the flyer",
            case.id
        );
        after
            .apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: flyer,
                }]),
            )
            .expect("a flying creature can block");
    }
}

#[test]
fn blue_gain_flying_batch_cards_lose_flying_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_374_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        activate_flying(&mut e, creature, case.id);
        assert!(
            e.effective_has_keyword(creature, Keyword::Flying),
            "{}",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: CR 514.2 ends the grant",
            case.id
        );
    }
}

#[test]
fn blue_gain_flying_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_375_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: flying needs its exact mana",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: the rejected activation grants nothing",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
