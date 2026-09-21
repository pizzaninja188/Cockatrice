//! Reviewed dies-trigger and cast/attack-trigger scenarios: Beamsaw Prospector, Mintstrosity,
//! Greedy Freebooter, Maalfeld Twins, Spring Splasher and Crackling Cyclops.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 603.6c (dies
//! triggers), 111.1 (tokens), 701.18 (scry), 508.1/603.2c (attack triggers and entity-scoped
//! targets), 611.2c and 613 layer 7c (until-end-of-turn pumps), and 601.2/603.2 (cast triggers).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
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

/// A two-player game advanced to the declare-attackers step with both pools refilled.
fn combat_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

/// Seat `card_id` on top of P0's library so a scry/look effect sees it.
fn inject_library_top(e: &mut GameEngine, card_id: &str) -> u32 {
    let id = inject_library_card(e, 0, card_id);
    let library = &mut e.state.players[0].library;
    library.retain(|object_id| *object_id != id);
    library.push_front(id);
    id
}

/// Kill `source` with Murder so the committed battlefield-to-graveyard move runs through the
/// state-based-action funnel and emits the CR 603.6c dies event. Returns the batch in which the
/// dies trigger, if any, is the only stack object waiting on P0's priority.
fn kill_with_murder_to_open_trigger(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    let slot = hand_index_for_card(e, 0, "murder");
    e.apply_command(0, &cast_spell(slot, target_object(source)))
        .expect("cast Murder");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("murder resolves");
}

#[test]
fn issue_dies_beamsaw_prospector() {
    let mut e = engine(721_001);
    let source = inject_creature_with_stats(&mut e, 0, "beamsaw_prospector", 2, 1);
    assert!(battlefield_token_oids(&e, 0, "lander").is_empty());
    kill_with_murder_to_open_trigger(&mut e, source);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "lander").len(),
        1,
        "the dies trigger creates one Lander token"
    );
}

#[test]
fn issue_dies_mintstrosity() {
    let mut e = engine(721_002);
    let source = inject_creature_with_stats(&mut e, 0, "mintstrosity", 3, 1);
    kill_with_murder_to_open_trigger(&mut e, source);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "food").len(),
        1,
        "the dies trigger creates one Food token"
    );
}

#[test]
fn issue_dies_greedy_freebooter() {
    let mut e = engine(721_003);
    let source = inject_creature_with_stats(&mut e, 0, "greedy_freebooter", 1, 1);
    let top = inject_library_top(&mut e, "grizzly_bears");

    kill_with_murder_to_open_trigger(&mut e, source);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);

    // The trigger resolves scry first and parks on the top-card decision before making the token.
    e.apply_command(0, &pass()).expect("p0 pass");
    let batch = e.apply_command(1, &pass()).expect("trigger parks on scry");
    let choice = find_resolution_choice(&batch).expect("scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert!(choice.candidate_object_ids.contains(&top));
    assert!(
        battlefield_token_oids(&e, 0, "treasure").is_empty(),
        "the token waits for the scry decision"
    );

    // Keeping the scried card on top (empty bottom pile) resumes into the Treasure token.
    semantic::accepted(&mut e, 0, &submit_resolution_choice(Vec::new()));
    assert_eq!(e.state.players[0].library.front(), Some(&top));
    assert_eq!(
        battlefield_token_oids(&e, 0, "treasure").len(),
        1,
        "the trigger creates one Treasure token after the scry"
    );
}

#[test]
fn issue_dies_maalfeld_twins() {
    let mut e = engine(721_004);
    let source = inject_creature_with_stats(&mut e, 0, "maalfeld_twins", 4, 4);
    kill_with_murder_to_open_trigger(&mut e, source);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "zombie_b_2_2").len(),
        2,
        "the dies trigger creates exactly two Zombie tokens"
    );
}

#[test]
fn issue_dies_spring_splasher() {
    let mut e = combat_engine(721_005);
    let splasher = inject_creature_with_stats(&mut e, 0, "spring_splasher", 2, 1);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);

    e.apply_command(0, &declare_attackers(vec![splasher]))
        .expect("declare Spring Splasher as an attacker");
    assert_eq!(e.state.pending_triggers.len(), 1);
    assert!(
        e.apply_command(0, &choose_trigger_target(own)).is_err(),
        "a creature the controller controls is not controlled by the defending player"
    );
    assert!(
        e.apply_command(0, &choose_trigger_target(splasher))
            .is_err(),
        "the attacking source is not controlled by the defending player"
    );
    e.apply_command(0, &choose_trigger_target(opposing))
        .expect("choose a creature the defending player controls");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(opposing), Some(2), "5 - 3");
    assert_eq!(
        e.effective_toughness(opposing),
        Some(5),
        "-3/-0 leaves toughness alone"
    );
    assert_eq!(
        e.effective_power(own),
        Some(2),
        "an untargeted creature is unchanged"
    );
    assert_eq!(
        e.effective_power(splasher),
        Some(2),
        "the source is unchanged"
    );
}

#[test]
fn issue_dies_crackling_cyclops() {
    let mut e = engine(721_006);
    let cyclops = inject_creature_with_stats(&mut e, 0, "crackling_cyclops", 0, 4);

    // A noncreature spell the controller casts pumps the Cyclops by +3/+0 until end of turn.
    inject_card_into_hand(&mut e, 0, "lightning_bolt");
    let slot = hand_index_for_card(&e, 0, "lightning_bolt");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_player_damage(1, 3)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(cyclops), Some(3));
    assert_eq!(e.effective_toughness(cyclops), Some(4));

    // Casting a creature spell does not add another +3/+0.
    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    let bear_slot = hand_index_for_card(&e, 0, "grizzly_bears");
    semantic::accepted(&mut e, 0, &cast_spell(bear_slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(cyclops),
        Some(3),
        "a creature spell does not trigger the cast pump"
    );

    // An opponent's noncreature spell does not trigger the controller-scoped cast trigger.
    e.apply_command(0, &pass()).expect("p0 pass");
    inject_card_into_hand(&mut e, 1, "lightning_bolt");
    let opp_slot = hand_index_for_card(&e, 1, "lightning_bolt");
    semantic::accepted(&mut e, 1, &cast_spell(opp_slot, target_player_damage(0, 3)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(cyclops),
        Some(3),
        "an opponent's noncreature spell does not trigger the controller cast trigger"
    );
}
