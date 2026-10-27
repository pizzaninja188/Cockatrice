//! Re-render the actual Solemn search Program against a live Ashiok static.
use super::*;
use crate::state::{TriggerAbilityOrigin, TriggerUseKey};

#[test]
fn ashiok_refreshes_the_existing_solemn_program_search_menu() {
    let registry = tricerules_cards::registry::global();
    let mut engine = GameEngine::new_with_default_decks(registry, 261_027_740, &[0, 1], 20)
        .expect("two-player engine");
    engine.state.opening = None;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;

    // Synthetic physical placement and stack occurrence isolate the renderer; the captured
    // triggered definition and compiled Program are the actual Solemn Simulacrum definition.
    let source = engine.state.players[1]
        .library
        .pop_front()
        .expect("source object");
    engine.state.players[1].battlefield.push(source);
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.card_id = "solemn_simulacrum".into();
    object.zone = Zone::Battlefield;
    object.controller = 1;
    object.base_controller = 1;
    let face = registry.get("solemn_simulacrum").unwrap().primary_face();
    let ability = face.triggered_abilities[0].clone();
    let definition = tricerules_card_model::AbilityDefinitionId {
        card_id: "solemn_simulacrum".into(),
        face_id: face.face_id.clone(),
        ability_path: vec![ability.ability_id.clone()],
    };
    let generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let mut top = engine.observer_return_item(source, 1);
    top.id = engine.state.next_object_id;
    top.ability_text = Some("Solemn entry".into());
    top.source_permanent_id = Some(source);
    top.source_owner = Some(1);
    top.source_zone_change = generation;
    top.trigger_use_key = Some(TriggerUseKey {
        object_id: source,
        zone_change_generation: generation,
        ability_origin: TriggerAbilityOrigin::Printed(definition.clone()),
    });
    top.ability_index = Some(0);
    top.triggered_ability = Some(ability);
    top.is_triggered = true;
    top.is_copy = false;
    engine.state.stack.push(top.clone());
    let owner = tricerules_card_model::ProgramOwnerId::ability(definition);
    let run = engine
        .begin_program_run(&top, vec![owner])
        .expect("actual compiled Program");
    let mut events = Vec::new();
    assert!(matches!(
        engine.run_program_with_previous(&top, run, EffectResult::default(), &mut events),
        Ok(ResolutionProgress::Parked)
    ));

    let menu = |engine: &GameEngine| {
        let event = engine
            .program_choose_choice_event()
            .expect("parked menu renders");
        let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) = event.ev else {
            panic!("expected a resolution menu");
        };
        choice
    };
    assert_eq!(menu(&engine).resolution_branches.len(), 1);
    let ashiok = engine.state.players[0]
        .library
        .pop_front()
        .expect("Ashiok fixture object");
    engine.state.players[0].battlefield.push(ashiok);
    let object = engine.state.objects.get_mut(&ashiok).unwrap();
    object.card_id = "ashiok,_dream_render".into();
    object.zone = Zone::Battlefield;
    object.controller = 0;
    object.base_controller = 0;
    let refreshed = menu(&engine);
    assert!(
        refreshed.resolution_branches.is_empty(),
        "the prohibited branch is not re-rendered"
    );
    let Some(PendingResolution {
        continuation: ResolutionContinuation::ProgramChoose { stack, choose },
        ..
    }) = engine.state.pending_resolution.as_ref()
    else {
        panic!("the original Program choice remains parked");
    };
    assert!(engine
        .program_choose_selected_branch(
            stack,
            &choose.occurrence,
            choose.original_chooser,
            &choose.frozen_game_condition_results,
            0
        )
        .is_err());

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != ashiok);
    engine.state.objects.get_mut(&ashiok).unwrap().zone = Zone::Graveyard;
    assert_eq!(
        menu(&engine).resolution_branches.len(),
        1,
        "the live menu restores the branch"
    );
}
