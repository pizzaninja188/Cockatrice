use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn disappear_engine(seed: u64, cards: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", cards), deck_with("island", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn advance_to_end_step(engine: &mut GameEngine) {
    let active = engine.state.active_player_id();
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 1 to beginning of combat");
    engine
        .apply_command(active, &primitive_yield())
        .expect("beginning of combat advance");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_command(active, &primitive_yield())
            .expect("declare no attackers");
    }
    engine
        .apply_command(active, &primitive_yield())
        .expect("end combat to main 2");
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 2 to end step");
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
}

fn bounce_own_bear(engine: &mut GameEngine) {
    let bear = relocate_to_battlefield(engine, 0, "grizzly_bears", false);
    ensure_card_in_hand(engine, 0, "unsummon");
    grant_pool(engine, 0);
    let unsummon = hand_index_for_card(engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(bear)))
        .expect("cast Unsummon");
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Hand);
}

#[test]
fn insectoid_exterminator_uses_history_from_before_entry_and_parks_scry() {
    let mut engine = disappear_engine(
        189_101,
        &["insectoid_exterminator", "unsummon", "grizzly_bears"],
    );
    bounce_own_bear(&mut engine);
    relocate_to_battlefield(&mut engine, 0, "insectoid_exterminator", false);
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "Disappear trigger");

    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("scry choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryTop);
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the scried card on top");
    assert!(engine.state.pending_resolution.is_none());

    let mut no_departure = disappear_engine(189_102, &["insectoid_exterminator"]);
    relocate_to_battlefield(&mut no_departure, 0, "insectoid_exterminator", false);
    advance_to_end_step(&mut no_departure);
    assert!(
        no_departure.state.stack.is_empty(),
        "the intervening-if condition suppresses the trigger"
    );
}

#[test]
fn putrid_pals_enters_with_exactly_two_counters_only_after_a_departure() {
    let mut enabled = disappear_engine(189_201, &["putrid_pals", "unsummon", "grizzly_bears"]);
    bounce_own_bear(&mut enabled);
    let pals = move_ready_to_battlefield(&mut enabled, 0, "putrid_pals");
    assert_eq!(
        enabled.state.objects[&pals].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(enabled.effective_power(pals), Some(5));
    assert_eq!(enabled.effective_toughness(pals), Some(5));

    let mut disabled = disappear_engine(189_202, &["putrid_pals"]);
    let pals = move_ready_to_battlefield(&mut disabled, 0, "putrid_pals");
    assert_eq!(
        disabled.state.objects[&pals].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(disabled.effective_power(pals), Some(3));
    assert_eq!(disabled.effective_toughness(pals), Some(3));
}

#[test]
fn west_wind_entry_sacrifices_a_token_and_enables_its_disappear_draw() {
    let mut engine = disappear_engine(189_301, &["west_wind_avatar", "raise_the_alarm", "plains"]);
    ensure_card_in_hand(&mut engine, 0, "raise_the_alarm");
    grant_pool(&mut engine, 0);
    let alarm = hand_index_for_card(&engine, 0, "raise_the_alarm");
    engine
        .apply_command(0, &cast_spell(alarm, vec![]))
        .expect("cast Raise the Alarm");
    resolve_entire_stack_two_player(&mut engine);
    let token = battlefield_token_oids(&engine, 0, "soldier_w_1_1")[0];
    let land = relocate_to_battlefield(&mut engine, 0, "forest", false);

    ensure_card_in_hand(&mut engine, 0, "west_wind_avatar");
    grant_pool(&mut engine, 0);
    let avatar = hand_index_for_card(&engine, 0, "west_wind_avatar");
    engine
        .apply_command(0, &cast_spell(avatar, vec![]))
        .expect("cast West Wind Avatar");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "entry trigger on the stack");
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("optional sacrifice branch")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_command(0, &select_branch(0))
        .expect("choose the sacrifice branch");
    let candidates = &engine
        .state
        .pending_resolution
        .as_ref()
        .expect("token or land payment")
        .presentation
        .candidates;
    assert!(candidates.contains(&token));
    assert!(candidates.contains(&land));
    engine
        .apply_command(0, &submit_resolution_choice(vec![token]))
        .expect("sacrifice the token");
    assert_eq!(engine.state.players[0].life, 23);
    assert!(!engine.state.players[0].battlefield.contains(&token));
    assert!(
        engine
            .state
            .turn_history
            .current
            .player(0)
            .permanent_left_battlefield
    );

    let library_before = engine.state.players[0].library.len();
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "Disappear draw trigger");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
}

#[test]
fn west_wind_attack_accepts_a_land_rejects_a_creature_and_can_be_declined() {
    for pay in [true, false] {
        let mut engine = disappear_engine(
            189_310 + u64::from(pay),
            &["west_wind_avatar", "grizzly_bears"],
        );
        let avatar = relocate_to_battlefield(&mut engine, 0, "west_wind_avatar", false);
        let land = relocate_to_battlefield(&mut engine, 0, "forest", false);
        let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        engine.apply_command(0, &primitive_yield()).unwrap();
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &declare_attackers(vec![avatar]))
            .expect("attack with West Wind Avatar");
        answer_trigger_order_in_engine_order(&mut engine);
        assert_eq!(engine.state.stack.len(), 1, "attack trigger");
        pass_both_players(&mut engine);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .expect("optional attack payment")
                .presentation
                .choice_kind,
            ChoiceKind::ResolutionBranch
        );
        if pay {
            engine.apply_command(0, &select_branch(0)).unwrap();
            let candidates = &engine
                .state
                .pending_resolution
                .as_ref()
                .expect("land payment")
                .presentation
                .candidates;
            assert!(candidates.contains(&land));
            assert!(!candidates.contains(&bear));
            engine
                .apply_command(0, &submit_resolution_choice(vec![land]))
                .expect("sacrifice the land");
            assert_eq!(engine.state.players[0].life, 23);
            assert!(
                engine
                    .state
                    .turn_history
                    .current
                    .player(0)
                    .permanent_left_battlefield
            );
        } else {
            engine
                .apply_command(
                    0,
                    &submit_resolution_decision(ResolutionChoiceDecision::Decline),
                )
                .expect("decline the optional sacrifice");
            assert_eq!(engine.state.players[0].life, 20);
            assert!(
                !engine
                    .state
                    .turn_history
                    .current
                    .player(0)
                    .permanent_left_battlefield
            );
        }
    }
}
