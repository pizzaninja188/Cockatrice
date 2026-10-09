//! Exact Oracle behavior for Vivid Grove's entry replacement and two mana abilities.
//!
//! Oracle and rulings checked against Scryfall on 2026-09-27; no rulings were returned. CR 122.6
//! governs entering with counters, CR 614.12 governs the tapped entry replacement, CR 605.1a and
//! 605.3b classify and immediately resolve the mana abilities, and CR 105.4 / 106.1 distinguish
//! colored mana from colorless mana.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, ChoiceKind, CostChoiceKind, CostObjectRef,
    CostSelection, CounterRemovalSelection,
};

const VIVID_GROVE: &str = "vivid_grove";

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn enter_vivid_grove(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, VIVID_GROVE);
    let slot = hand_index_for_card(engine, 0, VIVID_GROVE);
    engine
        .apply_command(0, &play_land(slot))
        .expect("play Vivid Grove as a land");

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the two applicable self-entry replacements require a CR 616 choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(
        pending.presentation.candidates.len(),
        2,
        "the tapped-entry and Charge-counter replacements are both presented"
    );
    let application = pending.presentation.candidates[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![application]))
        .expect("choose the next Vivid Grove entry replacement");
    assert!(
        engine.state.pending_resolution.is_none(),
        "both entry replacements complete after their order is chosen"
    );
    source
}

fn mana_pool(engine: &GameEngine) -> ManaPool {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn advance_to_grove_controller_next_main1(engine: &mut GameEngine) {
    end_active_turn(engine, 0);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 1);

    end_active_turn(engine, 1);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 0);
}

fn activate_mana_option(
    engine: &GameEngine,
    source: u32,
    ability_index: u32,
    option: u32,
    cost_selection: Option<CostSelection>,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(engine, source, ability_index, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for constructs an ActivateAbility command");
    };
    ability.mana_option_index = option;
    if let Some(selection) = cost_selection {
        ability.cost_selections.push(selection);
    }
    command
}

fn published_charge_selection(
    engine: &mut GameEngine,
    source: u32,
    expected_available: u32,
) -> CostSelection {
    let legal = engine.initial_response_batch();
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
        .expect("Vivid Grove publishes its Charge-counter cost");
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

#[test]
fn vivid_grove_enters_tapped_adds_green_and_spends_its_two_counters_atomically() {
    let mut engine = engine(202_609_270);
    let source = enter_vivid_grove(&mut engine);

    assert!(engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        2,
        "Vivid Grove enters with exactly two Charge counters"
    );
    let command_index = engine.state.command_index;
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, vec![]))
        .expect_err("the entering tapped land cannot pay a tap cost");
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 0));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        2
    );

    advance_to_grove_controller_next_main1(&mut engine);
    engine
        .apply_command(0, &activate_mana_option(&engine, source, 0, 0, None))
        .expect("tap Vivid Grove to add one green mana");
    assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 1, 0));
    assert!(engine.state.objects[&source].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "the green mana ability is immediate"
    );

    advance_to_grove_controller_next_main1(&mut engine);
    let first_charge_selection = published_charge_selection(&mut engine, source, 2);
    engine
        .apply_command(
            0,
            &activate_mana_option(&engine, source, 1, 1, Some(first_charge_selection.clone())),
        )
        .expect("tap Vivid Grove, remove a Charge counter, and add blue mana");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        1
    );
    assert_eq!(mana_pool(&engine), (0, 1, 0, 0, 0, 0));
    assert!(engine.state.objects[&source].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "the chosen blue mana is immediate"
    );

    advance_to_grove_controller_next_main1(&mut engine);
    let second_charge_selection = published_charge_selection(&mut engine, source, 1);
    engine
        .apply_command(
            0,
            &activate_mana_option(&engine, source, 1, 3, Some(second_charge_selection)),
        )
        .expect("tap Vivid Grove, remove the last Charge counter, and add red mana");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(mana_pool(&engine), (0, 0, 0, 1, 0, 0));
    assert!(engine.state.objects[&source].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "the chosen red mana is immediate"
    );

    advance_to_grove_controller_next_main1(&mut engine);
    let mana_before_rejection = mana_pool(&engine);
    let command_index = engine.state.command_index;
    engine
        .apply_command(
            0,
            &activate_mana_option(&engine, source, 1, 3, Some(first_charge_selection)),
        )
        .expect_err("Vivid Grove cannot remove a Charge counter when none remain");
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(mana_pool(&engine), mana_before_rejection);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0,
        "the rejected activation removes no counters"
    );
    assert!(
        !engine.state.objects[&source].tapped,
        "the rejected activation does not tap"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn vivid_grove_counter_ability_selects_each_color_and_rejects_colorless() {
    let colors = [
        (0, (1, 0, 0, 0, 0, 0)),
        (1, (0, 1, 0, 0, 0, 0)),
        (2, (0, 0, 1, 0, 0, 0)),
        (3, (0, 0, 0, 1, 0, 0)),
        (4, (0, 0, 0, 0, 1, 0)),
    ];

    for (index, (option, expected_pool)) in colors.into_iter().enumerate() {
        let mut engine = engine(202_609_280 + index as u64);
        let source = enter_vivid_grove(&mut engine);
        assert!(engine.state.objects[&source].tapped);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            2
        );
        advance_to_grove_controller_next_main1(&mut engine);

        let selection = published_charge_selection(&mut engine, source, 2);
        engine
            .apply_command(
                0,
                &activate_mana_option(&engine, source, 1, option, Some(selection)),
            )
            .expect("choose one of the five colored mana options");
        assert_eq!(mana_pool(&engine), expected_pool, "mana option {option}");
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            1
        );
        assert!(engine.state.objects[&source].tapped);
        assert!(engine.state.stack.is_empty(), "mana is added immediately");
    }

    let mut engine = engine(202_609_285);
    let source = enter_vivid_grove(&mut engine);
    advance_to_grove_controller_next_main1(&mut engine);
    let selection = published_charge_selection(&mut engine, source, 2);
    let before = mana_pool(&engine);
    let command_index = engine.state.command_index;
    engine
        .apply_command(
            0,
            &activate_mana_option(&engine, source, 1, 5, Some(selection)),
        )
        .expect_err("Vivid Grove offers no colorless mana option");
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(mana_pool(&engine), before);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        2,
        "an invalid option pays no Charge counter"
    );
    assert!(!engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());
}
