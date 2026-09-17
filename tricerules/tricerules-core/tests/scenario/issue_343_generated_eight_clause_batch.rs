//! Issue #343 — the eight reviewed damage, counter, loot, and linked-exile Standard cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 120.4b/701.8 govern the damage-marked opposing-creature destruction; CR 611.2a/514.2/702.15
//! the +2/+2 lifelink pump and its cleanup expiry; CR 602.2/122.1 the parameterized activated
//! +1/+1 counter; CR 603.2/202.3 the mana-value-four cast counter and its below-threshold
//! exclusion; CR 701.9/111.10a the private loot followed by the Treasure; CR 610.3 the artifact
//! linked exile and its return; CR 404/401 the any-graveyard bottom move; and CR 603.6a/701.3 the
//! Equipment landfall pump of the attached creature.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, RuledCommand,
};

fn deck_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_trigger_targets(ids: &[u32], decline: bool) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline,
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

fn graveyard_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }]
}

fn mark_damaged(engine: &mut GameEngine, object_id: u32) {
    let generation = engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .turn_history
        .current
        .damaged_objects
        .push((object_id, generation));
}

#[test]
fn issue_343_stingblade_assassin_destroys_only_a_damage_marked_opposing_creature() {
    let mut engine = deck_engine(
        343_001,
        &["stingblade_assassin"],
        &["grizzly_bears", "hill_giant"],
    );
    let marked = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let undamaged = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    mark_damaged(&mut engine, marked);

    ensure_in_hand(&mut engine, 0, "stingblade_assassin");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "stingblade_assassin");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Stingblade Assassin");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "the ETB trigger waits for a mandatory target"
    );

    for illegal in [undamaged, own] {
        assert!(
            engine
                .apply_command(0, &choose_trigger_targets(&[illegal], false))
                .is_err(),
            "an undamaged or own creature is not a legal target"
        );
    }
    engine
        .apply_command(0, &choose_trigger_targets(&[marked], false))
        .expect("target the damage-marked opposing creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&marked].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&undamaged].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&own].zone, Zone::Battlefield);
}

#[test]
fn issue_343_give_in_to_violence_pumps_lifelink_and_expires() {
    let mut engine = deck_engine(343_002, &["give_in_to_violence"], &[]);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "give_in_to_violence");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "give_in_to_violence");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Give In to Violence");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(4));
    assert_eq!(engine.effective_toughness(bear), Some(4));
    assert!(engine.effective_has_keyword(bear, Keyword::Lifelink));

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(bear),
        Some(2),
        "CR 514.2: the pump expires at cleanup"
    );
    assert_eq!(engine.effective_toughness(bear), Some(2));
    assert!(
        !engine.effective_has_keyword(bear, Keyword::Lifelink),
        "the lifelink grant expires at cleanup"
    );
}

#[test]
fn issue_343_toadstool_admirer_activation_pays_mana_and_adds_a_counter() {
    let mut engine = deck_engine(343_003, &["toadstool_admirer"], &[]);
    let oid = move_ready_to_battlefield(&mut engine, 0, "toadstool_admirer");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    assert!(
        engine
            .apply_command(0, &activate_ability_for(&engine, oid, 0, vec![]))
            .is_err(),
        "{{3}}{{G}} is not payable with two mana"
    );
    assert_eq!(
        engine.state.objects[&oid].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability_for(&engine, oid, 0, vec![]))
        .expect("activate the {3}{G} counter ability");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&oid].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn issue_343_lurking_lizards_counts_only_mana_value_four_or_greater() {
    for (seed, spell, gift, expected) in [
        (
            343_004,
            "hill_giant",
            ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            1,
        ),
        (
            343_005,
            "grizzly_bears",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            0,
        ),
    ] {
        let mut engine = deck_engine(seed, &["lurking_lizards", spell], &[]);
        let lizards = move_ready_to_battlefield(&mut engine, 0, "lurking_lizards");
        ensure_in_hand(&mut engine, 0, spell);
        give_mana(&mut engine, 0, gift);
        let slot = hand_index_for_card(&engine, 0, spell);
        engine
            .apply_command(0, &cast_spell(slot, vec![]))
            .expect("cast the probe spell");
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&lizards].counter_count(CounterKind::PlusOnePlusOne),
            expected,
            "CR 202.3: only a mana value four or greater spell counts ({spell})"
        );
    }
}

#[test]
fn issue_343_collectors_vault_loots_privately_then_creates_a_treasure() {
    let mut engine = deck_engine(343_006, &["collectors_vault"], &[]);
    let vault = move_ready_to_battlefield(&mut engine, 0, "collectors_vault");
    let known = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    inject_library_card(&mut engine, 0, "forest");
    let library_before = engine.state.players[0].library.len();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability_for(&engine, vault, 0, vec![]))
        .expect("activate Collector's Vault");
    engine
        .apply_command(0, &pass())
        .expect("active player passes on the ability");
    let parked = engine
        .apply_command(1, &pass())
        .expect("the ability resolves to the discard choice");

    let choice = find_resolution_choice(&parked).expect("private discard choice");
    assert_eq!(
        choice.deciding_player_id, 0,
        "the discard is controller-private"
    );
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 1,
        "CR 121.2: the draw happens before the discard"
    );
    assert!(
        battlefield_token_oids(&engine, 0, "treasure").is_empty(),
        "the Treasure is created after the discard, in printed order"
    );
    assert!(choice.candidate_object_ids.contains(&known));

    engine
        .apply_command(0, &submit_resolution_choice(vec![known]))
        .expect("discard the known card");
    assert!(engine.state.players[0].graveyard.contains(&known));
    assert_eq!(
        battlefield_token_oids(&engine, 0, "treasure").len(),
        1,
        "CR 111.10a: the Treasure is created after the discard"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_343_white_auracite_linked_exiles_until_it_leaves() {
    let decks = Some(vec![
        deck_with("plains", &["white_auracite"]),
        deck_with("forest", &["broken_wings", "broken_wings"]),
    ]);
    let mut engine = GameEngine::new(343_007, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let victim = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "white_auracite");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 2,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "white_auracite");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast White Auracite");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    for illegal in [land, own] {
        assert!(
            engine
                .apply_command(0, &choose_trigger_targets(&[illegal], false))
                .is_err(),
            "a land or a permanent you control is not a legal target"
        );
    }
    engine
        .apply_command(0, &choose_trigger_targets(&[victim], false))
        .expect("exile the opposing nonland permanent");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&victim].zone, Zone::Exile);

    let auracite = battlefield_object_for_card(&engine, 0, "white_auracite");
    ensure_card_in_hand(&mut engine, 1, "broken_wings");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &pass())
        .expect("P0 passes priority");
    let slot = hand_index_for_card(&engine, 1, "broken_wings");
    engine
        .apply_command(1, &cast_spell(slot, target_object(auracite)))
        .expect("destroy White Auracite");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&auracite].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&victim].zone,
        Zone::Battlefield,
        "CR 610.3: the linked card returns when the source leaves"
    );
    assert_eq!(engine.state.objects[&victim].controller, 1);
}

#[test]
fn issue_343_hoverstone_pilgrim_bottoms_a_card_from_any_graveyard() {
    let mut engine = deck_engine(
        343_008,
        &["hoverstone_pilgrim"],
        &["grizzly_bears", "counterspell"],
    );
    let own = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    let opposing = inject_graveyard_card(&mut engine, 1, "counterspell");
    let pilgrim = move_ready_to_battlefield(&mut engine, 0, "hoverstone_pilgrim");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, pilgrim, 0, graveyard_target(opposing)),
        )
        .expect("bottoms an opponent's graveyard card");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Library);
    assert_eq!(engine.state.players[1].library.back(), Some(&opposing));
    assert!(!engine.state.players[1].graveyard.contains(&opposing));
    assert_eq!(
        engine.state.objects[&own].zone,
        Zone::Graveyard,
        "the untargeted own-graveyard card stays put"
    );
}

#[test]
fn issue_343_adventuring_gear_landfall_pumps_the_equipped_creature() {
    let mut engine = deck_engine(343_009, &["adventuring_gear"], &[]);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let gear = move_ready_to_battlefield(&mut engine, 0, "adventuring_gear");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, gear, 0, target_object(bear)),
        )
        .expect("equip Adventuring Gear");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&gear].attached_to,
        Some(AttachmentRecipient::Object(bear)),
        "CR 701.3: the Equipment is attached to the chosen creature"
    );

    ensure_in_hand(&mut engine, 0, "island");
    let slot = hand_index_for_card(&engine, 0, "island");
    engine
        .apply_command(0, &play_land(slot))
        .expect("play a land to trigger landfall");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(bear),
        Some(4),
        "CR 603.6a/611.2a: the landfall pump resolves on the attached creature"
    );
    assert_eq!(engine.effective_toughness(bear), Some(4));

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(bear),
        Some(2),
        "CR 514.2: the landfall pump expires at cleanup"
    );
    assert_eq!(engine.effective_toughness(bear), Some(2));
}

#[test]
fn issue_343_scenarios_reach_main1() {
    let engine = deck_engine(343_010, &[], &[]);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
}
