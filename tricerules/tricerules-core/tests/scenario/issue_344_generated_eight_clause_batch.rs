//! Issue #344 — the eight reviewed Raid, landfall, counter-loot, Equipment, and Aura Standard
//! cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 603.2/121 govern the controller's second-spell cast trigger; CR 603.6a/611.2a/514.2 the
//! landfall +1/+0 pump and its cleanup expiry; CR 701.6/701.9/121 the counter-and-loot; CR
//! 701.9/121 the discard-then-draw-two; CR 301.5/701.3/111.10a the Equipment Ally token and its
//! self-attachment; CR 614.1c/122/508.1 the conditional Raid entry counter; CR 603.6/508.1/120 the
//! intervening Raid damage trigger; and CR 603.6/701.14/115.1 the Aura fight with its up-to-one
//! target.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{AttachmentRecipient, TurnStep, Zone};

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
                    ..Default::default()
                })
                .collect(),
        })),
    }
}

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

fn plus_one_counters(engine: &GameEngine, object_id: u32) -> u32 {
    engine.state.objects[&object_id].counter_count(CounterKind::PlusOnePlusOne)
}

#[test]
fn issue_344_illvoi_operative_counters_on_the_second_spell() {
    let mut engine = deck_engine(344_001, &["illvoi_operative"], &[]);
    let operative = move_ready_to_battlefield(&mut engine, 0, "illvoi_operative");
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );

    let first = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(first, target_player(1)))
        .expect("cast the first spell");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        plus_one_counters(&engine, operative),
        0,
        "CR 603.2: the first spell of the turn must not trigger the second-spell ability"
    );

    let second = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(second, target_player(1)))
        .expect("cast the second spell");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        plus_one_counters(&engine, operative),
        1,
        "the second spell of the turn puts one +1/+1 counter on Illvoi Operative"
    );
}

#[test]
fn issue_344_icecave_crasher_landfall_pumps_then_expires() {
    let mut engine = deck_engine(344_002, &["icecave_crasher"], &[]);
    let crasher = move_ready_to_battlefield(&mut engine, 0, "icecave_crasher");
    assert_eq!(engine.effective_power(crasher), Some(4));
    assert_eq!(engine.effective_toughness(crasher), Some(4));

    inject_card_into_hand(&mut engine, 0, "island");
    let land = hand_index_for_card(&engine, 0, "island");
    engine
        .apply_command(0, &play_land(land))
        .expect("play a land to trigger landfall");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(crasher),
        Some(5),
        "CR 603.6a/611.2a: the landfall pump gives +1/+0 until cleanup"
    );
    assert_eq!(engine.effective_toughness(crasher), Some(4));

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(crasher),
        Some(4),
        "CR 514.2: the +1/+0 pump expires at cleanup"
    );
    assert_eq!(engine.effective_toughness(crasher), Some(4));
}

#[test]
fn issue_344_refute_counters_then_loots_with_a_private_discard() {
    let mut engine = deck_engine(344_003, &["grizzly_bears"], &[]);
    inject_library_card(&mut engine, 1, "forest");
    let known = inject_card_into_hand(&mut engine, 1, "forest");
    inject_card_into_hand(&mut engine, 1, "refute");
    let library_before = engine.state.players[1].library.len();

    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let bear = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(bear, vec![]))
        .expect("cast a creature spell");
    let bear_spell = engine.state.stack.last().expect("creature spell").id;
    engine.apply_command(0, &pass()).expect("active passes");

    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    let refute = hand_index_for_card(&engine, 1, "refute");
    engine
        .apply_command(1, &cast_spell(refute, stack_target(bear_spell)))
        .expect("counter the creature spell");
    engine.apply_command(1, &pass()).expect("caster passes");
    let parked = engine
        .apply_command(0, &pass())
        .expect("Refute resolves to the loot");

    let choice = find_resolution_choice(&parked).expect("private discard choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(
        count_card_id_in_graveyard(&engine, 0, "grizzly_bears"),
        1,
        "the countered creature spell goes to its owner's graveyard"
    );
    assert_eq!(
        engine.state.players[1].library.len(),
        library_before - 1,
        "CR 121.2: Refute draws before the discard parks"
    );
    assert!(choice.candidate_object_ids.contains(&known));

    engine
        .apply_command(1, &submit_resolution_choice(vec![known]))
        .expect("discard the known card");
    assert!(engine.state.players[1].graveyard.contains(&known));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_344_romantic_rendezvous_discards_then_draws_two() {
    let mut engine = deck_engine(344_004, &[], &[]);
    inject_card_into_hand(&mut engine, 0, "romantic_rendezvous");
    inject_library_card(&mut engine, 0, "island");
    inject_library_card(&mut engine, 0, "island");
    let known = inject_card_into_hand(&mut engine, 0, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let library_before = engine.state.players[0].library.len();

    let slot = hand_index_for_card(&engine, 0, "romantic_rendezvous");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Romantic Rendezvous");
    engine.apply_command(0, &pass()).expect("active passes");
    let parked = engine
        .apply_command(1, &pass())
        .expect("the sorcery resolves to the discard");

    let choice = find_resolution_choice(&parked).expect("private discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before,
        "CR 701.9: the discard is chosen before the two cards are drawn"
    );
    assert!(choice.candidate_object_ids.contains(&known));

    engine
        .apply_command(0, &submit_resolution_choice(vec![known]))
        .expect("discard the known card");
    assert!(engine.state.players[0].graveyard.contains(&known));
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 2,
        "the draw two happens after the discard"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_344_kyoshi_battle_fan_creates_an_ally_and_attaches_to_it() {
    let mut engine = deck_engine(344_005, &["kyoshi_battle_fan"], &[]);
    let fan = move_ready_to_battlefield(&mut engine, 0, "kyoshi_battle_fan");
    resolve_entire_stack_two_player(&mut engine);

    let tokens = battlefield_token_oids(&engine, 0, "ally_w_1_1");
    assert_eq!(tokens.len(), 1, "the ETB creates one Ally token");
    let ally = tokens[0];
    assert_eq!(
        engine.state.objects[&fan].attached_to,
        Some(AttachmentRecipient::Object(ally)),
        "CR 301.5/701.3: the Equipment attaches to the token it just created"
    );
}

#[test]
fn issue_344_goblin_boarders_enters_with_a_counter_only_when_attacked() {
    let mut unattacked = deck_engine(344_006, &["goblin_boarders"], &[]);
    let boarders = move_ready_to_battlefield(&mut unattacked, 0, "goblin_boarders");
    assert_eq!(
        plus_one_counters(&unattacked, boarders),
        0,
        "CR 614.1c: no attack this turn means no entry counter"
    );

    let mut attacked = deck_engine(344_007, &["goblin_boarders"], &[]);
    attacked.state.turn_history.current.player_mut(0).attacked = true;
    let boarders = move_ready_to_battlefield(&mut attacked, 0, "goblin_boarders");
    assert_eq!(
        plus_one_counters(&attacked, boarders),
        1,
        "CR 508.1: attacking this turn grants the conditional entry counter"
    );
    assert_eq!(attacked.effective_power(boarders), Some(4));
    assert_eq!(attacked.effective_toughness(boarders), Some(3));
}

#[test]
fn issue_344_gorehorn_raider_raid_damage_needs_the_intervening_condition() {
    let mut unattacked = deck_engine(344_008, &[], &[]);
    inject_card_into_hand(&mut unattacked, 0, "gorehorn_raider");
    give_mana(
        &mut unattacked,
        0,
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&unattacked, 0, "gorehorn_raider");
    unattacked
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Gorehorn Raider without attacking");
    pass_both_players(&mut unattacked);
    assert!(
        unattacked.state.pending_triggers.is_empty(),
        "CR 603.4: the Raid trigger must not stage when the controller did not attack"
    );

    let mut engine = deck_engine(344_009, &[], &["hill_giant"]);
    let giant = inject_creature_with_stats(&mut engine, 1, "hill_giant", 7, 7);
    inject_card_into_hand(&mut engine, 0, "gorehorn_raider");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );
    engine.state.turn_history.current.player_mut(0).attacked = true;
    let slot = hand_index_for_card(&engine, 0, "gorehorn_raider");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Gorehorn Raider after attacking");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "the Raid ETB trigger waits for its mandatory any-target"
    );
    engine
        .apply_command(0, &choose_trigger_targets(&[giant], false))
        .expect("target the opposing creature");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&giant].damage, 2,
        "CR 508.1/120: Gorehorn Raider deals two damage to the chosen target"
    );
}

#[test]
fn issue_344_pitiless_fists_fight_accepts_up_to_one_opposing_creature() {
    let mut engine = deck_engine(344_010, &[], &["grizzly_bears", "hill_giant"]);
    inject_card_into_hand(&mut engine, 0, "pitiless_fists");
    let enchanted = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 8);
    let other = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let victim = inject_creature_with_stats(&mut engine, 1, "hill_giant", 6, 6);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "pitiless_fists");
    engine
        .apply_command(0, &cast_spell(slot, target_object(enchanted)))
        .expect("cast Pitiless Fists");
    engine.apply_command(0, &pass()).expect("active passes");
    engine
        .apply_command(1, &pass())
        .expect("the Aura resolves and its ETB fights");

    assert!(
        engine
            .apply_command(0, &choose_trigger_targets(&[other], false))
            .is_err(),
        "CR 115.1: a creature the aura's controller controls is not a legal fight target"
    );
    engine
        .apply_command(0, &choose_trigger_targets(&[victim], false))
        .expect("fight the chosen opposing creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(enchanted),
        Some(4),
        "the +2/+2 Aura modifier applies during the fight"
    );
    assert_eq!(
        engine.state.objects[&victim].damage, 4,
        "CR 701.14: the enchanted creature deals its power to the opposing creature"
    );
    assert_eq!(
        engine.state.objects[&enchanted].damage, 6,
        "the opposing creature deals its power back"
    );
}

#[test]
fn issue_344_pitiless_fists_fight_may_be_declined() {
    let mut engine = deck_engine(344_011, &[], &["grizzly_bears"]);
    inject_card_into_hand(&mut engine, 0, "pitiless_fists");
    let enchanted = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let victim = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "pitiless_fists");
    engine
        .apply_command(0, &cast_spell(slot, target_object(enchanted)))
        .expect("cast Pitiless Fists");
    engine.apply_command(0, &pass()).expect("active passes");
    engine
        .apply_command(1, &pass())
        .expect("the Aura resolves and its ETB fights");
    engine
        .apply_command(0, &choose_trigger_targets(&[], false))
        .expect("CR 601.2c: decline the up-to-one fight target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&victim].damage, 0);
    assert_eq!(engine.state.objects[&enchanted].damage, 0);
    assert_eq!(engine.state.objects[&victim].zone, Zone::Battlefield);
}

#[test]
fn issue_344_scenarios_reach_main1() {
    let engine = deck_engine(344_012, &[], &[]);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
}
