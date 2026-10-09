//! Actual-card coverage for Trailtracker Scout's mana ability and expend trigger.
//!
//! Exact Oracle text and three WotC rulings checked against Scryfall on 2026-09-30.
//! CR 700.14 governs the once-per-turn expend threshold; CR 115.1d/603.3d and 608.2b
//! govern choosing and revalidating the optional graveyard target.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, TargetRef, TargetRefKind,
};

const TRAILTRACKER_SCOUT: &str = "trailtracker_scout";
type ManaPool = (u32, u32, u32, u32, u32, u32);

fn mana_pool(engine: &GameEngine) -> ManaPool {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                "grizzly_bears",
                "grizzly_bears",
                "grizzly_bears",
                "grizzly_bears",
                "grizzly_bears",
                "grizzly_bears",
            ],
        ),
        deck_with("island", &[]),
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

fn choose_graveyard_target(object_id: u32) -> tricerules_proto::ruled::v1::RuledCommand {
    tricerules_proto::ruled::v1::RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                kind: TargetRefKind::Graveyard as i32,
                object_id,
                group_index: 0,
                ..Default::default()
            }],
            ..Default::default()
        })),
    }
}

fn choose_no_trigger_target() -> tricerules_proto::ruled::v1::RuledCommand {
    tricerules_proto::ruled::v1::RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            ..Default::default()
        })),
    }
}

fn cast_two_mana_spell(engine: &mut GameEngine) {
    ensure_in_hand(engine, 0, "grizzly_bears");
    give_mana(
        engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let command = cast_spell(hand_index_for_card(engine, 0, "grizzly_bears"), vec![]);
    semantic::accepted(engine, 0, &command);
}

fn cast_to_eight_mana_spent(engine: &mut GameEngine) {
    for spell in 0..4 {
        cast_two_mana_spell(engine);
        if spell < 3 {
            assert!(engine.state.pending_triggers.is_empty());
            resolve_entire_stack_two_player(engine);
        }
    }
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        8
    );
    assert_eq!(engine.state.pending_triggers.len(), 1);
}

#[test]
fn trailtracker_scout_returns_a_target_permanent_card_at_expend_eight() {
    let mut engine = engine(20_260_930);
    let scout = inject_permanent_on_battlefield(&mut engine, 0, TRAILTRACKER_SCOUT);
    let characteristics = engine
        .characteristics(scout)
        .expect("scout characteristics");
    assert_eq!(characteristics.names, ["Trailtracker Scout"]);
    let mut types = characteristics.types;
    types.sort();
    assert_eq!(types, ["Creature", "Raccoon", "Scout"]);
    assert_eq!(characteristics.power, Some(1));
    assert_eq!(characteristics.toughness, Some(3));
    assert_eq!(characteristics.mana_value, 2);
    let artifact = inject_graveyard_card(&mut engine, 0, "sol_ring");
    let land = inject_graveyard_card(&mut engine, 0, "forest");
    let own_instant = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    let opponent_artifact = inject_graveyard_card(&mut engine, 1, "sol_ring");

    cast_to_eight_mana_spent(&mut engine);

    let ability_key = u64::from(scout) << 32;
    let valid_targets = &engine.initial_response_batch().legal_by_player[&0]
        .valid_targets_by_ability[&ability_key]
        .groups[0]
        .valid_graveyard_ids;
    assert!(valid_targets.contains(&artifact));
    assert!(valid_targets.contains(&land));
    assert!(!valid_targets.contains(&own_instant));
    assert!(!valid_targets.contains(&opponent_artifact));

    let illegal = choose_graveyard_target(own_instant);
    assert!(engine.apply_command(0, &illegal).is_err());
    let illegal = choose_graveyard_target(opponent_artifact);
    assert!(engine.apply_command(0, &illegal).is_err());

    engine
        .apply_command(0, &choose_graveyard_target(land))
        .expect("choose a permanent card from the controller's graveyard");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&land].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&land));
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&own_instant].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&opponent_artifact].zone,
        Zone::Graveyard
    );

    // The threshold triggers once per turn, even if more mana is spent on spells.
    cast_two_mana_spell(&mut engine);
    assert!(engine.state.pending_triggers.is_empty());
    resolve_entire_stack_two_player(&mut engine);
}

#[test]
fn trailtracker_scout_can_decline_its_optional_target() {
    let mut engine = engine(20_260_938);
    inject_permanent_on_battlefield(&mut engine, 0, TRAILTRACKER_SCOUT);
    let artifact = inject_graveyard_card(&mut engine, 0, "sol_ring");
    cast_to_eight_mana_spent(&mut engine);

    engine
        .apply_command(0, &choose_no_trigger_target())
        .expect("choose no target for the up-to-one group");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert!(!engine.state.players[0].hand.contains(&artifact));
}

#[test]
fn trailtracker_scout_adds_the_chosen_color_without_using_the_stack() {
    let expected = [
        (1, 0, 0, 0, 0, 0),
        (0, 1, 0, 0, 0, 0),
        (0, 0, 1, 0, 0, 0),
        (0, 0, 0, 1, 0, 0),
        (0, 0, 0, 0, 1, 0),
    ];

    for (option, expected_pool) in expected.into_iter().enumerate() {
        let mut engine = engine(20_260_931 + option as u64);
        let scout = inject_permanent_on_battlefield(&mut engine, 0, TRAILTRACKER_SCOUT);
        let mut command = activate_ability_for(&engine, scout, 0, vec![]);
        let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
            unreachable!("constructed a mana ability activation")
        };
        activation.mana_option_index = option as u32;

        engine
            .apply_command(0, &command)
            .expect("tap Trailtracker Scout and choose a color");
        assert_eq!(mana_pool(&engine), expected_pool, "mana option {option}");
        assert!(engine.state.objects[&scout].tapped);
        assert!(engine.state.stack.is_empty());
    }
}
