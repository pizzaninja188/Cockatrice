//! Actual-card scenarios for the reviewed `Destroy target land.` family.
//!
//! Expectations are printed Oracle values. CR 115.1a and 601.2c require one land
//! target as the spell is cast; CR 608.2b revalidates that target; CR 701.8 moves
//! the destroyed land to its owner's graveyard.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "sinkhole",
            mana: ManaGift {
                b: 2,
                ..Default::default()
            },
        },
        Case {
            id: "rain_of_tears",
            mana: ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "ice_storm",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "stone_rain",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "winters_grasp",
            mana: ManaGift {
                g: 2,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "craterize",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "volcanic_upheaval",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[card]), deck_with("forest", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    ensure_in_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error}", case.id));
    assert_eq!(e.state.stack.len(), 1, "{}: spell is on the stack", case.id);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Battlefield,
        "{}: casting does not destroy the target",
        case.id
    );
    e.state.stack[0].id
}

#[test]
fn destroy_land_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(2_030_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        cast_at(&mut e, &case, land);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Graveyard,
            "{}: the target land is destroyed",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&land),
            "{}: the land is in its owner's graveyard",
            case.id
        );
        assert!(!e.state.players[0].graveyard.contains(&land), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn destroy_land_batch_cards_reject_illegal_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(2_031_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{}: a creature is not a land target",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected cast spends no mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn destroy_land_batch_cards_handle_stale_or_expired_state() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(2_032_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        let spell = cast_at(&mut e, &case, land);
        e.state.players[1].battlefield.retain(|oid| *oid != land);
        e.state.objects.get_mut(&land).expect("land").zone = Zone::Exile;
        e.state.players[1].exile.push(land);
        *e.state.zone_change_generation.entry(land).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&land].zone, Zone::Exile, "{}", case.id);
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the spell did not resolve",
            case.id
        );
        assert!(e.state.players[0].graveyard.contains(&spell), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
