//! Exact deck-corpus coverage for Phyrexia's Core and Slobad, Goblin Tinkerer.
//!
//! Scryfall Oracle and WotC-sourced rulings checked 2026-09-28. CR 605.1a and 605.3b govern
//! Phyrexia's Core's immediate colorless mana ability. CR 602.2b and 701.21a govern artifact
//! sacrifice as an activation cost; CR 608.2b governs a target sacrificed to pay that cost.
//! CR 702.12b and 514.2 govern Slobad's temporary indestructible effect and marked-damage cleanup.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, RuledCommand};

fn sacrifice_artifact_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "phyrexias_core",
                "slobad,_goblin_tinkerer",
                "sol_ring",
                "mind_stone",
                "myr_retriever",
                "grizzly_bears",
            ],
        ),
        deck_with(
            "mountain",
            &["shock", "disenchant", "sol_ring", "mind_stone"],
        ),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn ability_with_permanent_cost(
    engine: &GameEngine,
    source: u32,
    ability_index: u32,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
    selected_cost_permanent: u32,
    cost_index: u32,
) -> RuledCommand {
    let mut command = activate_ability_with_costs(
        source,
        ability_index,
        targets,
        vec![permanent_cost_selection(
            cost_index,
            selected_cost_permanent,
        )],
    );
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("constructed an ability activation")
    };
    activation.expected_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    command
}

#[test]
fn deck_coverage_phyrexias_core_adds_colorless_mana_without_the_stack() {
    let mut engine = sacrifice_artifact_engine(20_260_929);
    let core = move_ready_to_battlefield(&mut engine, 0, "phyrexias_core");

    let activation = activate_ability_for(&engine, core, 0, vec![]);
    semantic::accepted(&mut engine, 0, &activation);

    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.objects[&core].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "the mana ability resolves immediately"
    );

    let mana_before = engine.state.players[0].mana_pool.colorless;
    engine
        .apply_command(0, &activate_ability_for(&engine, core, 0, vec![]))
        .expect_err("a tapped Core cannot activate its mana ability again");
    assert_eq!(engine.state.players[0].mana_pool.colorless, mana_before);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn deck_coverage_phyrexias_core_pays_its_artifact_cost_before_gaining_life() {
    let mut engine = sacrifice_artifact_engine(20_260_930);
    let core = move_ready_to_battlefield(&mut engine, 0, "phyrexias_core");
    let artifact = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine.state.players[0].life = 12;
    let command = ability_with_permanent_cost(&engine, core, 1, vec![], artifact, 2);

    semantic::accepted(&mut engine, 0, &command);

    assert!(engine.state.objects[&core].tapped);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        engine.state.players[0].life, 12,
        "life gain waits for resolution"
    );
    assert_eq!(engine.state.stack.len(), 1, "life abilities use the stack");

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 13);
    assert_eq!(engine.state.objects[&core].zone, Zone::Battlefield);
}

#[test]
fn deck_coverage_phyrexias_core_rejects_nonartifact_and_opponent_costs_atomically() {
    let mut engine = sacrifice_artifact_engine(20_260_931);
    let core = move_ready_to_battlefield(&mut engine, 0, "phyrexias_core");
    let creature = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_artifact = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    for invalid_cost_permanent in [creature, opponent_artifact] {
        let before_command = engine.state.command_index;
        let command =
            ability_with_permanent_cost(&engine, core, 1, vec![], invalid_cost_permanent, 2);
        engine
            .apply_command(0, &command)
            .expect_err("only an artifact controlled by the activator can be sacrificed");
        assert_eq!(engine.state.command_index, before_command);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
        assert!(!engine.state.objects[&core].tapped);
        assert_eq!(
            engine.state.objects[&invalid_cost_permanent].zone,
            Zone::Battlefield
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn deck_coverage_slobad_saves_an_artifact_creature_and_clears_damage_at_cleanup() {
    let mut engine = sacrifice_artifact_engine(20_260_932);
    let slobad = move_ready_to_battlefield(&mut engine, 0, "slobad,_goblin_tinkerer");
    let artifact_cost = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let myr = move_ready_to_battlefield(&mut engine, 0, "myr_retriever");
    engine
        .state
        .objects
        .get_mut(&slobad)
        .unwrap()
        .summoning_sick = true;
    relocate_to_hand(&mut engine, 1, "shock");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &pass())
        .expect("active player passes priority");
    let shock_slot = hand_index_for_card(&engine, 1, "shock");
    semantic::accepted(&mut engine, 1, &cast_spell(shock_slot, target_object(myr)));
    assert_eq!(engine.state.stack.len(), 1);
    engine
        .apply_command(1, &pass())
        .expect("opponent passes with Shock on the stack");

    let activation =
        ability_with_permanent_cost(&engine, slobad, 0, target_object(myr), artifact_cost, 0);
    semantic::accepted(&mut engine, 0, &activation);
    assert_eq!(engine.state.objects[&artifact_cost].zone, Zone::Graveyard);
    assert!(
        !engine.state.objects[&slobad].tapped,
        "the ability has no tap cost"
    );

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&myr].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&myr].damage, 2);
    assert!(engine.effective_has_keyword(myr, Keyword::Indestructible));

    end_active_turn(&mut engine, 0);

    assert_eq!(engine.state.objects[&myr].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&myr].damage, 0);
    assert!(!engine.effective_has_keyword(myr, Keyword::Indestructible));
}

#[test]
fn deck_coverage_slobad_can_target_an_artifact_sacrificed_to_pay_its_cost() {
    let mut engine = sacrifice_artifact_engine(20_260_933);
    let slobad = move_ready_to_battlefield(&mut engine, 0, "slobad,_goblin_tinkerer");
    let artifact = move_ready_to_battlefield(&mut engine, 0, "mind_stone");
    let command =
        ability_with_permanent_cost(&engine, slobad, 0, target_object(artifact), artifact, 0);

    semantic::accepted(&mut engine, 0, &command);

    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert!(!engine.effective_has_keyword(artifact, Keyword::Indestructible));
}

#[test]
fn deck_coverage_slobad_rejects_an_invalid_target_or_sacrifice_choice_atomically() {
    let mut engine = sacrifice_artifact_engine(20_260_934);
    let slobad = move_ready_to_battlefield(&mut engine, 0, "slobad,_goblin_tinkerer");
    let artifact = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let creature = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_artifact = move_ready_to_battlefield(&mut engine, 1, "mind_stone");

    let invalid_target =
        ability_with_permanent_cost(&engine, slobad, 0, target_object(creature), artifact, 0);
    engine
        .apply_command(0, &invalid_target)
        .expect_err("Slobad can target only an artifact");
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&slobad].tapped);
    assert!(engine.state.stack.is_empty());

    let invalid_cost = ability_with_permanent_cost(
        &engine,
        slobad,
        0,
        target_object(artifact),
        opponent_artifact,
        0,
    );
    engine
        .apply_command(0, &invalid_cost)
        .expect_err("Slobad can sacrifice only an artifact it controls");
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&opponent_artifact].zone,
        Zone::Battlefield
    );
    assert!(!engine.state.objects[&slobad].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn deck_coverage_slobad_can_target_an_opponents_artifact() {
    let mut engine = sacrifice_artifact_engine(20_260_935);
    let slobad = move_ready_to_battlefield(&mut engine, 0, "slobad,_goblin_tinkerer");
    let artifact_cost = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let opponent_artifact = move_ready_to_battlefield(&mut engine, 1, "mind_stone");
    let activation = ability_with_permanent_cost(
        &engine,
        slobad,
        0,
        target_object(opponent_artifact),
        artifact_cost,
        0,
    );

    semantic::accepted(&mut engine, 0, &activation);
    assert_eq!(engine.state.objects[&artifact_cost].zone, Zone::Graveyard);

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&opponent_artifact].zone,
        Zone::Battlefield
    );
    assert!(engine.effective_has_keyword(opponent_artifact, Keyword::Indestructible));
}

#[test]
fn deck_coverage_sacrifice_artifact_cards_have_exact_registry_faces() {
    let registry = tricerules_cards::registry::global();
    let core = registry.get("phyrexias_core").expect("Core is registered");
    assert_eq!(core.name, "Phyrexia's Core");
    assert_eq!(core.primary_face().name, "Phyrexia's Core");
    assert_eq!(core.primary_face().types, ["Land"]);
    assert_eq!(core.primary_face().activated_abilities.len(), 2);

    let slobad = registry
        .get("slobad,_goblin_tinkerer")
        .expect("Slobad is registered");
    let face = slobad.primary_face();
    assert_eq!(face.name, "Slobad, Goblin Tinkerer");
    assert_eq!(
        face.mana_cost,
        tricerules_cards::ManaCost::parse("{1}{R}").unwrap()
    );
    assert_eq!(face.power, Some(1));
    assert_eq!(face.toughness, Some(2));
    assert_eq!(face.activated_abilities.len(), 1);
}
