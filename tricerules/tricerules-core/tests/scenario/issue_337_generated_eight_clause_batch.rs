//! Issue #337 — the eight reviewed pump, cost, ETB, and linked-exile Standard cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 611.2a/514.2/701.26 govern the +1/+3 flying pump with untap and its cleanup expiry;
//! CR 601.2f/118.7a the {2} attacking-target reduction; CR 603.6 the gain-one-then-draw ETB
//! (typed order is asserted in the registry test); CR 702.2b the source-excluding deathtouch
//! grant and its expiry; CR 603.2/701.22a the private scry on becoming tapped; CR 111.10a the
//! two-Treasure ETB; CR 301.5/701.3 the Equipment entry attach plus untap; and CR 610.3 the
//! linked exile until the source leaves.

use super::helpers::*;
use tricerules_cards::Keyword;
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

fn pool_total(engine: &GameEngine, player: usize) -> u32 {
    let pool = &engine.state.players[player].mana_pool;
    pool.white + pool.blue + pool.black + pool.red + pool.green + pool.colorless
}

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

fn battlefield_id(engine: &GameEngine, card_id: &str) -> u32 {
    engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == card_id && object.zone == Zone::Battlefield)
        .unwrap_or_else(|| panic!("missing {card_id} on the battlefield"))
        .id
}

#[test]
fn issue_337_acrobatic_leap_pumps_flying_untaps_and_expires() {
    let mut engine = deck_engine(337_001, &["acrobatic_leap"], &[]);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&bear).expect("bear").tapped = true;
    ensure_in_hand(&mut engine, 0, "acrobatic_leap");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "acrobatic_leap");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Acrobatic Leap");
    assert_eq!(
        engine.effective_power(bear),
        Some(2),
        "the pump applies only on resolution"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(3));
    assert_eq!(engine.effective_toughness(bear), Some(5));
    assert!(engine.effective_has_keyword(bear, Keyword::Flying));
    assert!(!engine.state.objects[&bear].tapped, "CR 701.26: untap");

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(bear),
        Some(2),
        "CR 514.2: the pump expires at cleanup"
    );
    assert_eq!(engine.effective_toughness(bear), Some(2));
    assert!(
        !engine.effective_has_keyword(bear, Keyword::Flying),
        "the until-end-of-turn flying grant expires at cleanup"
    );
}

#[test]
fn issue_337_depower_reduces_only_for_an_attacking_target() {
    let decks = Some(vec![
        deck_with("island", &["depower", "depower", "depower"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(337_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let idle = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("attack with the bearer");

    ensure_in_hand(&mut engine, 0, "depower");
    let slot = hand_index_for_card(&engine, 0, "depower");
    let batch = engine.initial_response_batch();
    let published = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    let application = published
        .targeted_cost_reduction_applications
        .first()
        .expect("attacking-target reduction application");
    assert_eq!(application.generic_mana, 2, "CR 601.2f: {{2}} less");
    assert!(application
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == attacker));
    assert!(
        !application
            .qualifying_targets
            .iter()
            .any(|candidate| candidate.object_id == idle),
        "a nonattacking creature does not qualify"
    );

    // The reduced cost is exactly {U}: one blue is enough for the attacking target.
    let command = cast_spell(slot, target_object(attacker));
    engine.state.players[0].mana_pool.blue = 1;
    engine
        .apply_command(0, &command)
        .expect("the attacking target reduces the cost to {U}");
    assert_eq!(pool_total(&engine, 0), 0);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(attacker),
        Some(0),
        "CR 611.2a: the -4/-0 clamps power at zero"
    );

    // The same spell at the nonattacker needs the full {2}{U}.
    ensure_in_hand(&mut engine, 0, "depower");
    let slot = hand_index_for_card(&engine, 0, "depower");
    engine.state.players[0].mana_pool.blue = 1;
    let command = cast_spell(slot, target_object(idle));
    let index = engine.state.command_index;
    assert!(
        engine.apply_command(0, &command).is_err(),
        "an unreduced Depower cannot be cast for {{U}}"
    );
    assert_eq!(engine.state.command_index, index);
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    engine
        .apply_command(0, &command)
        .expect("the full {2}{U} pays for a nonattacking target");
    assert_eq!(pool_total(&engine, 0), 0);
}

#[test]
fn issue_337_inspiring_overseer_gains_a_life_and_draws_on_entry() {
    let mut engine = deck_engine(337_003, &["inspiring_overseer"], &[]);
    move_ready_to_battlefield(&mut engine, 0, "inspiring_overseer");
    let life_before = engine.state.players[0].life;
    let library_before = engine.state.players[0].library.len();
    let hand_before = engine.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, life_before + 1);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
}

#[test]
fn issue_337_blooming_stinger_grants_deathtouch_to_another_creature_and_expires() {
    let mut engine = deck_engine(337_004, &["blooming_stinger"], &[]);
    let other = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let stinger = move_ready_to_battlefield(&mut engine, 0, "blooming_stinger");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(
        engine
            .apply_command(0, &choose_trigger_targets(&[stinger], false))
            .is_err(),
        "the source is not a legal \"another\" target"
    );
    engine
        .apply_command(0, &choose_trigger_targets(&[other], false))
        .expect("grant deathtouch to another creature");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.effective_has_keyword(other, Keyword::Deathtouch));

    end_active_turn(&mut engine, 0);
    assert!(
        !engine.effective_has_keyword(other, Keyword::Deathtouch),
        "CR 514.2: the deathtouch grant expires at cleanup"
    );
}

#[test]
fn issue_337_attentive_sunscribe_scries_when_it_becomes_tapped() {
    let mut engine = deck_engine(337_005, &[], &[]);
    inject_library_card(&mut engine, 0, "forest");
    inject_library_card(&mut engine, 0, "island");
    let sunscribe = inject_creature_on_battlefield(&mut engine, 0, "attentive_sunscribe");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![sunscribe]))
        .expect("attack with Attentive Sunscribe");
    assert!(engine.state.objects[&sunscribe].tapped);
    assert_eq!(engine.state.stack.len(), 1, "the becomes-tapped trigger");

    let choice = (0..6)
        .find_map(|_| {
            let player = engine.state.priority_player_id();
            let batch = engine
                .apply_command(player, &pass())
                .expect("pass toward the scry resolution");
            find_resolution_choice(&batch)
        })
        .expect("scry 1 choice");
    assert_eq!(
        choice.deciding_player_id, 0,
        "the scry is controller-private"
    );
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(choice.candidate_object_ids.len(), 1);
    engine
        .apply_command(
            0,
            &submit_resolution_choice(choice.candidate_object_ids.clone()),
        )
        .expect("put the seen card on the bottom");
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn issue_337_rapacious_dragon_creates_two_treasures_on_entry() {
    let mut engine = deck_engine(337_006, &["rapacious_dragon"], &[]);
    move_ready_to_battlefield(&mut engine, 0, "rapacious_dragon");
    resolve_entire_stack_two_player(&mut engine);
    let treasures = battlefield_token_oids(&engine, 0, "treasure");
    assert_eq!(treasures.len(), 2, "CR 111.10a: two Treasure tokens");
    assert!(
        battlefield_token_oids(&engine, 1, "treasure").is_empty(),
        "the tokens are created under the Dragon's controller"
    );
}

#[test]
fn issue_337_super_suit_attaches_and_untaps_on_entry() {
    let mut engine = deck_engine(337_007, &["super_suit"], &[]);
    let tapped = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&tapped).expect("bear").tapped = true;
    let suit = move_ready_to_battlefield(&mut engine, 0, "super_suit");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_targets(&[tapped], false))
        .expect("attach Super Suit to the chosen creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&suit].attached_to,
        Some(AttachmentRecipient::Object(tapped)),
        "CR 301.5/701.3: the Equipment attaches on entry"
    );
    assert!(!engine.state.objects[&tapped].tapped, "CR 701.26: untap");
}

#[test]
fn issue_337_super_villain_lockup_linked_exiles_until_it_leaves() {
    let decks = Some(vec![
        deck_with("plains", &["super_villain_lockup"]),
        deck_with("forest", &["broken_wings", "broken_wings"]),
    ]);
    let mut engine = GameEngine::new(337_008, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let tapped = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.objects.get_mut(&tapped).expect("bear").tapped = true;
    let untapped = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    ensure_card_in_hand(&mut engine, 0, "super_villain_lockup");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "super_villain_lockup");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Super Villain Lockup");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(
        engine
            .apply_command(0, &choose_trigger_targets(&[untapped], false))
            .is_err(),
        "an untapped creature is not a legal target"
    );
    engine
        .apply_command(0, &choose_trigger_targets(&[tapped], false))
        .expect("target the tapped creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&tapped].zone, Zone::Exile);
    assert_eq!(engine.state.active_event_observers.len(), 1);

    let lockup = battlefield_id(&engine, "super_villain_lockup");
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
        .apply_command(1, &cast_spell(slot, target_object(lockup)))
        .expect("destroy the Lockup");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&lockup].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&tapped].zone,
        Zone::Battlefield,
        "CR 610.3: the linked card returns when the source leaves"
    );
    assert_eq!(engine.state.objects[&tapped].controller, 1);
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn issue_337_scenarios_reach_main1() {
    let engine = deck_engine(337_009, &[], &[]);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
}
