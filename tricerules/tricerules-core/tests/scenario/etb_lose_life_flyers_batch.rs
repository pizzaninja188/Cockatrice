//! Actual-card scenarios for the queue-v26 flying entry life-loss cohort.
//!
//! Expectations are printed Oracle values. CR 702.9a-b grants flying; CR 603.6a
//! stages the entry trigger; CR 119.3 makes the printed amount life loss, not a
//! payment or damage; CR 603.3d/608.2c put the trigger on the stack and resolve
//! its instructions in order.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    loss: u32,
    power: u32,
    toughness: u32,
}

const CASES: [Case; 4] = [
    Case {
        id: "foul_imp",
        loss: 2,
        power: 2,
        toughness: 2,
    },
    Case {
        id: "necrogen_scudder",
        loss: 3,
        power: 3,
        toughness: 3,
    },
    Case {
        id: "dread_reaper",
        loss: 5,
        power: 6,
        toughness: 5,
    },
    Case {
        id: "vampiric_spirit",
        loss: 4,
        power: 4,
        toughness: 3,
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &[card, "grizzly_bears"]),
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
fn etb_lose_life_flyers_batch_cards_lose_the_printed_life_and_have_flying() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_070_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: the printed life is not lost at entry",
            case.id
        );
        assert!(
            !e.state.stack.is_empty() || !e.state.pending_triggers.is_empty(),
            "{}: the entry trigger is waiting to resolve",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.players[0].life,
            20 - case.loss as i32,
            "{}: the controller loses exactly the printed amount",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert!(
            e.effective_has_keyword(source, Keyword::Flying),
            "{}: printed flying",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn etb_lose_life_flyers_batch_cards_scope_the_loss_to_their_controller() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_071_000 + index as u64, case.id);
        let opponent_life = e.state.players[1].life;
        let opponent_hand = e.state.players[1].hand.len();
        move_ready_to_battlefield(&mut e, 0, case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life, opponent_life,
            "{}: the opponent loses no life",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: the opponent draws nothing",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            20 - case.loss as i32,
            "{}",
            case.id
        );
    }
}

#[test]
fn etb_lose_life_flyers_batch_cards_lose_the_game_at_exactly_the_printed_life() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_072_000 + index as u64, case.id);
        e.state.players[0].life = case.loss as i32;
        move_ready_to_battlefield(&mut e, 0, case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[0].life, 0, "{}", case.id);
        assert!(
            e.state.players[0].has_lost,
            "{}: zero life loses the game",
            case.id
        );
    }
}

#[test]
fn foul_imp_can_be_cast_below_its_entry_life_loss() {
    let case = &CASES[0];
    let mut e = engine(26_073_000, case.id);
    e.state.players[0].life = 1;
    // Foul Imp costs {B}{B}.
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            ..ManaGift::default()
        },
    );
    let slot = inject_card_into_hand(&mut e, 0, case.id);
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("life loss is an effect, so the spell is castable at 1 life");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    assert_eq!(
        e.state.players[0].life, -1,
        "the entry effect still resolves and subtracts the printed two life"
    );
    assert!(e.state.players[0].has_lost, "negative life loses the game");
}
