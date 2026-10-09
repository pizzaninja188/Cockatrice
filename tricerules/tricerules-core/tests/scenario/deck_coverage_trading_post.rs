//! Actual-card coverage for all four Trading Post activated abilities.
//!
//! Exact Oracle data and rulings checked against Scryfall on 2026-09-30; Scryfall returned no
//! rulings for this Oracle identity. CR 602.2a-b and 701.21a cover activation and sacrifice costs;
//! CR 115.1c and 608.2b cover the graveyard target; CR 118.3b and 119.4 cover paying life, while
//! CR 119.3 and 701.7a cover life gain and token creation.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, TargetRef, TargetRefKind};

const TRADING_POST: &str = "trading_post";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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

fn graveyard_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        kind: TargetRefKind::Graveyard as i32,
        object_id,
        group_index: 0,
        ..Default::default()
    }]
}

fn with_costs(
    engine: &GameEngine,
    source: u32,
    ability_index: u32,
    targets: Vec<TargetRef>,
    cost_selections: Vec<tricerules_proto::ruled::v1::CostSelection>,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(engine, source, ability_index, targets);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("constructed an ability activation")
    };
    activation.cost_selections = cost_selections;
    command
}

#[test]
fn trading_post_discard_ability_gains_four_life_on_resolution() {
    let mut engine = engine(20_260_930);
    let post = inject_permanent_on_battlefield(&mut engine, 0, TRADING_POST);
    let discarded = engine.state.players[0].hand[0];
    engine.state.players[0].life = 16;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    let command = with_costs(&engine, post, 0, vec![], vec![hand_cost_selection(2, 0)]);
    semantic::accepted(&mut engine, 0, &command);

    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, 16);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
}

#[test]
fn trading_post_life_ability_creates_a_white_goat() {
    let mut engine = engine(20_260_931);
    let post = inject_permanent_on_battlefield(&mut engine, 0, TRADING_POST);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    let command = with_costs(&engine, post, 1, vec![], vec![]);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(engine.state.players[0].life, 19, "life is paid as a cost");
    assert!(battlefield_token_oids(&engine, 0, "goat_w_0_1").is_empty());

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(battlefield_token_oids(&engine, 0, "goat_w_0_1").len(), 1);
}

#[test]
fn trading_post_sacrifices_a_creature_to_return_a_target_artifact() {
    let mut engine = engine(20_260_932);
    let post = inject_permanent_on_battlefield(&mut engine, 0, TRADING_POST);
    let creature = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let artifact = inject_graveyard_card(&mut engine, 0, "sol_ring");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    let command = with_costs(
        &engine,
        post,
        2,
        graveyard_target(artifact),
        vec![permanent_cost_selection(2, creature)],
    );
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&artifact].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&artifact));
}

#[test]
fn trading_post_rejects_invalid_recovery_targets_and_sacrifice_costs_atomically() {
    let mut engine = engine(20_260_934);
    let post = inject_permanent_on_battlefield(&mut engine, 0, TRADING_POST);
    let creature = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let own_nonartifact = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opponent_artifact = inject_graveyard_card(&mut engine, 1, "sol_ring");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    for invalid_target in [own_nonartifact, opponent_artifact] {
        let command = with_costs(
            &engine,
            post,
            2,
            graveyard_target(invalid_target),
            vec![permanent_cost_selection(2, creature)],
        );
        engine
            .apply_command(0, &command)
            .expect_err("the target must be an artifact card in the activator's graveyard");
        assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
        assert!(!engine.state.objects[&post].tapped);
        assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&invalid_target].zone, Zone::Graveyard);
        assert!(engine.state.stack.is_empty());
    }

    let nonartifact_cost = with_costs(
        &engine,
        post,
        3,
        vec![],
        vec![permanent_cost_selection(2, creature)],
    );
    engine
        .apply_command(0, &nonartifact_cost)
        .expect_err("the draw ability requires sacrificing an artifact");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(!engine.state.objects[&post].tapped);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn trading_post_sacrifices_an_artifact_to_draw_a_card() {
    let mut engine = engine(20_260_933);
    let post = inject_permanent_on_battlefield(&mut engine, 0, TRADING_POST);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let hand_before = engine.state.players[0].hand.len();

    let command = with_costs(
        &engine,
        post,
        3,
        vec![],
        vec![permanent_cost_selection(2, artifact)],
    );
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
}
