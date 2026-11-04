//! Actual As Foretold definition in prepared scenarios; dev-disabled reachability is separate.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

fn game() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        7_313_811,
        &[17, 29],
        20,
        Some(vec![vec!["island".into(); 40]; 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn main(engine: &mut GameEngine) {
    for _ in 0..20 {
        if !engine.state.stack.is_empty() {
            pass_priority_round(engine);
        } else if engine.state.turn_step == TurnStep::Main1 {
            return;
        } else {
            engine
                .apply_scenario_command(engine.state.priority_player_id(), &primitive_yield())
                .unwrap();
        }
    }
    panic!("Main1 not reached");
}

#[test]
fn as_foretold_paid_cast_and_exact_own_upkeep_counters() {
    let mut engine = game();
    inject_card_into_hand(&mut engine, 0, "as_foretold");
    give_mana(
        &mut engine,
        17,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let command = cast_spell(hand_index_for_card(&engine, 0, "as_foretold"), vec![]);
    engine.apply_scenario_command(17, &command).unwrap();
    pass_priority_round(&mut engine);
    let source = *engine.state.players[0]
        .battlefield
        .iter()
        .find(|id| engine.state.objects[id].card_id == "as_foretold")
        .unwrap();
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Time),
        0
    );
    end_active_turn(&mut engine, 17);
    main(&mut engine);
    assert_eq!(engine.state.active_player_id(), 29);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Time),
        0
    );
    end_active_turn(&mut engine, 29);
    main(&mut engine);
    assert_eq!(engine.state.active_player_id(), 17);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Time),
        1
    );
    end_active_turn(&mut engine, 17);
    main(&mut engine);
    end_active_turn(&mut engine, 29);
    main(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Time),
        2
    );
}

#[test]
fn as_foretold_actual_bolt_offer_uses_only_selected_exact_copy() {
    let mut engine = game();
    let first = inject_permanent_on_battlefield(&mut engine, 0, "as_foretold");
    let second = inject_permanent_on_battlefield(&mut engine, 0, "as_foretold");
    for id in [first, second] {
        engine
            .state
            .objects
            .get_mut(&id)
            .unwrap()
            .set_counter(CounterKind::Time, 1);
    }
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    let slot = hand_index_for_card(&engine, 0, "lightning_bolt") as u32;
    let offers = engine.initial_response_batch();
    let offer = offers.legal_by_player[&17]
        .hand_actions
        .iter()
        .find(|a| a.hand_index == slot)
        .unwrap();
    assert_eq!(offer.as_foretold_costs.len(), 2);
    let selected = offer
        .as_foretold_costs
        .iter()
        .find(|c| c.source.unwrap().object_id == second)
        .unwrap()
        .source;
    let command = rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
            source: Some(rv1::CastSource {
                location: Some(rv1::cast_source::Location::HandIndex(slot)),
                expected_zone_change_generation: None,
            }),
            cast_method: rv1::CastMethod::Normal as i32,
            as_foretold_source: selected,
            targets: vec![rv1::TargetRef {
                object_id: 29,
                kind: rv1::TargetRefKind::Player as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    };
    engine.apply_command(17, &command).unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[1].life, 17);
    assert_eq!(engine.state.objects[&second].zone, Zone::Battlefield);
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    let offers = engine.initial_response_batch();
    let offer = offers.legal_by_player[&17]
        .hand_actions
        .iter()
        .find(|a| a.card_name == "Lightning Bolt")
        .unwrap();
    assert_eq!(offer.as_foretold_costs.len(), 1);
    assert_eq!(offer.as_foretold_costs[0].source.unwrap().object_id, first);
}
