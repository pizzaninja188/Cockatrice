//! Issue #316 — the seven reviewed single-clause completion cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 115.1/115.6 govern target selection (including "up to N"); CR 608.2b rechecks every chosen
//! target's predicate at resolution; CR 601.2i and 603.2 govern the cast trigger; CR 404.2 keeps
//! graveyards public; CR 701 governs the destroy and exile keyword actions; CR 611.2a and 514.2
//! govern the until-end-of-turn pump and its cleanup expiry; CR 702.7 governs first strike.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind,
};

fn deck_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", own),
        deck_with("mountain", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_permanent_targets(ids: &[u32]) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: ids
                .iter()
                .copied()
                .map(|object_id| TargetRef {
                    object_id,
                    kind: TargetRefKind::Permanent as i32,
                    ..Default::default()
                })
                .collect(),
        })),
    }
}

fn choose_graveyard_targets(ids: &[u32]) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: ids
                .iter()
                .copied()
                .map(|object_id| TargetRef {
                    object_id,
                    kind: TargetRefKind::Graveyard as i32,
                    ..Default::default()
                })
                .collect(),
        })),
    }
}

fn three_player_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &["firebrand_archer", "lightning_bolt"]),
        deck_with("mountain", &["lightning_bolt"]),
        deck_with("mountain", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    while engine.state.turn_step != TurnStep::Main1 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("advance three-player turn");
    }
    engine
}

fn resolve_three_player(engine: &mut GameEngine) {
    while !engine.state.stack.is_empty() {
        for _ in 0..engine.state.players.len() {
            if engine.state.stack.is_empty() {
                break;
            }
            let player = engine.state.priority_player_id();
            engine
                .apply_command(player, &pass())
                .expect("three-player priority pass");
        }
    }
}

#[test]
fn issue_316_rip_the_seams_destroys_tapped_creatures_and_fizzles_when_untapped() {
    let mut engine = deck_engine(
        316_001,
        &["threadbind_clique_rip_the_seams"],
        &["grizzly_bears", "storm_crow"],
    );
    let tapped = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let untapped = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");
    engine
        .state
        .objects
        .get_mut(&tapped)
        .expect("tapped fixture")
        .tapped = true;

    ensure_in_hand(&mut engine, 0, "threadbind_clique_rip_the_seams");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "threadbind_clique_rip_the_seams");

    assert!(
        engine
            .apply_command(0, &cast_spell_face(slot, target_object(untapped), 1))
            .is_err(),
        "an untapped creature is not a legal Rip the Seams target"
    );
    engine
        .apply_command(0, &cast_spell_face(slot, target_object(tapped), 1))
        .expect("cast Rip the Seams at a tapped creature");
    assert_eq!(
        engine.state.objects[&tapped].zone,
        Zone::Battlefield,
        "the creature is destroyed only on resolution"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&tapped].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&untapped].zone, Zone::Battlefield);

    let mut fizzle = deck_engine(
        316_002,
        &["threadbind_clique_rip_the_seams"],
        &["grizzly_bears"],
    );
    let target = inject_creature_on_battlefield(&mut fizzle, 1, "grizzly_bears");
    fizzle
        .state
        .objects
        .get_mut(&target)
        .expect("fixture")
        .tapped = true;
    ensure_in_hand(&mut fizzle, 0, "threadbind_clique_rip_the_seams");
    grant_pool(&mut fizzle, 0);
    let slot = hand_index_for_card(&fizzle, 0, "threadbind_clique_rip_the_seams");
    fizzle
        .apply_command(0, &cast_spell_face(slot, target_object(target), 1))
        .expect("cast at a tapped creature");
    fizzle
        .state
        .objects
        .get_mut(&target)
        .expect("fixture")
        .tapped = false;
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(
        fizzle.state.objects[&target].zone,
        Zone::Battlefield,
        "CR 608.2b: a target untapped before resolution is no longer legal"
    );
    assert!(fizzle.state.stack.is_empty());
}

#[test]
fn issue_316_entry_destroy_offers_only_artifacts_and_enchantments_and_may_decline() {
    for (seed, source_card) in [
        (316_010, "chomping_changeling"),
        (316_011, "disruptive_stormbrood_petty_revenge"),
    ] {
        let mut engine = deck_engine(
            seed,
            &[source_card],
            &["bonesplitter", "crusade", "grizzly_bears"],
        );
        let artifact = move_ready_to_battlefield(&mut engine, 1, "bonesplitter");
        let enchantment = move_ready_to_battlefield(&mut engine, 1, "crusade");
        let creature = move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
        move_ready_to_battlefield(&mut engine, 0, source_card);

        let published = engine.initial_response_batch();
        let targets = published.legal_by_player[&0]
            .valid_targets_by_ability
            .values()
            .next()
            .expect("pending entry trigger targets");
        assert_eq!(targets.groups[0].min, 0);
        assert_eq!(targets.groups[0].max, 1);
        let mut offered = targets.groups[0].valid_permanent_ids.clone();
        offered.sort_unstable();
        let mut expected = vec![artifact, enchantment];
        expected.sort_unstable();
        assert_eq!(offered, expected, "{source_card} offers only the two types");

        assert!(
            engine
                .apply_command(0, &choose_permanent_targets(&[creature]))
                .is_err(),
            "{source_card} must reject a creature target"
        );
        engine
            .apply_command(0, &choose_permanent_targets(&[enchantment]))
            .expect("destroy the enchantment");
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.objects[&enchantment].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    }

    let mut decline = deck_engine(
        316_012,
        &["chomping_changeling"],
        &["bonesplitter", "crusade"],
    );
    let artifact = move_ready_to_battlefield(&mut decline, 1, "bonesplitter");
    let enchantment = move_ready_to_battlefield(&mut decline, 1, "crusade");
    move_ready_to_battlefield(&mut decline, 0, "chomping_changeling");
    decline
        .apply_command(0, &choose_permanent_targets(&[]))
        .expect("decline the optional target");
    resolve_entire_stack_two_player(&mut decline);
    assert_eq!(decline.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(decline.state.objects[&enchantment].zone, Zone::Battlefield);
}

#[test]
fn issue_316_griffnaut_tracker_exiles_up_to_two_cards_from_a_single_graveyard() {
    let mut engine = deck_engine(316_020, &["griffnaut_tracker"], &[]);
    let own_first = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let own_second = inject_graveyard_card(&mut engine, 0, "storm_crow");
    let opposing = inject_graveyard_card(&mut engine, 1, "forest");
    move_ready_to_battlefield(&mut engine, 0, "griffnaut_tracker");

    let published = engine.initial_response_batch();
    let targets = published.legal_by_player[&0]
        .valid_targets_by_ability
        .values()
        .next()
        .expect("pending entry trigger targets");
    let group = &targets.groups[0];
    assert_eq!((group.min, group.max), (0, 2));
    assert!(group.same_graveyard);
    let mut offered = group.valid_graveyard_ids.clone();
    offered.sort_unstable();
    let mut expected = vec![own_first, own_second, opposing];
    expected.sort_unstable();
    assert_eq!(offered, expected);

    assert!(
        engine
            .apply_command(0, &choose_graveyard_targets(&[own_first, opposing]),)
            .is_err(),
        "two cards from different graveyards are not a legal cohort"
    );
    engine
        .apply_command(0, &choose_graveyard_targets(&[]))
        .expect("decline the optional exile");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&own_first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&own_second].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Graveyard);

    let mut exile_two = deck_engine(316_021, &["griffnaut_tracker"], &[]);
    let first = inject_graveyard_card(&mut exile_two, 1, "grizzly_bears");
    let second = inject_graveyard_card(&mut exile_two, 1, "storm_crow");
    let untouched = inject_graveyard_card(&mut exile_two, 0, "forest");
    move_ready_to_battlefield(&mut exile_two, 0, "griffnaut_tracker");
    exile_two
        .apply_command(0, &choose_graveyard_targets(&[first, second]))
        .expect("exile two cards from one graveyard");
    resolve_entire_stack_two_player(&mut exile_two);
    assert_eq!(exile_two.state.objects[&first].zone, Zone::Exile);
    assert_eq!(exile_two.state.objects[&second].zone, Zone::Exile);
    assert_eq!(exile_two.state.objects[&untouched].zone, Zone::Graveyard);

    let mut exile_one = deck_engine(316_022, &["griffnaut_tracker"], &[]);
    let single = inject_graveyard_card(&mut exile_one, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut exile_one, 0, "griffnaut_tracker");
    exile_one
        .apply_command(0, &choose_graveyard_targets(&[single]))
        .expect("exile one card");
    resolve_entire_stack_two_player(&mut exile_one);
    assert_eq!(exile_one.state.objects[&single].zone, Zone::Exile);
}

#[test]
fn issue_316_firebrand_archer_pings_each_opponent_once_per_owner_noncreature_cast() {
    let mut engine = deck_engine(
        316_030,
        &["firebrand_archer", "lightning_bolt", "grizzly_bears"],
        &["lightning_bolt"],
    );
    relocate_to_battlefield(&mut engine, 0, "firebrand_archer", false);
    grant_pool(&mut engine, 0);

    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt, target_player(1)))
        .expect("cast a noncreature spell");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "the bolt and exactly one Archer trigger"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[1].life, 16,
        "three from the bolt and one from the Archer"
    );

    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    let bear = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(bear, vec![]))
        .expect("cast a creature spell");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "creature spells do not trigger the Archer"
    );
    resolve_entire_stack_two_player(&mut engine);

    ensure_in_hand(&mut engine, 1, "lightning_bolt");
    grant_pool(&mut engine, 1);
    engine
        .apply_command(0, &pass())
        .expect("pass priority to the opponent");
    let opposing_bolt = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_command(1, &cast_spell(opposing_bolt, target_player(0)))
        .expect("opponent casts a noncreature spell");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "an opponent's cast does not trigger the controller-only ability"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 16);

    let mut multiplayer = three_player_engine(316_031);
    relocate_to_battlefield(&mut multiplayer, 0, "firebrand_archer", false);
    grant_pool(&mut multiplayer, 0);
    ensure_in_hand(&mut multiplayer, 0, "lightning_bolt");
    let bolt = hand_index_for_card(&multiplayer, 0, "lightning_bolt");
    multiplayer
        .apply_command(0, &cast_spell(bolt, target_player(1)))
        .expect("cast into a three-player game");
    resolve_three_player(&mut multiplayer);
    assert_eq!(multiplayer.state.players[0].life, 20);
    assert_eq!(
        multiplayer.state.players[1].life, 16,
        "the targeted opponent takes the bolt and one ping"
    );
    assert_eq!(
        multiplayer.state.players[2].life, 19,
        "every other opponent takes the ping"
    );
}

#[test]
fn issue_316_first_strike_pumps_add_power_only_and_expire_at_cleanup() {
    for (seed, card_id, mana, expected_power) in [
        (
            316_040,
            "kindled_fury",
            ManaGift {
                r: 1,
                ..Default::default()
            },
            3,
        ),
        (
            316_041,
            "sure_strike",
            ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            5,
        ),
    ] {
        let mut engine = deck_engine(seed, &[card_id], &["grizzly_bears"]);
        let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        assert_eq!(engine.effective_power(bear), Some(2));
        assert_eq!(engine.effective_toughness(bear), Some(2));
        assert!(!engine.effective_has_keyword(bear, Keyword::FirstStrike));

        ensure_in_hand(&mut engine, 0, card_id);
        give_mana(&mut engine, 0, mana);
        let slot = hand_index_for_card(&engine, 0, card_id);
        engine
            .apply_command(0, &cast_spell(slot, target_object(bear)))
            .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
        assert!(
            !engine.effective_has_keyword(bear, Keyword::FirstStrike),
            "{card_id} applies only on resolution"
        );
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.effective_power(bear), Some(expected_power));
        assert_eq!(
            engine.effective_toughness(bear),
            Some(2),
            "{card_id} grants no toughness"
        );
        assert!(engine.effective_has_keyword(bear, Keyword::FirstStrike));

        end_active_turn(&mut engine, 0);
        assert_eq!(engine.effective_power(bear), Some(2), "{card_id} expires");
        assert!(!engine.effective_has_keyword(bear, Keyword::FirstStrike));
    }
}

#[test]
fn issue_316_petty_revenge_destroys_power_three_but_not_power_four() {
    let mut engine = deck_engine(316_050, &["disruptive_stormbrood_petty_revenge"], &[]);
    let three_power = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 3, 3);
    let four_power = inject_creature_with_stats(&mut engine, 1, "storm_crow", 4, 4);

    ensure_in_hand(&mut engine, 0, "disruptive_stormbrood_petty_revenge");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "disruptive_stormbrood_petty_revenge");

    assert!(
        engine
            .apply_command(0, &cast_spell_face(slot, target_object(four_power), 1))
            .is_err(),
        "a power-4 creature is not a legal Petty Revenge target"
    );
    engine
        .apply_command(0, &cast_spell_face(slot, target_object(three_power), 1))
        .expect("cast Petty Revenge at a power-3 creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&three_power].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&four_power].zone, Zone::Battlefield);

    let mut fizzle = deck_engine(316_051, &["disruptive_stormbrood_petty_revenge"], &[]);
    let target = inject_creature_with_stats(&mut fizzle, 1, "grizzly_bears", 3, 3);
    ensure_in_hand(&mut fizzle, 0, "disruptive_stormbrood_petty_revenge");
    grant_pool(&mut fizzle, 0);
    let slot = hand_index_for_card(&fizzle, 0, "disruptive_stormbrood_petty_revenge");
    fizzle
        .apply_command(0, &cast_spell_face(slot, target_object(target), 1))
        .expect("cast at a power-3 creature");
    fizzle
        .state
        .objects
        .get_mut(&target)
        .expect("fixture")
        .power = Some(4);
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(
        fizzle.state.objects[&target].zone,
        Zone::Battlefield,
        "CR 608.2b: the power bound is rechecked on resolution"
    );
    assert!(fizzle.state.stack.is_empty());
}
