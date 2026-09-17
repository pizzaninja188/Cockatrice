//! Issue #328 — the eight reviewed exile, activation, and trigger cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 508.1/506.4 govern the attacking-only exile and its recheck after the creature leaves
//! combat; CR 118.12a/602.2b/701.21 govern the sacrifice-as-cost enchantment destruction;
//! CR 503.1/603.2b/120.3 govern the upkeep controller-relative self-damage; CR 701.25 governs the
//! private surveil partition; CR 611.2a/514.2 govern the haste grant and its cleanup expiry;
//! CR 110.4a/608.2b govern the permanent-card graveyard return and its stale-target fizzle;
//! CR 509.1b/611.2c govern the bounded can't-block restriction; and CR 611.2a/701.22 govern the
//! shared-target pump + first strike + scry one.

use super::helpers::*;
use tricerules_cards::primitives::ContinuousEffectKind;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, BlockPair, ChoiceKind, ChooseTriggerTarget, RuledCommand, TargetRef,
    TargetRefKind,
};

fn deck_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

/// Put `card_ids` on top of `player`'s library, first entry on top, and return their OIDs.
fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    engine.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        engine.state.players[player].library.push_front(oid);
    }
    oids
}

/// Pass both players once so the next resolution runs; returns the batch that parked or resolved.
fn resolve_to_park(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = 1 - first;
    engine
        .apply_command(first, &pass())
        .expect("first pass on the stack item");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves the stack item")
}

fn advance_to_active_player_upkeep(engine: &mut GameEngine, player: i32) {
    for _ in 0..60 {
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let player_index = engine
                    .state
                    .player_idx(cleanup_player)
                    .expect("cleanup player");
                let excess = engine.state.players[player_index].hand.len() - 7;
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_command(actor, &command)
            .expect("pass toward the requested upkeep");
        if engine.state.active_player_id() == player && engine.state.turn_step == TurnStep::Upkeep {
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}

fn move_graveyard_object_to_exile(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|id| *id != object_id);
    engine.state.players[player].exile.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("graveyard object")
        .zone = Zone::Exile;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn choose_graveyard_target(engine: &mut GameEngine, object_id: u32) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![TargetRef {
                        object_id,
                        damage_amount: 0,
                        group_index: 0,
                        kind: TargetRefKind::Graveyard as i32,
                    }],
                })),
            },
        )
        .expect("choose the graveyard target");
}

fn pending_graveyard_targets(engine: &GameEngine, batch: &RuledEventBatch) -> Vec<u32> {
    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("graveyard target pending");
    let key = (pending.source_permanent_id as u64) << 32 | pending.ability_index as u64;
    batch
        .legal_by_player
        .get(&0)
        .expect("controller legal actions")
        .valid_targets_by_ability
        .get(&key)
        .expect("graveyard legal target group")
        .groups[0]
        .valid_graveyard_ids
        .clone()
}

fn permanent_targets(ids: &[u32]) -> Vec<TargetRef> {
    ids.iter()
        .map(|object_id| TargetRef {
            object_id: *object_id,
            damage_amount: 0,
            group_index: 0,
            kind: TargetRefKind::Permanent as i32,
        })
        .collect()
}

fn has_combat_restriction(engine: &GameEngine) -> bool {
    engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| matches!(effect.kind, ContinuousEffectKind::CombatRestriction(_)))
}

#[test]
fn issue_328_not_on_my_watch_exiles_attackers_and_fizzles_when_they_leave_combat() {
    let mut engine = deck_engine(
        328_001,
        &["not_on_my_watch", "grizzly_bears"],
        &["grizzly_bears"],
    );
    engine
        .apply_command(0, &primitive_yield())
        .expect("main 1 to beginning of combat");
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.apply_command(0, &pass()).expect("active pass");
    engine.apply_command(1, &pass()).expect("defender pass");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut engine, 0, "not_on_my_watch");

    let slot = hand_index_for_card(&engine, 0, "not_on_my_watch");
    assert!(
        engine
            .apply_command(0, &cast_spell(slot, target_object(bystander)))
            .is_err(),
        "CR 508.1: a creature that is not attacking is not a legal target"
    );
    let slot = hand_index_for_card(&engine, 0, "not_on_my_watch");
    engine
        .apply_command(0, &cast_spell(slot, target_object(attacker)))
        .expect("cast at the declared attacker");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&attacker].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&bystander].zone, Zone::Battlefield);

    // A separate game removes the declared attacker from combat before resolution.
    let mut fizzle = deck_engine(
        328_002,
        &["not_on_my_watch", "grizzly_bears"],
        &["grizzly_bears"],
    );
    fizzle
        .apply_command(0, &primitive_yield())
        .expect("main 1 to beginning of combat");
    let leaving = inject_creature_on_battlefield(&mut fizzle, 0, "grizzly_bears");
    fizzle.apply_command(0, &pass()).expect("active pass");
    fizzle.apply_command(1, &pass()).expect("defender pass");
    fizzle
        .apply_command(0, &declare_attackers(vec![leaving]))
        .expect("declare the attacker");
    give_mana(
        &mut fizzle,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut fizzle, 0, "not_on_my_watch");
    let slot = hand_index_for_card(&fizzle, 0, "not_on_my_watch");
    fizzle
        .apply_command(0, &cast_spell(slot, target_object(leaving)))
        .expect("cast at the attacker");
    fizzle
        .state
        .combat
        .as_mut()
        .expect("combat state")
        .attacking
        .retain(|object_id| *object_id != leaving);
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(
        fizzle.state.objects[&leaving].zone,
        Zone::Battlefield,
        "CR 506.4/608.2b: a creature that stops attacking is no longer legal"
    );
    assert!(fizzle.state.stack.is_empty());
}

#[test]
fn issue_328_felidar_cub_sacrifices_itself_to_destroy_only_enchantments() {
    let mut engine = deck_engine(328_010, &["felidar_cub"], &["crusade", "grizzly_bears"]);
    let cub = relocate_to_battlefield(&mut engine, 0, "felidar_cub", false);
    let enchantment = inject_permanent_on_battlefield(&mut engine, 1, "crusade");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    apply_ability(&mut engine, 0, cub, 0, target_object(creature))
        .expect_err("only enchantments are legal targets");
    assert_eq!(engine.state.objects[&cub].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());

    apply_ability(&mut engine, 0, cub, 0, target_object(enchantment))
        .expect("sacrifice the Cub and target the enchantment");
    assert_eq!(
        engine.state.objects[&cub].zone,
        Zone::Graveyard,
        "CR 118.12a: the sacrifice is paid as a cost"
    );
    assert_eq!(engine.state.objects[&enchantment].zone, Zone::Battlefield);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&enchantment].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
}

#[test]
fn issue_328_ravenous_giant_damages_only_its_controller_each_upkeep() {
    let mut engine = deck_engine(328_020, &["ravenous_giant"], &[]);
    relocate_to_battlefield(&mut engine, 0, "ravenous_giant", false);
    advance_to_active_player_upkeep(&mut engine, 0);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 19,
        "CR 120.3: the source's controller loses 1 life"
    );
    assert_eq!(
        engine.state.players[1].life, 20,
        "the opponent is untouched by the untargeted controller-only damage"
    );
}

#[test]
fn issue_328_rune_sealed_wall_taps_for_a_private_surveil_one() {
    let mut engine = deck_engine(328_030, &["rune-sealed_wall"], &[]);
    let wall = move_ready_to_battlefield(&mut engine, 0, "rune-sealed_wall");
    let top = seat_on_top(&mut engine, 0, &["grizzly_bears", "storm_crow"]);
    let library_before = engine.state.players[0].library.len();

    apply_ability(&mut engine, 0, wall, 0, vec![]).expect("activate the {T} surveil ability");
    assert!(engine.state.objects[&wall].tapped, "the tap symbol is paid");
    let parked = resolve_to_park(&mut engine);
    let choice = find_resolution_choice(&parked).expect("surveil parks a private choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(choice.candidate_object_ids, vec![top[0]]);
    assert!(choice.public_reveal.is_none(), "surveil stays private");

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("put the surveilled card into the graveyard");
    assert!(engine.state.players[0].graveyard.contains(&top[0]));
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert!(permanents_moved_in(&completion)
        .iter()
        .any(|moved| moved.object_id == top[0]));
}

#[test]
fn issue_328_axgard_cavalry_taps_to_grant_haste_with_cleanup_expiry() {
    let mut engine = deck_engine(328_040, &["axgard_cavalry"], &["grizzly_bears"]);
    let cavalry = move_ready_to_battlefield(&mut engine, 0, "axgard_cavalry");
    let friendly = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    assert!(!engine.effective_has_keyword(friendly, Keyword::Haste));

    apply_ability(&mut engine, 0, cavalry, 0, vec![])
        .expect_err("the granted creature is a mandatory target");
    assert!(!engine.state.objects[&cavalry].tapped);
    apply_ability(&mut engine, 0, cavalry, 0, target_object(land))
        .expect_err("a land is not a legal creature target");
    assert!(!engine.state.objects[&cavalry].tapped);

    apply_ability(&mut engine, 0, cavalry, 0, target_object(opposing))
        .expect("grant haste to any creature");
    assert!(
        engine.state.objects[&cavalry].tapped,
        "the tap symbol is a cost"
    );
    assert!(
        !engine.effective_has_keyword(opposing, Keyword::Haste),
        "the keyword applies only on resolution"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.effective_has_keyword(opposing, Keyword::Haste));

    end_active_turn(&mut engine, 0);
    assert!(
        !engine.effective_has_keyword(opposing, Keyword::Haste),
        "CR 514.2: the until-end-of-turn grant expires at cleanup"
    );
}

#[test]
fn issue_328_elvish_regrower_returns_controller_permanent_cards_to_hand() {
    let mut engine = deck_engine(328_050, &["elvish_regrower"], &[]);
    let creature_card = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let artifact_card = inject_graveyard_card(&mut engine, 0, "bonesplitter");
    let instant_card = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    let opposing_card = inject_graveyard_card(&mut engine, 1, "storm_crow");

    move_ready_to_battlefield(&mut engine, 0, "elvish_regrower");
    let batch = engine.initial_response_batch();
    let mut candidates = pending_graveyard_targets(&engine, &batch);
    candidates.sort_unstable();
    let mut expected = vec![creature_card, artifact_card];
    expected.sort_unstable();
    assert_eq!(
        candidates, expected,
        "CR 110.4a: only controller-owned permanent cards are legal"
    );
    for illegal in [instant_card, opposing_card] {
        assert!(
            engine
                .apply_command(
                    0,
                    &RuledCommand {
                        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                            decline: false,
                            selected_modes: Vec::new(),
                            targets: vec![TargetRef {
                                object_id: illegal,
                                damage_amount: 0,
                                group_index: 0,
                                kind: TargetRefKind::Graveyard as i32,
                            }],
                        })),
                    },
                )
                .is_err(),
            "nonpermanent and opponent-owned cards are illegal targets"
        );
    }

    choose_graveyard_target(&mut engine, creature_card);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&creature_card].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&creature_card));
    assert_eq!(engine.state.objects[&artifact_card].zone, Zone::Graveyard);

    // A stale graveyard generation makes the chosen instruction fizzle.
    let mut stale = deck_engine(328_051, &["elvish_regrower"], &[]);
    let target = inject_graveyard_card(&mut stale, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut stale, 0, "elvish_regrower");
    let batch = stale.initial_response_batch();
    let _ = pending_graveyard_targets(&stale, &batch);
    choose_graveyard_target(&mut stale, target);
    move_graveyard_object_to_exile(&mut stale, 0, target);
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&target].zone, Zone::Exile);
    assert!(!stale.state.players[0].hand.contains(&target));
    assert!(stale.state.stack.is_empty());
}

#[test]
fn issue_328_beat_a_path_stops_up_to_two_creatures_from_blocking() {
    let mut engine = deck_engine(
        328_060,
        &["bellowing_bruiser_beat_a_path", "grizzly_bears"],
        &["grizzly_bears", "grizzly_bears"],
    );
    let blocker_a = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let blocker_b = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "bellowing_bruiser_beat_a_path");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "bellowing_bruiser_beat_a_path");
    engine
        .apply_command(
            0,
            &cast_spell_face(slot, permanent_targets(&[blocker_a, blocker_b]), 1),
        )
        .expect("cast Beat a Path at two creatures");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        has_combat_restriction(&engine),
        "the resolved can't-block restriction is installed"
    );

    engine
        .apply_command(0, &primitive_yield())
        .expect("main 1 to beginning of combat");
    engine.apply_command(0, &pass()).expect("active pass");
    engine.apply_command(1, &pass()).expect("defender pass");
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    engine.apply_command(0, &pass()).expect("active pass");
    engine.apply_command(1, &pass()).expect("defender pass");
    let legal = &engine.initial_response_batch().legal_by_player[&1];
    assert!(
        !legal
            .legal_block_pairs
            .iter()
            .any(|pair| { pair.blocker_id == blocker_a || pair.blocker_id == blocker_b }),
        "CR 509.1b: the restricted creatures cannot block"
    );
    assert!(
        engine
            .apply_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker_a,
                }]),
            )
            .is_err(),
        "declaring the restricted block is rejected"
    );

    // Declining the optional targets leaves both creatures able to block.
    let mut decline = deck_engine(
        328_061,
        &["bellowing_bruiser_beat_a_path"],
        &["grizzly_bears"],
    );
    inject_creature_on_battlefield(&mut decline, 1, "grizzly_bears");
    ensure_in_hand(&mut decline, 0, "bellowing_bruiser_beat_a_path");
    give_mana(
        &mut decline,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&decline, 0, "bellowing_bruiser_beat_a_path");
    decline
        .apply_command(0, &cast_spell_face(slot, vec![], 1))
        .expect("declining the optional targets is legal");
    resolve_entire_stack_two_player(&mut decline);
    assert!(
        !has_combat_restriction(&decline),
        "declining the optional targets installs no restriction"
    );

    // The until-end-of-turn restriction expires at cleanup.
    let mut expiry = deck_engine(
        328_062,
        &["bellowing_bruiser_beat_a_path"],
        &["grizzly_bears"],
    );
    let target = inject_creature_on_battlefield(&mut expiry, 1, "grizzly_bears");
    ensure_in_hand(&mut expiry, 0, "bellowing_bruiser_beat_a_path");
    give_mana(
        &mut expiry,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&expiry, 0, "bellowing_bruiser_beat_a_path");
    expiry
        .apply_command(0, &cast_spell_face(slot, permanent_targets(&[target]), 1))
        .expect("cast Beat a Path");
    resolve_entire_stack_two_player(&mut expiry);
    assert!(has_combat_restriction(&expiry));
    end_active_turn(&mut expiry, 0);
    assert!(
        !has_combat_restriction(&expiry),
        "the restriction expires at cleanup"
    );
}

#[test]
fn issue_328_kindled_heroism_pumps_then_scries_after_the_target_is_chosen() {
    let mut engine = deck_engine(328_070, &["kindled_heroism"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let top = seat_on_top(&mut engine, 0, &["storm_crow", "hill_giant"]);
    assert_eq!(engine.effective_power(bear), Some(2));

    ensure_in_hand(&mut engine, 0, "kindled_heroism");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "kindled_heroism");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Kindled Heroism");
    let parked = resolve_to_park(&mut engine);

    assert_eq!(
        engine.effective_power(bear),
        Some(3),
        "the pump resolves before the scry choice"
    );
    assert_eq!(engine.effective_toughness(bear), Some(2));
    assert!(engine.effective_has_keyword(bear, Keyword::FirstStrike));
    let choice = find_resolution_choice(&parked).expect("scry parks a choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(choice.candidate_object_ids, vec![top[0]]);

    let _completion = engine
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("bottom the scried card");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        *engine.state.players[0].library.back().expect("library"),
        top[0]
    );
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.effective_power(bear), Some(2), "the pump expires");
    assert!(!engine.effective_has_keyword(bear, Keyword::FirstStrike));
}
