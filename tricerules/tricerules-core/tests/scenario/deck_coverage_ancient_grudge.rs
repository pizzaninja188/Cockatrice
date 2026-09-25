//! Exact deck-corpus coverage for Ancient Grudge.
//!
//! Oracle and rulings checked 2026-09-25. CR 115.1a governs choosing the artifact target,
//! CR 601.2 governs casting with an alternative cost, CR 701.8 governs destroying the permanent,
//! and CR 702.34 governs casting from the graveyard and exiling the spell after it leaves the stack.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, CastMethod, CastSpell};

const ANCIENT_GRUDGE: &str = "ancient_grudge";

#[test]
fn ancient_grudge_destroys_an_artifact_and_uses_flashback() {
    let mut engine = GameEngine::new(20_260_942, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let first_artifact_creature = inject_creature_on_battlefield(&mut engine, 1, "ornithopter");
    let second_artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let spell = inject_card_into_hand(&mut engine, 0, ANCIENT_GRUDGE);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, ANCIENT_GRUDGE);

    let command_index = engine.state.command_index;
    engine
        .apply_command(0, &cast_spell_face(slot, target_object(creature), 0))
        .expect_err("Ancient Grudge cannot target a creature");
    assert_eq!(engine.state.command_index, command_index);

    semantic::accepted(
        &mut engine,
        0,
        &cast_spell_face(slot, target_object(first_artifact_creature), 0),
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&first_artifact_creature].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&second_artifact].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);

    let generation = engine.state.zone_change_generation[&spell];
    let flashback = tricerules_proto::ruled::v1::RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            targets: target_object(second_artifact),
            ..Default::default()
        })),
    };

    let command_index = engine.state.command_index;
    engine
        .apply_command(0, &flashback)
        .expect_err("flashback cannot be cast without paying its green alternative cost");
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    semantic::accepted(&mut engine, 0, &flashback);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&second_artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Exile);
    assert!(
        engine.apply_command(0, &flashback).is_err(),
        "the exiled physical spell cannot be flashback-cast again"
    );
}
