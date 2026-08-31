use crate::helpers::*;

fn ability_texts(engine: &mut GameEngine, oid: u32) -> Vec<String> {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == oid)
                .map(|object| {
                    object
                        .activated_abilities
                        .iter()
                        .map(|ability| ability.text.clone())
                        .collect()
                }),
            _ => None,
        })
        .unwrap_or_default()
}

fn activate_mana_option(permanent_id: u32, ability_index: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability(permanent_id, ability_index, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}

fn remove_permanent(engine: &mut GameEngine, player: usize, oid: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != oid);
    engine.state.players[player].graveyard.push(oid);
    engine.state.objects.get_mut(&oid).expect("permanent").zone = tricerules_core::Zone::Graveyard;
}

#[test]
fn gift_grants_its_land_controller_an_annotated_undoable_mana_ability() {
    let decks = Some(vec![
        deck_with("forest", &["gift_of_paradise"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine =
        GameEngine::new(65_001, &[0, 1], 20, decks, true).expect("Gift of Paradise card data");
    advance_to_main1_from_game_start(&mut engine);
    let opponent_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    ensure_card_in_hand(&mut engine, 0, "gift_of_paradise");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "gift_of_paradise");
    engine
        .apply_command(0, &cast_spell(slot, target_object(opponent_land)))
        .expect("cast Gift on opponent's land");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 23, "Gift's ETB trigger");
    assert_eq!(
        ability_texts(&mut engine, opponent_land),
        vec![
            "Forest — activated ability (activated_01)".to_string(),
            "Forest — activated ability (static_01/activated_01)".to_string(),
        ],
        "printed ability precedes the granted ability"
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, opponent_land),
        vec!["Forest — activated ability (static_01/activated_01)"],
        "only the nonintrinsic ability is annotated"
    );

    engine.apply_command(0, &pass()).expect("pass priority");
    let priority_before = engine.state.priority_player_id();
    let batch = engine
        .apply_command(1, &activate_mana_option(opponent_land, 1, 1))
        .expect("land controller activates the granted blue option");
    assert_eq!(engine.state.players[1].mana_pool.blue, 2);
    assert!(engine.state.objects[&opponent_land].tapped);
    assert!(engine.state.stack.is_empty(), "mana ability does not stack");
    assert_eq!(engine.state.priority_player_id(), priority_before);
    assert_eq!(batch.legal_by_player[&1].undoable_mana_abilities, 1);

    engine
        .apply_command(1, &undo_mana_ability())
        .expect("undo granted mana ability");
    assert_eq!(engine.state.players[1].mana_pool.blue, 0);
    assert!(!engine.state.objects[&opponent_land].tapped);

    let gift = battlefield_object_for_card(&engine, 0, "gift_of_paradise");
    remove_permanent(&mut engine, 0, gift);
    assert_eq!(
        ability_texts(&mut engine, opponent_land),
        vec!["Forest — activated ability (activated_01)"],
        "the grant disappears with its Aura"
    );
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 1, opponent_land).is_empty(),
        "the Effects label disappears with its Aura"
    );
    assert!(
        engine
            .apply_command(1, &activate_mana_option(opponent_land, 1, 0))
            .is_err(),
        "the former granted index is no longer legal"
    );
}

#[test]
fn hermetic_study_ability_survives_removal_of_the_granting_aura() {
    let decks = Some(vec![
        deck_with("island", &["hermetic_study"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine =
        GameEngine::new(65_002, &[0, 1], 20, decks, true).expect("Hermetic Study card data");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "hermetic_study");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "hermetic_study");
    engine
        .apply_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast Hermetic Study");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        ability_texts(&mut engine, creature),
        vec!["Grizzly Bears — activated ability (static_01/activated_01)"]
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, creature),
        vec!["Grizzly Bears — activated ability (static_01/activated_01)"]
    );
    engine
        .apply_command(0, &activate_ability(creature, 0, target_player(1)))
        .expect("activate granted damage ability");
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .and_then(|item| item.source_permanent_id),
        Some(creature),
        "the enchanted creature is the ability source"
    );

    let study = battlefield_object_for_card(&engine, 0, "hermetic_study");
    remove_permanent(&mut engine, 0, study);
    assert!(ability_texts(&mut engine, creature).is_empty());
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, creature).is_empty());

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[1].life, 19,
        "the captured granted ability resolves after its Aura leaves"
    );
}
