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

fn registry() -> &'static tricerules_card_model::CardRegistry {
    Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/divert_disaster.ron"),
                include_str!("../../../tricerules-cards/data/eject.ron"),
                include_str!("../../../tricerules-cards/data/counterspell.ron"),
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!(
                    "../../../tricerules-cards/data/generated/c/confusticate_and_bebother.ron"
                ),
                COPY_ANY_SPELL,
                MIXED_TARGET_COUNTER,
                COUNTER_THEN_SOFT_COUNTER,
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
