use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, CounterKind, EffectDuration, Keyword};
use tricerules_core::{
    state::{AffectedScope, ContinuousEffect},
    GameEngine, TurnStep,
};
use tricerules_proto::ruled::v1::BlockPair;

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    let active_player = engine.state.active_player_id();
    let defending_player = engine
        .state
        .sole_defending_player_id()
        .expect("sole defending player");
    engine
        .apply_command(active_player, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_command(active_player, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_command(defending_player, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(engine: &mut GameEngine) -> RuledEventBatch {
    let active_player = engine.state.active_player_id();
    let defending_player = engine
        .state
        .sole_defending_player_id()
        .expect("sole defending player");
    engine
        .apply_command(active_player, &pass())
        .expect("active player passes after declaring attackers");
    engine
        .apply_command(defending_player, &pass())
        .expect("defender passes after attackers are declared")
}

#[test]
fn dark_endurance_reduces_only_for_a_blocking_target() {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("swamp", &["dark_endurance", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(160_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 1, "dark_endurance");
    let attacker = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare blocker");
    engine
        .apply_command(0, &pass())
        .expect("active player pass");

    let slot = hand_index_for_card(&engine, 1, "dark_endurance");
    let published = &engine.initial_response_batch().legal_by_player[&1].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    let reduction = published
        .targeted_cost_reduction_applications
        .first()
        .expect("blocking-target reduction");
    assert_eq!(reduction.generic_mana, 1);
    assert!(reduction
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == blocker));
    assert!(!reduction
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == attacker));

    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let command_before = engine.state.command_index;
    assert!(engine
        .apply_command(1, &cast_spell(slot, target_object(attacker)))
        .is_err());
    assert_eq!(engine.state.command_index, command_before);
    assert_eq!(engine.state.players[1].mana_pool.black, 1);

    engine
        .apply_command(1, &cast_spell(slot, target_object(blocker)))
        .expect("blocking target reduces Dark Endurance to {B}");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.characteristics(blocker).unwrap().power, Some(4));
    assert!(engine.effective_has_keyword(blocker, Keyword::Indestructible));
}

#[test]
fn blocker_power_evasion_uses_derived_power_and_composes_with_flying() {
    let decks = Some(vec![
        deck_with("forest", &["foggy_swamp_vinebender"]),
        deck_with(
            "forest",
            &["giant_spider", "giant_spider", "elfsworn_giant"],
        ),
    ]);
    let mut engine = GameEngine::new(160_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let vinebender = move_ready_to_battlefield(&mut engine, 0, "foggy_swamp_vinebender");
    let low_reach = relocate_to_battlefield(&mut engine, 1, "giant_spider", false);
    let boosted_reach = relocate_to_battlefield(&mut engine, 1, "giant_spider", false);
    let high_reach = relocate_to_battlefield(&mut engine, 1, "elfsworn_giant", false);
    let timestamp = engine.state.command_index;
    engine
        .state
        .objects
        .get_mut(&boosted_reach)
        .expect("boosted spider")
        .add_counters(CounterKind::PlusOnePlusOne, 1, timestamp);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(vinebender),
        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Flying),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![vinebender]))
        .expect("declare Vinebender");
    pass_to_declare_blockers(&mut engine);

    let legal = &engine.initial_response_batch().legal_by_player[&1];
    let legal_blockers = legal
        .legal_block_pairs
        .iter()
        .map(|pair| pair.blocker_id)
        .collect::<Vec<_>>();
    assert_eq!(legal_blockers, vec![boosted_reach, high_reach]);
    assert!(engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: vinebender,
                blocker_id: low_reach,
            }]),
        )
        .is_err());
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: vinebender,
                blocker_id: boosted_reach,
            }]),
        )
        .expect("derived power 3 and reach satisfy both restrictions");
}

#[test]
fn maximum_blocker_count_rejects_two_without_mutating_combat() {
    let decks = Some(vec![
        deck_with("forest", &["safewright_cavalry"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(160_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let cavalry = move_ready_to_battlefield(&mut engine, 0, "safewright_cavalry");
    let first = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![cavalry]))
        .expect("declare Cavalry");
    pass_to_declare_blockers(&mut engine);

    let command_before = engine.state.command_index;
    assert!(engine
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: cavalry,
                    blocker_id: first,
                },
                BlockPair {
                    attacker_id: cavalry,
                    blocker_id: second,
                },
            ]),
        )
        .is_err());
    assert_eq!(engine.state.command_index, command_before);
    assert!(engine
        .state
        .combat
        .as_ref()
        .expect("combat")
        .blockers
        .is_empty());
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: cavalry,
                blocker_id: first,
            }]),
        )
        .expect("one blocker is legal");
}

#[test]
fn maximum_one_plus_menace_makes_blocking_impossible_even_with_requirements() {
    let decks = Some(vec![
        deck_with("forest", &["safewright_cavalry"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(160_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let cavalry = move_ready_to_battlefield(&mut engine, 0, "safewright_cavalry");
    let first = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    for blocker in [first, second] {
        engine
            .state
            .objects
            .get_mut(&blocker)
            .expect("blocker")
            .must_block_if_able = true;
    }
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(cavalry),
        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Menace),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![cavalry]))
        .expect("declare Cavalry");
    let batch = pass_to_declare_blockers(&mut engine);
    assert!(
        engine
            .state
            .combat
            .as_ref()
            .expect("combat")
            .blockers_declared
    );
    assert_eq!(blockers_declared_in(&batch)[0].block_pairs, vec![]);
    assert!(engine.initial_response_batch().legal_by_player[&1]
        .required_blocker_ids
        .is_empty());
}

#[test]
fn blocker_caps_are_per_attacker_and_player_ids_are_not_seat_indices() {
    let decks = Some(vec![
        deck_with("forest", &["safewright_cavalry", "safewright_cavalry"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(160_005, &[10, 20], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let first_attacker = move_ready_to_battlefield(&mut engine, 0, "safewright_cavalry");
    let second_attacker = move_ready_to_battlefield(&mut engine, 0, "safewright_cavalry");
    let first_blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let second_blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(
            10,
            &declare_attackers(vec![first_attacker, second_attacker]),
        )
        .expect("declare both attackers");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            20,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: first_attacker,
                    blocker_id: first_blocker,
                },
                BlockPair {
                    attacker_id: second_attacker,
                    blocker_id: second_blocker,
                },
            ]),
        )
        .expect("one blocker on each attacker respects both independent caps");
}

#[test]
fn removing_safewright_cavalrys_abilities_removes_its_blocker_cap() {
    let decks = Some(vec![
        deck_with("forest", &["safewright_cavalry"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(160_006, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let cavalry = move_ready_to_battlefield(&mut engine, 0, "safewright_cavalry");
    let first = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(cavalry),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![cavalry]))
        .expect("declare Cavalry");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: cavalry,
                    blocker_id: first,
                },
                BlockPair {
                    attacker_id: cavalry,
                    blocker_id: second,
                },
            ]),
        )
        .expect("ability removal clears the maximum-one restriction");
}

#[test]
fn safewright_cavalry_targets_only_an_elf_with_its_pump_ability() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &["safewright_cavalry", "llanowar_elves", "grizzly_bears"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(160_007, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let cavalry = move_ready_to_battlefield(&mut engine, 0, "safewright_cavalry");
    let elf = relocate_to_battlefield(&mut engine, 0, "llanowar_elves", false);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );

    let command_before = engine.state.command_index;
    assert!(engine
        .apply_command(
            0,
            &activate_ability_for(&engine, cavalry, 0, target_object(bear))
        )
        .is_err());
    assert_eq!(engine.state.command_index, command_before);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 5);
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, cavalry, 0, target_object(elf)),
        )
        .expect("activate targeting an Elf");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.characteristics(elf).unwrap().power, Some(3));
    assert_eq!(engine.characteristics(elf).unwrap().toughness, Some(3));
}
