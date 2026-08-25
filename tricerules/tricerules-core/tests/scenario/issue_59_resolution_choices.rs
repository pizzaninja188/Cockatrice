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
        })),
    }
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
