//! Actual-card scenarios for the reviewed First strike cohort.
//!
//! Expectations are printed Oracle values. CR 702.7 and 510.4 govern the first combat damage step.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "coiled_tinviper",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            body: (2, 1),
        },
        Case {
            id: "head_of_security",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
        },
        Case {
            id: "ramirez_depietro",
            mana: ManaGift {
                u: 1,
                b: 2,
                c: 3,
                ..Default::default()
            },
            body: (4, 3),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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

fn attack_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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

fn cast_creature(e: &mut GameEngine, case: &Case) -> u32 {
    let slot = inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast the creature");
    let pool = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "{}: exact cast cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn first_strike_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_510_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, &case);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_power(creature),
            Some(case.body.0),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.body.1),
            "{}",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, Keyword::FirstStrike),
            "{}: printed keyword",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn first_strike_batch_cards_deal_damage_in_the_first_strike_step() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = attack_engine(9_511_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::FirstStrikeDamage,
            "{}: first-strike step",
            case.id
        );
        assert_eq!(
            e.state.players[1].life,
            20 - i32::try_from(case.body.0).expect("power fits"),
            "{}: damage in the first-strike step",
            case.id
        );
        pass_both_players(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - i32::try_from(case.body.0).expect("power fits"),
            "{}: no additional regular-step damage",
            case.id
        );
    }
}

#[test]
fn first_strike_batch_cards_kill_a_vanilla_blocker_before_return_damage() {
    for (index, case) in cases().into_iter().enumerate() {
        let decks = Some(vec![
            deck_with("forest", &[case.id]),
            deck_with("forest", &["grizzly_bears"]),
        ]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            9_512_000 + index as u64,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_declare_attackers(&mut e);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the first-strike attacker");
        e.apply_scenario_command(0, &pass())
            .expect("active pass attackers");
        e.apply_scenario_command(1, &pass())
            .expect("defender pass attackers");
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare the vanilla blocker");
        e.apply_scenario_command(0, &pass())
            .expect("active pass blockers");
        e.apply_scenario_command(1, &pass())
            .expect("defender pass blockers");
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::FirstStrikeDamage,
            "{}: first-strike step",
            case.id
        );
        assert_ne!(
            e.state.objects.get(&blocker).map(|object| object.zone),
            Some(Zone::Battlefield),
            "{}: the blocker dies to first-strike damage",
            case.id
        );
        assert_eq!(
            e.state.objects[&attacker].damage, 0,
            "{}: the first-strike attacker takes no return damage",
            case.id
        );
    }
}
