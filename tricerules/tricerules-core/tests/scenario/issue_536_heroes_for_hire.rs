//! Actual-card scenarios for Heroes for Hire (#536).
//!
//! Expectations come from the printed Oracle text. CR 603.6a stages the enters trigger and
//! CR 111.10a defines the Treasure token. CR 602.2/118.3 govern the "Sacrifice a Treasure"
//! activation cost. "You may play it this turn" lets the controller play the exact exiled
//! object (CR 400.7) until this turn's cleanup step (CR 514.2), after which it stays in exile.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::state::ExilePlayPermissionScope;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1 as rv1;

const EXILE_TOP: u32 = 0;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &["heroes_for_hire"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn seat_on_top(engine: &mut GameEngine, card_id: &str) -> u32 {
    let top = inject_library_card(engine, 0, card_id);
    engine.state.players[0].library.retain(|&oid| oid != top);
    engine.state.players[0].library.push_front(top);
    top
}

fn sacrifice_treasure(
    engine: &mut GameEngine,
    source: u32,
    treasure: u32,
) -> Result<rv1::RuledEventBatch, tricerules_core::EngineError> {
    let mut command = activate_ability_with_costs(
        source,
        EXILE_TOP,
        vec![],
        vec![permanent_cost_selection(0, treasure)],
    );
    let Some(rv1::ruled_command::Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.expected_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    engine.apply_scenario_command(0, &command)
}

fn play_land_from_exile(object_id: u32, generation: u64) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
            resolution_prompt_id: 0,
            source: Some(rv1::LandSource {
                location: Some(rv1::land_source::Location::ExileObjectId(object_id)),
                expected_zone_change_generation: Some(generation),
            }),
            face_index: 0,
        })),
    }
}

fn advance_to_turn_instance(engine: &mut GameEngine, target: u64) {
    let mut commands = 0;
    while engine.state.turn_instance < target {
        resolve_cleanup_discards_if_any(engine);
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("advance turn");
        commands += 1;
        assert!(commands < 200, "turn advancement stalled");
    }
}

/// Enter Heroes for Hire through the real trigger and return (source, treasures).
fn heroes_with_treasures(engine: &mut GameEngine) -> (u32, Vec<u32>) {
    let heroes = move_ready_to_battlefield(engine, 0, "heroes_for_hire");
    assert_eq!(engine.state.stack.len(), 1, "one enters trigger");
    resolve_entire_stack_two_player(engine);
    let treasures = battlefield_token_oids(engine, 0, "treasure");
    (heroes, treasures)
}

#[test]
fn issue_536_enters_and_creates_exactly_three_treasures() {
    let mut engine = engine(536_001);
    let (heroes, treasures) = heroes_with_treasures(&mut engine);
    assert_eq!(treasures.len(), 3, "three Treasure tokens");
    assert_eq!(engine.state.objects[&heroes].zone, Zone::Battlefield);
    assert!(battlefield_token_oids(&engine, 1, "treasure").is_empty());
}

#[test]
fn issue_536_sacrifice_a_treasure_grants_a_this_turn_permission_for_the_exact_card() {
    let mut engine = engine(536_002);
    let (heroes, treasures) = heroes_with_treasures(&mut engine);
    let top = seat_on_top(&mut engine, "grizzly_bears");

    sacrifice_treasure(&mut engine, heroes, treasures[0]).expect("sacrifice a Treasure");
    assert!(
        !engine.state.objects.contains_key(&treasures[0]),
        "the sacrificed Treasure token ceases to exist"
    );
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 2);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&top].zone, Zone::Exile);
    let permission = engine
        .state
        .active_exile_play_permissions
        .iter()
        .find(|permission| permission.object_id == top)
        .expect("play permission for the exiled card");
    assert_eq!(permission.player_id, 0);
    assert_eq!(permission.source_label, "Heroes for Hire");
    assert_eq!(permission.scope, ExilePlayPermissionScope::PlayCard);
    assert_eq!(
        permission.zone_change_generation,
        engine.state.zone_change_generation[&top]
    );
    assert_eq!(
        permission.expires_at_cleanup_turn_instance,
        Some(engine.state.turn_instance),
        "the permission ends at this turn's cleanup step"
    );
    let legal = &engine.initial_response_batch().legal_by_player[&0];
    assert_eq!(legal.exile_play_permission_groups[0].object_ids, [top]);
    assert!(
        legal
            .zone_cast_actions
            .iter()
            .any(|action| action.object_id == top),
        "the exiled spell is castable this turn"
    );
    assert!(engine.initial_response_batch().legal_by_player[&1]
        .exile_play_permission_groups
        .is_empty());
}

#[test]
fn issue_536_exiled_land_can_be_played_this_turn() {
    let mut engine = engine(536_003);
    let (heroes, treasures) = heroes_with_treasures(&mut engine);
    let top = seat_on_top(&mut engine, "mountain");
    sacrifice_treasure(&mut engine, heroes, treasures[0]).expect("sacrifice a Treasure");
    resolve_entire_stack_two_player(&mut engine);

    engine
        .apply_scenario_command(
            0,
            &play_land_from_exile(top, engine.state.zone_change_generation[&top]),
        )
        .expect("play the exiled land this turn");
    assert_eq!(engine.state.objects[&top].zone, Zone::Battlefield);
    assert!(engine.state.active_exile_play_permissions.is_empty());
}

#[test]
fn issue_536_permission_ends_at_cleanup_and_the_card_stays_exiled() {
    let mut engine = engine(536_004);
    let (heroes, treasures) = heroes_with_treasures(&mut engine);
    let top = seat_on_top(&mut engine, "grizzly_bears");
    sacrifice_treasure(&mut engine, heroes, treasures[0]).expect("sacrifice a Treasure");
    resolve_entire_stack_two_player(&mut engine);
    let granted_turn = engine.state.turn_instance;

    advance_to_turn_instance(&mut engine, granted_turn + 1);
    assert!(
        engine.state.active_exile_play_permissions.is_empty(),
        "the opponent's turn has no permission"
    );
    assert_eq!(engine.state.objects[&top].zone, Zone::Exile);

    advance_to_turn_instance(&mut engine, granted_turn + 2);
    assert!(
        !engine.initial_response_batch().legal_by_player[&0]
            .zone_cast_actions
            .iter()
            .any(|action| action.object_id == top),
        "the controller's next turn has no permission either"
    );
    assert_eq!(engine.state.objects[&top].zone, Zone::Exile);
}

#[test]
fn issue_536_only_a_treasure_pays_the_cost() {
    let mut engine = engine(536_005);
    let (heroes, _) = heroes_with_treasures(&mut engine);
    let food = inject_permanent_on_battlefield(&mut engine, 0, "food");
    let top = seat_on_top(&mut engine, "grizzly_bears");

    assert!(
        sacrifice_treasure(&mut engine, heroes, food).is_err(),
        "a Food isn't a Treasure"
    );
    assert!(engine.state.objects.contains_key(&food));
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&top].zone, Zone::Library);
}
