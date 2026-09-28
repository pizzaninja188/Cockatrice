//! Arcane Signet and Command Tower choose mana from the engine-bound Commander identity.

use super::helpers::*;
use tricerules_core::{EngineDeck, GameEngine};

fn commander_mana_engine() -> GameEngine {
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
            commanders: vec!["atraxa,_praetors_voice".to_owned()],
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
