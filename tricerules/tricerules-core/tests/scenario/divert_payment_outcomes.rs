use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_core::Zone;

const COPY_ANY_SPELL: &str = r#"(
    id: "copy_any_spell_fixture",
    name: "Copy Any Spell Fixture",
    face_id: "copy_any_spell_fixture",
    mana_cost: "{0}",
    types: ["Instant"],
    spell_effect: [CopyTargetSpell(count: 1, spell_filter: (card_type: None))],
)"#;

const MIXED_TARGET_COUNTER: &str = r#"(
    id: "mixed_target_counter_fixture",
    name: "Mixed Target Counter Fixture",
    face_id: "mixed_target_counter_fixture",
    mana_cost: "{0}",
    types: ["Instant"],
    spell_effect: [],
    spell_program: Some((body: (steps: [
        (id: "counter", bind: ["payment"], kind: Perform(CounterTargetSpell(
            spell_filter: (), unless_controller_pays: Some(2)
        ))),
        (id: "create_lander_if_paid", kind: IfResult(
            condition: CounterPayment(result: "payment", outcome: Paid),
            yes: (steps: [(id: "lander", kind: Perform(CreateTokens(
                token: "lander", count: 1, who: Controller, tapped: false,
                sacrifice_timing: None
            )))], yields: []),
            no: (steps: [], yields: [])
        )),
        (id: "draw_if_not_offered", kind: IfResult(
            condition: CounterPayment(result: "payment", outcome: NotOffered),
            yes: (steps: [(id: "draw", kind: Perform(Draw(
                who: Controller, count: 1
            )))], yields: []),
            no: (steps: [], yields: [])
        )),
        (id: "damage_creature", kind: Perform(DamageTarget(
            amount: 1, target: (kind: Creature)
        )))
    ], yields: []), target_bindings: [
        (instruction_id: "counter", role: Spell, target_group_index: 0),
        (instruction_id: "damage_creature", role: Target, target_group_index: 1),
    ])),
    targeting: Some((groups: [
        (min: 1, max: 1, prompt: "Choose target spell", effect_indices: []),
        (min: 1, max: 1, prompt: "Choose target creature", effect_indices: []),
    ])),
)"#;

const COUNTER_THEN_SOFT_COUNTER: &str = r#"(
    id: "counter_then_soft_counter_fixture",
    name: "Counter Then Soft Counter Fixture",
    face_id: "counter_then_soft_counter_fixture",
    mana_cost: "{0}",
    types: ["Instant"],
    spell_effect: [],
    spell_program: Some((body: (steps: [
        (id: "counter_once", kind: Perform(CounterTargetSpell(
            spell_filter: (), unless_controller_pays: None
        ))),
        (id: "counter_again", bind: ["payment"], kind: Perform(CounterTargetSpell(
            spell_filter: (), unless_controller_pays: Some(2)
        ))),
        (id: "draw_if_not_offered", kind: IfResult(
            condition: CounterPayment(result: "payment", outcome: NotOffered),
            yes: (steps: [(id: "not_offered_draw", kind: Perform(Draw(
                who: Controller, count: 1
            )))], yields: []),
            no: (steps: [], yields: [])
        )),
        (id: "draw_after_counter_sequence", kind: Perform(Draw(
            who: Controller, count: 1
        )))
    ], yields: []), target_bindings: [
        (instruction_id: "counter_once", role: Spell, target_group_index: 0),
        (instruction_id: "counter_again", role: Spell, target_group_index: 0),
    ])),
    targeting: Some((groups: [
        (min: 1, max: 1, prompt: "Choose target spell", effect_indices: []),
    ])),
)"#;

const DEPARTURE_FORCED_UNPAID_COUNTER: &str = r#"(
    id: "departure_forced_unpaid_counter_fixture",
    name: "Departure Forced Unpaid Counter Fixture",
    face_id: "departure_forced_unpaid_counter_fixture",
    mana_cost: "{0}",
    types: ["Instant"],
    spell_effect: [],
    spell_program: Some((body: (steps: [
        (id: "counter", bind: ["payment"], kind: Perform(CounterTargetSpell(
            spell_filter: (), unless_controller_pays: Some(2)
        ))),
        (id: "draw_on_forced_unpaid", kind: IfResult(
            condition: CounterPayment(result: "payment", outcome: ForcedUnpaid),
            yes: (steps: [(id: "forced_unpaid_draw", kind: Perform(Draw(
                who: Controller, count: 1
            )))], yields: []),
            no: (steps: [], yields: [])
        )),
        (id: "draw_after_result", kind: Perform(Draw(
            who: Controller, count: 1
        )))
    ], yields: []), target_bindings: [
        (instruction_id: "counter", role: Spell, target_group_index: 0),
    ])),
    targeting: Some((groups: [(
        min: 1, max: 1, prompt: "Choose target spell", effect_indices: [],
    )])),
)"#;

fn registry() -> &'static tricerules_card_model::CardRegistry {
    Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/divert_disaster.ron"),
                include_str!("../../../tricerules-cards/data/eject.ron"),
                include_str!("../../../tricerules-cards/data/counterspell.ron"),
                include_str!("../../../tricerules-cards/data/convolute.ron"),
                include_str!("../../../tricerules-cards/data/lightning_bolt.ron"),
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!(
                    "../../../tricerules-cards/data/generated/c/confusticate_and_bebother.ron"
                ),
                COPY_ANY_SPELL,
                MIXED_TARGET_COUNTER,
                COUNTER_THEN_SOFT_COUNTER,
                DEPARTURE_FORCED_UNPAID_COUNTER,
            ],
            &[include_str!(
                "../../../tricerules-cards/data/tokens/lander.ron"
            )],
        )
        .expect("counter-payment scenario registry validates"),
    ))
}

fn stack_target(group_index: u32, object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        kind: TargetRefKind::Stack as i32,
        group_index,
        ..Default::default()
    }
}

fn setup_uncounterable_divert(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with(
            "island",
            &["eject", "copy_any_spell_fixture", "counterspell"],
        ),
        deck_with("forest", &["divert_disaster"]),
    ]);
    let mut engine = GameEngine::new(registry(), seed, &[0, 1], 20, decks, true)
        .expect("uncounterable Divert scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    let permanent = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "eject");
    ensure_in_hand(&mut engine, 1, "divert_disaster");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 5,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "eject"),
                target_object(permanent),
            ),
        )
        .expect("cast the uncounterable target spell");
    let eject = engine.state.stack.last().expect("Eject on the stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent priority to cast Divert");
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "divert_disaster"),
                vec![stack_target(0, eject)],
            ),
        )
        .expect("cast Divert targeting Eject");
    (engine, eject)
}

#[test]
fn divert_paid_against_uncounterable_eject_creates_one_lander_and_keeps_eject() {
    let (mut engine, eject) = setup_uncounterable_divert(75_131);
    pass_priority_round(&mut engine);

    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Divert offers the target controller payment");
    assert_eq!(payment.deciding_player, 0);
    assert_eq!(payment.presentation.choice_kind, ChoiceKind::ManaPayment);
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("accept and commit the payment");

    assert!(engine.state.stack.iter().any(|item| item.id == eject));
    assert_eq!(engine.state.objects[&eject].zone, Zone::Stack);
    assert_eq!(battlefield_token_oids(&engine, 1, "lander").len(), 1);
    assert!(battlefield_token_oids(&engine, 0, "lander").is_empty());
}

#[test]
fn divert_declined_against_uncounterable_eject_creates_no_lander_and_keeps_eject() {
    let (mut engine, eject) = setup_uncounterable_divert(75_132);
    pass_priority_round(&mut engine);

    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Divert offers the target controller payment");
    assert_eq!(payment.deciding_player, 0);
    assert_eq!(payment.presentation.choice_kind, ChoiceKind::ManaPayment);
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::Decline)
        .expect("accept the decline");

    assert!(engine.state.stack.iter().any(|item| item.id == eject));
    assert_eq!(engine.state.objects[&eject].zone, Zone::Stack);
    assert!(battlefield_token_oids(&engine, 0, "lander").is_empty());
    assert!(battlefield_token_oids(&engine, 1, "lander").is_empty());
}

#[test]
fn confusticate_unbound_modal_counter_uses_its_chosen_mode_target() {
    let decks = Some(vec![
        deck_with("island", &["opt"]),
        deck_with("forest", &["confusticate_and_bebother"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_136, &[0, 1], 20, decks, true)
        .expect("unbound modal counter scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "opt");
    ensure_in_hand(&mut engine, 1, "confusticate_and_bebother");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "opt"), Vec::new()),
        )
        .expect("cast the spell targeted by Confusticate");
    let opt = engine.state.stack.last().expect("Opt on the stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass to the player with Confusticate");
    engine
        .apply_scenario_command(
            1,
            &cast_modal_spell(
                hand_index_for_card(&engine, 1, "confusticate_and_bebother"),
                vec![(0, vec![stack_target(0, opt)])],
            ),
        )
        .expect("choose Confusticate's unbound modal counter program");

    pass_priority_round(&mut engine);
    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the mode-local stack target receives the fixed-cost offer");
    assert_eq!(payment.deciding_player, 0);
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::Decline)
        .expect("decline Confusticate's payment");

    assert_eq!(engine.state.objects[&opt].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn mixed_target_counter_publishes_not_offered_and_resolves_its_legal_operation() {
    let decks = Some(vec![
        deck_with("island", &["opt", "counterspell"]),
        deck_with("forest", &["mixed_target_counter_fixture"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_133, &[0, 1], 20, decks, true)
        .expect("mixed-target counter scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "opt");
    ensure_in_hand(&mut engine, 0, "counterspell");
    ensure_in_hand(&mut engine, 1, "mixed_target_counter_fixture");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "opt"), Vec::new()),
        )
        .expect("cast the stack spell that becomes illegal before the mixed spell resolves");
    let opt = engine.state.stack.last().expect("Opt on the stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass to the opponent");
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "mixed_target_counter_fixture"),
                vec![
                    stack_target(0, opt),
                    TargetRef {
                        object_id: creature,
                        group_index: 1,
                        ..Default::default()
                    },
                ],
            ),
        )
        .expect("cast the counter-and-damage spell with both targets");
    engine
        .apply_scenario_command(1, &pass())
        .expect("pass for the response");
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "counterspell"),
                vec![stack_target(0, opt)],
            ),
        )
        .expect("counter the first target before the mixed spell resolves");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&opt].zone, Zone::Graveyard);

    let controller_draws = engine.state.turn_history.current.player(1).cards_drawn;
    pass_priority_round(&mut engine);

    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        controller_draws + 1,
        "the `NotOffered` result takes its distinct follow-up"
    );
    assert_eq!(engine.state.objects[&creature].damage, 1);
    assert!(battlefield_token_oids(&engine, 1, "lander").is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn removed_target_makes_later_soft_counter_not_offered_and_continues_once() {
    let decks = Some(vec![
        deck_with("island", &["opt"]),
        deck_with("forest", &["counter_then_soft_counter_fixture"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_135, &[0, 1], 20, decks, true)
        .expect("serial counter-payment scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "opt");
    ensure_in_hand(&mut engine, 1, "counter_then_soft_counter_fixture");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "opt"), Vec::new()),
        )
        .expect("cast the spell targeted by both counter instructions");
    let opt = engine.state.stack.last().expect("Opt on the stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass to the player with the counter program");
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "counter_then_soft_counter_fixture"),
                vec![stack_target(0, opt)],
            ),
        )
        .expect("cast the program with two counter instructions targeting Opt");

    let draws_before = engine.state.turn_history.current.player(1).cards_drawn;
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&opt].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        draws_before + 2,
        "NotOffered is distinct and the later operation resolves once"
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn copied_and_original_divert_keep_independent_paid_and_declined_results() {
    let (mut engine, eject) = setup_uncounterable_divert(75_134);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut engine, 0, "copy_any_spell_fixture");

    engine
        .apply_scenario_command(1, &pass())
        .expect("pass for the copy spell");
    let original_divert = engine
        .state
        .stack
        .iter()
        .find(|item| item.card_id == "divert_disaster")
        .expect("original Divert is on the stack")
        .id;
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "copy_any_spell_fixture"),
                vec![stack_target(0, original_divert)],
            ),
        )
        .expect("copy the opponent's Divert");
    pass_priority_round(&mut engine);
    let target_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the copied Divert asks its controller to choose targets");
    assert_eq!(target_choice.deciding_player, 0);
    assert!(target_choice.presentation.candidates.contains(&eject));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![eject]))
        .expect("keep the copied Divert pointed at the uncounterable Eject");
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.is_copy && item.card_id == "divert_disaster"));

    pass_priority_round(&mut engine);
    let copy_payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the copied Divert resolves first and offers payment");
    assert_eq!(copy_payment.deciding_player, 0);
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay for the copied Divert first");
    assert!(engine.state.stack.iter().any(|item| item.id == eject));
    assert_eq!(battlefield_token_oids(&engine, 0, "lander").len(), 1);
    assert!(battlefield_token_oids(&engine, 1, "lander").is_empty());

    pass_priority_round(&mut engine);
    let original_payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the original Divert offers its own independent payment choice");
    assert_eq!(original_payment.deciding_player, 0);
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::Decline)
        .expect("decline the original Divert after the copy paid");

    assert!(engine.state.stack.iter().any(|item| item.id == eject));
    assert_eq!(engine.state.objects[&eject].zone, Zone::Stack);
    assert_eq!(battlefield_token_oids(&engine, 0, "lander").len(), 1);
    assert!(battlefield_token_oids(&engine, 1, "lander").is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn departed_soft_counter_payer_does_not_leave_a_blocking_payment_prompt() {
    let decks = Some(vec![
        deck_with("forest", &["lightning_bolt"]),
        deck_with("island", &["convolute"]),
        deck_with("forest", &["opt"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_137, &[0, 1, 2], 20, decks, true)
        .expect("three-player departed-payment scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    ensure_in_hand(&mut engine, 1, "convolute");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "lightning_bolt"),
                target_player(2),
            ),
        )
        .expect("B casts Lightning Bolt");
    let bolt = engine.state.stack.last().expect("Bolt on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("B passes to A");
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "convolute"),
                vec![stack_target(0, bolt)],
            ),
        )
        .expect("A casts Convolute targeting B's spell");
    let convolute = engine.state.stack.last().expect("Convolute on stack").id;

    pass_priority_round(&mut engine);
    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the target controller receives the standalone payment offer");
    assert_eq!(payment.deciding_player, 0);
    assert!(matches!(
        payment.continuation,
        ResolutionContinuation::ManaPayment { .. }
    ));
    let old_prompt_id = payment.resolution_prompt_id.get();
    assert_ne!(old_prompt_id, 0);

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("B concedes while A and C survive");

    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackObjectRemoved(removed))
                    if removed.object_id == bolt
            ))
            .count(),
        1,
        "a payer-owned spell leaves the stack through an exact removal event"
    );
    assert!(
        !departure.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::PermanentMoved(moved)) if moved.object_id == bolt
        )),
        "leaving the game is not a zone move"
    );
    assert!(
        !departure.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == bolt
        )),
        "leaving the game is not resolution"
    );
    assert!(
        !departure.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::StackObjectCountered(countered)) if countered.object_id == bolt
        )),
        "leaving the game is not a counter action"
    );
    assert!(!engine.state.is_terminal(), "A and C remain in the game");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[1].has_lost && !engine.state.players[2].has_lost);

    let mut late_survivor_answer = submit_resolution_decision(ResolutionChoiceDecision::PayMana);
    let Some(Cmd::SubmitResolutionChoice(answer)) = late_survivor_answer.cmd.as_mut() else {
        unreachable!();
    };
    answer.resolution_prompt_id = old_prompt_id;
    let state_before_stale_answer = format!("{:?}", engine.state);
    let command_index_before_stale_answer = engine.state.command_index;
    assert!(
        engine
            .apply_scenario_command(2, &late_survivor_answer)
            .is_err(),
        "C cannot answer B's old payment prompt"
    );
    assert_eq!(
        format!("{:?}", engine.state),
        state_before_stale_answer,
        "a stale response leaves engine state and accumulated events unchanged"
    );
    assert_eq!(
        engine.state.command_index,
        command_index_before_stale_answer
    );
    assert!(
        engine.state.pending_resolution.is_none(),
        "departure must retire the exact standalone payment prompt instead of hanging the game"
    );
    assert!(
        !engine.state.objects.contains_key(&bolt),
        "the departed payer's targeted spell is removed without a counter action"
    );
    assert!(!engine.state.stack.iter().any(|item| item.id == convolute));
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackResolved(resolved)) if resolved.object_id == convolute
            ))
            .count(),
        1,
        "the retained Convolute resolution completes once"
    );
    assert!(
        !departure.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::ResolutionChoiceRequired(choice))
                if choice.resolution_prompt_id == old_prompt_id
                    && choice.choice_kind == ChoiceKind::ManaPayment as i32
        )),
        "departure must not delegate or republish the departed payer's mana offer"
    );
}

#[test]
fn two_player_payer_departure_keeps_the_terminal_game_boundary() {
    let decks = Some(vec![
        deck_with("island", &["lightning_bolt"]),
        deck_with("forest", &["convolute"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_140, &[0, 1], 20, decks, true)
        .expect("two-player terminal departure engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    ensure_in_hand(&mut engine, 1, "convolute");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "lightning_bolt"),
                target_player(1),
            ),
        )
        .expect("P0 casts Bolt");
    let bolt = engine.state.stack.last().expect("Bolt on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("P0 passes to Convolute");
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "convolute"),
                vec![stack_target(0, bolt)],
            ),
        )
        .expect("P1 casts Convolute at Bolt");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("payment parked before concession")
            .deciding_player,
        0
    );

    engine
        .apply_scenario_command(0, &concede())
        .expect("P0 concedes the two-player game");

    assert!(engine.state.is_terminal());
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[1].has_lost);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn departed_counter_source_does_not_resume_a_surviving_payer_choice() {
    let decks = Some(vec![
        deck_with("island", &["lightning_bolt"]),
        deck_with("forest", &["convolute"]),
        deck_with("forest", &["opt"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_141, &[0, 1, 2], 20, decks, true)
        .expect("three-player source departure engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    ensure_in_hand(&mut engine, 1, "convolute");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "lightning_bolt"),
                target_player(2),
            ),
        )
        .expect("P0 casts Bolt at C");
    let bolt = engine.state.stack.last().expect("Bolt on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("P0 passes to P1");
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "convolute"),
                vec![stack_target(0, bolt)],
            ),
        )
        .expect("P1 casts Convolute at Bolt");
    let convolute = engine.state.stack.last().expect("Convolute on stack").id;
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("P0 receives the payment choice")
            .deciding_player,
        0
    );

    let departure = engine
        .apply_scenario_command(1, &concede())
        .expect("P1 concedes while the payer and C survive");

    assert!(!engine.state.is_terminal());
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.stack.iter().any(|item| item.id == convolute));
    assert!(engine.state.stack.iter().any(|item| item.id == bolt));
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackObjectRemoved(removed))
                    if removed.object_id == convolute
            ))
            .count(),
        1,
        "the departed counter source is removed by exact OID without claiming its target"
    );
    assert!(!departure.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectRemoved(removed)) if removed.object_id == bolt
    )));
    assert!(!departure.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == ChoiceKind::ManaPayment as i32
    )));
}

#[test]
fn departing_controller_moves_a_foreign_owned_stack_spell_to_exile_once() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        deck_with("island", &["opt"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_144, &[0, 1, 2], 20, decks, true)
        .expect("foreign-owned stack departure engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "lightning_bolt"),
                target_player(1),
            ),
        )
        .expect("P0 casts Bolt");
    let bolt = engine.state.stack.last().expect("Bolt on stack").id;
    // Casting a card owned by another player leaves the spell controlled by its caster.
    // This fixture directly models that already-established ownership identity.
    engine
        .state
        .objects
        .get_mut(&bolt)
        .expect("Bolt object")
        .owner = 2;
    let generation_before = engine.state.zone_change_generation[&bolt];

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("P0 leaves while two players survive");

    assert!(!engine.state.is_terminal());
    assert!(!engine.state.stack.iter().any(|item| item.id == bolt));
    assert_eq!(engine.state.objects[&bolt].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&bolt].owner, 2);
    assert_eq!(
        engine.state.zone_change_generation[&bolt],
        generation_before + 1
    );
    let moves: Vec<_> = departure
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::PermanentMoved(moved)) if moved.object_id == bolt => Some(moved),
            _ => None,
        })
        .collect();
    assert_eq!(
        moves.len(),
        1,
        "the actual exile move is published exactly once"
    );
    assert_eq!(moves[0].owner_player_id, 2);
    assert_eq!(
        moves[0].destination,
        tricerules_proto::ruled::v1::permanent_moved::Destination::Exile as i32
    );
    assert!(
        !departure.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::StackObjectRemoved(removed)) if removed.object_id == bolt
        )),
        "a real exile move is not duplicated as a no-destination removal"
    );
}

#[test]
fn simultaneously_departing_owner_and_controller_remove_the_spell_without_an_exile_move() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        deck_with("island", &["opt"]),
        deck_with("forest", &[]),
        deck_with("mountain", &["lightning_bolt"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_146, &[0, 1, 2, 3], 20, decks, true)
        .expect("simultaneous-departure stack engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "lightning_bolt"),
                target_player(1),
            ),
        )
        .expect("P0 casts Bolt");
    let bolt = engine.state.stack.last().expect("Bolt on stack").id;
    engine
        .state
        .objects
        .get_mut(&bolt)
        .expect("Bolt object")
        .owner = 2;
    while engine.state.priority_player_id() != 3 {
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("pass priority to the surviving P3");
    }
    ensure_in_hand(&mut engine, 3, "lightning_bolt");
    give_mana(
        &mut engine,
        3,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(
            3,
            &cast_spell(
                hand_index_for_card(&engine, 3, "lightning_bolt"),
                target_player(1),
            ),
        )
        .expect("surviving P3 casts an unrelated same-name Bolt");
    let survivor_bolt = engine.state.stack.last().expect("P3 Bolt on stack").id;
    assert_ne!(survivor_bolt, bolt);
    engine
        .state
        .players
        .iter_mut()
        .find(|player| player.id == 2)
        .expect("P2")
        .has_lost = true;

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("P0 and the Bolt's owner P2 leave in the same reconciliation");

    assert!(!engine.state.is_terminal(), "P1 and P3 remain in the game");
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .filter(|player| player.has_lost)
            .count(),
        2,
        "both departing seats are removed while two seats survive"
    );
    assert!(!engine.state.objects.contains_key(&bolt));
    assert!(!engine.state.stack.iter().any(|item| item.id == bolt));
    assert!(engine.state.objects.contains_key(&survivor_bolt));
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == survivor_bolt));
    assert_eq!(engine.state.objects[&survivor_bolt].zone, Zone::Stack);
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackObjectRemoved(removed)) if removed.object_id == bolt
            ))
            .count(),
        1,
        "an object whose owner and controller both leave has no destination move"
    );
    assert!(!departure.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == bolt
    )));
    assert!(!departure.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectRemoved(removed)) if removed.object_id == survivor_bolt
    )));
    assert!(!departure.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == survivor_bolt
    )));
}

#[test]
fn departed_controller_exiles_a_parked_foreign_owned_parent_before_removing_owned_target() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt", "convolute"]),
        deck_with("island", &["opt"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_145, &[0, 1, 2], 20, decks, true)
        .expect("foreign-owned parked parent departure engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    ensure_in_hand(&mut engine, 0, "convolute");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "lightning_bolt"),
                target_player(1),
            ),
        )
        .expect("P0 casts Bolt");
    let bolt = engine.state.stack.last().expect("Bolt on stack").id;
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "convolute"),
                vec![stack_target(0, bolt)],
            ),
        )
        .expect("P0 casts Convolute at their own spell");
    let convolute = engine.state.stack.last().expect("Convolute on stack").id;
    engine
        .state
        .objects
        .get_mut(&convolute)
        .expect("Convolute object")
        .owner = 2;
    let generation_before = engine.state.zone_change_generation[&convolute];
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("P0 receives the payment offer for their Bolt")
            .deciding_player,
        0
    );

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("P0 leaves while P1 and P2 survive");

    assert!(!engine.state.is_terminal());
    assert_eq!(engine.state.objects[&convolute].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&convolute].owner, 2);
    assert_eq!(
        engine.state.zone_change_generation[&convolute],
        generation_before + 1
    );
    let parent_moves = departure
        .events
        .iter()
        .filter(|event| matches!(
            &event.ev,
            Some(Ev::PermanentMoved(moved))
                if moved.object_id == convolute
                    && moved.owner_player_id == 2
                    && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Exile as i32
        ))
        .count();
    assert_eq!(
        parent_moves, 1,
        "the detached foreign-owned parent is exiled exactly once"
    );
    assert!(
        !departure.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::StackObjectRemoved(removed)) if removed.object_id == convolute
        )),
        "the detached parent is excluded from no-destination stack removal"
    );
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackObjectRemoved(removed)) if removed.object_id == bolt
            ))
            .count(),
        1,
        "the departing player's owned target is removed without a zone move"
    );
}

#[test]
fn departed_payer_is_typed_forced_unpaid_and_later_instruction_runs_once() {
    let decks = Some(vec![
        deck_with("island", &["lightning_bolt"]),
        deck_with("forest", &["departure_forced_unpaid_counter_fixture"]),
        deck_with("island", &["opt"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_138, &[0, 1, 2], 20, decks, true)
        .expect("three-player forced-unpaid scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    ensure_in_hand(&mut engine, 1, "departure_forced_unpaid_counter_fixture");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "lightning_bolt"),
                target_player(2),
            ),
        )
        .expect("B casts Bolt at surviving C");
    let bolt = engine.state.stack.last().expect("Bolt on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("B passes to A");
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "departure_forced_unpaid_counter_fixture"),
                vec![stack_target(0, bolt)],
            ),
        )
        .expect("A casts the outcome-observing counter at Bolt");
    let source = engine.state.stack.last().expect("counter on stack").id;
    pass_priority_round(&mut engine);
    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the soft counter parks for B's decision");
    assert_eq!(payment.deciding_player, 0);
    assert!(matches!(
        payment.continuation,
        ResolutionContinuation::ManaPayment { .. }
    ));
    let draws_before = engine.state.turn_history.current.player(1).cards_drawn;

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("B concedes while A and C survive");

    assert!(!engine.state.is_terminal());
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.objects.contains_key(&bolt));
    assert!(!engine.state.stack.iter().any(|item| item.id == source));
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        draws_before + 2,
        "the ForcedUnpaid branch and the following instruction each draw exactly once"
    );
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackResolved(resolved)) if resolved.object_id == source
            ))
            .count(),
        1,
        "the parked frame completes exactly once"
    );
}

#[test]
fn departing_divert_payer_does_not_create_a_lander() {
    let decks = Some(vec![
        deck_with("island", &["eject"]),
        deck_with("forest", &["divert_disaster"]),
        deck_with("forest", &["opt"]),
    ]);
    let mut engine = GameEngine::new(registry(), 75_139, &[0, 1, 2], 20, decks, true)
        .expect("three-player Divert departure scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    let permanent = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "eject");
    ensure_in_hand(&mut engine, 1, "divert_disaster");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 5,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "eject"),
                target_object(permanent),
            ),
        )
        .expect("B casts Eject at C's creature");
    let eject = engine.state.stack.last().expect("Eject on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("B passes to A");
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "divert_disaster"),
                vec![stack_target(0, eject)],
            ),
        )
        .expect("A casts Divert at Eject");
    let divert = engine.state.stack.last().expect("Divert on stack").id;
    pass_priority_round(&mut engine);
    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("B receives Divert's payment offer");
    assert_eq!(payment.deciding_player, 0);

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("B concedes while A and C survive");

    assert!(!engine.state.is_terminal());
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.objects.contains_key(&eject));
    assert!(!engine.state.stack.iter().any(|item| item.id == divert));
    assert!(battlefield_token_oids(&engine, 1, "lander").is_empty());
    assert!(battlefield_token_oids(&engine, 2, "lander").is_empty());
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackResolved(resolved)) if resolved.object_id == divert
            ))
            .count(),
        1,
        "Divert completes once with ForcedUnpaid, so its Paid-only token branch stays false"
    );
}
