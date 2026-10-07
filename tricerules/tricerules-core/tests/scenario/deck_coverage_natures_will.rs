//! Nature's Will groups simultaneous combat damage by damaged player.
//!
//! CR 510.2 and 603.2c put simultaneous combat damage and its grouped triggers in one damage-step
//! event. First-strike and regular combat damage remain separate events. Nature's Will's
//! "that player" is the damaged player; Enduring Curiosity remains one trigger per creature.

use super::helpers::*;
use tricerules_core::{GameEngine, TurnStep};
use tricerules_proto::ruled::v1::{AttackAssignment, DeclareAttackers};

const NATURES_WILL: &str = "natures_will";
const ENDURING_CURIOSITY: &str = "enduring_curiosity";

fn engine() -> GameEngine {
    let mut engine = GameEngine::new(
        202_610_061,
        &[0, 1, 2, 3],
        20,
        Some(vec![forest_only_deck(); 4]),
        true,
    )
    .expect("new four-player engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn advance_to_step(engine: &mut GameEngine, wanted: TurnStep) {
    for _ in 0..120 {
        if engine.state.turn_step == wanted {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("game did not reach {wanted:?}");
}

fn resolve_waiting_stack(engine: &mut GameEngine) {
    for _ in 0..120 {
        if engine.state.stack.is_empty()
            && engine.state.pending_triggers.is_empty()
            && engine.state.pending_trigger_order.is_none()
            && engine.state.pending_resolution.is_none()
        {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("trigger stack did not settle");
}

fn triggers_from(engine: &GameEngine, source: u32) -> Vec<&tricerules_core::state::StagedTrigger> {
    let pending = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("trigger ordering prompt");
    pending
        .candidates
        .iter()
        .filter(|trigger| trigger.source_permanent_id == source)
        .collect()
}

#[test]
fn groups_by_damaged_player_and_resets_between_first_and_regular_damage_steps() {
    let mut engine = engine();
    let natures_will = inject_permanent_on_battlefield(&mut engine, 0, NATURES_WILL);
    let curiosity = inject_creature_on_battlefield(&mut engine, 0, ENDURING_CURIOSITY);
    let swiftblade = inject_creature_on_battlefield(&mut engine, 0, "boros_swiftblade");
    let p1_bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let p2_bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    let p0_land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let p1_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let p1_second_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let p2_land = inject_permanent_on_battlefield(&mut engine, 2, "forest");
    let p3_land = inject_permanent_on_battlefield(&mut engine, 3, "forest");
    engine.state.objects.get_mut(&p0_land).unwrap().tapped = true;

    engine
        .apply_command(0, &primitive_yield())
        .expect("begin combat");
    advance_to_step(&mut engine, TurnStep::DeclareAttackers);
    let legal = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .clone();
    let assignments = [(swiftblade, 1), (p1_bear, 1), (p2_bear, 2)]
        .into_iter()
        .map(|(attacker, defender)| {
            *legal
                .iter()
                .find(|entry| {
                    entry.attacker_object_id == attacker && entry.defending_player_id == defender
                })
                .expect("legal attack assignment")
        })
        .collect::<Vec<AttackAssignment>>();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers { assignments })),
            },
        )
        .expect("split attackers between two defenders");

    advance_to_step(&mut engine, TurnStep::FirstStrikeDamage);
    let first_strike_natures_triggers = triggers_from(&engine, natures_will);
    assert_eq!(first_strike_natures_triggers.len(), 1);
    assert_eq!(
        first_strike_natures_triggers[0]
            .trigger_context
            .affected_player,
        Some(1)
    );
    assert_eq!(triggers_from(&engine, curiosity).len(), 1);
    resolve_waiting_stack(&mut engine);

    assert!(engine.state.objects[&p1_land].tapped);
    assert!(engine.state.objects[&p1_second_land].tapped);
    assert!(!engine.state.objects[&p2_land].tapped);
    assert!(!engine.state.objects[&p3_land].tapped);
    assert!(!engine.state.objects[&p0_land].tapped);

    advance_to_step(&mut engine, TurnStep::CombatDamage);
    let regular_natures_triggers = triggers_from(&engine, natures_will);
    assert_eq!(regular_natures_triggers.len(), 2);
    let mut affected_players = regular_natures_triggers
        .iter()
        .map(|trigger| trigger.trigger_context.affected_player.unwrap())
        .collect::<Vec<_>>();
    affected_players.sort_unstable();
    assert_eq!(affected_players, [1, 2]);
    assert_eq!(
        triggers_from(&engine, curiosity).len(),
        3,
        "Enduring Curiosity keeps its per-creature cardinality"
    );
    resolve_waiting_stack(&mut engine);

    assert!(engine.state.objects[&p1_land].tapped);
    assert!(engine.state.objects[&p1_second_land].tapped);
    assert!(engine.state.objects[&p2_land].tapped);
    assert!(!engine.state.objects[&p3_land].tapped);
    assert!(!engine.state.objects[&p0_land].tapped);
    assert_eq!(engine.state.players[1].life, 14);
    assert_eq!(engine.state.players[2].life, 18);
}
