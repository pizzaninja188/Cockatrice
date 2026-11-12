//! Actual-card scenarios for the reviewed Agonizing Syphon damage-and-life family.
//!
//! Expectations are printed Oracle values. CR 115.1 provides the single any-target;
//! CR 120.1/120.3 apply the damage; CR 119.3 gains the life; CR 608.2b fizzles the
//! whole spell when its only target is illegal, so no life is gained.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: i32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "agonizing_syphon",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            amount: 3,
        },
        Case {
            id: "essence_drain",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            amount: 3,
        },
        Case {
            id: "vampiric_feast",
            mana: ManaGift {
                b: 2,
                c: 5,
                ..Default::default()
            },
            amount: 4,
        },
        Case {
            id: "void_helix",
            mana: ManaGift {
                b: 2,
                c: 4,
                ..Default::default()
            },
            amount: 5,
        },
        Case {
            id: "warleaders_helix",
            mana: ManaGift {
                r: 1,
                w: 1,
                c: 2,
                ..Default::default()
            },
            amount: 4,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
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

fn cast_at(e: &mut GameEngine, case: &Case, targets: Vec<TargetRef>) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, targets))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    slot
}

#[test]
fn agonizing_syphon_batch_cards_deal_damage_to_a_player_and_gain_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(790_000 + index as u64);
        let p0_before = e.state.players[0].life;
        let p1_before = e.state.players[1].life;
        let slot = cast_at(&mut e, &case, target_player(1));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            p1_before - case.amount,
            "{}: printed damage",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            p0_before + case.amount,
            "{}: printed life gain",
            case.id
        );
        assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn agonizing_syphon_batch_cards_deal_damage_to_a_creature_and_gain_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(791_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let p0_before = e.state.players[0].life;
        let slot = cast_at(&mut e, &case, target_object(target));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: lethal damage",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            p0_before + case.amount,
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn agonizing_syphon_batch_cards_fizzle_without_life_gain() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(792_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let p0_before = e.state.players[0].life;
        let p1_before = e.state.players[1].life;
        let slot = cast_at(&mut e, &case, target_object(target));
        e.state.players[1]
            .battlefield
            .retain(|candidate| *candidate != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[0].life, p0_before, "{}: no life", case.id);
        assert_eq!(e.state.players[1].life, p1_before, "{}: no damage", case.id);
        assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn agonizing_syphon_batch_cards_reject_a_land_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(793_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let pool_before = mana_pool(&e);
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
                .is_err(),
            "{}: a land is not an any-target",
            case.id
        );
        assert_eq!(e.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert_eq!(mana_pool(&e), pool_before, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

fn mana_pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
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
