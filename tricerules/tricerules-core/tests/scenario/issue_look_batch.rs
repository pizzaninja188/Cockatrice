//! Reviewed look-selection and combat-trigger scenarios: Frontier Seeker, Eclipsed Elf, Eclipsed
//! Boggart, Eclipsed Merrow, Staunch Crewmate, Wild Pack Squad, Might of the Ancestors and
//! Tatyova, Benthic Druid.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21. Every expectation is the
//! reviewed printed Oracle behavior. Governance: CR 603.6a (entry triggers), 701.18/701.23 (look
//! and reveal), 121.1 (draw), 119.3 (life gain), 611.2c and 613 layer 6 (until-end-of-turn grants),
//! 508.1/603.2 (beginning-of-combat triggers), and 305/603.6a (landfall).

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_proto::ruled::v1::TargetRefKind;

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
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

/// Inject a library card at the top so a look-at-the-top effect sees it.
fn inject_library_top(e: &mut GameEngine, card_id: &str) -> u32 {
    let id = inject_library_card(e, 0, card_id);
    let library = &mut e.state.players[0].library;
    library.retain(|object_id| *object_id != id);
    library.push_front(id);
    id
}

fn cast_creature(e: &mut GameEngine, card: &str) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, vec![]));
    pass_both_players(e);
}

fn resolve_to_choice(e: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    e.apply_command(0, &pass()).expect("active passes");
    e.apply_command(1, &pass()).expect("resolve to a choice")
}

fn look_choice_candidates(e: &mut GameEngine) -> Vec<u32> {
    let batch = resolve_to_choice(e);
    let choice = find_resolution_choice(&batch).expect("look choice");
    choice.candidate_object_ids.clone()
}

#[test]
fn issue_look_frontier_seeker() {
    let mut e = engine(720_001);
    let plains = inject_library_top(&mut e, "plains");
    cast_creature(&mut e, "frontier_seeker");
    let candidates = look_choice_candidates(&mut e);
    assert!(candidates.contains(&plains));
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![plains]));
    assert!(e.state.players[0].hand.contains(&plains));

    // A plain creature card is neither a Mount creature nor a Plains card.
    let mut illegal = engine(720_009);
    let unrelated = inject_library_top(&mut illegal, "grizzly_bears");
    cast_creature(&mut illegal, "frontier_seeker");
    let _ = look_choice_candidates(&mut illegal);
    assert!(illegal
        .apply_command(0, &submit_resolution_choice(vec![unrelated]))
        .is_err());
}

#[test]
fn issue_look_eclipsed_elf() {
    let mut e = engine(720_002);
    let elf = inject_library_top(&mut e, "llanowar_elves");
    let forest = inject_library_top(&mut e, "forest");
    let unrelated = inject_library_top(&mut e, "grizzly_bears");
    cast_creature(&mut e, "eclipsed_elf");
    let candidates = look_choice_candidates(&mut e);
    assert!(candidates.contains(&elf));
    assert!(candidates.contains(&forest));
    assert!(
        e.apply_command(0, &submit_resolution_choice(vec![unrelated]))
            .is_err(),
        "a plain creature card is not an Elf, Swamp, or Forest card"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![elf]));
    assert!(e.state.players[0].hand.contains(&elf));
}

#[test]
fn issue_look_eclipsed_boggart() {
    let mut e = engine(720_003);
    let goblin = inject_library_top(&mut e, "crazed_goblin");
    let mountain = inject_library_top(&mut e, "mountain");
    let unrelated = inject_library_top(&mut e, "grizzly_bears");
    cast_creature(&mut e, "eclipsed_boggart");
    let candidates = look_choice_candidates(&mut e);
    assert!(candidates.contains(&goblin));
    assert!(candidates.contains(&mountain));
    assert!(
        e.apply_command(0, &submit_resolution_choice(vec![unrelated]))
            .is_err(),
        "a plain creature card is not a Goblin, Swamp, or Mountain card"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![goblin]));
    assert!(e.state.players[0].hand.contains(&goblin));
}

#[test]
fn issue_look_eclipsed_merrow() {
    let mut e = engine(720_004);
    let merfolk = inject_library_top(&mut e, "coral_merfolk");
    let island = inject_library_top(&mut e, "island");
    let unrelated = inject_library_top(&mut e, "grizzly_bears");
    cast_creature(&mut e, "eclipsed_merrow");
    let candidates = look_choice_candidates(&mut e);
    assert!(candidates.contains(&merfolk));
    assert!(candidates.contains(&island));
    assert!(
        e.apply_command(0, &submit_resolution_choice(vec![unrelated]))
            .is_err(),
        "a plain creature card is not a Merfolk, Plains, or Island card"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![island]));
    assert!(e.state.players[0].hand.contains(&island));
}

#[test]
fn issue_look_staunch_crewmate() {
    // The artifact half.
    let mut e = engine(720_005);
    let artifact = inject_library_top(&mut e, "swiftfoot_boots");
    cast_creature(&mut e, "staunch_crewmate");
    let candidates = look_choice_candidates(&mut e);
    assert!(candidates.contains(&artifact));
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![artifact]));
    assert!(e.state.players[0].hand.contains(&artifact));

    // The Pirate half, with a plain creature rejected.
    let mut pirate_engine = engine(720_010);
    let pirate = inject_library_top(&mut pirate_engine, "marauding_mako");
    let unrelated = inject_library_top(&mut pirate_engine, "grizzly_bears");
    cast_creature(&mut pirate_engine, "staunch_crewmate");
    let candidates = look_choice_candidates(&mut pirate_engine);
    assert!(candidates.contains(&pirate));
    assert!(
        pirate_engine
            .apply_command(0, &submit_resolution_choice(vec![unrelated]))
            .is_err(),
        "a creature with neither the artifact card type nor the Pirate subtype is not legal"
    );
    semantic::accepted(
        &mut pirate_engine,
        0,
        &submit_resolution_choice(vec![pirate]),
    );
    assert!(pirate_engine.state.players[0].hand.contains(&pirate));
}

#[test]
fn issue_look_wild_pack_squad() {
    let mut e = engine(720_006);
    let squad = inject_creature_on_battlefield(&mut e, 0, "wild_pack_squad");
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    assert_eq!(e.state.pending_triggers.len(), 1, "combat trigger waits");
    e.apply_command(0, &choose_trigger_target(target))
        .expect("choose the target creature");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(target, Keyword::FirstStrike));
    assert!(e.effective_has_keyword(target, Keyword::Vigilance));
    assert!(!e.effective_has_keyword(squad, Keyword::FirstStrike));
}

#[test]
fn issue_look_might_of_the_ancestors() {
    let mut e = engine(720_007);
    inject_permanent_on_battlefield(&mut e, 0, "might_of_the_ancestors");
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    assert_eq!(e.state.pending_triggers.len(), 1);
    assert!(
        e.apply_command(0, &choose_trigger_target(opposing))
            .is_err(),
        "only a creature you control is a legal target"
    );
    e.apply_command(0, &choose_trigger_target(own))
        .expect("choose a creature you control");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(own), Some(4));
    assert!(e.effective_has_keyword(own, Keyword::Vigilance));
    assert_eq!(e.effective_power(opposing), Some(2));
}

#[test]
fn issue_look_tatyova_benthic_druid() {
    let mut e = engine(720_008);
    inject_creature_on_battlefield(&mut e, 0, "tatyova,_benthic_druid");
    let life_before = e.state.players[0].life;

    // A nonland permanent entering does not trigger landfall.
    cast_creature(&mut e, "grizzly_bears");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].life, life_before,
        "nonland entry ignores landfall"
    );

    // Playing a land fires landfall: gain one life and draw a card.
    inject_card_into_hand(&mut e, 0, "forest");
    let hand_after_inject = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "forest");
    semantic::accepted(&mut e, 0, &play_land(slot));
    pass_both_players(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, life_before + 1);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_after_inject - 1 + 1,
        "the played land leaves hand and the trigger draws one"
    );
}
