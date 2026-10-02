//! Deterministic board-complexity guard for the characteristics hot path.

use crate::helpers::{
    advance_to_main1_from_game_start, declare_attackers, declare_blockers,
    inject_creature_on_battlefield, inject_permanent_on_battlefield, pass_both_players,
    primitive_yield, BlockPair, GameEngine,
};
use std::time::{Duration, Instant};
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep};

#[test]
fn big_board_characteristics_full_turn_stays_bounded() {
    const CREATURES_PER_PLAYER: usize = 100;
    const ANTHEM_PAIRS: usize = 10;

    let mut engine = GameEngine::new(0x613_0704, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let attackers: Vec<u32> = (0..CREATURES_PER_PLAYER)
        .map(|_| inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"))
        .collect();
    let blockers: Vec<u32> = (0..CREATURES_PER_PLAYER)
        .map(|_| inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears"))
        .collect();

    // A realistic worst-case mix: several global layer-6 and layer-7 effects, each dynamically
    // re-evaluated for every permanent throughout legal-action generation, combat, event
    // serialization, and SBA checks.
    for timestamp in 0..ANTHEM_PAIRS as u64 {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::AllCreatures,
            kind: ContinuousEffectKind::PtModify {
                delta_power: 1,
                delta_toughness: 1,
            },
            condition: None,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp,
        });
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::AllCreatures,
            kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Vigilance),
            condition: None,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp,
        });
    }

    let started = Instant::now();

    engine
        .apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);

    engine
        .apply_command(0, &declare_attackers(attackers.clone()))
        .expect("declare big attacker set");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);

    let block_pairs = attackers
        .iter()
        .zip(&blockers)
        .map(|(&attacker_id, &blocker_id)| BlockPair {
            attacker_id,
            blocker_id,
        })
        .collect();
    engine
        .apply_command(1, &declare_blockers(block_pairs))
        .expect("declare big blocker set");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::CombatDamage);

    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::EndCombat);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main2);

    engine
        .apply_command(0, &primitive_yield())
        .expect("main2 to end step");
    engine
        .apply_command(0, &primitive_yield())
        .expect("end step through cleanup");
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.active_player_id(), 1);
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);

    let elapsed = started.elapsed();
    if !cfg!(debug_assertions) {
        assert!(
            elapsed < Duration::from_secs(2),
            "big-board full turn took {elapsed:?}, expected < 2s in release"
        );
    }
}

#[test]
fn early_layer_board_characteristics_stays_bounded() {
    use tricerules_cards::primitives::{
        BasicLandType, PermanentTypeFilter, TargetFilter, TargetKind, TypeLineAddition,
        TypeLineReplacement,
    };
    let mut engine = GameEngine::new(0x305_6138, &[0, 1], 20, None, true).unwrap();
    let creatures = (0..24)
        .map(|index| inject_creature_on_battlefield(&mut engine, index % 2, "grizzly_bears"))
        .collect::<Vec<_>>();
    let forests = (0..12)
        .map(|index| inject_permanent_on_battlefield(&mut engine, index % 2, "forest"))
        .collect::<Vec<_>>();
    let islands = (0..12)
        .map(|index| inject_permanent_on_battlefield(&mut engine, index % 2, "island"))
        .collect::<Vec<_>>();
    let mut add_effect = |affected, kind, timestamp| {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected,
            kind,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp,
        })
    };
    add_effect(
        AffectedScope::PermanentsMatching {
            reference_player: 0,
            filter: Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Land],
                ..Default::default()
            }),
            exclude: None,
        },
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            land_types: vec![BasicLandType::Forest],
            ..Default::default()
        }),
        1,
    );
    for (index, &oid) in creatures[..3].iter().enumerate() {
        add_effect(
            AffectedScope::Single(oid),
            ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
                card_types: vec![PermanentTypeFilter::Land],
                creature_types: Vec::new(),
                land_types: Vec::new(),
            }),
            2 + index as u64,
        );
    }
    for &oid in &islands[..2] {
        add_effect(
            AffectedScope::Single(oid),
            ContinuousEffectKind::Layer4SetBasicLandType(BasicLandType::Forest),
            5,
        );
    }
    let objects = creatures
        .iter()
        .chain(&forests)
        .chain(&islands)
        .copied()
        .collect::<Vec<_>>();
    let started = Instant::now();
    for reverse in [false, true, false] {
        for index in 0..objects.len() {
            let oid = objects[if reverse {
                objects.len() - 1 - index
            } else {
                index
            }];
            let result = engine.characteristics(oid).unwrap();
            if creatures[..3].contains(&oid) {
                assert!(
                    result.has_type("Land") && result.has_type("Forest") && !result.is_creature()
                );
            } else if creatures[3..].contains(&oid) {
                assert!(result.is_creature() && !result.has_type("Forest"));
            } else {
                assert!(result.has_type("Land") && result.has_type("Forest"));
                if islands[..2].contains(&oid) {
                    assert!(!result.has_type("Island"));
                }
                if islands[2..].contains(&oid) {
                    assert!(result.has_type("Island"));
                }
            }
            assert!(
                result
                    .types
                    .iter()
                    .filter(|kind| kind.as_str() == "Forest")
                    .count()
                    <= 1
            );
        }
    }
    let elapsed = started.elapsed();
    println!("48-object early-layer dependency queries: {elapsed:?}");
    if !cfg!(debug_assertions) {
        assert!(
            elapsed < Duration::from_secs(2),
            "early-layer board queries took {elapsed:?}, expected < 2s in release"
        );
    }
}
