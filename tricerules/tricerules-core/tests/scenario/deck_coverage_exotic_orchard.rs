//! Exact Exotic Orchard identity and its opponent-land mana ability.
//!
//! The pinned Oracle line and five Wizards-origin rulings were checked 2026-10-06. CR 106.1a-b,
//! 106.5, 106.7 and 605.2 govern color filtering, current output, ignored costs and zero output.

use super::helpers::*;
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, CardRegistry, Layout, ManaAmount,
    SpellEffectKind,
};
use tricerules_core::{GameEngine, Zone};

fn orchard_engine(seed: u64, player_ids: &[i32]) -> GameEngine {
    let decks = Some(
        player_ids
            .iter()
            .map(|_| deck_with("forest", &[]))
            .collect(),
    );
    let mut engine = GameEngine::new(seed, player_ids, 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn orchard_engine_with_mana_reflection(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["mana_reflection"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "mana_reflection");
    engine
}

fn orchard_ability_info(
    engine: &mut GameEngine,
    source: u32,
) -> tricerules_proto::ruled::v1::AbilityInfo {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == source)
                .and_then(|object| object.activated_abilities.first())
                .cloned(),
            _ => None,
        })
        .expect("Orchard publishes its activated mana ability")
}

fn activate_mana_option(engine: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn mana_pool(engine: &GameEngine, player: usize) -> ManaPool {
    let pool = &engine.state.players[player].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

#[test]
fn exotic_orchard_registers_exact_identity_and_dynamic_tap_mana_ability() {
    let card = CardRegistry::global()
        .get("exotic_orchard")
        .expect("reviewed Exotic Orchard definition");
    assert_eq!(card.id, "exotic_orchard");
    assert_eq!(card.name, "Exotic Orchard");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "exotic_orchard");
    assert_eq!(face.mana_cost.to_string(), "");
    assert_eq!(face.types, vec!["Land".to_string()]);
    assert!(face.colors().is_empty());
    assert_eq!(face.activated_abilities.len(), 1);
    let ability = &face.activated_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "produce_opponent_land_mana");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(ability.costs, vec![AbilityCost::Tap]);
    assert!(ability.is_mana_ability());
    assert_eq!(ability.mana_options().unwrap(), &Vec::<ManaAmount>::new());
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::ProduceManaFromOpponentLands { options }] if options.is_empty()
    ));
}

#[test]
fn exotic_orchard_can_tap_for_zero_when_opponents_have_only_colorless_lands() {
    let mut engine = orchard_engine(20_261_006, &[0, 1]);
    let orchard = inject_permanent_on_battlefield(&mut engine, 0, "exotic_orchard");
    inject_permanent_on_battlefield(&mut engine, 1, "terrain_generator");

    let info = orchard_ability_info(&mut engine, orchard);
    assert!(info.is_mana_ability);
    assert!(info.mana_produced.is_empty());
    let before = engine.state.command_index;
    let command = activate_mana_option(&engine, orchard, 0);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(engine.state.command_index, before + 1);
    assert!(engine.state.objects[&orchard].tapped);
    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 0, 0));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn exotic_orchard_cycles_require_a_seed_and_follow_later_land_changes() {
    let mut engine = orchard_engine(20_261_007, &[0, 1, 2]);
    let orchard = inject_permanent_on_battlefield(&mut engine, 0, "exotic_orchard");
    inject_permanent_on_battlefield(&mut engine, 1, "exotic_orchard");
    inject_permanent_on_battlefield(&mut engine, 2, "exotic_orchard");

    let info = orchard_ability_info(&mut engine, orchard);
    assert!(info.is_mana_ability);
    assert!(
        info.mana_produced.is_empty(),
        "unseeded cycles produce no color"
    );

    inject_permanent_on_battlefield(&mut engine, 2, "forest");
    let info = orchard_ability_info(&mut engine, orchard);
    assert_eq!(info.mana_produced, "G");
    let invalid = engine.apply_command(0, &activate_mana_option(&engine, orchard, 1));
    assert!(invalid.is_err(), "a one-option Orchard rejects index 1");
    assert!(!engine.state.objects[&orchard].tapped);
    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 0, 0));

    let command = activate_mana_option(&engine, orchard, 0);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 1, 0));
    assert!(
        engine.state.stack.is_empty(),
        "the mana ability uses no stack"
    );
    assert_eq!(engine.state.objects[&orchard].zone, Zone::Battlefield);
}

#[test]
fn exotic_orchard_uses_the_wubrg_union_of_opponents_lands_ignoring_costs_and_restrictions() {
    let mut engine = orchard_engine(20_261_008, &[0, 1, 2, 3]);
    let orchard = inject_permanent_on_battlefield(&mut engine, 0, "exotic_orchard");
    inject_permanent_on_battlefield(&mut engine, 1, "forest");
    inject_permanent_on_battlefield(&mut engine, 2, "island");
    let beacon = inject_permanent_on_battlefield(&mut engine, 3, "interplanar_beacon");
    engine.state.objects.get_mut(&beacon).unwrap().tapped = true;

    let info = orchard_ability_info(&mut engine, orchard);
    assert!(info.is_mana_ability);
    assert_eq!(info.mana_produced, "W/U/B/R/G");
    let command = activate_mana_option(&engine, orchard, 3);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 1, 0, 0));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn mana_reflection_doubles_exotic_orchards_tap_output() {
    let mut engine = orchard_engine_with_mana_reflection(20_261_009);
    let orchard = inject_permanent_on_battlefield(&mut engine, 0, "exotic_orchard");
    inject_permanent_on_battlefield(&mut engine, 1, "forest");

    let info = orchard_ability_info(&mut engine, orchard);
    assert_eq!(info.mana_produced, "GG");
    let command = activate_mana_option(&engine, orchard, 0);
    semantic::accepted(&mut engine, 0, &command);

    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 2, 0));
    assert!(engine.state.objects[&orchard].tapped);
    assert!(engine.state.stack.is_empty());
}
