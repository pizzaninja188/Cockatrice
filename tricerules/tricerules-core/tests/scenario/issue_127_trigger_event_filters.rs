use super::helpers::*;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            cast_spell: None,
            chosen_combat_defender: None,
            payment: None,
            restricted_mana: vec![],
        })),
    }
}

#[test]
fn issue_127_entry_counters_are_included_in_the_trigger_power_check() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &[
                "vicious_clown",
                "endless_one",
                "endless_one",
                "instill_infection",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(127_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_battlefield(&mut engine, 0, "vicious_clown", false);
    ensure_in_hand(&mut engine, 0, "endless_one");
    grant_pool(&mut engine, 0);

    let endless_one = hand_index_for_card(&engine, 0, "endless_one");
    engine
        .apply_command(0, &cast_spell_x(endless_one, vec![], 3))
        .expect("cast Endless One with X=3");
    pass_both_players(&mut engine);

    assert!(
        engine.state.stack.is_empty(),
        "a creature that enters as a 3/3 must not trigger Vicious Clown"
    );
    let too_large = battlefield_object_for_card(&engine, 0, "endless_one");
    ensure_in_hand(&mut engine, 0, "instill_infection");
    let infection = hand_index_for_card(&engine, 0, "instill_infection");
    engine
        .apply_command(0, &cast_spell(infection, target_object(too_large)))
        .expect("cast Instill Infection after entry");
    pass_both_players(&mut engine);
    assert_eq!(engine.effective_power(too_large), Some(2));
    assert!(
        engine.state.stack.is_empty(),
        "shrinking the entrant later must not create the missed trigger"
    );

    ensure_in_hand(&mut engine, 0, "endless_one");
    let endless_one = hand_index_for_card(&engine, 0, "endless_one");
    engine
        .apply_command(0, &cast_spell_x(endless_one, vec![], 2))
        .expect("cast Endless One with X=2");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "a 2/2 entrant must trigger");
    pass_both_players(&mut engine);
    let clown = battlefield_object_for_card(&engine, 0, "vicious_clown");
    assert_eq!(engine.effective_power(clown), Some(4));
}

#[test]
fn issue_127_continuous_effects_are_included_in_the_entry_power_check() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["vicious_clown", "glorious_anthem", "endless_one"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(127_002, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let clown = relocate_to_battlefield(&mut engine, 0, "vicious_clown", false);
    ensure_in_hand(&mut engine, 0, "glorious_anthem");
    ensure_in_hand(&mut engine, 0, "endless_one");
    grant_pool(&mut engine, 0);

    let anthem = hand_index_for_card(&engine, 0, "glorious_anthem");
    engine
        .apply_command(0, &cast_spell(anthem, vec![]))
        .expect("cast Glorious Anthem");
    pass_both_players(&mut engine);

    let endless_one = hand_index_for_card(&engine, 0, "endless_one");
    engine
        .apply_command(0, &cast_spell_x(endless_one, vec![], 2))
        .expect("cast Endless One with X=2");
    pass_both_players(&mut engine);

    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.effective_power(clown), Some(3));
    let entrant = battlefield_object_for_card(&engine, 0, "endless_one");
    assert_eq!(engine.effective_power(entrant), Some(3));
}

#[test]
fn issue_127_a_collected_trigger_is_not_rechecked_after_entry() {
    let decks = Some(vec![
        deck_with("forest", &["vicious_clown", "endless_one", "giant_growth"]),
        deck_with("mountain", &[]),
    ]);
    let mut engine = GameEngine::new(127_003, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let clown = relocate_to_battlefield(&mut engine, 0, "vicious_clown", false);
    ensure_in_hand(&mut engine, 0, "endless_one");
    ensure_in_hand(&mut engine, 0, "giant_growth");
    grant_pool(&mut engine, 0);

    let endless_one = hand_index_for_card(&engine, 0, "endless_one");
    engine
        .apply_command(0, &cast_spell_x(endless_one, vec![], 2))
        .expect("cast Endless One with X=2");
    pass_both_players(&mut engine);
    let entrant = battlefield_object_for_card(&engine, 0, "endless_one");
    assert_eq!(engine.state.stack.len(), 1, "trigger is already collected");

    let growth = hand_index_for_card(&engine, 0, "giant_growth");
    engine
        .apply_command(0, &cast_spell(growth, target_object(entrant)))
        .expect("cast Giant Growth over the trigger");
    pass_both_players(&mut engine);
    assert_eq!(engine.effective_power(entrant), Some(5));
    assert_eq!(engine.state.stack.len(), 1, "the entry trigger remains");

    pass_both_players(&mut engine);
    assert_eq!(engine.effective_power(clown), Some(4));
}

#[test]
fn issue_127_each_simultaneous_eligible_entrant_triggers_once() {
    let decks = Some(vec![
        deck_with("plains", &["vicious_clown", "raise_the_alarm"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(127_004, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let clown = relocate_to_battlefield(&mut engine, 0, "vicious_clown", false);
    ensure_in_hand(&mut engine, 0, "raise_the_alarm");
    grant_pool(&mut engine, 0);

    let alarm = hand_index_for_card(&engine, 0, "raise_the_alarm");
    engine
        .apply_command(0, &cast_spell(alarm, vec![]))
        .expect("cast Raise the Alarm");
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("two triggers require ordering");
    assert_eq!(pending.candidates.len(), 2);
    answer_trigger_order_in_engine_order(&mut engine);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(clown), Some(6));
}

#[test]
fn issue_127_another_and_controller_restrictions_are_enforced() {
    let decks = Some(vec![
        deck_with("mountain", &["vicious_clown"]),
        deck_with("plains", &["savannah_lions"]),
    ]);
    let mut engine = GameEngine::new(127_005, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "vicious_clown");
    grant_pool(&mut engine, 0);

    let clown_index = hand_index_for_card(&engine, 0, "vicious_clown");
    engine
        .apply_command(0, &cast_spell(clown_index, vec![]))
        .expect("cast Vicious Clown");
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty(), "the Clown excludes itself");
    let clown = battlefield_object_for_card(&engine, 0, "vicious_clown");

    end_active_turn(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 1, "savannah_lions");
    grant_pool(&mut engine, 1);
    let lion = hand_index_for_card(&engine, 1, "savannah_lions");
    engine
        .apply_command(1, &cast_spell(lion, vec![]))
        .expect("opponent casts Savannah Lions");
    pass_both_players(&mut engine);

    assert!(
        engine.state.stack.is_empty(),
        "opponent's entrant does not trigger"
    );
    assert_eq!(engine.effective_power(clown), Some(2));
}

#[test]
fn issue_127_mentor_optional_payment_draws_exactly_one_card() {
    let decks = Some(vec![
        deck_with("plains", &["mentor_of_the_meek", "savannah_lions"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(127_006, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_battlefield(&mut engine, 0, "mentor_of_the_meek", false);
    ensure_in_hand(&mut engine, 0, "savannah_lions");
    grant_pool(&mut engine, 0);

    let lion = hand_index_for_card(&engine, 0, "savannah_lions");
    engine
        .apply_command(0, &cast_spell(lion, vec![]))
        .expect("cast Savannah Lions");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert!(engine.state.pending_resolution.is_some());

    engine
        .apply_command(0, &select_branch(0))
        .expect("choose to pay {1}");
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {1}");

    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert!(engine.state.pending_resolution.is_none());
}
