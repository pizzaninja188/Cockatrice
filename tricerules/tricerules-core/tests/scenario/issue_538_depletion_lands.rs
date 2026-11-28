//! Actual-card scenarios for the depletion-counter lands (#538).
//!
//! Expectations come from the printed Oracle text; Scryfall publishes no rulings for these cards.
//! CR 122.1 makes a depletion counter an ordinary named counter with no rules of its own, CR
//! 614.1c/614.12 govern entering tapped with counters, CR 118.3 makes the counter removal a cost,
//! CR 605.1a/605.3b classify and immediately resolve each mana ability including its trailing
//! instruction, CR 701.21a governs the sacrifice, CR 502.3 the untap step and CR 503.1/603.2 the
//! upkeep trigger.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CostChoiceKind, CostObjectRef, CostSelection,
    CounterRemovalSelection,
};

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn engine(seed: u64, cards: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", cards), deck_with("forest", &[])]);
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

fn pool_of(symbol: char, count: u32) -> ManaPool {
    match symbol {
        'W' => (count, 0, 0, 0, 0, 0),
        'U' => (0, count, 0, 0, 0, 0),
        'B' => (0, 0, count, 0, 0, 0),
        'R' => (0, 0, 0, count, 0, 0),
        'G' => (0, 0, 0, 0, count, 0),
        _ => unreachable!(),
    }
}

fn depletion(engine: &GameEngine, land: u32) -> u32 {
    engine.state.objects[&land].counter_count(CounterKind::Depletion)
}

fn activate_mana_option(
    engine: &GameEngine,
    source: u32,
    option: u32,
    cost_selection: Option<CostSelection>,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for constructs an ActivateAbility command");
    };
    ability.mana_option_index = option;
    if let Some(selection) = cost_selection {
        ability.cost_selections.push(selection);
    }
    command
}

fn published_depletion_selection(
    engine: &mut GameEngine,
    source: u32,
    expected_available: u32,
) -> CostSelection {
    let legal = engine.initial_response_batch();
    let ability_key = u64::from(source) << 32;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&ability_key];
    assert!(
        choices.non_mana_costs_payable,
        "tap and depletion costs are payable"
    );
    let choice = choices
        .choices
        .iter()
        .find(|choice| choice.kind() == CostChoiceKind::RemoveCounters)
        .expect("the land publishes its depletion-counter cost");
    let removal = choice
        .counter_removal
        .as_ref()
        .expect("counter-removal options");
    assert_eq!(removal.count, 1);
    assert_eq!(removal.options.len(), 1);
    assert_eq!(removal.options[0].label, "depletion");
    assert_eq!(removal.options[0].available_count, expected_available);
    let source_ref = removal.source.as_ref().expect("source counter payment");
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

/// Play the land and order its two self-entry replacements (CR 616.1).
fn play_two_counter_land(engine: &mut GameEngine, card_id: &str) -> u32 {
    let source = inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_scenario_command(0, &play_land(slot))
        .expect("play the depletion land");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("tapped entry and depletion counters are both applicable replacements");
    let application = pending.presentation.candidates[0];
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![application]))
        .expect("order the entry replacements");
    assert!(engine.state.pending_resolution.is_none());
    source
}

fn advance_to_controller_next_main1(engine: &mut GameEngine) {
    end_active_turn(engine, 0);
    advance_to_main1_from_game_start(engine);
    end_active_turn(engine, 1);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 0);
}

fn advance_to_active_player_upkeep(engine: &mut GameEngine, player: i32) {
    for _ in 0..80 {
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let index = engine
                    .state
                    .player_idx(cleanup_player)
                    .expect("cleanup player");
                let excess = engine.state.players[index].hand.len() - 7;
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("pass toward the requested upkeep");
        if engine.state.active_player_id() == player
            && engine.state.turn_step == tricerules_core::TurnStep::Upkeep
        {
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}

#[test]
fn two_counter_depletion_lands_enter_tapped_and_sacrifice_only_after_the_last_counter() {
    for (index, (card_id, color)) in [
        ("hickory_woodlot", 'G'),
        ("remote_farm", 'W'),
        ("peat_bog", 'B'),
        ("sandstone_needle", 'R'),
        ("saprazzan_skerry", 'U'),
    ]
    .into_iter()
    .enumerate()
    {
        let mut engine = engine(538_100 + index as u64, &[]);
        let land = play_two_counter_land(&mut engine, card_id);
        assert!(
            engine.state.objects[&land].tapped,
            "{card_id} enters tapped"
        );
        assert_eq!(
            depletion(&engine, land),
            2,
            "{card_id} enters with two counters"
        );
        assert_eq!(
            engine.state.objects[&land].counters.len(),
            1,
            "{card_id}: only depletion counters"
        );

        advance_to_controller_next_main1(&mut engine);
        let first = published_depletion_selection(&mut engine, land, 2);
        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 0, Some(first)))
            .expect("tap and remove a depletion counter for two mana");
        assert_eq!(mana_pool(&engine), pool_of(color, 2), "{card_id}");
        assert!(
            engine.state.stack.is_empty(),
            "{card_id}: mana ability is immediate"
        );
        assert_eq!(depletion(&engine, land), 1);
        assert_eq!(
            engine.state.objects[&land].zone,
            Zone::Battlefield,
            "{card_id}: one counter remains, so the land is not sacrificed"
        );
        let command_index = engine.state.command_index;
        engine
            .apply_scenario_command(0, &undo_mana_ability())
            .expect_err("a source rider ability is not float-undoable");
        assert_eq!(engine.state.command_index, command_index);

        advance_to_controller_next_main1(&mut engine);
        assert!(
            !engine.state.objects[&land].tapped,
            "an ordinary land untaps"
        );
        let last = published_depletion_selection(&mut engine, land, 1);
        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 0, Some(last)))
            .expect("tap and remove the last depletion counter");
        assert_eq!(mana_pool(&engine), pool_of(color, 2), "{card_id}");
        assert!(engine.state.stack.is_empty(), "{card_id}: no stack object");
        assert_eq!(
            engine.state.objects[&land].zone,
            Zone::Graveyard,
            "{card_id}: removing the last depletion counter sacrifices the land"
        );
        assert!(engine.state.players[0].graveyard.contains(&land));
    }
}

#[test]
fn recharging_depletion_lands_skip_untap_while_charged_and_recover_after_upkeep() {
    for (index, (card_id, first, second)) in [
        ("veldt", 'G', 'W'),
        ("lava_tubes", 'B', 'R'),
        ("river_delta", 'U', 'B'),
        ("timberline_ridge", 'R', 'G'),
        ("land_cap", 'W', 'U'),
    ]
    .into_iter()
    .enumerate()
    {
        let mut engine = engine(538_200 + index as u64, &[card_id]);
        let land = move_ready_to_battlefield(&mut engine, 0, card_id);
        assert_eq!(depletion(&engine, land), 0);

        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 1, None))
            .expect("tap for the second color");
        assert_eq!(mana_pool(&engine), pool_of(second, 1), "{card_id}");
        assert!(
            engine.state.stack.is_empty(),
            "{card_id}: mana ability is immediate"
        );
        assert!(engine.state.objects[&land].tapped);
        assert_eq!(
            depletion(&engine, land),
            1,
            "{card_id}: puts one depletion counter"
        );
        assert_eq!(engine.state.objects[&land].counters.len(), 1);
        engine
            .apply_scenario_command(0, &undo_mana_ability())
            .expect_err("a source rider ability is not float-undoable");

        // P0's next untap step: still charged, so the land stays tapped (CR 502.3).
        advance_to_active_player_upkeep(&mut engine, 1);
        advance_to_active_player_upkeep(&mut engine, 0);
        assert!(
            engine.state.objects[&land].tapped,
            "{card_id}: a charged land doesn't untap"
        );
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            depletion(&engine, land),
            0,
            "{card_id}: the upkeep trigger removes the depletion counter"
        );
        assert!(engine.state.objects[&land].tapped);

        // The following untap step finds it uncharged and untaps it normally.
        advance_to_active_player_upkeep(&mut engine, 1);
        advance_to_active_player_upkeep(&mut engine, 0);
        assert!(
            !engine.state.objects[&land].tapped,
            "{card_id}: an uncharged land untaps"
        );
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            depletion(&engine, land),
            0,
            "removing from zero does nothing"
        );

        engine.state.players[0].mana_pool = Default::default();
        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 0, None))
            .expect("tap for the first color");
        assert_eq!(mana_pool(&engine), pool_of(first, 1), "{card_id}");
        assert_eq!(depletion(&engine, land), 1);
    }
}
