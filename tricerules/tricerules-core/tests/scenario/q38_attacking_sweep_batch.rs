//! Actual-card scenarios for the queue-v38 attacking-creature sweep cohort.
//!
//! Expectations are printed Oracle values. CR 508.1 makes the attacking designation; CR 120.3
//! deals damage to each attacking creature without targeting; CR 704.5g applies lethal damage;
//! CR 107.4f lets a {W/P} symbol be paid with 2 life, and CR 119.4 rejects that payment at 1 life.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::FlexPipPayment;

struct Case {
    id: &'static str,
    mana: ManaGift,
    pay_life: bool,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "marrow_shards",
            mana: ManaGift::default(),
            pay_life: true,
        },
        Case {
            id: "rain_of_blades",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            pay_life: false,
        },
        Case {
            id: "sandstorm",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            pay_life: false,
        },
    ]
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
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

fn advance_to_declare_attackers(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    for _ in 0..e.state.players.len() {
        if e.state.turn_step == TurnStep::DeclareAttackers {
            return;
        }
        let player = e.state.priority_player_id();
        e.apply_scenario_command(player, &pass())
            .expect("pass in beginning of combat");
    }
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
}

fn advance_to_declare_blockers(e: &mut GameEngine) {
    e.apply_scenario_command(0, &pass())
        .expect("active player passes after attackers");
    e.apply_scenario_command(1, &pass())
        .expect("defender passes after attackers");
    assert_eq!(e.state.turn_step, TurnStep::DeclareBlockers);
}

fn cast_sweep(e: &mut GameEngine, case: &Case, white_payment: bool) {
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    if e.state.priority_player_id() != 0 {
        e.apply_scenario_command(1, &pass())
            .expect("pass priority back to the active player");
    }
    if case.pay_life {
        let life_before = e.state.players[0].life;
        e.apply_scenario_command(
            0,
            &cast_spell_flex(
                slot,
                vec![],
                vec![FlexPipPayment {
                    pip_index: 0,
                    pay_life: true,
                    mana_color: 0,
                }],
            ),
        )
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
        assert_eq!(
            e.state.players[0].life,
            life_before - 2,
            "{}: the Phyrexian symbol was paid with 2 life",
            case.id
        );
    } else {
        give_mana(e, 0, gift(&case.mana));
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    }
    resolve_entire_stack_two_player(e);
    if white_payment {
        assert_eq!(
            {
                let p = &e.state.players[0].mana_pool;
                (p.white, p.blue, p.black, p.red, p.green, p.colorless)
            },
            (0, 0, 0, 0, 0, 0),
            "{}: exact mana",
            case.id
        );
    }
}

#[test]
fn q38_attacking_sweep_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(38_050_001);
    let attacker = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 1);
    let bystander = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    advance_to_declare_attackers(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    advance_to_declare_blockers(&mut e);
    cast_sweep(&mut e, case, true);
    assert_eq!(e.state.objects[&attacker].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&bystander].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&bystander].damage, 0);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q38_attacking_sweep_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(38_050_002);
    let attacker = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 1);
    advance_to_declare_attackers(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    advance_to_declare_blockers(&mut e);
    cast_sweep(&mut e, case, false);
    assert_eq!(e.state.objects[&attacker].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].life, 18,
        "Marrow Shards paid with 2 life"
    );
}

#[test]
fn q38_attacking_sweep_cards_damage_each_attacking_creature_only() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_051_000 + index as u64);
        let own_attacker = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 1);
        let own_bystander = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let defender = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        advance_to_declare_attackers(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![own_attacker]))
            .expect("declare attacker");
        advance_to_declare_blockers(&mut e);
        cast_sweep(&mut e, case, !case.pay_life);
        assert_eq!(
            e.state.objects[&own_attacker].zone,
            Zone::Graveyard,
            "{}: the attacking 2/1 dies",
            case.id
        );
        for other in [own_bystander, defender] {
            assert_eq!(
                e.state.objects[&other].zone,
                Zone::Battlefield,
                "{}: nonattacking creatures are untouched",
                case.id
            );
            assert_eq!(e.state.objects[&other].damage, 0, "{}", case.id);
        }
    }
}
