//! Actual-card scenarios for the queue-v35 up-to-two target pump cohort.
//!
//! Expectations are printed Oracle values. CR 115.1/115.3 allow zero, one or
//! two distinct targets for "up to two target creatures"; CR 611.2a/514.2 end
//! the pump at cleanup and CR 613.4c applies it.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump: (i32, i32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "dauntless_onslaught",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            pump: (2, 2),
        },
        Case {
            id: "mischief_and_mayhem",
            mana: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            pump: (4, 4),
        },
        Case {
            id: "nahiris_stoneblades",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            pump: (2, 0),
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
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
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

fn cast_at(e: &mut GameEngine, case: &Case, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q35_two_target_pump_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(35_080_001);
    let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
    let second = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
    cast_at(
        &mut e,
        case,
        [target_object(first), target_object(second)].concat(),
    );
    assert_eq!(
        (e.effective_power(first), e.effective_toughness(first)),
        (Some(7), Some(7)),
        "Dauntless Onslaught pumps the first target"
    );
    assert_eq!(
        (e.effective_power(second), e.effective_toughness(second)),
        (Some(7), Some(7)),
        "Dauntless Onslaught pumps the second target"
    );
}

#[test]
fn q35_two_target_pump_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(35_080_002);
    let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
    let second = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
    cast_at(
        &mut e,
        case,
        [target_object(first), target_object(second)].concat(),
    );
    assert_eq!(
        (e.effective_power(first), e.effective_toughness(first)),
        (Some(9), Some(9)),
        "Mischief and Mayhem pumps the first target"
    );
    assert_eq!(
        (e.effective_power(second), e.effective_toughness(second)),
        (Some(9), Some(9)),
        "Mischief and Mayhem pumps the second target"
    );
}

#[test]
fn q35_two_target_pump_cards_pump_each_chosen_creature_once() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_081_000 + index as u64);
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        let second = inject_creature_with_stats(&mut e, 1, "savannah_lions", 3, 3);
        let bystander = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_at(
            &mut e,
            case,
            [target_object(first), target_object(second)].concat(),
        );
        assert_eq!(
            (e.effective_power(first), e.effective_toughness(first)),
            (
                Some((5 + case.pump.0) as u32),
                Some((5 + case.pump.1) as u32)
            ),
            "{}: first printed pump",
            case.id
        );
        assert_eq!(
            (e.effective_power(second), e.effective_toughness(second)),
            (
                Some((3 + case.pump.0) as u32),
                Some((3 + case.pump.1) as u32)
            ),
            "{}: second printed pump",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: an untargeted creature is unaffected",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(first), e.effective_toughness(first)),
            (Some(5), Some(5)),
            "{}: the pump ends with the turn",
            case.id
        );
    }
}

#[test]
fn q35_two_target_pump_cards_accept_zero_one_or_two_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_082_000 + index as u64);
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        cast_at(&mut e, case, vec![]);
        assert_eq!(
            (e.state.players[1].life, e.effective_power(first)),
            (20, Some(5)),
            "{}: zero targets is legal and pumps nothing",
            case.id
        );

        let mut e = engine(35_082_100 + index as u64);
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        cast_at(&mut e, case, target_object(first));
        assert_eq!(
            (e.effective_power(first), e.effective_toughness(first)),
            (
                Some((5 + case.pump.0) as u32),
                Some((5 + case.pump.1) as u32)
            ),
            "{}: one target gets the printed pump",
            case.id
        );

        let mut e = engine(35_082_200 + index as u64);
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(
                0,
                &cast_spell(slot, [target_object(first), target_object(first)].concat()),
            )
            .is_err(),
            "{}: the same creature cannot fill both target slots",
            case.id
        );
    }
}
