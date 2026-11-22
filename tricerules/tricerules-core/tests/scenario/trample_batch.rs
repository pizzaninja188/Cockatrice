//! Actual-card scenarios for the reviewed Trample cohort.
//!
//! Expectations are printed Oracle values. CR 702.19a-c govern lethal damage assignment and excess trample damage.
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
            id: "ebony_rhino",
            mana: ManaGift {
                c: 7,
                ..Default::default()
            },
            body: (4, 5),
        },
        Case {
            id: "gorn_captain",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            body: (4, 3),
        },
        Case {
            id: "rootbreaker_wurm",
            mana: ManaGift {
                g: 2,
                c: 5,
                ..Default::default()
            },
            body: (6, 6),
        },
        Case {
            id: "war_mammoth",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            body: (3, 3),
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
fn trample_batch_cards_cast_with_exact_printed_characteristics() {
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
            e.effective_has_keyword(creature, Keyword::Trample),
            "{}: printed keyword",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn trample_batch_cards_deal_unblocked_combat_damage_to_the_player() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = attack_engine(9_511_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - i32::try_from(case.body.0).expect("power fits"),
            "{}: unblocked combat damage",
            case.id
        );
        pass_both_players(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - i32::try_from(case.body.0).expect("power fits"),
            "{}: damage was dealt once",
            case.id
        );
    }
}

#[test]
fn trample_batch_cards_assign_lethal_and_trample_the_excess() {
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
            .expect("declare the trample attacker");
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
        assert!(
            e.state.combat.as_ref().unwrap().assign_combat_damage_phase,
            "{}: trample opens damage assignment",
            case.id
        );
        let life_before = e.state.players[1].life;
        let excess = case.body.0 - 2;
        let batch = e
            .apply_scenario_command(
                0,
                &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 2)], excess),
            )
            .expect("assign lethal and trample excess");
        assert_eq!(
            e.state.players[1].life,
            life_before - i32::try_from(excess).expect("excess fits"),
            "{}: excess damage reaches the player",
            case.id
        );
        let dead: Vec<u32> = permanents_moved_in(&batch)
            .iter()
            .map(|permanent| permanent.object_id)
            .collect();
        assert!(dead.contains(&blocker), "{}: the blocker dies", case.id);
        assert_eq!(
            e.state.objects[&blocker].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
    }
}
