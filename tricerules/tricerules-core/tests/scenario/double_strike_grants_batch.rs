//! Actual-card scenarios for the queue-v28 double-strike grant cohort.
//!
//! Expectations are printed Oracle values. CR 115.1 chooses one creature target;
//! CR 613.1f and 702.4 grant double strike in layer 6 until end of turn; CR 514.2
//! removes the grant during cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::TurnStep;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "ticked_off",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "assault_strobe",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
        },
        Case {
            id: "double_cleave",
            mana: ManaGift {
                r: 1,
                c: 1,
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

fn cast_grant(e: &mut GameEngine, case: &Case, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
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
fn double_strike_grants_batch_cards_grant_double_strike_to_the_chosen_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_040_000 + index as u64, case.id);
        let target = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        assert!(!e.effective_has_keyword(target, Keyword::DoubleStrike));
        cast_grant(&mut e, &case, target);
        assert!(
            e.effective_has_keyword(target, Keyword::DoubleStrike),
            "{}: the chosen creature gains double strike",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::DoubleStrike),
            "{}: an unchosen creature is unaffected",
            case.id
        );
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(2), Some(2)),
            "{}: double strike does not change the body",
            case.id
        );

        advance_to_next_upkeep(&mut e);
        assert!(
            !e.effective_has_keyword(target, Keyword::DoubleStrike),
            "{}: cleanup ends the grant",
            case.id
        );
    }
}

#[test]
fn double_strike_grants_batch_cards_reject_a_noncreature_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_041_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal creature target",
            case.id
        );
        assert_eq!(
            count_card_id_in_graveyard(&e, 0, case.id),
            0,
            "{}: the rejected spell stays in hand",
            case.id
        );
    }
}
