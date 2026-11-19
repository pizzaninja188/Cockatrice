//! Actual-card scenarios for the queue-v38 flying sweep cohort.
//!
//! Expectations are printed Oracle values. CR 120.3 deals damage to every matching creature
//! without targeting; CR 702.9 defines the flying characteristic; CR 704.5g applies lethal
//! damage; CR 514.2 clears marked damage at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "gale_force",
            mana: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            amount: 5,
        },
        Case {
            id: "needle_storm",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            amount: 4,
        },
        Case {
            id: "squall",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            amount: 2,
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
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
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

fn cast_sweep(e: &mut GameEngine, case: &Case) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q38_flyer_sweep_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(38_040_001);
    let own_flier = inject_creature_with_stats(&mut e, 0, "wind_drake", 2, 2);
    let their_flier = inject_creature_with_stats(&mut e, 1, "wind_drake", 1, 1);
    let ground = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_sweep(&mut e, case);
    assert_eq!(e.state.objects[&own_flier].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&their_flier].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&ground].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&ground].damage, 0);
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q38_flyer_sweep_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(38_040_002);
    let victim = inject_creature_with_stats(&mut e, 0, "serra_angel", 5, 5);
    let survivor = inject_creature_with_stats(&mut e, 1, "serra_angel", 6, 6);
    cast_sweep(&mut e, case);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&survivor].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&survivor].damage, 5);
}

#[test]
fn q38_flyer_sweep_cards_damage_every_flying_creature_only() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_041_000 + index as u64);
        let own_ground = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let their_ground = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
        let own_flier = inject_creature_with_stats(&mut e, 0, "serra_angel", 10, 10);
        let their_flier = inject_creature_with_stats(&mut e, 1, "serra_angel", 10, 10);
        cast_sweep(&mut e, case);
        for flier in [own_flier, their_flier] {
            assert_eq!(e.state.objects[&flier].damage, case.amount, "{}", case.id);
        }
        for ground in [own_ground, their_ground] {
            assert_eq!(e.state.objects[&ground].damage, 0, "{}", case.id);
        }
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn q38_flyer_sweep_cards_clear_marked_damage_at_cleanup() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_042_000 + index as u64);
        let flier = inject_creature_with_stats(&mut e, 0, "serra_angel", 10, 10);
        cast_sweep(&mut e, case);
        assert_eq!(e.state.objects[&flier].damage, case.amount, "{}", case.id);
        end_active_turn(&mut e, 0);
        assert_eq!(e.state.objects[&flier].damage, 0, "{}", case.id);
    }
}
