use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::TurnStep;

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_command(1, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(engine: &mut GameEngine) -> RuledEventBatch {
    engine
        .apply_command(0, &pass())
        .expect("active player passes after declaring attackers");
    engine
        .apply_command(1, &pass())
        .expect("defender passes after attackers are declared")
}

#[test]
fn frilled_sea_serpent_rejects_blocks_and_drives_automatic_empty_blocks() {
    let mut engine = GameEngine::new(77_001, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let serpent = inject_creature_on_battlefield(&mut engine, 0, "frilled_sea_serpent");
    let ordinary_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 7,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(serpent, 0, vec![]))
        .expect("activate Frilled Sea Serpent");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, serpent),
        vec!["Can't be blocked"],
        "the active unblockable effect is visible in the battlefield feed"
    );
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![serpent, ordinary_attacker]))
        .expect("declare attackers");
    pass_to_declare_blockers(&mut engine);

    let err = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: serpent,
                blocker_id: blocker,
            }]),
        )
        .expect_err("the Serpent cannot be blocked this turn");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: ordinary_attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("the same creature can block the ordinary attacker");

    let mut auto = GameEngine::new(77_002, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut auto);
    let serpent = inject_creature_on_battlefield(&mut auto, 0, "frilled_sea_serpent");
    inject_creature_on_battlefield(&mut auto, 1, "grizzly_bears");
    give_mana(
        &mut auto,
        0,
        ManaGift {
            u: 7,
            ..Default::default()
        },
    );
    auto.apply_command(0, &activate_ability(serpent, 0, vec![]))
        .expect("activate Frilled Sea Serpent");
    resolve_entire_stack_two_player(&mut auto);
    advance_main1_to_declare_attackers(&mut auto);
    auto.apply_command(0, &declare_attackers(vec![serpent]))
        .expect("declare Serpent");
    let batch = pass_to_declare_blockers(&mut auto);
    assert!(
        auto.state
            .combat
            .as_ref()
            .expect("combat")
            .blockers_declared
    );
    assert_eq!(blockers_declared_in(&batch)[0].block_pairs, vec![]);
}

#[test]
fn frilled_sea_serpent_does_not_undo_a_block_declared_before_activation() {
    let mut engine = GameEngine::new(77_003, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let serpent = inject_creature_on_battlefield(&mut engine, 0, "frilled_sea_serpent");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![serpent]))
        .expect("declare Serpent");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: serpent,
                blocker_id: blocker,
            }]),
        )
        .expect("declare block before the Serpent ability resolves");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 7,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability(serpent, 0, vec![]))
        .expect("activate after blockers");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.combat.as_ref().expect("combat").blockers[&serpent],
        vec![blocker]
    );
}

#[test]
fn goblin_smuggler_uses_derived_power_and_revalidates_its_target() {
    let mut engine = GameEngine::new(77_004, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let smuggler = inject_creature_on_battlefield(&mut engine, 0, "goblin_smuggler");
    let small = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let large = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 3, 3);
    let key = (smuggler as u64) << 32;
    let targets = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
        [&key]
        .valid_permanent_ids;
    assert_eq!(targets, &[small]);

    let err = engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(large)))
        .expect_err("power 3 is not a legal target");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert!(!engine.state.objects[&smuggler].tapped);

    engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(small)))
        .expect("activate targeting a power-2 creature");
    engine
        .state
        .objects
        .get_mut(&small)
        .expect("small creature")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);
    resolve_entire_stack_two_player(&mut engine);

    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![small]))
        .expect("declare the now-power-3 creature");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: small,
                blocker_id: blocker,
            }]),
        )
        .expect("the stale target made the ability fail to resolve");
}

#[test]
fn goblin_smuggler_effect_persists_if_power_increases_after_resolution() {
    let mut engine = GameEngine::new(77_005, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let smuggler = inject_creature_on_battlefield(&mut engine, 0, "goblin_smuggler");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let ordinary_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(target)))
        .expect("activate Goblin Smuggler");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, target),
        vec!["Can't be blocked"],
        "the chosen creature reports the resolved combat restriction"
    );
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![target, ordinary_attacker]))
        .expect("declare attackers");
    pass_to_declare_blockers(&mut engine);
    let err = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: target,
                blocker_id: blocker,
            }]),
        )
        .expect_err("the resolved restriction does not recheck power");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
}

#[test]
fn destructive_tampering_tracks_current_flying_status_and_later_creatures() {
    let decks = Some(vec![
        deck_with("mountain", &["destructive_tampering"]),
        deck_with("mountain", &[]),
    ]);
    let mut engine = GameEngine::new(77_006, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "destructive_tampering");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "destructive_tampering");
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, vec![])]))
        .expect("cast the blocking-restriction mode");
    resolve_entire_stack_two_player(&mut engine);

    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let ground = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let bird_grabber = inject_creature_on_battlefield(&mut engine, 1, "goblin_bird-grabber");
    let flyer = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");

    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, ground),
        vec!["Can't block"]
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, bird_grabber),
        vec!["Can't block"]
    );
    assert!(zone_view_rules_annotation_labels(&mut engine, 1, flyer).is_empty());

    engine.apply_command(0, &pass()).expect("pass priority");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(1, &activate_ability(bird_grabber, 0, vec![]))
        .expect("grant Flying after Destructive Tampering resolves");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.effective_has_keyword(bird_grabber, Keyword::Flying));
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, bird_grabber),
        vec!["Flying"],
        "gaining Flying removes the dynamic restriction label but retains the granted keyword"
    );

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_to_declare_blockers(&mut engine);

    let legal = &engine.initial_response_batch().legal_by_player[&1];
    let legal_pairs: Vec<_> = legal
        .legal_block_pairs
        .iter()
        .map(|pair| (pair.blocker_id, pair.attacker_id))
        .collect();
    assert_eq!(legal_pairs, [(bird_grabber, attacker), (flyer, attacker)]);
    let err = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: ground,
            }]),
        )
        .expect_err("a later-entering nonflyer cannot block");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: bird_grabber,
            }]),
        )
        .expect("a creature that gained Flying can block");
}

#[test]
fn cant_be_blocked_coexists_with_menace_and_must_block_requirements() {
    let mut engine = GameEngine::new(77_007, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let smuggler = inject_creature_on_battlefield(&mut engine, 0, "goblin_smuggler");
    let menace = inject_creature_on_battlefield(&mut engine, 0, "goblin_trailblazer");
    let ordinary = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&blocker)
        .expect("blocker")
        .must_block_if_able = true;

    engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(menace)))
        .expect("activate Goblin Smuggler on a creature with menace");
    resolve_entire_stack_two_player(&mut engine);
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![menace, ordinary]))
        .expect("declare attackers");
    pass_to_declare_blockers(&mut engine);

    let legal = &engine.initial_response_batch().legal_by_player[&1];
    assert_eq!(legal.required_blocker_ids, vec![blocker]);
    let legal_pairs: Vec<_> = legal
        .legal_block_pairs
        .iter()
        .map(|pair| (pair.blocker_id, pair.attacker_id))
        .collect();
    assert_eq!(legal_pairs, [(blocker, ordinary)]);
    let err = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: menace,
                blocker_id: blocker,
            }]),
        )
        .expect_err("cant-be-blocked rejects the assignment before menace can matter");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: ordinary,
                blocker_id: blocker,
            }]),
        )
        .expect("must-block is satisfied by the other legal attacker");
}

#[test]
fn legal_block_pairs_exclude_pair_specific_flying_restrictions() {
    let mut engine = GameEngine::new(77_009, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let ground_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let flying_attacker = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    let ground_blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(
            0,
            &declare_attackers(vec![ground_attacker, flying_attacker]),
        )
        .expect("declare mixed attackers");
    pass_to_declare_blockers(&mut engine);

    let legal = &engine.initial_response_batch().legal_by_player[&1];
    let legal_pairs: Vec<_> = legal
        .legal_block_pairs
        .iter()
        .map(|pair| (pair.blocker_id, pair.attacker_id))
        .collect();
    assert_eq!(legal_pairs, [(ground_blocker, ground_attacker)]);
}

#[test]
fn chosen_combat_restriction_expires_at_cleanup() {
    let mut engine = GameEngine::new(77_008, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let smuggler = inject_creature_on_battlefield(&mut engine, 0, "goblin_smuggler");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(target)))
        .expect("activate Goblin Smuggler");
    resolve_entire_stack_two_player(&mut engine);

    end_active_turn(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 1);
    advance_to_main1_from_game_start(&mut engine);

    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![target]))
        .expect("declare the formerly restricted creature");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: target,
                blocker_id: blocker,
            }]),
        )
        .expect("the until-end-of-turn restriction expired during cleanup");
}

#[test]
fn chosen_combat_restriction_does_not_follow_a_zone_change() {
    let decks = Some(vec![
        deck_with("island", &["unsummon"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(77_009, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let smuggler = inject_creature_on_battlefield(&mut engine, 0, "goblin_smuggler");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(target)))
        .expect("activate Goblin Smuggler");
    resolve_entire_stack_two_player(&mut engine);

    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Unsummon");
    resolve_entire_stack_two_player(&mut engine);
    let returned = put_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(
        returned, target,
        "the helper deliberately reuses the ObjectId"
    );

    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![returned]))
        .expect("declare the new object represented by the reused id");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: returned,
                blocker_id: blocker,
            }]),
        )
        .expect("the previous object's restriction was cleared on zone change");
}
