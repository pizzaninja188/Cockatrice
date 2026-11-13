//! Actual-card scenarios for the reviewed "Flying; enters, draw a card, then
//! discard a card" family.
//!
//! Expectations are printed Oracle values. CR 702.9a-b define Flying;
//! CR 603.6a stages the entry trigger; CR 121 draws first and CR 701.9 then
//! applies the mandatory private discard.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "owl_familiar",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "quicksilver_fisher",
            power: 4,
            toughness: 3,
        },
        Case {
            id: "screeching_drake",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "sky-eel_school",
            power: 3,
            toughness: 3,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card, "grizzly_bears"]),
        deck_with("forest", &[]),
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

#[test]
fn flying_etb_loot_batch_cards_have_flying_and_loot_on_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_270_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        assert!(
            e.effective_has_keyword(source, Keyword::Flying),
            "{}: the printed Flying keyword is active",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: the entry trigger is on the stack",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: the controller drew one card",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: the draw came from the controller's library",
            case.id
        );
        let pending = e
            .state
            .pending_resolution
            .as_ref()
            .expect("the mandatory discard is pending");
        let discarded = pending.presentation.candidates[0];
        e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
            .expect("discard one card");
        assert_eq!(
            e.state.objects[&discarded].zone,
            Zone::Graveyard,
            "{}: the discarded card goes to its owner's graveyard",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: draw one then discard one is net zero",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn flying_etb_loot_batch_cards_make_the_discard_mandatory() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_271_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.pending_resolution.is_some(),
            "{}: the discard is pending",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
                .is_err(),
            "{}: an empty discard choice is rejected",
            case.id
        );
        let discarded = e
            .state
            .pending_resolution
            .as_ref()
            .expect("pending discard")
            .presentation
            .candidates[0];
        e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
            .expect("a legal discard completes the trigger");
    }
}

#[test]
fn flying_etb_loot_batch_cards_draw_the_top_card_then_discard() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_272_000 + index as u64, case.id);
        let known = inject_library_card(&mut e, 0, "forest");
        e.state.players[0]
            .library
            .retain(|candidate| *candidate != known);
        e.state.players[0].library.push_front(known);
        move_ready_to_battlefield(&mut e, 0, case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&known].zone,
            Zone::Hand,
            "{}: the top card was drawn",
            case.id
        );
        let pending = e
            .state
            .pending_resolution
            .as_ref()
            .expect("the mandatory discard is pending");
        assert!(
            pending.presentation.candidates.contains(&known),
            "{}: the drawn card is a legal discard choice",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![known]))
            .expect("discard the drawn card");
        assert_eq!(
            e.state.objects[&known].zone,
            Zone::Graveyard,
            "{}: the drawn card is discarded",
            case.id
        );
    }
}
