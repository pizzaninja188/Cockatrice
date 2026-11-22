//! Actual-card scenarios for the queue-v28 +X/+0 pump cohort.
//!
//! Expectations are printed Oracle values. CR 107.3 chooses X as the spell is cast;
//! CR 115.1 chooses one creature target; CR 611.2a/613.4c apply the pump until end of
//! turn; CR 514.2 restores the printed body during cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::TurnStep;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "enrage",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "bloodcurdling_scream",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "howl_from_beyond",
            mana: ManaGift {
                b: 1,
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
    let decks = Some(vec![
        deck_with("mountain", &[card, "grizzly_bears", "savannah_lions"]),
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

fn cast_pump(e: &mut GameEngine, case: &Case, target: u32, x: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell_x(slot, target_object(target), x))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

fn advance_to_next_upkeep(e: &mut GameEngine) {
    let starting_turn = e.state.turn_instance;
    for _ in 0..240 {
        if e.state.turn_instance > starting_turn && e.state.turn_step == TurnStep::Upkeep {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance to the next upkeep");
    }
    panic!("turn advancement stalled before the next upkeep");
}

#[test]
fn x_pumps_batch_cards_add_x_power_and_no_toughness() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_050_000 + index as u64, case.id);
        let target = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        cast_pump(&mut e, &case, target, 3);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(5), Some(2)),
            "{}: +3/+0",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(1)),
            "{}: an unchosen creature is unaffected",
            case.id
        );

        advance_to_next_upkeep(&mut e);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(2), Some(2)),
            "{}: cleanup ends the pump",
            case.id
        );
    }
}

#[test]
fn x_pumps_batch_cards_accept_x_zero_and_reject_an_unpaid_x() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_051_000 + index as u64, case.id);
        let target = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        cast_pump(&mut e, &case, target, 0);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(2), Some(2)),
            "{}: X=0 is legal",
            case.id
        );

        let mut unpaid = engine(28_052_000 + index as u64, case.id);
        let target = move_ready_to_battlefield(&mut unpaid, 0, "grizzly_bears");
        ensure_in_hand(&mut unpaid, 0, case.id);
        give_mana(
            &mut unpaid,
            0,
            ManaGift {
                r: if case.id == "enrage" { 1 } else { 0 },
                b: if case.id == "enrage" { 0 } else { 1 },
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&unpaid, 0, case.id);
        assert!(
            unpaid
                .apply_scenario_command(0, &cast_spell_x(slot, target_object(target), 2))
                .is_err(),
            "{}: two generic mana are required for X=2",
            case.id
        );
        assert_eq!(
            (
                unpaid.effective_power(target),
                unpaid.effective_toughness(target)
            ),
            (Some(2), Some(2)),
            "{}: a rejected cast changes nothing",
            case.id
        );
    }
}
