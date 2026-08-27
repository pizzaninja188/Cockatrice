use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ResolutionChoiceDecision, RuledCommand, SelectedSpellMode,
    SubmitResolutionChoice,
};

fn engine() -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("mountain".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(5900, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            cast_spell: None,
            chosen_combat_defender: None,
        })),
    }
}

fn crime_with_bolt(e: &mut GameEngine) {
    inject_card_into_hand(e, 0, "lightning_bolt");
    grant_pool(e, 0);
    e.apply_command(
        0,
        &cast_spell(
            hand_index_for_card(e, 0, "lightning_bolt"),
            target_player(1),
        ),
    )
    .unwrap();
}

#[test]
fn issue_171_apothecary_targets_first_and_chooses_keyword_at_resolution() {
    for (choice, keyword) in [
        (0, tricerules_cards::Keyword::Menace),
        (1, tricerules_cards::Keyword::Lifelink),
    ] {
        let mut e = engine();
        let apothecary = inject_creature_on_battlefield(&mut e, 0, "rattleback_apothecary");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        crime_with_bolt(&mut e);
        assert!(e.state.pending_resolution.is_none());
        assert!(e.state.pending_triggers.front().is_some());
        let choose = |oid| RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                targets: target_object(oid),
                decline: false,
                selected_modes: vec![],
            })),
        };
        assert!(e.apply_command(0, &choose(opponent)).is_err());
        e.apply_command(0, &choose(apothecary)).unwrap();
        assert!(
            e.state.pending_resolution.is_none(),
            "keyword is not a modal trigger choice"
        );
        pass_both_players(&mut e);
        assert_eq!(
            e.state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            ChoiceKind::ResolutionBranch
        );
        assert!(e.apply_command(0, &select_branch(99)).is_err());
        assert!(e.apply_command(1, &select_branch(choice)).is_err());
        e.apply_command(0, &select_branch(choice)).unwrap();
        assert!(e
            .characteristics(apothecary)
            .unwrap()
            .keywords
            .contains(&keyword));
        assert_eq!(e.state.turn_history.current.player(0).crimes_committed, 1);
        assert!(
            e.apply_command(0, &select_branch(choice)).is_err(),
            "no repeated continuation"
        );
        resolve_entire_stack_two_player(&mut e);
        end_active_turn(&mut e, 0);
        assert!(!e
            .characteristics(apothecary)
            .unwrap()
            .keywords
            .contains(&keyword));
    }
}

fn servant_combat_trigger(crime: bool) -> (GameEngine, u32, u32) {
    let mut e = engine();
    let servant = inject_creature_on_battlefield(&mut e, 0, "servant_of_the_stinger");
    let object = e.state.objects.get_mut(&servant).unwrap();
    object.power = Some(1);
    object.toughness = Some(3);
    let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    if crime {
        crime_with_bolt(&mut e);
        resolve_entire_stack_two_player(&mut e);
    }
    e.apply_command(0, &primitive_yield()).unwrap();
    pass_both_players(&mut e);
    e.apply_command(0, &declare_attackers(vec![servant]))
        .unwrap();
    pass_both_players(&mut e);
    pass_both_players(&mut e);
    assert_eq!(e.state.players[1].life, if crime { 16 } else { 19 });
    (e, servant, other)
}

#[test]
fn issue_171_servant_sacrifices_only_itself_then_searches() {
    let (mut e, servant, other) = servant_combat_trigger(true);
    pass_both_players(&mut e);
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .expect("optional sacrifice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    e.apply_command(0, &select_branch(0)).unwrap();
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![servant]
    );
    assert!(e
        .apply_command(0, &submit_resolution_choice(vec![other]))
        .is_err());
    e.apply_command(0, &submit_resolution_choice(vec![servant]))
        .unwrap();
    assert!(e.state.players[0].graveyard.contains(&servant));
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("search follows payment");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibrarySearch);
    let searched = pending.presentation.candidates[0];
    e.apply_command(0, &submit_resolution_choice(vec![searched]))
        .unwrap();
    assert!(e.state.players[0].hand.contains(&searched));
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn issue_171_servant_checks_condition_and_never_searches_without_payment() {
    let (e, _, _) = servant_combat_trigger(false);
    assert!(e.state.stack.is_empty(), "no Crime means no combat trigger");
    for failure in ["condition", "decline", "departed", "stolen"] {
        let (mut e, servant, _) = servant_combat_trigger(true);
        let hand_before = e.state.players[0].hand.len();
        match failure {
            // Exercise the resolution-side intervening-if check independently of its monotonic
            // normal turn history, as other condition tests do with controlled fixtures.
            "condition" => e.state.turn_history.current.player_mut(0).crimes_committed = 0,
            "departed" => {
                e.state.players[0].battlefield.retain(|id| *id != servant);
                e.state.players[0].graveyard.push(servant);
                e.state.objects.get_mut(&servant).unwrap().zone = tricerules_core::Zone::Graveyard;
                e.state.zone_change_generation.insert(servant, 1);
            }
            "stolen" => {
                e.state.players[0].battlefield.retain(|id| *id != servant);
                e.state.players[1].battlefield.push(servant);
                let object = e.state.objects.get_mut(&servant).unwrap();
                object.controller = 1;
                object.base_controller = 1;
            }
            _ => {}
        }
        pass_both_players(&mut e);
        if failure == "decline" {
            e.apply_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::Decline),
            )
            .unwrap();
        }
        assert!(
            e.state.pending_resolution.is_none(),
            "{failure}: no library prompt"
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before);
        assert!(e.state.stack.is_empty());
    }
}

#[test]
fn issue_171_servant_rejects_a_returned_source_during_payment() {
    let (mut e, servant, _) = servant_combat_trigger(true);
    pass_both_players(&mut e);
    e.apply_command(0, &select_branch(0)).unwrap();
    e.state.zone_change_generation.insert(servant, 1);
    let history = e.state.turn_history.clone();
    assert!(e
        .apply_command(0, &submit_resolution_choice(vec![servant]))
        .is_err());
    assert!(e.state.players[0].battlefield.contains(&servant));
    assert_eq!(e.state.turn_history, history);
    assert!(e.state.pending_resolution.is_some());
    e.apply_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn issue_171_apothecary_fizzles_without_prompt_when_target_blinks() {
    let mut e = engine();
    let source = inject_creature_on_battlefield(&mut e, 0, "rattleback_apothecary");
    crime_with_bolt(&mut e);
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                targets: target_object(source),
                decline: false,
                selected_modes: vec![],
            })),
        },
    )
    .unwrap();
    e.state.zone_change_generation.insert(source, 1);
    pass_both_players(&mut e);
    assert!(e.state.pending_resolution.is_none());
    assert!(!e
        .characteristics(source)
        .unwrap()
        .keywords
        .contains(&tricerules_cards::Keyword::Menace));
    assert_eq!(e.state.stack.len(), 1, "the original Bolt remains");
}

#[test]
fn issue_59_calibration_cards_are_registered() {
    let registry = CardRegistry::global();

    for card_id in ["trufflesnout", "sparktongue_dragon", "crypt_lurker"] {
        assert!(
            registry.get(card_id).is_some(),
            "issue #59 requires {card_id} to be implemented"
        );
    }
}

#[test]
fn trufflesnout_trigger_mode_is_chosen_before_it_reaches_the_stack() {
    let mut engine = engine();
    let trufflesnout = inject_card_into_hand(&mut engine, 0, "trufflesnout");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "trufflesnout");
    engine
        .apply_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Trufflesnout");
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("modal ETB choice must be pending");
    assert!(pending.ability.modal.is_some());
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: vec![SelectedSpellMode {
                        mode_index: 0,
                        targets: Vec::new(),
                    }],
                    targets: Vec::new(),
                })),
            },
        )
        .expect("choose counter mode");
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .objects
            .get(&trufflesnout)
            .expect("Trufflesnout")
            .counter_count(tricerules_cards::primitives::CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn crypt_lurker_discard_branch_pays_then_draws() {
    let mut engine = engine();
    inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let discarded = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "crypt_lurker");
    engine
        .apply_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Crypt Lurker");
    pass_both_players(&mut engine); // creature resolves, ETB trigger reaches stack
    pass_both_players(&mut engine); // ETB parks for branch choice
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("branch choice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .expect("branch choice")
            .continuation,
        ResolutionContinuation::AuthoredBranch {
            branch: tricerules_core::state::PendingResolutionBranch {
                stage: PendingResolutionBranchStage::Selecting,
                ..
            },
            ..
        }
    ));
    engine
        .apply_command(0, &select_branch(1))
        .expect("choose discard branch");
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .expect("discard payment")
            .continuation,
        ResolutionContinuation::AuthoredBranch {
            branch: tricerules_core::state::PendingResolutionBranch {
                stage: PendingResolutionBranchStage::PayingObjects { .. },
                ..
            },
            ..
        }
    ));
    engine
        .apply_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("discard creature card");
    assert_eq!(
        engine
            .state
            .objects
            .get(&discarded)
            .expect("discarded card")
            .zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn crypt_lurker_rejects_a_stale_discard_without_consuming_the_choice() {
    let mut engine = engine();
    inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let candidate = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "crypt_lurker");
    engine
        .apply_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Crypt Lurker");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &select_branch(1))
        .expect("choose discard branch");

    engine.state.players[0].hand.retain(|oid| *oid != candidate);
    engine.state.players[0].graveyard.push(candidate);
    engine
        .state
        .objects
        .get_mut(&candidate)
        .expect("candidate")
        .zone = tricerules_core::Zone::Graveyard;
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![candidate]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
}

#[test]
fn sparktongue_payment_creates_a_separately_targeted_reflexive_trigger() {
    let mut engine = engine();
    inject_card_into_hand(&mut engine, 0, "sparktongue_dragon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "sparktongue_dragon");
    engine
        .apply_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Sparktongue Dragon");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &select_branch(0))
        .expect("choose mana branch");
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .expect("mana payment")
            .continuation,
        ResolutionContinuation::AuthoredBranch {
            branch: tricerules_core::state::PendingResolutionBranch {
                stage: PendingResolutionBranchStage::PayingMana { .. },
                ..
            },
            ..
        }
    ));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::PayMana),
        )
        .expect("pay {2}{R}");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_player(1),
                })),
            },
        )
        .expect("target opponent with reflexive trigger");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[1].life, 17);
}
