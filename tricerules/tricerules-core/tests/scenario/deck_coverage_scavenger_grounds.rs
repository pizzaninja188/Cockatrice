//! Actual-card coverage for Scavenger Grounds' colorless mana and graveyard-exile abilities.
//!
//! Oracle and rulings were verified against Scryfall on 2026-09-26, including that the source
//! Desert may pay its own sacrifice cost and is in its owner's graveyard before the effect resolves.
//! CR 305.6 covers land mana abilities, 602.2a-b activation costs, 605.1a mana abilities,
//! 701.21a sacrifice, 701.13a exile, 113.7a the ability's independence from its source, and
//! 608.2h the resolution-time application of the exile effect.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

const SCAVENGER_GROUNDS: &str = "scavenger_grounds";

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn sacrifice_activation(engine: &GameEngine, source: u32, desert: u32) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(source, 1, vec![], vec![permanent_cost_selection(2, desert)]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("constructed an activation command")
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
fn scavenger_grounds_produces_one_colorless_mana_without_the_stack() {
    let mut engine = engine(202_609_320);
    let source = inject_permanent_on_battlefield(&mut engine, 0, SCAVENGER_GROUNDS);

    apply_ability(&mut engine, 0, source, 0, vec![])
        .expect("activate Scavenger Grounds' colorless mana ability");

    let pool = &engine.state.players[0].mana_pool;
    assert_eq!(pool.colorless, 1);
    assert_eq!(
        (pool.white, pool.blue, pool.black, pool.red, pool.green),
        (0, 0, 0, 0, 0)
    );
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(
        engine.state.stack.is_empty(),
        "the mana ability resolves immediately"
    );
}

#[test]
fn scavenger_grounds_rejects_non_deserts_and_opponent_deserts_without_partial_payment() {
    let mut engine = engine(202_609_321);
    let source = inject_permanent_on_battlefield(&mut engine, 0, SCAVENGER_GROUNDS);
    let own_non_desert = inject_permanent_on_battlefield(&mut engine, 0, "island");
    let opponent_desert = inject_permanent_on_battlefield(&mut engine, 1, "lonely_arroyo");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    for (selected, reason) in [
        (own_non_desert, "an Island is not a Desert"),
        (opponent_desert, "the opponent controls this Desert"),
    ] {
        engine
            .apply_command(0, &sacrifice_activation(&engine, source, selected))
            .unwrap_err();
        assert_eq!(engine.state.players[0].mana_pool.colorless, 2, "{reason}");
        assert_eq!(
            engine.state.objects[&source].zone,
            Zone::Battlefield,
            "{reason}"
        );
        assert!(!engine.state.objects[&source].tapped, "{reason}");
        assert_eq!(
            engine.state.objects[&selected].zone,
            Zone::Battlefield,
            "{reason}"
        );
        assert!(engine.state.stack.is_empty(), "{reason}");
    }
}

#[test]
fn scavenger_grounds_sacrifices_another_desert_then_exiles_all_graveyards_at_resolution() {
    let mut engine = engine(202_609_322);
    let source = inject_permanent_on_battlefield(&mut engine, 0, SCAVENGER_GROUNDS);
    let sacrificed_desert = inject_permanent_on_battlefield(&mut engine, 0, "lonely_arroyo");
    let own_before = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opponent_before = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &sacrifice_activation(&engine, source, sacrificed_desert))
        .expect("pay {2}, tap, and sacrifice a Desert");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&sacrificed_desert].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&own_before].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opponent_before].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1, "the effect waits on the stack");

    let own_late = inject_graveyard_card(&mut engine, 0, "forest");
    let opponent_late = inject_graveyard_card(&mut engine, 1, "island");
    resolve_entire_stack_two_player(&mut engine);

    for object in [
        own_before,
        opponent_before,
        own_late,
        opponent_late,
        sacrificed_desert,
    ] {
        assert_eq!(engine.state.objects[&object].zone, Zone::Exile);
    }
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn scavenger_grounds_can_sacrifice_itself_and_exile_its_graveyard_card_on_resolution() {
    let mut engine = engine(202_609_323);
    let source = inject_permanent_on_battlefield(&mut engine, 0, SCAVENGER_GROUNDS);
    let own_before = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opponent_before = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    let activation = sacrifice_activation(&engine, source, source);
    engine
        .apply_command(0, &activation)
        .expect("Scavenger Grounds may sacrifice itself as its Desert cost");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&own_before].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opponent_before].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the ability persists after its source leaves"
    );

    let own_late = inject_graveyard_card(&mut engine, 0, "forest");
    let opponent_late = inject_graveyard_card(&mut engine, 1, "island");
    resolve_entire_stack_two_player(&mut engine);

    for object in [source, own_before, opponent_before, own_late, opponent_late] {
        assert_eq!(engine.state.objects[&object].zone, Zone::Exile);
    }
    assert!(engine.state.stack.is_empty());
}
