use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, CostObjectRef, CostObjectRefs, CostSelection,
};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn choose_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_153_chaos_spewer_declining_without_a_creature_completes_blight() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_103,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["chaos_spewer"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "chaos_spewer");
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let slot = hand_index_for_card(&engine, 0, "chaos_spewer");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap();
    pass_both_players(&mut engine);
    let spewer = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "chaos_spewer" && object.zone == Zone::Battlefield)
        .expect("Chaos Spewer enters before its trigger resolves")
        .id;
    // Remove the source after its trigger is on the stack. Blight then has no creature to
    // affect, while the captured triggered program still resolves under CR 608.2m.
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != spewer);
    engine.state.players[0].graveyard.push(spewer);
    engine.state.objects.get_mut(&spewer).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(spewer)
        .or_default() += 1;
    pass_both_players(&mut engine);
    // Already-floating mana must remain available for explicit pip selection in the client.
    engine.state.players[0].mana_pool.colorless = 2;
    let payment_batch = engine.apply_scenario_command(0, &choose_branch(0)).unwrap();
    let payment = payment_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) if choice.choice_kind == ChoiceKind::ManaPayment as i32 => Some(choice),
            _ => None,
        })
        .expect("choosing Pay must open a staged mana payment");
    assert_eq!(payment.generic_mana_cost, 2);
    assert!(
        payment.mana_cost.is_empty(),
        "generic costs use the existing pip picker"
    );
    assert!(payment.payment_currently_legal);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    let declined = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
                    decision: ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(find_resolution_choice(&declined).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine
        .state
        .objects
        .values()
        .all(|object| { object.counter_count(CounterKind::MinusOneMinusOne) == 0 }));
}

#[test]
fn issue_153_departed_optional_program_payer_takes_otherwise_without_delegation() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_106,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("mountain", &["chaos_spewer"]),
            deck_with("forest", &[]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "chaos_spewer");
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let slot = hand_index_for_card(&engine, 0, "chaos_spewer");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("P0 casts Chaos Spewer");
    pass_priority_round(&mut engine);
    let spewer = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "chaos_spewer" && object.zone == Zone::Battlefield)
        .expect("Chaos Spewer resolves before its trigger")
        .id;
    let trigger = engine
        .state
        .stack
        .last_mut()
        .expect("Spewer trigger on stack");
    // Keep the source owned and controlled by P0 while P1 controls its captured trigger.
    // This isolates the optional payer departure from physical-source removal.
    trigger.controller = 1;
    engine.state.players[1].mana_pool.colorless = 2;
    pass_priority_round(&mut engine);
    let menu = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("trigger offers its compiled optional Choose")
        .deciding_player;
    assert_eq!(menu, 1);

    let payment = engine
        .apply_scenario_command(1, &choose_branch(0))
        .expect("P1 selects the paid branch to open its optional payment");
    let payment_prompt = find_resolution_choice(&payment).expect("staged payment prompt");
    assert_eq!(payment_prompt.deciding_player_id, 1);
    assert_eq!(payment_prompt.choice_kind, ChoiceKind::ManaPayment as i32);
    assert_eq!(engine.state.players[1].mana_pool.colorless, 2);

    let departure = engine
        .apply_scenario_command(1, &concede())
        .expect("P1 leaves while the optional Choose payment is pending");
    assert!(engine.state.players[1].has_lost);
    assert!(engine.state.objects.contains_key(&spewer));
    assert_eq!(engine.state.objects[&spewer].zone, Zone::Battlefield);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(find_resolution_choice(&departure).is_none());
    assert!(departure.events.iter().all(|event| !matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == ChoiceKind::ResolutionBranch as i32
    )));
    assert_eq!(
        engine.state.objects[&spewer].counter_count(CounterKind::MinusOneMinusOne),
        0
    );
}

#[test]
fn issue_153_parked_choose_survives_source_leave_and_same_oid_return() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_110,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["chaos_spewer"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "chaos_spewer");
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let slot = hand_index_for_card(&engine, 0, "chaos_spewer");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Chaos Spewer");
    pass_both_players(&mut engine);
    let spewer = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "chaos_spewer" && object.zone == Zone::Battlefield)
        .expect("Spewer is the source of the pending trigger")
        .id;
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&spewer)
        .copied()
        .unwrap_or(0);
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch,
        "the triggered program is already parked at Choose before the source changes zones"
    );

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != spewer);
    engine.state.players[0].graveyard.push(spewer);
    engine.state.objects.get_mut(&spewer).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(spewer)
        .or_default() += 1;
    engine.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != spewer);
    engine.state.players[0].battlefield.push(spewer);
    engine.state.objects.get_mut(&spewer).unwrap().zone = Zone::Battlefield;
    *engine
        .state
        .zone_change_generation
        .entry(spewer)
        .or_default() += 1;
    let returned_generation = engine.state.zone_change_generation[&spewer];
    assert!(returned_generation >= source_generation + 2);

    engine.state.players[0].mana_pool.colorless = 2;
    let payment = engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("the parked menu and controller survive source leave-return");
    assert_eq!(
        find_resolution_choice(&payment).unwrap().choice_kind,
        ChoiceKind::ManaPayment as i32
    );
    let blight = submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::Decline)
        .expect("declining the payment enters the captured Otherwise block");
    let blight =
        find_resolution_choice(&blight).expect("returned Spewer is a current Blight candidate");
    assert_eq!(blight.choice_kind, ChoiceKind::CostObjects as i32);
    assert!(blight.candidate_object_ids.contains(&spewer));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![spewer]))
        .expect("choose the current incarnation of the returned Spewer");
    assert_eq!(
        engine.state.zone_change_generation[&spewer],
        returned_generation
    );
    assert_eq!(engine.state.objects[&spewer].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&spewer].counter_count(CounterKind::MinusOneMinusOne),
        2
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn program_blight_completion_resumes_following_tail_once() {
    const PROGRAM: &str = r#"(
        id: "blight_program_tail_fixture",
        name: "Blight Program Tail Fixture",
        face_id: "blight_program_tail_fixture",
        mana_cost: "{0}",
        types: ["Sorcery"],
        spell_program: Some((body: (steps: [
            (id: "pay_or_blight", kind: Choose(
                choice: "pay_or_blight", chooser: Controller, optional: true,
                branches: [(
                    branch_id: "pay_2", presentation: Fallback, cost: Mana("{2}"),
                    body: (steps: [], yields: []),
                )],
                otherwise: (steps: [
                    (id: "blight_2", kind: Perform(Blight(count: 2))),
                ], yields: []),
            )),
            (id: "tail_draw", bind: ["tail_draw"], kind: Perform(Draw(
                who: Controller, count: 1,
            ))),
        ], yields: []), target_bindings: [])),
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                PROGRAM,
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/doubling_season.ron"),
            ],
            &[],
        )
        .expect("the Blight-tail fixture registry compiles"),
    ));
    let decks = Some(vec![
        vec!["blight_program_tail_fixture".into(); 30],
        vec!["mountain".into(); 30],
    ]);
    let mut engine = GameEngine::new(registry, 153_111, &[0, 1], 20, decks, true)
        .expect("Blight-tail fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let target = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 12, 12);
    ensure_card_in_hand(&mut engine, 0, "blight_program_tail_fixture");
    let source_slot = hand_index_for_card(&engine, 0, "blight_program_tail_fixture");
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_scenario_command(0, &cast_spell(source_slot, vec![]))
        .expect("cast the synthetic Blight-tail program");
    let offered = find_resolution_choice(&pass_both_players_with_batch(&mut engine))
        .expect("Choose offers the optional Mana branch before Blight");
    assert_eq!(offered.choice_kind, ChoiceKind::ResolutionBranch as i32);
    let blight_prompt = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
                    decision: ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("decline Mana and enter the Otherwise block");
    let blight = find_resolution_choice(&blight_prompt).expect("Blight parks before its tail");
    assert_eq!(blight.choice_kind, ChoiceKind::CostObjects as i32);
    assert_eq!((blight.min, blight.max), (1, 1));
    assert_eq!(blight.candidate_object_ids.len(), 1);
    assert_eq!(blight.candidate_object_ids, vec![target]);
    let counter_replacement = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![target]))
        .expect("choose a creature for Blight");
    let replacement = find_resolution_choice(&counter_replacement)
        .expect("two Doubling Seasons require a real replacement-order pause");
    assert_eq!(
        replacement.choice_kind,
        ChoiceKind::ReplacementEffect as i32
    );
    assert_eq!(replacement.candidate_object_ids.len(), 2);
    let completed = engine
        .apply_scenario_command(
            0,
            &submit_resolution_choice(vec![replacement.candidate_object_ids[0]]),
        )
        .expect("order the counter replacement effects and resume the program tail");
    assert!(find_resolution_choice(&completed).is_none());
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::MinusOneMinusOne),
        8,
        "both effect-counter doublers apply before the program continues"
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "the Draw after the resultless Blight runs exactly once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_153_chaos_spewer_requires_blight_after_declining_mana() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_104,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["chaos_spewer"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "chaos_spewer");
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let slot = hand_index_for_card(&engine, 0, "chaos_spewer");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap();
    pass_both_players(&mut engine);
    let choose_batch = pass_both_players_with_batch(&mut engine);
    let spewer = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "chaos_spewer" && object.zone == Zone::Battlefield)
        .expect("Chaos Spewer remains a Blight candidate")
        .id;
    let offered = find_resolution_choice(&choose_batch)
        .expect("Chaos Spewer trigger offers Pay or Otherwise");
    assert_eq!(offered.resolution_branches.len(), 1);
    let pay = &offered.resolution_branches[0];
    assert_eq!(pay.branch_index, 0);
    assert_eq!(pay.label, "Pay {2}");
    assert_eq!(pay.cost_text, "{2}");
    let presentation = pay
        .presentation
        .as_ref()
        .expect("pay branch carries its owner-bound presentation");
    assert_eq!(presentation.card_id, "chaos_spewer");
    assert_eq!(
        presentation.path.last().map(|part| part.id.as_str()),
        Some("pay_2")
    );
    assert_eq!(presentation.fallback_text, "Pay {2}");
    let first_mountain = inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    let second_mountain = inject_permanent_on_battlefield(&mut engine, 0, "mountain");

    let payment_batch = engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choosing Pay opens staged mana payment");
    let payment = payment_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) if choice.choice_kind == ChoiceKind::ManaPayment as i32 => Some(choice),
            _ => None,
        })
        .expect("choosing Pay must open a staged mana payment");
    assert_eq!(payment.generic_mana_cost, 2);
    assert!(payment.mana_cost.is_empty());
    assert!(!payment.payment_currently_legal);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let prompt_id = payment.resolution_prompt_id;
    let pending_presentation = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("staged Choose payment remains parked")
        .presentation
        .clone();
    apply_ability(&mut engine, 0, first_mountain, 0, vec![])
        .expect("float the first mana while the Choose payment is open");
    let newly_payable = apply_ability(&mut engine, 0, second_mountain, 0, vec![])
        .expect("float the second mana while the Choose payment is open");
    assert_eq!(engine.state.players[0].mana_pool.red, 2);
    let newly_payable_offer = find_resolution_choice(&newly_payable)
        .expect("the second mana republishes the same payment with current readiness");
    assert_eq!(newly_payable_offer.resolution_prompt_id, prompt_id);
    assert!(newly_payable_offer.payment_currently_legal);
    let rewound = engine
        .apply_scenario_command(0, &undo_mana_ability())
        .expect("Undo rewinds the last float without closing the payment offer");
    assert_eq!(engine.state.players[0].mana_pool.red, 1);
    assert!(engine.state.objects[&first_mountain].tapped);
    assert!(!engine.state.objects[&second_mountain].tapped);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Undo preserves the unresolved Choose payment");
    let refreshed_prompt_id = pending.resolution_prompt_id.get();
    assert_eq!(
        refreshed_prompt_id, prompt_id,
        "Undo changes readiness without replacing the logical payment occurrence"
    );
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::ManaPayment);
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(
        format!("{:?}", pending.presentation),
        format!("{:?}", pending_presentation)
    );
    let republished_payment = find_resolution_choice(&rewound)
        .expect("Undo republishes the same pending ManaPayment offer");
    assert_eq!(republished_payment.resolution_prompt_id, prompt_id);
    assert!(!republished_payment.payment_currently_legal);
    let declined =
        submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::Decline).unwrap();
    let blight =
        find_resolution_choice(&declined).expect("declining payment enters Blight directly");
    assert_eq!(blight.choice_kind, ChoiceKind::CostObjects as i32);
    assert_eq!((blight.min, blight.max), (1, 1));
    assert!(blight.candidate_object_ids.contains(&spewer));
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .is_err());
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![spewer]))
        .expect("Blight completes the parked Choose program");
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert!(!engine.state.objects[&first_mountain].tapped);
    assert!(!engine.state.objects[&second_mountain].tapped);

    assert_eq!(
        engine.state.objects[&spewer].counter_count(CounterKind::MinusOneMinusOne),
        2
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_153_chaos_spewer_paying_choose_cost_skips_blight() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_105,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["chaos_spewer"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "chaos_spewer");
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let slot = hand_index_for_card(&engine, 0, "chaos_spewer");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap();
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    let spewer = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "chaos_spewer" && object.zone == Zone::Battlefield)
        .expect("Chaos Spewer is the battlefield source")
        .id;
    engine.state.players[0].mana_pool.colorless = 2;

    engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choosing Pay opens the explicit mana-payment offer");
    let paid = submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay the exact generic {2} cost");
    assert!(find_resolution_choice(&paid).is_none());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        engine.state.objects[&spewer].counter_count(CounterKind::MinusOneMinusOne),
        0,
        "the paid empty branch does not execute Otherwise Blight"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_153_resolution_blight_parks_before_discard_and_defers_lethal_sba() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_102,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["dream_seizer", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "dream_seizer");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&bear)
        .unwrap()
        .add_counters(CounterKind::MinusOneMinusOne, 1, 0);
    engine.state.players[0].mana_pool.black = 1;
    engine.state.players[0].mana_pool.colorless = 3;
    let slot = hand_index_for_card(&engine, 0, "dream_seizer");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap();
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    engine.apply_scenario_command(0, &choose_branch(0)).unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::CostObjects,
        "Blight must be paid before the opponent chooses a discard"
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![bear]))
        .unwrap();
    assert_eq!(engine.state.objects[&bear].zone, Zone::Battlefield);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!(
        pending
            .continuation
            .stack()
            .unwrap()
            .item
            .blight_receipts
            .len(),
        1
    );
    let discard = pending.presentation.candidates[0];
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![discard]))
        .unwrap();
    assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

fn blight_selection(cost_index: u32, object_id: u32, zone_change_generation: u64) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id,
                zone_change_generation,
            }],
        })),
    }
}

#[test]
fn issue_153_rejected_blight_is_atomic_and_receipts_bind_the_incarnation() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_104,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["gristle_glutton"]),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .unwrap();
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    // Use a three-seat fixture directly in main phase so controller, not owner, governs eligibility.
    engine.state.turn_step = tricerules_core::TurnStep::Main1;
    let source = relocate_to_battlefield(&mut engine, 0, "gristle_glutton", false);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = false;
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", true);
    engine.state.objects.get_mut(&bear).unwrap().controller = 0;
    engine.state.objects.get_mut(&bear).unwrap().base_controller = 0;
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != bear);
    engine.state.players[0].battlefield.push(bear);
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);
    let before = engine.state.command_index;
    for selected in [
        blight_selection(1, bear, generation + 1),
        blight_selection(1, source, u64::MAX),
    ] {
        assert!(engine
            .apply_scenario_command(
                0,
                &activate_ability_with_costs(source, 0, vec![], vec![selected])
            )
            .is_err());
        assert_eq!(engine.state.command_index, before);
        assert!(!engine.state.objects[&source].tapped);
        assert_eq!(
            engine.state.objects[&bear].counter_count(CounterKind::MinusOneMinusOne),
            0
        );
        assert!(engine.state.stack.is_empty());
    }
    engine
        .apply_scenario_command(
            0,
            &activate_ability_with_costs(
                source,
                0,
                vec![],
                vec![blight_selection(1, bear, generation)],
            ),
        )
        .unwrap();
    let receipt = engine.state.stack.last().unwrap().blight_receipts[0];
    assert_eq!(receipt.player, 0);
    assert_eq!(receipt.count, 1);
    assert_eq!(receipt.creature.unwrap().object_id, bear);
    assert_eq!(receipt.creature.unwrap().zone_change_generation, generation);
    assert_eq!(engine.state.objects[&bear].owner, 1);
}

#[test]
fn issue_153_blackthorn_etb_pays_before_drawing_and_losing_life() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_105,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["blighted_blackthorn"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "blighted_blackthorn");
    engine.state.players[0].mana_pool.black = 1;
    engine.state.players[0].mana_pool.colorless = 4;
    let slot = hand_index_for_card(&engine, 0, "blighted_blackthorn");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap();
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    let before = engine.state.players[0].hand.len();
    engine.apply_scenario_command(0, &choose_branch(0)).unwrap();
    assert_eq!(engine.state.players[0].hand.len(), before);
    let oid = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![oid]))
        .unwrap();
    assert_eq!(
        engine.state.objects[&oid].counter_count(CounterKind::MinusOneMinusOne),
        2
    );
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
    assert_eq!(engine.state.players[0].life, 19);
}

fn cast_blight(
    object_id: u32,
    generation: u64,
) -> tricerules_proto::ruled::v1::CastCostGroupSelection {
    tricerules_proto::ruled::v1::CastCostGroupSelection {
        repetitions: None,
        group_index: 0,
        option_index: 0,
        selected_object: Some(
            tricerules_proto::ruled::v1::cast_cost_group_selection::SelectedObject::PermanentId(
                object_id,
            ),
        ),
        expected_zone_change_generation: generation,
        battlefield_objects: None,
    }
}

#[test]
fn issue_153_copied_cinder_inherits_payment_without_blighting_again() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_106,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["cinder_strike", "grizzly_bears"]),
            deck_with("island", &["twincast", "colossal_dreadmaw"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "cinder_strike");
    ensure_card_in_hand(&mut engine, 1, "twincast");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[1].mana_pool.blue = 2;
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);
    let slot = hand_index_for_card(&engine, 0, "cinder_strike");
    engine
        .apply_scenario_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![cast_blight(bear, generation)],
            ),
        )
        .unwrap();
    let original = engine.state.stack.last().unwrap().clone();
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass priority to the Twincast player");
    let slot = hand_index_for_card(&engine, 1, "twincast");
    engine
        .apply_scenario_command(1, &cast_spell(slot, target_object(original.id)))
        .unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![target]))
        .unwrap();
    let copy = engine.state.stack.iter().find(|item| item.is_copy).unwrap();
    assert_eq!(copy.blight_receipts, original.blight_receipts);
    assert_eq!(copy.cast_cost_receipts, original.cast_cost_receipts);
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::MinusOneMinusOne),
        1
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&target].damage, 4);
}

#[test]
fn issue_153_wild_unraveling_requires_exactly_one_additional_payment() {
    for blight in [true, false] {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            153_107,
            &[0, 1],
            20,
            Some(vec![
                deck_with("mountain", &["lightning_bolt"]),
                deck_with("island", &["wild_unraveling", "grizzly_bears"]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        ensure_card_in_hand(&mut engine, 0, "lightning_bolt");
        ensure_card_in_hand(&mut engine, 1, "wild_unraveling");
        let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
        engine.state.players[0].mana_pool.red = 1;
        engine.state.players[1].mana_pool.blue = 2;
        engine.state.players[1].mana_pool.colorless = 1;
        let slot = hand_index_for_card(&engine, 0, "lightning_bolt");
        engine
            .apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .unwrap();
        let target = engine.state.stack.last().unwrap().id;
        engine
            .apply_scenario_command(0, &pass())
            .expect("pass priority to the Wild Unraveling player");
        let slot = hand_index_for_card(&engine, 1, "wild_unraveling");
        assert!(engine
            .apply_scenario_command(1, &cast_spell(slot, target_object(target)))
            .is_err());
        assert_eq!(engine.state.players[1].mana_pool.blue, 2);
        let generation = engine
            .state
            .zone_change_generation
            .get(&bear)
            .copied()
            .unwrap_or(0);
        let selection = if blight {
            cast_blight(bear, generation)
        } else {
            tricerules_proto::ruled::v1::CastCostGroupSelection {
                group_index: 0,
                option_index: 1,
                ..Default::default()
            }
        };
        engine
            .apply_scenario_command(
                1,
                &cast_spell_with_cast_cost_groups(slot, target_object(target), vec![selection]),
            )
            .unwrap();
        assert_eq!(
            engine.state.players[1].mana_pool.colorless,
            u32::from(blight)
        );
        assert_eq!(
            engine.state.objects[&bear].zone,
            if blight {
                Zone::Graveyard
            } else {
                Zone::Battlefield
            }
        );
        pass_both_players(&mut engine);
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.players[1].life, 20);
    }
}

#[test]
fn issue_153_cinder_strike_uses_paid_receipt_for_one_damage_instruction() {
    for paid in [false, true] {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            153_101,
            &[0, 1],
            20,
            Some(vec![
                deck_with("mountain", &["cinder_strike", "grizzly_bears"]),
                deck_with("forest", &["colossal_dreadmaw"]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        ensure_card_in_hand(&mut engine, 0, "cinder_strike");
        let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
        engine.state.players[0].mana_pool.red = 1;
        let generation = engine
            .state
            .zone_change_generation
            .get(&bear)
            .copied()
            .unwrap_or(0);
        let selections = if paid {
            vec![cast_blight(bear, generation)]
        } else {
            vec![]
        };
        let slot = hand_index_for_card(&engine, 0, "cinder_strike");
        engine
            .apply_scenario_command(
                0,
                &cast_spell_with_cast_cost_groups(slot, target_object(target), selections),
            )
            .unwrap();
        assert_eq!(
            engine.state.stack.last().unwrap().blight_receipts.len(),
            usize::from(paid)
        );
        pass_both_players(&mut engine);
        assert_eq!(
            engine.state.objects[&target].damage,
            if paid { 4 } else { 2 }
        );
    }
}

#[test]
fn issue_153_activation_publishes_one_creature_and_blights_without_targeting() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_100,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["gristle_glutton", "tatterkite"]),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "gristle_glutton", false);
    let prohibited = relocate_to_battlefield(&mut engine, 0, "tatterkite", false);
    let opponent = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = false;
    let generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let legal = engine.initial_response_batch();
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&(u64::from(source) << 32)];
    assert_eq!(
        choices.choices.len(),
        1,
        "Blight must publish one nontargeted cost selection"
    );
    assert_eq!(choices.choices[0].candidate_ids, [source]);
    assert!(!choices.choices[0].candidate_ids.contains(&prohibited));
    assert!(!choices.choices[0].candidate_ids.contains(&opponent));
    engine
        .apply_scenario_command(
            0,
            &activate_ability_with_costs(
                source,
                0,
                vec![],
                vec![blight_selection(1, source, generation)],
            ),
        )
        .unwrap();
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::MinusOneMinusOne),
        1
    );
    assert!(engine.state.stack.last().unwrap().targets.is_empty());
}
