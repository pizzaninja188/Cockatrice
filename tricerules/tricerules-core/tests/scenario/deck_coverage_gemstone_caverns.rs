//! Prepared actual-card scenarios. The server pilot separately proves legal normal startup.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

const ID: &str = "gemstone_caverns";
fn game() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        705_110,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]); 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}
fn mana(engine: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}
fn reject(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_scenario_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn gemstone_actual_ordinary_land_play_has_no_luck_and_only_colorless() {
    let mut engine = game();
    let id = inject_card_into_hand(&mut engine, 0, ID);
    let command = play_land(hand_index_for_card(&engine, 0, ID));
    engine.apply_scenario_command(0, &command).unwrap();
    assert_eq!(engine.state.objects[&id].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&id].counter_count(CounterKind::Luck),
        0
    );
    assert_eq!(engine.state.lands_played_this_turn, 1);
    let invalid = mana(&engine, id, 1);
    reject(&mut engine, 0, &invalid);
    let command = mana(&engine, id, 0);
    engine.apply_scenario_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn gemstone_actual_luck_threshold_produces_each_color_once_without_colorless_or_scaling() {
    for luck in [1, 3] {
        for option in 0..5 {
            let mut engine = game();
            let source = inject_permanent_on_battlefield(&mut engine, 0, ID);
            engine
                .state
                .objects
                .get_mut(&source)
                .unwrap()
                .set_counter(CounterKind::Luck, luck);
            let invalid = mana(&engine, source, 5);
            reject(&mut engine, 0, &invalid);
            let command = mana(&engine, source, option);
            engine.apply_scenario_command(0, &command).unwrap();
            let pool = &engine.state.players[0].mana_pool;
            let values = [
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless,
            ];
            for (index, amount) in values.into_iter().enumerate() {
                assert_eq!(amount, u32::from(index == option as usize));
            }
            assert!(engine.state.objects[&source].tapped);
            assert!(engine.state.stack.is_empty());
        }
    }
}

#[test]
fn gemstone_actual_removing_last_luck_restores_only_colorless() {
    let mut engine = game();
    let source = inject_permanent_on_battlefield(&mut engine, 0, ID);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Luck, 1);
    // Prepared counter removal isolates the live output condition; no claim of a separate card.
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Luck, 0);
    let invalid = mana(&engine, source, 4);
    reject(&mut engine, 0, &invalid);
    let command = mana(&engine, source, 0);
    engine.apply_scenario_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
}

#[test]
fn gemstone_actual_opening_uses_printed_profile_and_exact_exile_then_finishes() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        705_111,
        &[0, 1],
        20,
        Some(vec![deck_with(ID, &[]); 2]),
        false,
    )
    .unwrap();
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine
        .apply_scenario_command(
            chooser,
            &RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(rv1::ChooseStartingPlayer {
                    starting_player_id: 0,
                })),
            },
        )
        .unwrap();
    for actor in [0, 1] {
        engine
            .apply_scenario_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::Mulligan(rv1::MulliganDecision { keep: true })),
                },
            )
            .unwrap();
    }
    let first = engine.state.players[1].hand[0];
    let other = engine.state.players[1].hand[1];
    let reference = |id| rv1::CostObjectRef {
        object_id: id,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&id)
            .copied()
            .unwrap_or(0),
    };
    let entry = RuledCommand {
        cmd: Some(Cmd::BeginWithGemstoneCaverns(
            rv1::BeginWithGemstoneCaverns {
                source: Some(reference(first)),
                exile: Some(reference(other)),
            },
        )),
    };
    reject(&mut engine, 0, &entry);
    let batch = engine.apply_scenario_command(1, &entry).unwrap();
    assert_eq!(
        engine.state.objects[&first].counter_count(CounterKind::Luck),
        1
    );
    assert_eq!(engine.state.objects[&first].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&other].zone, Zone::Exile);
    assert!(!engine.state.objects[&other].face_down);
    assert_eq!(engine.state.lands_played_this_turn, 0);
    assert_eq!(
        batch.legal_by_player[&1]
            .opening
            .as_ref()
            .unwrap()
            .gemstone_caverns
            .len(),
        5
    );
    assert!(batch.legal_by_player[&0]
        .opening
        .as_ref()
        .unwrap()
        .gemstone_caverns
        .is_empty());
    engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::FinishOpeningHandActions(
                    rv1::FinishOpeningHandActions {},
                )),
            },
        )
        .unwrap();
    assert!(engine.state.opening.is_none());
    assert!(engine.state.stack.is_empty());
}
