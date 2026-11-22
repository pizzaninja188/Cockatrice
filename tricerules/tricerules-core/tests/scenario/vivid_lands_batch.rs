//! Actual-card scenarios for the reviewed Vivid land cycle.
//!
//! Expectations are printed Oracle text. CR 614.1c-d/614.12 apply the two entry
//! replacements and CR 616.1 orders them; CR 118.3/602.2 makes the Charge counter
//! an activation cost, while CR 122.6 covers entering with counters. CR
//! 605.1a/605.3b resolve the mana abilities immediately, and CR 105.4/106.1
//! distinguish the printed color from any-color output.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, ChoiceKind, CostChoiceKind, CostObjectRef,
    CostSelection, CounterRemovalSelection,
};

struct Case {
    id: &'static str,
    color: (u32, u32, u32, u32, u32, u32),
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "vivid_marsh",
            color: (0, 0, 1, 0, 0, 0),
        },
        Case {
            id: "vivid_creek",
            color: (0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "vivid_crag",
            color: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "vivid_meadow",
            color: (1, 0, 0, 0, 0, 0),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn enter_vivid(e: &mut GameEngine, card: &str) -> u32 {
    ensure_in_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &play_land(slot))
        .expect("play the Vivid land");
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("both self-entry replacements require a CR 616 choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(
        pending.presentation.candidates.len(),
        2,
        "the tapped entry and Charge-counter replacements are both offered"
    );
    let application = pending.presentation.candidates[0];
    e.apply_scenario_command(0, &submit_resolution_choice(vec![application]))
        .expect("choose the entry replacement order");
    assert!(e.state.pending_resolution.is_none());
    battlefield_object_for_card(e, 0, card)
}

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn advance_to_controller_next_main1(e: &mut GameEngine) {
    end_active_turn(e, 0);
    advance_to_main1_from_game_start(e);
    assert_eq!(e.state.active_player_id(), 1);
    end_active_turn(e, 1);
    advance_to_main1_from_game_start(e);
    assert_eq!(e.state.active_player_id(), 0);
}

fn published_charge_selection(
    e: &mut GameEngine,
    source: u32,
    expected_available: u32,
) -> CostSelection {
    let legal = e.initial_response_batch();
    let ability_key = (u64::from(source) << 32) | 1;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&ability_key];
    assert!(
        choices.non_mana_costs_payable,
        "tap and Charge costs are payable"
    );
    let choice = choices
        .choices
        .iter()
        .find(|choice| choice.kind() == CostChoiceKind::RemoveCounters)
        .expect("the Vivid land publishes its Charge-counter cost");
    let removal = choice
        .counter_removal
        .as_ref()
        .expect("counter-removal cost carries its options");
    assert_eq!(removal.count, 1);
    assert_eq!(removal.options.len(), 1, "the fixed cost removes Charge");
    assert_eq!(removal.options[0].label, "charge");
    assert_eq!(removal.options[0].available_count, expected_available);
    let source_ref = removal.source.as_ref().expect("source counter payment");
    assert_eq!(source_ref.object_id, source);
    CostSelection {
        cost_index: choice.cost_index,
        selection: Some(Selection::CounterRemoval(CounterRemovalSelection {
            source: Some(CostObjectRef {
                object_id: source_ref.object_id,
                zone_change_generation: source_ref.zone_change_generation,
            }),
            option_id: removal.options[0].option_id,
        })),
    }
}

fn any_color_command(
    e: &mut GameEngine,
    source: u32,
    option: u32,
    expected_available: u32,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let selection = published_charge_selection(e, source, expected_available);
    let mut command = activate_ability_for(e, source, 1, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for builds an ActivateAbility command")
    };
    activation.mana_option_index = option;
    activation.cost_selections.push(selection);
    command
}

fn activate_any_color(
    e: &mut GameEngine,
    source: u32,
    option: u32,
    expected_available: u32,
) -> bool {
    let command = any_color_command(e, source, option, expected_available);
    e.apply_scenario_command(0, &command).is_ok()
}

fn apply_saved_any_color(
    e: &mut GameEngine,
    source: u32,
    option: u32,
    selection: CostSelection,
) -> bool {
    let mut command = activate_ability_for(e, source, 1, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for builds an ActivateAbility command")
    };
    activation.mana_option_index = option;
    activation.cost_selections.push(selection);
    e.apply_scenario_command(0, &command).is_ok()
}

#[test]
fn vivid_lands_batch_cards_enter_tapped_with_two_charge_counters() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_500_000 + index as u64, case.id);
        let source = enter_vivid(&mut e, case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Charge),
            2,
            "{}: exactly two Charge counters",
            case.id
        );
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the tapped land cannot use its mana ability immediately",
            case.id
        );
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}

#[test]
fn vivid_lands_batch_cards_add_their_color_and_spend_a_charge_for_any_color() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut printed = engine(10_510_000 + index as u64, case.id);
        let source = enter_vivid(&mut printed, case.id);
        advance_to_controller_next_main1(&mut printed);
        apply_ability(&mut printed, 0, source, 0, vec![]).expect("tap for the printed color");
        assert_eq!(pool(&printed, 0), case.color, "{}", case.id);
        assert!(printed.state.stack.is_empty(), "{}", case.id);
        assert_eq!(
            printed.state.objects[&source].counter_count(CounterKind::Charge),
            2,
            "{}: the printed-color ability does not spend counters",
            case.id
        );

        let expected = [
            (0, (1, 0, 0, 0, 0, 0)),
            (1, (0, 1, 0, 0, 0, 0)),
            (2, (0, 0, 1, 0, 0, 0)),
            (3, (0, 0, 0, 1, 0, 0)),
            (4, (0, 0, 0, 0, 1, 0)),
        ];
        for (offset, (option, mana)) in expected.into_iter().enumerate() {
            let mut e = engine(10_520_000 + index as u64 * 10 + offset as u64, case.id);
            let source = enter_vivid(&mut e, case.id);
            advance_to_controller_next_main1(&mut e);
            assert!(
                activate_any_color(&mut e, source, option, 2),
                "{}: spend one Charge for option {option}",
                case.id
            );
            assert_eq!(pool(&e, 0), mana, "{} option {option}", case.id);
            assert_eq!(
                e.state.objects[&source].counter_count(CounterKind::Charge),
                1,
                "{} option {option}: exactly one Charge counter is removed",
                case.id
            );
            assert!(e.state.objects[&source].tapped, "{}", case.id);
            assert!(e.state.stack.is_empty(), "{}", case.id);
        }

        let mut two_uses = engine(10_525_000 + index as u64, case.id);
        let source = enter_vivid(&mut two_uses, case.id);
        advance_to_controller_next_main1(&mut two_uses);
        assert!(
            activate_any_color(&mut two_uses, source, 0, 2),
            "{}",
            case.id
        );
        advance_to_controller_next_main1(&mut two_uses);
        assert!(
            activate_any_color(&mut two_uses, source, 4, 1),
            "{}",
            case.id
        );
        assert_eq!(
            pool(&two_uses, 0),
            (0, 0, 0, 0, 1, 0),
            "{}: earlier mana emptied at end of turn, leaving the new green mana",
            case.id
        );
        assert_eq!(
            two_uses.state.objects[&source].counter_count(CounterKind::Charge),
            0,
            "{}: both Charge counters are spent",
            case.id
        );
    }
}

#[test]
fn vivid_lands_batch_cards_reject_empty_counter_activation() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_530_000 + index as u64, case.id);
        let source = enter_vivid(&mut e, case.id);
        advance_to_controller_next_main1(&mut e);
        assert!(activate_any_color(&mut e, source, 0, 2), "{}", case.id);
        advance_to_controller_next_main1(&mut e);
        let saved = published_charge_selection(&mut e, source, 1);
        assert!(
            apply_saved_any_color(&mut e, source, 0, saved.clone()),
            "{}",
            case.id
        );
        advance_to_controller_next_main1(&mut e);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Charge),
            0,
            "{}",
            case.id
        );
        assert!(
            !apply_saved_any_color(&mut e, source, 0, saved),
            "{}: no Charge counter remains to pay the any-color cost",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: the rejected activation does not tap the source",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: the rejected activation adds no mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Charge),
            0,
            "{}: the rejected activation removes no counter",
            case.id
        );

        let mut invalid = engine(10_535_000 + index as u64, case.id);
        let source = enter_vivid(&mut invalid, case.id);
        advance_to_controller_next_main1(&mut invalid);
        let selection = published_charge_selection(&mut invalid, source, 2);
        assert!(
            !apply_saved_any_color(&mut invalid, source, 5, selection),
            "{}: an out-of-range mana option is rejected",
            case.id
        );
        assert!(!invalid.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(
            invalid.state.objects[&source].counter_count(CounterKind::Charge),
            2,
            "{}",
            case.id
        );
        assert_eq!(pool(&invalid, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}
