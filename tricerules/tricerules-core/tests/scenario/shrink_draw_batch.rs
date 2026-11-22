//! Actual-card scenarios for the reviewed shrink / draw cohort.
//!
//! Expectations are printed Oracle values. CR 115.1a/601.2c choose the target as the
//! spell is cast; CR 613.4c applies the negative P/T effect; CR 704.5f destroys a
//! creature with lethal damage; CR 121.1 draws the top card; CR 608.2b makes an
//! illegal only target fizzle the spell with no card drawn.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: i32,
    toughness: i32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "afflict",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: -1,
            toughness: -1,
        },
        Case {
            id: "befuddle",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: -4,
            toughness: 0,
        },
        Case {
            id: "bewilder",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: -3,
            toughness: 0,
        },
        Case {
            id: "shocking_grasp",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            power: -2,
            toughness: 0,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card, "storm_crow", "hill_giant"]),
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

fn cast_at_target(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    let cast_slot = e.state.players[0].hand[hand_slot];
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    cast_slot
}

#[test]
fn shrink_draw_batch_cards_shrink_the_target_and_draw_exactly_one() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_670_000 + index as u64, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
        let slot = cast_at_target(&mut e, &case, target);
        let hand_before: Vec<u32> = e.state.players[0].hand.to_vec();
        let top = e.state.players[0]
            .library
            .front()
            .copied()
            .expect("library top");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.power).max(0) as u32),
                Some((2 + case.toughness).max(0) as u32)
            ),
            "{}: exact printed shrink",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(3), Some(3)),
            "{}: only the chosen creature",
            case.id
        );
        let mut expected_hand: Vec<u32> =
            hand_before.into_iter().filter(|oid| *oid != slot).collect();
        expected_hand.push(top);
        assert_eq!(
            e.state.players[0].hand, expected_hand,
            "{}: draws exactly the top card",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
        assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn shrink_draw_batch_cards_kill_a_creature_with_lethal_negative_toughness() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_671_000 + index as u64, case.id);
        let toughness = if case.toughness < 0 {
            (-case.toughness) as u32
        } else {
            4
        };
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, toughness);
        cast_at_target(&mut e, &case, target);
        resolve_entire_stack_two_player(&mut e);
        if case.toughness < 0 {
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Graveyard,
                "{}: zero-or-less toughness is lethal",
                case.id
            );
            assert!(
                e.state.players[1].graveyard.contains(&target),
                "{}: owner's graveyard",
                case.id
            );
        } else {
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Battlefield,
                "{}: a power-only shrink does not kill",
                case.id
            );
            assert_eq!(
                e.effective_power(target),
                Some((4 + case.power).max(0) as u32),
                "{}: power floors at zero",
                case.id
            );
        }
    }
}

#[test]
fn shrink_draw_batch_cards_fizzle_a_stale_target_without_drawing() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_672_000 + index as u64, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let slot = cast_at_target(&mut e, &case, target);
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        let hand_before: Vec<u32> = e.state.players[0].hand.to_vec();
        let library_before = e.state.players[0].library.clone();
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand, hand_before,
            "{}: a fizzled spell draws no card",
            case.id
        );
        assert_eq!(
            e.state.players[0].library, library_before,
            "{}: no card leaves the library",
            case.id
        );
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: the spell goes to its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn shrink_draw_batch_cards_reject_a_noncreature_target_without_paying() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_673_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let pool_before = pool(&e);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let rejected_slot = e.state.players[0].hand[hand_slot];
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal target",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand[hand_slot], rejected_slot,
            "{}",
            case.id
        );
        assert_eq!(pool(&e), pool_before, "{}: no mana spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
