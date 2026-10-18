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
fn issue_153_blight_owners_compile_their_exact_trigger_programs() {
    use tricerules_card_model::primitives::{
        CompiledInstructionKind, PlayerRecipient, ResolutionCost, SpellEffectKind,
    };

    let registry = tricerules_cards::registry::global();
    for (card_id, ability_id, count) in [
        ("blighted_blackthorn", "triggered_01", 2),
        ("blighted_blackthorn", "triggered_02", 2),
        ("dream_seizer", "triggered_01", 1),
    ] {
        let owner = tricerules_card_model::ProgramOwnerId::ability(
            tricerules_card_model::AbilityDefinitionId {
                card_id: card_id.into(),
                face_id: tricerules_card_model::CardFaceId::new(card_id).unwrap(),
                ability_path: vec![tricerules_card_model::AbilityId::new(ability_id).unwrap()],
            },
        );
        let program = registry
            .compiled_program_for_owner(&owner)
            .unwrap_or_else(|| panic!("{card_id}/{ability_id} must own its typed trigger program"));
        let [choose] = program.body.steps.as_slice() else {
            panic!("{card_id}/{ability_id} has one root Choose instruction")
        };
        let CompiledInstructionKind::Choose {
            chooser,
            optional,
            branches,
            otherwise,
            ..
        } = &choose.kind
        else {
            panic!("{card_id}/{ability_id} retains the optional Choose presentation")
        };
        assert_eq!(*chooser, PlayerRecipient::Controller);
        assert!(*optional);
        assert!(otherwise.steps.is_empty());
        let [branch] = branches.as_slice() else {
            panic!("{card_id}/{ability_id} has exactly one Blight branch")
        };
        assert_eq!(branch.cost, ResolutionCost::Blight { count });
        match card_id {
            "blighted_blackthorn" => {
                let [draw, lose_life] = branch.body.steps.as_slice() else {
                    panic!("Blackthorn draws once, then its controller loses life")
                };
                assert!(matches!(
                    &draw.kind,
                    CompiledInstructionKind::Perform(effect)
                        if matches!(effect.as_ref(), SpellEffectKind::Draw {
                            who: PlayerRecipient::Controller,
                            count: tricerules_card_model::primitives::Amount::Fixed(1),
                        })
                ));
                assert!(matches!(
                    &lose_life.kind,
                    CompiledInstructionKind::Perform(effect)
                        if matches!(effect.as_ref(), SpellEffectKind::LoseLife {
                            who: PlayerRecipient::Controller,
                            amount: tricerules_card_model::primitives::LifeAmount::Fixed(1),
                        })
                ));
            }
            "dream_seizer" => {
                let [discard] = branch.body.steps.as_slice() else {
                    panic!("Dream Seizer discards once per opponent")
                };
                assert!(matches!(
                    &discard.kind,
                    CompiledInstructionKind::Perform(effect)
                        if matches!(effect.as_ref(), SpellEffectKind::Discard {
                            who: PlayerRecipient::EachOpponent,
                            quantity: tricerules_card_model::primitives::DiscardQuantity::Exact(1),
                        })
                ));
            }
            _ => unreachable!(),
        }
    }
}

fn issue_153_blackthorn_etb_trigger_on_stack(seed: u64, player_ids: &[i32]) -> (GameEngine, u32) {
    let mut decks = vec![
        deck_with("swamp", &["blighted_blackthorn"]),
        deck_with("forest", &["grizzly_bears"]),
    ];
    if player_ids.len() == 3 {
        decks.push(deck_with("island", &["hill_giant"]));
    }
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        player_ids,
        20,
        Some(decks),
        true,
    )
    .expect("Blackthorn fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "blighted_blackthorn");
    engine.state.players[0].mana_pool.black = 1;
    engine.state.players[0].mana_pool.colorless = 4;
    let slot = hand_index_for_card(&engine, 0, "blighted_blackthorn");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Blighted Blackthorn");
    pass_priority_round(&mut engine);
    let blackthorn = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "blighted_blackthorn" && object.zone == Zone::Battlefield)
        .expect("Blackthorn enters and puts its ETB trigger on the stack")
        .id;
    assert!(engine.state.stack.last().is_some_and(|item| {
        item.source_permanent_id == Some(blackthorn)
            && item.ability_text.is_some()
            && item.controller == 0
    }));
    (engine, blackthorn)
}

fn issue_153_three_player_blackthorn_choose_for_p1(seed: u64) -> (GameEngine, u32, u32) {
    let (mut engine, blackthorn) = issue_153_blackthorn_etb_trigger_on_stack(seed, &[0, 1, 2]);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 12, 12);
    engine
        .state
        .stack
        .last_mut()
        .expect("Blackthorn trigger remains on the stack")
        .controller = 1;
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Blackthorn's optional Choose is offered to the captured trigger controller");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    assert!(engine.state.objects[&blackthorn].zone == Zone::Battlefield);
    (engine, blackthorn, target)
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
fn program_choose_blight_completion_resumes_branch_and_parent_tails_once() {
    const PROGRAM: &str = r#"(
        id: "program_choose_blight_tail_fixture",
        name: "Program Choose Blight Tail Fixture",
        face_id: "program_choose_blight_tail_fixture",
        mana_cost: "{0}",
        types: ["Creature"],
        power: 2,
        toughness: 2,
        activated_abilities: [(
            ability_id: "activated_01",
            presentation: Fallback,
            source_zone: Battlefield,
            costs: [],
            effect: [],
            program: Some((body: (steps: [
                (id: "paid_blight", kind: Choose(
                    choice: "paid_blight", chooser: Controller, optional: true,
                    branches: [(
                        branch_id: "blight_2", presentation: Fallback,
                        cost: Blight(count: 2), requirement: Always,
                        body: (steps: [
                            (id: "branch_draw", bind: ["branch_draw"], kind: Perform(Draw(
                                who: Controller, count: 1,
                            )))
                        ], yields: []),
                    )],
                    otherwise: (steps: [], yields: []),
                )),
                (id: "tail_draw", bind: ["tail_draw"], kind: Perform(Draw(
                    who: Controller, count: 1,
                ))),
            ], yields: []), target_bindings: [])),
            targeting: None,
            timing: Normal,
            conditions: [],
            activation_limit: None,
        )],
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                PROGRAM,
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/laezel,_vlaakiths_champion.ron"),
                include_str!("../../../tricerules-cards/data/doubling_season.ron"),
            ],
            &[],
        )
        .expect("the paid Program Choose Blight fixture compiles for an ability owner"),
    ));
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(registry, 153_121, &[0, 1], 20, decks, true)
        .expect("Program Choose Blight fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let source =
        inject_permanent_on_battlefield(&mut engine, 0, "program_choose_blight_tail_fixture");
    inject_permanent_on_battlefield(&mut engine, 0, "laezel,_vlaakiths_champion");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let target = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 20, 20);
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    apply_ability(&mut engine, 0, source, 0, vec![])
        .expect("activate the ability's Program Choose");
    let offered = find_resolution_choice(&pass_both_players_with_batch(&mut engine))
        .expect("the ability offers its paid Blight branch");
    assert_eq!(offered.choice_kind, ChoiceKind::ResolutionBranch as i32);
    engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choose the Blight payment");
    let blight_cost = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        blight_cost.presentation.choice_kind,
        ChoiceKind::CostObjects
    );
    assert!(blight_cost.presentation.candidates.contains(&target));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![target]))
        .expect("pay the Program Choose cost");
    let replacement = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        replacement.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(replacement.presentation.candidates.len(), 2);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::MinusOneMinusOne),
        0,
        "the Program remains parked until cost replacements finish"
    );
    let completed = engine
        .apply_scenario_command(
            0,
            &submit_resolution_choice(vec![replacement.presentation.candidates[0]]),
        )
        .expect("order the cost replacements and resume both Program continuations");
    assert!(find_resolution_choice(&completed).is_none());
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::MinusOneMinusOne),
        12
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 2,
        "the paid branch Draw and following parent Draw each run exactly once"
    );
    assert_eq!(
        completed
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text.contains("P0 blights 2 using")
            ))
            .count(),
        1
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

#[test]
fn issue_153_blackthorn_attack_trigger_pays_blight_and_runs_its_body_once() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_112,
        &[0, 1],
        20,
        Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_declare_attackers(&mut engine);
    let target = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 20, 20);
    let blackthorn = inject_creature_on_battlefield(&mut engine, 0, "blighted_blackthorn");
    engine
        .apply_scenario_command(0, &declare_attackers(vec![blackthorn]))
        .expect("attack with Blackthorn");
    pass_both_players(&mut engine);
    let menu = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Blackthorn's attack trigger offers its optional branch");
    assert_eq!(menu.deciding_player, 0);
    assert_eq!(menu.presentation.choice_kind, ChoiceKind::ResolutionBranch);
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("select Blight from the actual attack trigger");
    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the attack trigger parks on the Blight cost object");
    assert_eq!(payment.presentation.choice_kind, ChoiceKind::CostObjects);
    assert!(payment.presentation.candidates.contains(&blackthorn));
    assert!(payment.presentation.candidates.contains(&target));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![target]))
        .expect("pay the attack trigger's Blight cost");
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::MinusOneMinusOne),
        2
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1
    );
    assert_eq!(engine.state.players[0].life, 19);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_153_blackthorn_decline_and_unpayable_no_creature_skip_the_paid_body() {
    let (mut declined, blackthorn) = issue_153_blackthorn_etb_trigger_on_stack(153_113, &[0, 1]);
    pass_priority_round(&mut declined);
    assert_eq!(
        declined
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    let draws_before = declined.state.turn_history.current.player(0).cards_drawn;
    declined
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Blackthorn's optional Blight branch");
    assert_eq!(declined.state.players[0].life, 20);
    assert_eq!(
        declined.state.turn_history.current.player(0).cards_drawn,
        draws_before
    );
    assert_eq!(
        declined.state.objects[&blackthorn].counter_count(CounterKind::MinusOneMinusOne),
        0
    );
    assert!(declined.state.pending_resolution.is_none());
    assert!(declined.state.stack.is_empty());

    let (mut no_creature, blackthorn) = issue_153_blackthorn_etb_trigger_on_stack(153_114, &[0, 1]);
    no_creature.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != blackthorn);
    no_creature.state.players[0].graveyard.push(blackthorn);
    no_creature.state.objects.get_mut(&blackthorn).unwrap().zone = Zone::Graveyard;
    *no_creature
        .state
        .zone_change_generation
        .entry(blackthorn)
        .or_default() += 1;
    let no_creature_draws_before = no_creature.state.turn_history.current.player(0).cards_drawn;
    pass_priority_round(&mut no_creature);
    assert!(no_creature.state.pending_resolution.is_none());
    assert!(no_creature.state.stack.is_empty());
    assert_eq!(no_creature.state.players[0].life, 20);
    assert_eq!(
        no_creature.state.turn_history.current.player(0).cards_drawn,
        no_creature_draws_before
    );
}

#[test]
fn issue_153_program_blight_choice_rejects_wrong_stale_and_replayed_selection_atomically() {
    let (mut engine, blackthorn) = issue_153_blackthorn_etb_trigger_on_stack(153_115, &[0, 1]);
    let second_creature = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 8, 8);
    let noncreature = inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    let blackthorn_generation = engine
        .state
        .zone_change_generation
        .get(&blackthorn)
        .copied()
        .unwrap_or(0);
    pass_priority_round(&mut engine);
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("select the paid Blackthorn branch");
    let prompt = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(prompt.presentation.choice_kind, ChoiceKind::CostObjects);
    assert!(prompt.presentation.candidates.contains(&blackthorn));
    assert!(prompt.presentation.candidates.contains(&second_creature));
    assert!(!prompt.presentation.candidates.contains(&noncreature));

    let before_wrong = engine.state.command_index;
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![noncreature]))
        .is_err());
    assert_eq!(engine.state.command_index, before_wrong);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::CostObjects
    );
    assert_eq!(
        engine.state.objects[&blackthorn].counter_count(CounterKind::MinusOneMinusOne),
        0
    );
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before
    );

    *engine
        .state
        .zone_change_generation
        .entry(second_creature)
        .or_default() += 1;
    let before_stale = engine.state.command_index;
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![second_creature]))
        .is_err());
    assert_eq!(engine.state.command_index, before_stale);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::CostObjects
    );
    assert_eq!(
        engine.state.objects[&blackthorn].counter_count(CounterKind::MinusOneMinusOne),
        0
    );

    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![blackthorn]))
        .expect("a current candidate still completes the choice after rejected submissions");
    assert_eq!(
        engine.state.zone_change_generation[&blackthorn],
        blackthorn_generation
    );
    assert_eq!(
        engine.state.objects[&blackthorn].counter_count(CounterKind::MinusOneMinusOne),
        2
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1
    );
    assert_eq!(engine.state.players[0].life, 19);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    let after_success = engine.state.command_index;
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![blackthorn]))
        .is_err());
    assert_eq!(engine.state.command_index, after_success);
    assert_eq!(
        engine.state.objects[&blackthorn].counter_count(CounterKind::MinusOneMinusOne),
        2
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1
    );
    assert_eq!(engine.state.players[0].life, 19);
}

#[test]
fn issue_153_blackthorn_payer_departure_clears_object_and_replacement_waits_without_delegation() {
    let (mut object_wait, blackthorn, target) =
        issue_153_three_player_blackthorn_choose_for_p1(153_116);
    object_wait
        .apply_scenario_command(1, &choose_branch(0))
        .expect("P1 selects Blackthorn's paid branch");
    let object_choice = object_wait.state.pending_resolution.as_ref().unwrap();
    assert_eq!(object_choice.deciding_player, 1);
    assert_eq!(
        object_choice.presentation.choice_kind,
        ChoiceKind::CostObjects
    );
    assert!(object_choice.presentation.candidates.contains(&target));
    let p0_draws = object_wait.state.turn_history.current.player(0).cards_drawn;
    let p0_life = object_wait.state.players[0].life;
    let departure = object_wait
        .apply_scenario_command(1, &concede())
        .expect("the original payer can leave during the object choice");
    assert!(object_wait.state.players[1].has_lost);
    assert!(find_resolution_choice(&departure).is_none());
    assert!(object_wait.state.pending_resolution.is_none());
    assert!(object_wait.state.stack.is_empty());
    assert!(object_wait
        .state
        .objects
        .get(&target)
        .is_none_or(|object| object.counter_count(CounterKind::MinusOneMinusOne) == 0));
    assert!(!object_wait.state.players[1].battlefield.contains(&target));
    assert_eq!(
        object_wait.state.objects[&blackthorn].zone,
        Zone::Battlefield
    );
    assert_eq!(object_wait.state.players[0].life, p0_life);
    assert_eq!(
        object_wait.state.turn_history.current.player(0).cards_drawn,
        p0_draws
    );

    let (mut replacement_wait, blackthorn, target) =
        issue_153_three_player_blackthorn_choose_for_p1(153_117);
    inject_permanent_on_battlefield(&mut replacement_wait, 1, "laezel,_vlaakiths_champion");
    inject_permanent_on_battlefield(&mut replacement_wait, 1, "doubling_season");
    inject_permanent_on_battlefield(&mut replacement_wait, 1, "doubling_season");
    replacement_wait
        .apply_scenario_command(1, &choose_branch(0))
        .expect("P1 selects Blackthorn's paid branch before replacement");
    replacement_wait
        .apply_scenario_command(1, &submit_resolution_choice(vec![target]))
        .expect("P1 selects its own creature for the cost");
    let replacement = replacement_wait.state.pending_resolution.as_ref().unwrap();
    assert_eq!(replacement.deciding_player, 1);
    assert_eq!(
        replacement.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(replacement.presentation.candidates.len(), 2);
    assert_eq!(
        replacement_wait.state.objects[&target].counter_count(CounterKind::MinusOneMinusOne),
        0,
        "the selected cost has not been committed while replacements are parked"
    );
    let p0_draws = replacement_wait
        .state
        .turn_history
        .current
        .player(0)
        .cards_drawn;
    let p0_life = replacement_wait.state.players[0].life;
    let departure = replacement_wait
        .apply_scenario_command(1, &concede())
        .expect("the payer leaves while its cost is parked for replacement order");
    assert!(replacement_wait.state.players[1].has_lost);
    assert!(find_resolution_choice(&departure).is_none());
    assert!(replacement_wait.state.pending_resolution.is_none());
    assert!(replacement_wait.state.stack.is_empty());
    assert!(replacement_wait
        .state
        .objects
        .get(&target)
        .is_none_or(|object| object.counter_count(CounterKind::MinusOneMinusOne) == 0));
    assert!(!replacement_wait.state.players[1]
        .battlefield
        .contains(&target));
    assert_eq!(
        replacement_wait.state.objects[&blackthorn].zone,
        Zone::Battlefield
    );
    assert_eq!(replacement_wait.state.players[0].life, p0_life);
    assert_eq!(
        replacement_wait
            .state
            .turn_history
            .current
            .player(0)
            .cards_drawn,
        p0_draws
    );
    pass_priority_round(&mut replacement_wait);
}

#[test]
fn issue_153_blackthorn_terminal_payer_departure_clears_both_blight_waits() {
    for (replacement_wait, seed) in [(false, 153_121), (true, 153_122)] {
        let (mut engine, _blackthorn) = issue_153_blackthorn_etb_trigger_on_stack(seed, &[0, 1]);
        let target = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 20, 20);
        if replacement_wait {
            inject_permanent_on_battlefield(&mut engine, 0, "laezel,_vlaakiths_champion");
            inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
            inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
        }

        pass_priority_round(&mut engine);
        engine
            .apply_scenario_command(0, &choose_branch(0))
            .expect("P0 selects Blackthorn's paid branch");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .expect("the paid branch parks")
                .presentation
                .choice_kind,
            ChoiceKind::CostObjects
        );

        if replacement_wait {
            engine
                .apply_scenario_command(0, &submit_resolution_choice(vec![target]))
                .expect("P0 submits the cost before replacement order is chosen");
            let replacement = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("Lae'zel and the two Seasons park the cost")
                .presentation
                .choice_kind;
            assert_eq!(replacement, ChoiceKind::ReplacementEffect);
        }

        let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
        let life_before = engine.state.players[0].life;
        let terminal_batch = engine
            .apply_scenario_command(0, &concede())
            .expect("P0 concedes while the Blight payment is pending");

        assert!(engine.state.is_terminal());
        assert_eq!(engine.state.winner(), Some(1));
        assert!(engine.state.players[0].has_lost);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert!(
            engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null()
        );
        assert!(find_resolution_choice(&terminal_batch).is_none());
        assert!(terminal_batch.events.iter().all(|event| !matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )));
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before,
            "the abandoned branch never draws"
        );
        assert_eq!(engine.state.players[0].life, life_before);
        assert!(engine
            .state
            .objects
            .get(&target)
            .is_none_or(|object| { object.counter_count(CounterKind::MinusOneMinusOne) == 0 }));
    }
}

#[test]
fn issue_153_blackthorn_program_choose_doubling_season_does_not_modify_its_blight_cost() {
    let (mut engine, _blackthorn) = issue_153_blackthorn_etb_trigger_on_stack(153_118, &[0, 1]);
    let target = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 20, 20);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    pass_priority_round(&mut engine);
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("select Blackthorn's optional Blight branch");
    let completed = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![target]))
        .expect("choose the actual target creature");
    assert!(find_resolution_choice(&completed).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::MinusOneMinusOne),
        2,
        "Doubling Season does not modify counters paid as a cost"
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "the Choose branch Draw runs exactly once"
    );
    assert_eq!(engine.state.players[0].life, 19);
    assert_eq!(
        completed
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text.contains("P0 blights 2 using")
            ))
            .count(),
        1,
        "one completed cost produces one Blight receipt log"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_153_blackthorn_program_choose_orders_laezel_and_doubling_season_then_receipts_once() {
    let (mut engine, _blackthorn) = issue_153_blackthorn_etb_trigger_on_stack(153_120, &[0, 1]);
    let target = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 20, 20);
    inject_permanent_on_battlefield(&mut engine, 0, "laezel,_vlaakiths_champion");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    pass_priority_round(&mut engine);
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("select Blackthorn's optional Blight branch");
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![target]))
        .expect("choose the actual target creature");
    let replacement = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        replacement.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(replacement.presentation.candidates.len(), 2);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::MinusOneMinusOne),
        0,
        "the selected cost remains staged while the Season order is selected"
    );
    let ordered = engine
        .apply_scenario_command(
            0,
            &submit_resolution_choice(vec![replacement.presentation.candidates[1]]),
        )
        .expect("choose a Doubling Season after Lae'zel has auto-applied");
    assert!(find_resolution_choice(&ordered).is_none());
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::MinusOneMinusOne),
        12,
        "Lae'zel adds one counter, then both Seasons apply"
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "the Choose branch Draw runs exactly once"
    );
    assert_eq!(engine.state.players[0].life, 19);
    assert_eq!(
        ordered
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text.contains("P0 blights 2 using")
            ))
            .count(),
        1,
        "one completed cost produces one Blight receipt log"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_153_dream_seizer_three_player_discard_collects_private_apnap_choices() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        153_119,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("swamp", &["dream_seizer"]),
            deck_with("forest", &["grizzly_bears"]),
            deck_with("island", &["hill_giant"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "dream_seizer");
    ensure_card_in_hand(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 2, "hill_giant");
    engine.state.players[0].mana_pool.black = 1;
    engine.state.players[0].mana_pool.colorless = 3;
    let source_slot = hand_index_for_card(&engine, 0, "dream_seizer");
    engine
        .apply_scenario_command(0, &cast_spell(source_slot, vec![]))
        .expect("cast Dream Seizer");
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("select Dream Seizer's Blight branch");
    let dream_seizer = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "dream_seizer" && object.zone == Zone::Battlefield)
        .expect("Dream Seizer remains a legal Blight creature")
        .id;
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![dream_seizer]))
        .expect("pay Blight before any opponent chooses a discard");

    let first = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(first.deciding_player, 1, "the first opponent acts APNAP");
    assert_eq!(first.presentation.choice_kind, ChoiceKind::HandCards);
    let p1_card = engine.state.players[1]
        .hand
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "grizzly_bears")
        .expect("P1 has a known discardable card");
    assert!(first.presentation.candidates.contains(&p1_card));
    let p1_selection = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![p1_card]))
        .expect("P1 makes its private discard selection");
    let second = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(second.deciding_player, 2, "the next opponent follows APNAP");
    assert_eq!(second.presentation.choice_kind, ChoiceKind::HandCards);
    assert!(engine.state.players[1].hand.contains(&p1_card));
    assert_eq!(engine.state.objects[&p1_card].zone, Zone::Hand);
    assert!(!second.presentation.candidates.contains(&p1_card));
    assert!(second
        .presentation
        .candidates
        .iter()
        .all(|object_id| engine.state.players[2].hand.contains(object_id)));
    assert!(find_resolution_choice(&p1_selection).is_some_and(|choice| {
        choice.deciding_player_id == 2 && !choice.candidate_object_ids.contains(&p1_card)
    }));
    let p2_card = engine.state.players[2]
        .hand
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "hill_giant")
        .expect("P2 has a known discardable card");
    assert!(second.presentation.candidates.contains(&p2_card));
    engine
        .apply_scenario_command(2, &submit_resolution_choice(vec![p2_card]))
        .expect("P2 finishes the frozen player-set selection");
    assert_eq!(engine.state.objects[&p1_card].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&p2_card].zone, Zone::Graveyard);
    assert!(engine.state.players[1].graveyard.contains(&p1_card));
    assert!(engine.state.players[2].graveyard.contains(&p2_card));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
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
