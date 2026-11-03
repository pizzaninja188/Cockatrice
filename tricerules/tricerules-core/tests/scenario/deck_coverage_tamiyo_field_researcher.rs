//! Actual Tamiyo with prepared zones/mana/loyalty. Strict normal captures are separate.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

const ID: &str = "tamiyo,_field_researcher";

fn game() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_110_311,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]); 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}
fn tamiyo(engine: &mut GameEngine, loyalty: u32) -> u32 {
    let source = inject_permanent_on_battlefield(engine, 0, ID);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, loyalty);
    source
}
fn activate(engine: &mut GameEngine, source: u32, index: u32, targets: Vec<rv1::TargetRef>) {
    let command = activate_ability_for(engine, source, index, targets);
    engine.apply_scenario_command(0, &command).unwrap();
    pass_priority_round(engine);
}
fn reject(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_scenario_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn tamiyo_actual_cast_enters_with_four_and_plus_one_allows_zero_or_two_distinct_creatures() {
    let mut engine = game();
    let source = inject_card_into_hand(&mut engine, 0, ID);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            g: 1,
            w: 1,
            u: 1,
            ..Default::default()
        },
    );
    let command = cast_spell(hand_index_for_card(&engine, 0, ID), vec![]);
    engine.apply_scenario_command(0, &command).unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        4
    );
    let first = inject_creature_on_battlefield(&mut engine, 0, "llanowar_elves");
    let second = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let mut duplicate = target_object(first);
    duplicate.extend(target_object(first));
    let command = activate_ability_for(&engine, source, 0, duplicate);
    reject(&mut engine, 0, &command);
    let mut targets = target_object(first);
    targets.extend(target_object(second));
    activate(&mut engine, source, 0, targets);
    assert_eq!(engine.state.active_event_observers.len(), 2);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5
    );
    let command = activate_ability_for(&engine, source, 0, vec![]);
    reject(&mut engine, 0, &command); // One loyalty activation each turn.
    let mut zero = game();
    let source = tamiyo(&mut zero, 4);
    activate(&mut zero, source, 0, vec![]);
    assert!(zero.state.active_event_observers.is_empty());
    assert_eq!(
        zero.state.objects[&source].counter_count(CounterKind::Loyalty),
        5
    );
}

#[test]
fn tamiyo_actual_plus_one_draws_for_attacker_and_opponent_blocker() {
    let mut engine = game();
    let source = tamiyo(&mut engine, 4);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let mut targets = target_object(attacker);
    targets.extend(target_object(blocker));
    activate(&mut engine, source, 0, targets);
    let before = [
        engine.state.players[0].hand.len(),
        engine.state.players[1].hand.len(),
    ];
    engine
        .apply_scenario_command(0, &primitive_yield())
        .unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .unwrap();
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            1,
            &declare_blockers(vec![rv1::BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&attacker].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&blocker].zone, Zone::Graveyard);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_triggered)
            .count(),
        2
    );
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), before[0] + 2);
    assert_eq!(engine.state.players[1].hand.len(), before[1]);
}

#[test]
fn tamiyo_actual_minus_two_taps_noncreature_nonlands_and_skips_only_next_untap() {
    let mut engine = game();
    let source = tamiyo(&mut engine, 4);
    let ring = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let command = activate_ability_for(&engine, source, 1, target_object(land));
    reject(&mut engine, 0, &command);
    let mut targets = target_object(ring);
    targets.extend(target_object(creature));
    activate(&mut engine, source, 1, targets);
    assert!(engine.state.objects[&ring].tapped && engine.state.objects[&creature].tapped);
    assert!(!engine.state.objects[&land].tapped);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        2
    );
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.active_player_id(), 1);
    assert!(engine.state.objects[&ring].tapped && engine.state.objects[&creature].tapped);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 1);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 0);
    assert!(!engine.state.objects[&ring].tapped && !engine.state.objects[&creature].tapped);
}

#[test]
fn tamiyo_actual_ultimate_draws_three_after_source_dies_and_casts_actual_spell_without_mana() {
    let mut engine = game();
    let source = tamiyo(&mut engine, 7);
    let spell = inject_card_into_hand(&mut engine, 0, "divination");
    let before = engine.state.players[0].hand.len();
    activate(&mut engine, source, 2, vec![]);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), before + 3);
    assert_eq!(engine.state.static_emblems.len(), 1);
    let slot = hand_index_for_card(&engine, 0, "divination");
    let offers = &engine.initial_response_batch().legal_by_player[&0].hand_actions;
    assert_eq!(
        offers
            .iter()
            .filter(|action| action.hand_index == slot as u32)
            .count(),
        2
    );
    let mut command = cast_spell(slot, vec![]);
    let Some(rv1::ruled_command::Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        unreachable!()
    };
    cast.cast_method = rv1::CastMethod::WithoutManaCost as i32;
    engine.apply_scenario_command(0, &command).unwrap();
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), before + 4);
}

#[test]
fn tamiyo_actual_rejects_wrong_actor_insufficient_loyalty_and_three_targets_atomically() {
    let mut engine = game();
    let source = tamiyo(&mut engine, 4);
    let command = activate_ability_for(&engine, source, 0, vec![]);
    reject(&mut engine, 1, &command);
    let command = activate_ability_for(&engine, source, 2, vec![]);
    reject(&mut engine, 0, &command);
    let mut targets = Vec::new();
    for _ in 0..3 {
        let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        targets.extend(target_object(creature));
    }
    let command = activate_ability_for(&engine, source, 0, targets);
    reject(&mut engine, 0, &command);
}
