//! Actual-card scenarios for the reviewed `Destroy target tapped creature.` family.
//!
//! Expectations are printed Oracle values. CR 115.1a/601.2c and the printed
//! tapped restriction choose the target; CR 608.2b revalidates both the creature
//! and its tapped state; CR 701.8 moves the destroyed creature to its owner's
//! graveyard.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "death_stroke",
            mana: ManaGift {
                b: 2,
                ..Default::default()
            },
        },
        Case {
            id: "swift_response",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "assassinate",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "take_vengeance",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "vengeance",
            mana: ManaGift {
                w: 1,
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
    e.state.stack[0].id
}

#[test]
fn destroy_tapped_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(6_030_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&creature).expect("creature").tapped = true;
        cast_at(&mut e, &case, creature);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Graveyard,
            "{}: the tapped creature is destroyed",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&creature),
            "{}: the creature is in its owner's graveyard",
            case.id
        );
        assert!(
            !e.state.players[0].graveyard.contains(&creature),
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn destroy_tapped_batch_cards_reject_illegal_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(6_031_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{}: an untapped creature is not a legal target",
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
        assert!(!e.state.objects[&creature].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn destroy_tapped_batch_cards_handle_stale_or_expired_state() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(6_032_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&creature).expect("creature").tapped = true;
        let spell = cast_at(&mut e, &case, creature);
        e.state.objects.get_mut(&creature).expect("creature").tapped = false;
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}: the creature untapped before resolution and survives",
            case.id
        );
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
