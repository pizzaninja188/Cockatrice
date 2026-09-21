//! Reviewed direct-RON creature-trigger scenarios: Mary Jane Watson, Disruptor Wanderglyph,
//! Seasoned Consultant, Noggle Robber, Meteor Golem, Vinereap Mentor, Jumbo Cactuar and
//! Undercity Dire Rat.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 400.7 (zone
//! changes), CR 508.1/508.3 (attack declaration), CR 603.2c (one trigger per event), CR 603.6
//! (entry triggers), CR 701.13 (exile), and CR 111.10a-b (Treasure/Food).

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, TargetRef, TargetRefKind};

/// Advances to the next main phase of `active_player`, resolving cleanup discards along the way.
fn advance_to_next_main1(engine: &mut GameEngine, active_player: i32) {
    let starting_turn = engine.state.turn_instance;
    for _ in 0..240 {
        if engine.state.turn_instance > starting_turn
            && engine.state.active_player_id() == active_player
            && engine.state.turn_step == TurnStep::Main1
            && engine.state.priority_player_id() == active_player
        {
            return;
        }
        resolve_cleanup_discards_if_any(engine);
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("advance through the rest of the turn");
    }
    panic!("turn advancement stalled before the next main phase");
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    choose_trigger_target_of_kind(object_id, TargetRefKind::Permanent)
}

fn choose_trigger_card_target(object_id: u32) -> RuledCommand {
    choose_trigger_target_of_kind(object_id, TargetRefKind::Graveyard)
}

fn choose_trigger_target_of_kind(object_id: u32, kind: TargetRefKind) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: kind as i32,
                ..Default::default()
            }],
        })),
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast(e: &mut GameEngine, player: i32, card_id: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    semantic::accepted(e, player, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_misc9_mary_jane_watson_draws_once_per_turn_for_a_spider() {
    let mut e = engine(733_001);
    inject_creature_on_battlefield(&mut e, 0, "mary_jane_watson");
    let hand_before = cast_counting_hand(&mut e, 0, "giant_spider");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the Spider leaves hand and the once-per-turn trigger draws one"
    );

    // A second Spider in the same turn does not draw again (once each turn).
    let hand_before = cast_counting_hand(&mut e, 0, "giant_spider");
    assert_eq!(e.state.players[0].hand.len(), hand_before - 1);

    // The per-turn cap resets on the next turn: a Spider entering then draws again.
    advance_to_next_main1(&mut e, 0);
    let hand_before = cast_counting_hand(&mut e, 0, "giant_spider");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "a Spider entering on a later turn draws again"
    );

    // A non-Spider creature does not trigger the watcher.
    let mut no_draw = engine(733_011);
    inject_creature_on_battlefield(&mut no_draw, 0, "mary_jane_watson");
    let hand_before = cast_counting_hand(&mut no_draw, 0, "grizzly_bears");
    assert_eq!(no_draw.state.players[0].hand.len(), hand_before - 1);
}

/// Injects `card_id` into hand, records the hand size, casts it, and resolves. The returned size
/// includes the injected card, so a net-zero effect leaves the hand unchanged.
fn cast_counting_hand(e: &mut GameEngine, player: i32, card_id: &str) -> usize {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let hand_before = e.state.players[player as usize].hand.len();
    let slot = hand_index_for_card(e, player as usize, card_id);
    semantic::accepted(e, player, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(e);
    hand_before
}

#[test]
fn issue_misc9_disruptor_wanderglyph_exiles_an_opponent_graveyard_card() {
    let mut e = GameEngine::new(733_002, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let glyph = inject_creature_on_battlefield(&mut e, 0, "disruptor_wanderglyph");
    let their_card = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![glyph]))
        .expect("declare the Wanderglyph");
    assert_eq!(e.state.pending_triggers.len(), 1);
    semantic::accepted(&mut e, 0, &choose_trigger_card_target(their_card));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&their_card].zone, Zone::Exile);

    // A card in the controller's own graveyard is outside the Opponent scope.
    let mut bad = GameEngine::new(733_012, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut bad);
    let glyph = inject_creature_on_battlefield(&mut bad, 0, "disruptor_wanderglyph");
    let own_card = inject_graveyard_card(&mut bad, 0, "grizzly_bears");
    bad.apply_command(0, &declare_attackers(vec![glyph]))
        .expect("declare the Wanderglyph");
    assert!(
        bad.apply_command(0, &choose_trigger_card_target(own_card))
            .is_err(),
        "the controller's own graveyard is not a legal target"
    );
}

#[test]
fn issue_misc9_seasoned_consultant_pumps_when_three_attack() {
    let mut e = GameEngine::new(733_003, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let consultant = inject_creature_on_battlefield(&mut e, 0, "seasoned_consultant");
    let a = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let base_power = e.effective_power(consultant).expect("power");
    e.apply_command(0, &declare_attackers(vec![consultant, a, b]))
        .expect("declare three attackers");
    assert_eq!(
        e.state.stack.len(),
        1,
        "three attackers trigger the Consultant"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(consultant), Some(base_power + 2));

    // Two attackers do not meet the threshold.
    let mut no_trigger = GameEngine::new(733_013, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut no_trigger);
    let consultant = inject_creature_on_battlefield(&mut no_trigger, 0, "seasoned_consultant");
    let a = inject_creature_on_battlefield(&mut no_trigger, 0, "grizzly_bears");
    no_trigger
        .apply_command(0, &declare_attackers(vec![consultant, a]))
        .expect("declare two attackers");
    assert!(no_trigger.state.stack.is_empty() && no_trigger.state.pending_triggers.is_empty());
}

#[test]
fn issue_misc9_noggle_robber_makes_a_treasure_on_entry_and_death() {
    let mut e = engine(733_004);
    cast(&mut e, 0, "noggle_robber", vec![]);
    let robber = *e.state.players[0]
        .battlefield
        .iter()
        .find(|id| e.state.objects[id].card_id == "noggle_robber")
        .expect("the Robber entered");
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    cast(&mut e, 0, "murder", target_object(robber));
    assert_eq!(e.state.objects[&robber].zone, Zone::Graveyard);
    assert_eq!(
        battlefield_token_oids(&e, 0, "treasure").len(),
        2,
        "the dies trigger makes a second Treasure"
    );
}

#[test]
fn issue_misc9_vinereap_mentor_makes_food_on_entry_and_death() {
    let mut e = engine(733_005);
    cast(&mut e, 0, "vinereap_mentor", vec![]);
    let mentor = *e.state.players[0]
        .battlefield
        .iter()
        .find(|id| e.state.objects[id].card_id == "vinereap_mentor")
        .expect("the Mentor entered");
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);
    cast(&mut e, 0, "murder", target_object(mentor));
    assert_eq!(e.state.objects[&mentor].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 2);
}

#[test]
fn issue_misc9_meteor_golem_destroys_an_opponent_nonland_permanent() {
    let mut e = engine(733_006);
    let their_artifact = inject_permanent_on_battlefield(&mut e, 1, "swiftfoot_boots");
    cast(&mut e, 0, "meteor_golem", vec![]);
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "the entry trigger waits for a target"
    );
    semantic::accepted(&mut e, 0, &choose_trigger_target(their_artifact));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&their_artifact].zone, Zone::Graveyard);

    // An opponent's land is excluded, and a permanent the controller controls is not an opponent's.
    let mut bad = engine(733_016);
    let their_land = inject_permanent_on_battlefield(&mut bad, 1, "forest");
    let mine = inject_permanent_on_battlefield(&mut bad, 0, "swiftfoot_boots");
    cast(&mut bad, 0, "meteor_golem", vec![]);
    assert!(
        bad.apply_command(0, &choose_trigger_target(their_land))
            .is_err(),
        "a land is not a nonland permanent"
    );
    assert!(
        bad.apply_command(0, &choose_trigger_target(mine)).is_err(),
        "the controller's own permanent is not an opponent's"
    );
}

#[test]
fn issue_misc9_jumbo_cactuar_pumps_itself_when_attacking() {
    let mut e = GameEngine::new(733_007, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let cactuar = inject_creature_on_battlefield(&mut e, 0, "jumbo_cactuar");
    let base_power = e.effective_power(cactuar).expect("power");
    let base_toughness = e.effective_toughness(cactuar).expect("toughness");
    e.apply_command(0, &declare_attackers(vec![cactuar]))
        .expect("declare the Cactuar");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(cactuar), Some(base_power + 9999));
    assert_eq!(
        e.effective_toughness(cactuar),
        Some(base_toughness),
        "toughness is unchanged"
    );
}

#[test]
fn issue_misc9_undercity_dire_rat_makes_a_treasure_when_it_dies() {
    let mut e = engine(733_008);
    let rat = inject_creature_on_battlefield(&mut e, 0, "undercity_dire_rat");
    cast(&mut e, 0, "murder", target_object(rat));
    assert_eq!(e.state.objects[&rat].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
}
