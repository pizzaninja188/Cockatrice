//! Arcane Signet and Command Tower choose mana from the engine-bound Commander identity.

use super::helpers::*;
use tricerules_cards::Color;
use tricerules_core::{EngineDeck, GameEngine};

fn commander_mana_engine() -> GameEngine {
    commander_mana_engine_with_commanders(&["atraxa,_praetors_voice"])
}

fn commander_mana_engine_with_commanders(commanders: &[&str]) -> GameEngine {
    let mainboard = [
        "arcane_signet",
        "command_tower",
        "forest",
        "forest",
        "forest",
        "forest",
        "forest",
        "forest",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let decks = Some(vec![
        EngineDeck {
            mainboard,
            commanders: commanders.iter().map(|name| (*name).to_owned()).collect(),
        },
        EngineDeck {
            mainboard: vec!["island".to_owned(); 8],
            commanders: vec!["kami_of_the_crescent_moon".to_owned()],
        },
    ]);
    let mut engine = GameEngine::new_with_commander_decks(62026, &[0, 1], 20, decks, true)
        .expect("both Commander declarations resolve from card data");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn activate_for_mana(engine: &mut GameEngine, source: u32, option_index: u32) {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option_index;
    semantic::accepted(engine, 0, &command);
}

#[test]
fn signet_and_tower_publish_and_apply_commander_identity_choices() {
    let mut engine = commander_mana_engine();
    let signet = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
    let tower = inject_permanent_on_battlefield(&mut engine, 0, "command_tower");

    let batch = engine.initial_response_batch();
    let zone_view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("engine publishes a ruled zone view");
    for source in [signet, tower] {
        let info = zone_view.per_player[0]
            .battlefield_objects
            .iter()
            .find(|object| object.object_id == source)
            .expect("mana source appears in the controller's zone view")
            .activated_abilities
            .first()
            .expect("mana ability appears in the zone view");
        assert!(info.is_mana_ability);
        assert!(info.activatable);
        assert_eq!(info.mana_produced, "W/U/B/G");
    }

    // Atraxa is WUBG: choose B from Signet's third published option and G from Tower's fourth.
    activate_for_mana(&mut engine, signet, 2);
    activate_for_mana(&mut engine, tower, 3);
    assert!(engine.state.objects[&signet].tapped);
    assert!(engine.state.objects[&tower].tapped);
    assert_eq!(engine.state.players[0].mana_pool.black, 1);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
}

#[test]
fn same_player_two_commander_declaration_unions_color_identity_for_mana_choices() {
    // Commander legality such as Partner is not validated by this engine setup path; this
    // acceptance test covers only the engine's declared-card identity union.
    let mut engine =
        commander_mana_engine_with_commanders(&["atraxa,_praetors_voice", "daretti,_scrap_savant"]);
    assert_eq!(
        engine.state.players[0].color_identity,
        [
            Color::White,
            Color::Blue,
            Color::Black,
            Color::Red,
            Color::Green
        ]
    );
    let signet = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
    let tower = inject_permanent_on_battlefield(&mut engine, 0, "command_tower");
    let batch = engine.initial_response_batch();
    let zone_view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("engine publishes a ruled zone view");
    let signet_info = zone_view.per_player[0]
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == signet)
        .unwrap()
        .activated_abilities
        .first()
        .unwrap();
    assert_eq!(signet_info.mana_produced, "W/U/B/R/G");
    activate_for_mana(&mut engine, signet, 3);
    assert_eq!(engine.state.players[0].mana_pool.red, 1);
    let tower_info = zone_view.per_player[0]
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == tower)
        .unwrap()
        .activated_abilities
        .first()
        .unwrap();
    assert_eq!(tower_info.mana_produced, "W/U/B/R/G");
    activate_for_mana(&mut engine, tower, 3);
    assert!(engine.state.objects[&signet].tapped);
    assert!(engine.state.objects[&tower].tapped);
    assert_eq!(engine.state.players[0].mana_pool.red, 2);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn empty_commander_identity_keeps_signet_and_tower_legal_without_adding_mana() {
    // Eldrazi Devastator is a registry-backed colorless identity fixture. Engine deck setup binds
    // identity but does not enforce Commander eligibility, so this does not assert it is a legal
    // Commander card.
    for commanders in [vec![], vec!["eldrazi_devastator"]] {
        let mut engine = commander_mana_engine_with_commanders(&commanders);
        assert!(engine.state.players[0].color_identity.is_empty());
        let signet = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
        let tower = inject_permanent_on_battlefield(&mut engine, 0, "command_tower");
        let batch = engine.initial_response_batch();
        let zone_view = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::ZoneView(view)) => Some(view),
                _ => None,
            })
            .expect("engine publishes a ruled zone view");

        for source in [signet, tower] {
            let info = zone_view.per_player[0]
                .battlefield_objects
                .iter()
                .find(|object| object.object_id == source)
                .unwrap()
                .activated_abilities
                .first()
                .unwrap();
            assert!(info.is_mana_ability);
            assert!(info.activatable);
            assert!(info.mana_produced.is_empty());
        }

        let stack_len = engine.state.stack.len();
        activate_for_mana(&mut engine, signet, 0);
        activate_for_mana(&mut engine, tower, 0);
        assert!(engine.state.objects[&signet].tapped);
        assert!(engine.state.objects[&tower].tapped);
        assert_eq!(engine.state.players[0].mana_pool, Default::default());
        assert_eq!(engine.state.stack.len(), stack_len);
    }
}

#[test]
fn forged_signet_color_choice_is_rejected_without_mutating_game_state() {
    let mut engine = commander_mana_engine();
    let signet = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
    let stack_len = engine.state.stack.len();
    let command_index = engine.state.command_index;
    let mut stale = activate_ability_for(&engine, signet, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation += 1;

    assert!(engine.apply_command(0, &stale).is_err());
    assert!(!engine.state.objects[&signet].tapped);
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    assert_eq!(engine.state.stack.len(), stack_len);
    assert_eq!(engine.state.command_index, command_index);

    let mut forged = activate_ability_for(&engine, signet, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = forged.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 4; // Atraxa's WUBG identity has only four legal choices.

    assert!(engine.apply_command(0, &forged).is_err());
    assert!(!engine.state.objects[&signet].tapped);
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    assert_eq!(engine.state.stack.len(), stack_len);
    assert_eq!(engine.state.command_index, command_index);

    // A rejected forged option leaves the legal choices available and the source usable.
    activate_for_mana(&mut engine, signet, 2);
    assert!(engine.state.objects[&signet].tapped);
    assert_eq!(engine.state.players[0].mana_pool.black, 1);
}
