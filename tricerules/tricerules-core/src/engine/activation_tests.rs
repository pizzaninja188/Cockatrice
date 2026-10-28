use super::*;

const OPPONENT_CHOICE_LAND: &str = r#"(
    id: "opponent_choice_land", name: "Opponent Choice Land",
    face_id: "opponent_choice_land", types: ["Land"],
    activated_abilities: [(
        ability_id: "activated_01", presentation: Fallback,
        costs: [Mana("{3}"), Tap],
        effect: [
            Tap(subject: Chosen((kind: Creature, controller: You))),
            Tap(subject: Chosen((kind: Creature, controller: Opponent))),
            Fight(first: Chosen((kind: Creature, controller: You)),
                  second: Chosen((kind: Creature, controller: Opponent))),
        ],
        targeting: Some((groups: [
            (min: 1, max: 1, prompt: "Choose your creature", effect_indices: [0, 2]),
            (min: 1, max: 1, prompt: "Choose your creature", effect_indices: [1, 2],
             chooser: ChosenOpponent),
        ])),
    )],
)"#;

const AUTHORED_OPPONENT_CHOICE_LAND: &str = r#"(
    id: "authored_opponent_choice_land", name: "Authored Opponent Choice Land",
    face_id: "authored_opponent_choice_land", types: ["Land"],
    activated_abilities: [(
        ability_id: "activated_01", presentation: Fallback,
        costs: [Mana("{3}"), Tap], effect: [],
        program: Some((body: (steps: [
            (id: "damage_own", kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature, controller: You)))),
            (id: "damage_opponent", kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature, controller: Opponent))))
        ], yields: []), target_bindings: [
            (instruction_id: "damage_own", role: Target, target_group_index: 0),
            (instruction_id: "damage_opponent", role: Target, target_group_index: 1)
        ])),
        targeting: Some((groups: [
            (min: 1, max: 1, prompt: "Choose your creature", effect_indices: []),
            (min: 1, max: 1, prompt: "Choose an opponent's creature", effect_indices: [],
             chooser: ChosenOpponent)
        ]))
    )]
)"#;

const PAYMENT_CREATURE: &str = r#"(
    id: "payment_creature", name: "Payment Creature", face_id: "payment_creature",
    types: ["Creature"], power: Some(2), toughness: Some(2),
    activated_abilities: [(
        ability_id: "activated_01", presentation: Fallback,
        costs: [SacrificeSelf],
        effect: [ProduceMana(options: [(c: 3)])],
    )],
)"#;

const RESTRICTED_PAYMENT_LAND: &str = r#"(
    id: "restricted_payment_land", name: "Restricted Payment Land", face_id: "restricted_payment_land", types: ["Land"],
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback, costs: [Tap],
        effect: [ProduceMana(options: [(c: 3)], restriction: Some((
            restriction_id: "restriction_01", presentation: Fallback, activate_any_ability: true,
        )))],
    )],
)"#;

const SPELL_PAYMENT_LAND: &str = r#"(
    id: "spell_payment_land", name: "Spell Payment Land", face_id: "spell_payment_land", types: ["Land"],
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback, costs: [Tap],
        effect: [ProduceMana(options: [(c: 3)], restriction: Some((
            restriction_id: "restriction_01", presentation: Fallback, cast_spell: [(card_type: Some(Creature))],
        )))],
    )],
)"#;

const PAIN_PAYMENT_LAND: &str = r#"(
    id: "pain_payment_land", name: "Pain Payment Land", face_id: "pain_payment_land", types: ["Land"],
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback, costs: [Tap],
        effect: [ProduceMana(options: [(c: 3)]), DamagePlayer(amount: 1, who: Controller)],
    )],
)"#;

const SYNTHETIC_ACTIVATED_MODAL_DRAW: &str = r#"(
    id: "synthetic_modal_draw", name: "Synthetic Modal Draw", face_id: "synthetic_modal_draw",
    types: ["Artifact"],
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
        costs: [Mana("{4}"), Tap], effect: [], timing: SorcerySpeed,
        modal: Some((min_modes: 1, max_modes: 1, modes: [
            (mode_id: "double_any_permanent", presentation: Fallback,
                effects: [DoubleCounters(target: (kind: AnyPermanent))],
                targeting: Some((groups: [
                    (min: 1, max: 1, prompt: "Choose a permanent", effect_indices: [0])
                ]))
            ),
            (mode_id: "draw_one", presentation: Fallback,
                effects: [Draw(who: Controller, count: 1)]),
            (mode_id: "draw_two", presentation: Fallback,
                effects: [Draw(who: Controller, count: 2)])
        ]))
    )],
)"#;

const SYNTHETIC_ACTIVATED_MODAL_SELF_COUNTERS: &str = r#"(
    id: "synthetic_modal_self_counters", name: "Synthetic Modal Self Counters",
    face_id: "synthetic_modal_self_counters", types: ["Artifact"],
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
        costs: [Mana("{4}"), Tap], effect: [], timing: SorcerySpeed,
        modal: Some((min_modes: 1, max_modes: 1, modes: [
            (mode_id: "double_your_counters", presentation: Fallback,
                effects: [DoubleYourCounters]),
            (mode_id: "draw_one", presentation: Fallback,
                effects: [Draw(who: Controller, count: 1)])
        ]))
    )],
)"#;

const SYNTHETIC_MODAL_SELF_COUNTERS_WITH_TAIL: &str = r#"(
    id: "synthetic_modal_self_counters_with_tail",
    name: "Synthetic Modal Self Counters With Tail",
    face_id: "synthetic_modal_self_counters_with_tail", types: ["Artifact"],
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
        costs: [Mana("{4}"), Tap], effect: [], timing: SorcerySpeed,
        modal: Some((min_modes: 1, max_modes: 1, modes: [
            (mode_id: "double_your_counters", presentation: Fallback,
                effects: [DoubleYourCounters, DamagePlayer(amount: 1, who: EachOpponent)]),
            (mode_id: "draw_one", presentation: Fallback,
                effects: [Draw(who: Controller, count: 1)])
        ]))
    )],
)"#;

const SYNTHETIC_PROTECTED_MODAL_TARGET: &str = r#"(
    id: "synthetic_protected_modal_target", name: "Synthetic Protected Modal Target",
    face_id: "synthetic_protected_modal_target", types: ["Creature"],
    power: Some(2), toughness: Some(2),
    static_abilities: [(ability_id: "static_01", presentation: Fallback,
        definition: TargetingCostIncrease(protected: Source, actors: Opponents,
            actions: ActivatedAbilities, amount: 2))],
)"#;

fn choice_engine() -> (GameEngine, rv1::ActivateAbility) {
    choice_engine_players(&[10, 20])
}

fn choice_engine_players(players: &[PlayerId]) -> (GameEngine, rv1::ActivateAbility) {
    let magus = OPPONENT_CHOICE_LAND
        .replace("opponent_choice_land", "opponent_choice_magus")
        .replace("Opponent Choice Land", "Opponent Choice Magus")
        .replace(
            "types: [\"Land\"],",
            "types: [\"Creature\", \"Human\", \"Wizard\"], power: Some(5), toughness: Some(5),",
        );
    let taxed = PAYMENT_CREATURE
        .replace("payment_creature", "taxed_payment_creature")
        .replace("Payment Creature", "Taxed Payment Creature")
        .replace("activated_abilities:", r#"static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: TargetingCostIncrease(protected: Source, actors: All, actions: ActivatedAbilities, amount: 2))], activated_abilities:"#);
    let reduced = OPPONENT_CHOICE_LAND
        .replace("opponent_choice_land", "reduced_choice_land")
        .replace("Opponent Choice Land", "Reduced Choice Land")
        .replace(r#"costs: [Mana("{3}"), Tap],"#, r#"costs: [Mana("{3}"), Tap], cost_modifiers: [ConditionalGenericReduction(amount: 2, condition: BattlefieldCreatureCount(filter: (controllers: Controller, required_counter: Some(PlusOnePlusOne)), min: Some(1)))],"#);
    let snapshot = PAYMENT_CREATURE
        .replace("payment_creature", "snapshot_payment_creature")
        .replace("Payment Creature", "Snapshot Payment Creature")
        .replace("activated_abilities:", r#"triggered_abilities: [
            (ability_id: "triggered_01", presentation: Fallback, trigger: WheneverSelfBecomesTarget(source: Ability, source_controller: AnyPlayer), effect: [GainLife(amount: 1)]),
            (ability_id: "triggered_02", presentation: Fallback, trigger: WhenSelfLeavesBattlefield, effect: [GainLife(amount: 1)]),
            (ability_id: "triggered_03", presentation: Fallback, trigger: WheneverPlayerCommitsCrime(player: Controller), effect: [GainLife(amount: 1)]),
        ], activated_abilities:"#);
    let loyalty_walker = OPPONENT_CHOICE_LAND
        .replace("opponent_choice_land", "loyalty_choice_walker")
        .replace("Opponent Choice Land", "Loyalty Choice Walker")
        .replace(
            "types: [\"Land\"],",
            "supertypes: [\"Legendary\"], types: [\"Planeswalker\"], loyalty: 5,",
        )
        .replace(
            "costs: [Mana(\"{3}\"), Tap],",
            "costs: [Mana(\"{3}\"), Tap, Loyalty(1)],",
        );
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            include_str!("../../../tricerules-cards/data/laezel,_vlaakiths_champion.ron"),
            include_str!("../../../tricerules-cards/data/doubling_season.ron"),
            OPPONENT_CHOICE_LAND,
            PAYMENT_CREATURE,
            RESTRICTED_PAYMENT_LAND,
            SPELL_PAYMENT_LAND,
            PAIN_PAYMENT_LAND,
            &magus,
            &taxed,
            &reduced,
            &snapshot,
            &loyalty_walker,
        ],
        &[],
    )
    .unwrap();
    let decks = (0..players.len())
        .map(|_| EngineDeck {
            mainboard: vec!["forest".into(); 40],
            commanders: Vec::new(),
        })
        .collect();
    let mut engine = GameEngine::new_with_registry(
        602_301,
        players,
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let mut placed = Vec::new();
    for (player, card) in [
        (10, "Opponent Choice Land"),
        (10, "Grizzly Bears"),
        (20, "Grizzly Bears"),
    ] {
        engine
            .apply_dev_command(
                player,
                &rv1::DevCommand {
                    target_player_id: player,
                    dev: Some(rv1::dev_command::Dev::PutCardInZone(
                        rv1::DevPutCardInZone {
                            card_name: card.into(),
                            zone: rv1::DevZone::Battlefield as i32,
                            ready: true,
                        },
                    )),
                },
            )
            .unwrap();
        placed.push(engine.state.next_object_id - 1);
    }
    engine.state.players[0].mana_pool.colorless = 3;
    engine.reconcile_activated_ability_slots();
    let command = rv1::ActivateAbility {
        source_object_id: placed[0],
        ability_index: 0,
        expected_zone_change_generation: engine
            .payment_object_ref(placed[0])
            .zone_change_generation,
        targets: placed[1..]
            .iter()
            .enumerate()
            .map(|(group, &object_id)| rv1::TargetRef {
                object_id,
                group_index: group as u32,
                kind: rv1::TargetRefKind::Permanent as i32,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    (engine, command)
}

fn place_ready(engine: &mut GameEngine, player: PlayerId, name: &str) -> ObjectId {
    engine
        .apply_dev_command(
            player,
            &rv1::DevCommand {
                target_player_id: player,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: name.into(),
                        zone: rv1::DevZone::Battlefield as i32,
                        ready: true,
                    },
                )),
            },
        )
        .unwrap();
    engine.reconcile_activated_ability_slots();
    engine.state.next_object_id - 1
}

fn activated_modal_fixture(seed: u64) -> (GameEngine, ObjectId, ObjectId) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            SYNTHETIC_ACTIVATED_MODAL_DRAW,
            SYNTHETIC_PROTECTED_MODAL_TARGET,
        ],
        &[],
    )
    .expect("synthetic activated modal fixture compiles");
    let decks = (0..2)
        .map(|_| EngineDeck {
            mainboard: vec!["forest".into(); 40],
            commanders: Vec::new(),
        })
        .collect();
    let mut engine = GameEngine::new_with_registry(
        seed,
        &[10, 20],
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .expect("engine");
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 10, "Synthetic Modal Draw");
    let target = place_ready(&mut engine, 10, "Grizzly Bears");
    engine.state.players[0].mana_pool.colorless = 4;
    (engine, source, target)
}

fn self_counter_replacement_fixture(
    seed: u64,
    poison: u32,
    quest: u32,
) -> (GameEngine, ObjectId, ObjectId, ObjectId) {
    self_counter_replacement_fixture_with_tail(seed, poison, quest, "EachOpponent")
}

fn self_counter_replacement_fixture_with_tail(
    seed: u64,
    poison: u32,
    quest: u32,
    tail_who: &str,
) -> (GameEngine, ObjectId, ObjectId, ObjectId) {
    let card = SYNTHETIC_MODAL_SELF_COUNTERS_WITH_TAIL.replace("EachOpponent", tail_who);
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/vorinclex,_monstrous_raider.ron"),
            card.as_str(),
        ],
        &[],
    )
    .expect("synthetic self-counter tail fixture compiles");
    let decks = (0..3)
        .map(|_| EngineDeck {
            mainboard: vec!["forest".into(); 40],
            commanders: Vec::new(),
        })
        .collect();
    let mut engine = GameEngine::new_with_registry(
        seed,
        &[10, 20, 30],
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .expect("three-player engine");
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 10, "Synthetic Modal Self Counters With Tail");
    let own_vorinclex = place_ready(&mut engine, 10, "Vorinclex, Monstrous Raider");
    let opponent_vorinclex = place_ready(&mut engine, 20, "Vorinclex, Monstrous Raider");
    engine.state.players[0]
        .counters
        .insert(CounterKind::Poison, poison);
    if quest > 0 {
        engine.state.players[0]
            .counters
            .insert(CounterKind::Quest, quest);
    }
    engine.state.players[1]
        .counters
        .insert(CounterKind::Poison, 4);
    engine.state.players[1]
        .counters
        .insert(CounterKind::Quest, 2);
    engine.state.players[2]
        .counters
        .insert(CounterKind::Poison, 2);
    engine.state.players[2]
        .counters
        .insert(CounterKind::Quest, 5);
    engine.state.players[0].mana_pool.colorless = 4;
    (engine, source, own_vorinclex, opponent_vorinclex)
}

fn resolve_activation_to_counter_replacement_choice(
    engine: &mut GameEngine,
) -> (u64, rv1::ResolutionChoiceRequired) {
    resolve_activation_to_counter_replacement_choice_for(engine, 10)
}

fn resolve_activation_to_counter_replacement_choice_for(
    engine: &mut GameEngine,
    expected_deciding_player_id: PlayerId,
) -> (u64, rv1::ResolutionChoiceRequired) {
    let mut prompt_batch = None;
    for player in [10, 20, 30] {
        prompt_batch = Some(
            engine
                .apply_command(
                    player,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .expect("pass priority toward self-counter resolution"),
        );
    }
    let prompt_batch = prompt_batch.expect("three players pass priority");
    let choice = prompt_batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)
                if choice.choice_kind == rv1::ChoiceKind::ReplacementEffect as i32 =>
            {
                Some(choice.clone())
            }
            _ => None,
        })
        .expect("counter replacement order is offered");
    let prompt_id = prompt_batch
        .resolution_prompt_state
        .expect("active counter replacement prompt state is present")
        .resolution_prompt_id
        .expect("active counter replacement prompt has an answer id");
    assert_ne!(prompt_id, 0);
    assert_eq!(choice.resolution_prompt_id, prompt_id);
    assert_eq!(choice.deciding_player_id, expected_deciding_player_id);
    assert_eq!(choice.candidate_object_ids.len(), 2);
    assert_eq!(choice.replacement_options.len(), 2);
    for option in &choice.replacement_options {
        assert!(choice.candidate_object_ids.contains(&option.application_id));
    }
    (prompt_id, choice)
}

fn offered_counter_replacement_application_id(
    choice: &rv1::ResolutionChoiceRequired,
    source_object_id: ObjectId,
    effect_summary: &str,
) -> u32 {
    let option = choice
        .replacement_options
        .iter()
        .find(|option| {
            option.source_object_id == source_object_id
                && option.effect_summary.ends_with(effect_summary)
        })
        .expect("requested replacement is in the engine offer");
    assert!(choice.candidate_object_ids.contains(&option.application_id));
    option.application_id
}

fn paused_counter_replacement_stack_item_id(engine: &GameEngine) -> ObjectId {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("counter replacement resolution is paused");
    match &pending.continuation {
        crate::state::ResolutionContinuation::CounterReplacement {
            stack: Some(stack), ..
        } => stack.item.id,
        _ => panic!("pending choice is not a stack counter replacement"),
    }
}

fn raw_modal_activation(
    engine: &GameEngine,
    source: ObjectId,
    selected_modes: Vec<rv1::SelectedSpellMode>,
    targets: Vec<rv1::TargetRef>,
) -> RuledCommand {
    let source_ref = engine.payment_object_ref(source);
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
            rv1::ActivateAbility {
                source_object_id: source,
                ability_index: 0,
                expected_zone_change_generation: source_ref.zone_change_generation,
                targets,
                selected_modes,
                payment: Some(rv1::PaymentSelection {
                    expected_state_revision: engine.state.command_index,
                    source: Some(source_ref),
                    mana: Some(rv1::PaymentMana {
                        c: 4,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )),
    }
}

fn modal_permanent_target(object_id: ObjectId, generation: Option<u64>) -> rv1::TargetRef {
    rv1::TargetRef {
        object_id,
        group_index: 0,
        kind: rv1::TargetRefKind::Permanent as i32,
        expected_zone_change_generation: generation,
        ..Default::default()
    }
}

fn protected_modal_activation_fixture(seed: u64) -> (GameEngine, ObjectId, ObjectId, RuledCommand) {
    let (mut engine, source, _) = activated_modal_fixture(seed);
    let target = place_ready(&mut engine, 20, "Synthetic Protected Modal Target");
    engine.state.players[0].mana_pool.colorless = 6;
    let generation = engine.payment_object_ref(target).zone_change_generation;
    let mut command = raw_modal_activation(
        &engine,
        source,
        vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: vec![modal_permanent_target(target, Some(generation))],
        }],
        Vec::new(),
    );
    let Some(rv1::ruled_command::Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("raw modal activation creates an ActivateAbility command");
    };
    activation
        .payment
        .as_mut()
        .expect("raw modal activation includes a payment selection")
        .mana
        .as_mut()
        .expect("raw modal activation includes mana payment")
        .c = 6;
    (engine, source, target, command)
}

fn assert_modal_rejected_without_cost(
    engine: &mut GameEngine,
    command: RuledCommand,
    expected_error: &str,
) {
    let before = serde_json::to_value(&engine.state).expect("serialize state before rejection");
    let error = engine
        .apply_command(10, &command)
        .expect_err("invalid modal activation must be rejected");
    assert_eq!(
        error.to_string(),
        format!("illegal command: {expected_error}")
    );
    assert_eq!(
        serde_json::to_value(&engine.state).expect("serialize state after rejection"),
        before,
        "rejected modal input cannot pay or mutate state"
    );
}

#[test]
fn activated_modal_ability_captures_selected_mode_and_resolves_its_body() {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            SYNTHETIC_ACTIVATED_MODAL_DRAW,
        ],
        &[],
    )
    .expect("synthetic activated modal fixture compiles");
    let decks = (0..2)
        .map(|_| EngineDeck {
            mainboard: vec!["forest".into(); 40],
            commanders: Vec::new(),
        })
        .collect();
    let mut engine = GameEngine::new_with_registry(
        602_399,
        &[10, 20],
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .expect("engine");
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 10, "Synthetic Modal Draw");
    engine.state.players[0].mana_pool.colorless = 4;
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();
    let source_ref = engine.payment_object_ref(source);

    // Submit the raw command so the wire-selected mode reaches the actual activation dispatcher.
    let activation = rv1::ActivateAbility {
        source_object_id: source,
        ability_index: 0,
        expected_zone_change_generation: source_ref.zone_change_generation,
        selected_modes: vec![rv1::SelectedSpellMode {
            mode_index: 2,
            targets: Vec::new(),
        }],
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_ref),
            mana: Some(rv1::PaymentMana {
                c: 4,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let activation_batch = engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(activation)),
            },
        )
        .expect("pay {4} and activate the selected modal ability");

    let stack_object_id = engine.state.stack[0].id;
    let stack_pushed = activation_batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::StackPushed(pushed))
                if pushed.object_id == stack_object_id =>
            {
                Some(pushed)
            }
            _ => None,
        })
        .expect("activation publishes its stack item");
    assert_eq!(stack_pushed.chosen_mode_indices, vec![2]);
    assert_eq!(stack_pushed.chosen_mode_labels.len(), 1);
    assert_eq!(stack_pushed.chosen_mode_presentations.len(), 1);
    let mode_presentation = &stack_pushed.chosen_mode_presentations[0];
    assert!(!mode_presentation.fallback_text.is_empty());
    assert_eq!(
        stack_pushed.chosen_mode_labels[0],
        mode_presentation.fallback_text
    );
    assert!(mode_presentation.path.last().is_some_and(|component| {
        component.kind == rv1::PresentationPathKind::Mode as i32 && component.id == "draw_two"
    }));

    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0]
            .chosen_modes
            .first()
            .map(|mode| mode.mode_id.clone()),
        Some(tricerules_card_model::ModeId::new("draw_two").unwrap()),
        "the selected wire index must be captured as its stable authored mode id"
    );

    for player in [10, 20] {
        engine
            .apply_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("pass priority to resolve the ability");
    }

    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
}

#[test]
fn activated_modal_double_your_counters_doubles_only_the_ability_controller_with_replacements() {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/vorinclex,_monstrous_raider.ron"),
            SYNTHETIC_ACTIVATED_MODAL_SELF_COUNTERS,
        ],
        &[],
    )
    .expect("synthetic self-counter modal fixture compiles");
    let decks = (0..2)
        .map(|_| EngineDeck {
            mainboard: vec!["forest".into(); 40],
            commanders: Vec::new(),
        })
        .collect();
    let mut engine = GameEngine::new_with_registry(
        602_406,
        &[10, 20],
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .expect("engine");
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 10, "Synthetic Modal Self Counters");
    place_ready(&mut engine, 10, "Vorinclex, Monstrous Raider");
    engine.state.players[0]
        .counters
        .insert(tricerules_card_model::primitives::CounterKind::Poison, 3);
    engine.state.players[0]
        .counters
        .insert(tricerules_card_model::primitives::CounterKind::Quest, 2);
    engine.state.players[1]
        .counters
        .insert(tricerules_card_model::primitives::CounterKind::Poison, 1);
    engine.state.players[1]
        .counters
        .insert(tricerules_card_model::primitives::CounterKind::Quest, 5);
    engine.state.players[0]
        .counters
        .insert(tricerules_card_model::primitives::CounterKind::Charge, 0);
    engine.state.players[0].mana_pool.colorless = 4;

    let activation = raw_modal_activation(
        &engine,
        source,
        vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: Vec::new(),
        }],
        Vec::new(),
    );
    engine
        .apply_command(10, &activation)
        .expect("pay {4} and activate the targetless self-counter mode");

    let item = engine
        .state
        .stack
        .last()
        .expect("activated ability is on the stack")
        .clone();
    let mode_id = item.chosen_modes[0].mode_id.clone();
    let mut parked = crate::state::ParkedStackResolution::new(item.clone());
    parked.resume_effect_index = Some(1);
    let snapshot = engine.state.players[0]
        .counters
        .iter()
        .filter_map(|(&kind, &count)| (count > 0).then_some((kind, count)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let completion = crate::state::CounterPlacementCompletion::DoubleYourCounters {
        stack_id: item.id,
        mode_id: mode_id.clone(),
        leaf_index: 0,
        controller: 10,
        snapshot: snapshot.clone(),
    };
    let proposal = engine
        .proposed_player_counter_placement(
            10,
            10,
            snapshot.iter().map(|(&kind, &count)| (kind, count)),
            super::continuous::CounterPlacementOrigin::Effect,
        )
        .expect("self counter snapshot produces one player placement");
    let assert_rejected_without_mutation =
        |engine: &mut GameEngine,
         events: &[super::replacement::ProposedCounterPlacement],
         completion: &crate::state::CounterPlacementCompletion,
         parked: Option<&crate::state::ParkedStackResolution>| {
            let before = engine.diagnostic_snapshot().expect("snapshot before");
            let mut out = Vec::new();
            assert!(engine
                .commit_counter_placement_batch(events, completion, parked, &mut out)
                .is_err());
            assert!(out.is_empty());
            assert_eq!(
                engine.diagnostic_snapshot().expect("snapshot after"),
                before
            );
        };
    let mut invalid_frame = parked.clone();
    invalid_frame.item.id = invalid_frame.item.id.saturating_add(1);
    assert_rejected_without_mutation(
        &mut engine,
        std::slice::from_ref(&proposal),
        &completion,
        Some(&invalid_frame),
    );

    let mut invalid = completion.clone();
    if let crate::state::CounterPlacementCompletion::DoubleYourCounters { stack_id, .. } =
        &mut invalid
    {
        *stack_id = stack_id.saturating_add(1);
    }
    assert_rejected_without_mutation(
        &mut engine,
        std::slice::from_ref(&proposal),
        &invalid,
        Some(&parked),
    );

    let mut invalid = completion.clone();
    if let crate::state::CounterPlacementCompletion::DoubleYourCounters { mode_id, .. } =
        &mut invalid
    {
        *mode_id = tricerules_card_model::ModeId::new("draw_one").expect("mode id");
    }
    assert_rejected_without_mutation(
        &mut engine,
        std::slice::from_ref(&proposal),
        &invalid,
        Some(&parked),
    );

    let mut invalid = completion.clone();
    if let crate::state::CounterPlacementCompletion::DoubleYourCounters { leaf_index, .. } =
        &mut invalid
    {
        *leaf_index = 1;
    }
    assert_rejected_without_mutation(
        &mut engine,
        std::slice::from_ref(&proposal),
        &invalid,
        Some(&parked),
    );

    let mut invalid = completion.clone();
    if let crate::state::CounterPlacementCompletion::DoubleYourCounters { controller, .. } =
        &mut invalid
    {
        *controller = 20;
    }
    assert_rejected_without_mutation(
        &mut engine,
        std::slice::from_ref(&proposal),
        &invalid,
        Some(&parked),
    );

    let mut invalid_proposal = proposal.clone();
    invalid_proposal.actor = 20;
    assert_rejected_without_mutation(&mut engine, &[invalid_proposal], &completion, Some(&parked));

    let mut invalid_proposal = proposal.clone();
    invalid_proposal.recipient =
        super::replacement::CounterPlacementRecipient::Player { player_id: 20 };
    assert_rejected_without_mutation(&mut engine, &[invalid_proposal], &completion, Some(&parked));

    let mut invalid_proposal = proposal.clone();
    invalid_proposal.origin = super::continuous::CounterPlacementOrigin::Cost;
    assert_rejected_without_mutation(&mut engine, &[invalid_proposal], &completion, Some(&parked));
    assert_rejected_without_mutation(
        &mut engine,
        std::slice::from_ref(&proposal),
        &completion,
        None,
    );
    assert_rejected_without_mutation(
        &mut engine,
        std::slice::from_ref(&proposal),
        &crate::state::CounterPlacementCompletion::StackEffect {
            spell_label: "generic stack effect".into(),
        },
        None,
    );

    for player in [10, 20] {
        engine
            .apply_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("pass priority to resolve DoubleYourCounters");
    }

    assert_eq!(
        engine.state.players[0]
            .counters
            .get(&tricerules_card_model::primitives::CounterKind::Poison),
        Some(&9)
    );
    assert_eq!(
        engine.state.players[0]
            .counters
            .get(&tricerules_card_model::primitives::CounterKind::Quest),
        Some(&6)
    );
    assert_eq!(
        engine.state.players[0]
            .counters
            .get(&tricerules_card_model::primitives::CounterKind::Charge),
        Some(&0),
        "zero-count entries are not part of the snapshot"
    );
    assert_eq!(
        engine.state.players[1]
            .counters
            .get(&tricerules_card_model::primitives::CounterKind::Poison),
        Some(&1)
    );
    assert_eq!(
        engine.state.players[1]
            .counters
            .get(&tricerules_card_model::primitives::CounterKind::Quest),
        Some(&5)
    );

    engine.state.players[0].counters.clear();
    engine.state.players[0]
        .counters
        .insert(tricerules_card_model::primitives::CounterKind::Charge, 0);
    engine.state.players[0].mana_pool.colorless = 4;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("self-counter source remains on the battlefield")
        .tapped = false;
    let empty_snapshot_activation = raw_modal_activation(
        &engine,
        source,
        vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: Vec::new(),
        }],
        Vec::new(),
    );
    engine
        .apply_command(10, &empty_snapshot_activation)
        .expect("zero positive counters is a legal no-op after controller validation");
    for player in [10, 20] {
        engine
            .apply_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("pass priority after empty self-counter resolution");
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].counters.len(), 1);
    assert_eq!(
        engine.state.players[0]
            .counters
            .get(&tricerules_card_model::primitives::CounterKind::Charge),
        Some(&0)
    );
    assert_eq!(
        engine.state.players[1]
            .counters
            .get(&tricerules_card_model::primitives::CounterKind::Poison),
        Some(&1)
    );
}

#[test]
fn activated_modal_self_counter_replacement_order_uses_offered_ids_and_zero_result_resumes_tail() {
    for (seed, halve_first, expected_poison, expected_quest) in
        [(602_500, true, 1, 1), (602_501, false, 2, 2)]
    {
        let (mut engine, source, own_vorinclex, opponent_vorinclex) =
            self_counter_replacement_fixture(seed, 1, 1);
        let activation = raw_modal_activation(
            &engine,
            source,
            vec![rv1::SelectedSpellMode {
                mode_index: 0,
                targets: Vec::new(),
            }],
            Vec::new(),
        );
        engine
            .apply_command(10, &activation)
            .expect("pay {4} and activate the self-counter mode with a tail");

        let (prompt_id, choice) = resolve_activation_to_counter_replacement_choice(&mut engine);
        assert_eq!(
            engine.state.players[0].counters.get(&CounterKind::Poison),
            Some(&1),
            "the initial poison counter is unchanged while replacement order is parked"
        );
        assert_eq!(
            engine.state.players[0].counters.get(&CounterKind::Quest),
            Some(&1),
            "the initial quest counter is unchanged while replacement order is parked"
        );
        assert_eq!(
            engine.state.players[1].counters.get(&CounterKind::Poison),
            Some(&4)
        );
        assert_eq!(
            engine.state.players[1].counters.get(&CounterKind::Quest),
            Some(&2)
        );
        assert_eq!(
            engine.state.players[2].counters.get(&CounterKind::Poison),
            Some(&2)
        );
        assert_eq!(
            engine.state.players[2].counters.get(&CounterKind::Quest),
            Some(&5)
        );
        assert_eq!(engine.state.players[0].life, 20);
        assert_eq!(engine.state.players[1].life, 20);
        assert_eq!(engine.state.players[2].life, 20);

        let first_application_id = if halve_first {
            offered_counter_replacement_application_id(
                &choice,
                opponent_vorinclex,
                "halves counters opponents would put, rounded down",
            )
        } else {
            offered_counter_replacement_application_id(
                &choice,
                own_vorinclex,
                "doubles counters this player would put",
            )
        };
        engine
            .apply_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![first_application_id],
                            resolution_prompt_id: prompt_id,
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("select the offered replacement and resume the parked mode");

        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            engine.state.players[0].counters.get(&CounterKind::Poison),
            Some(&expected_poison)
        );
        assert_eq!(
            engine.state.players[0].counters.get(&CounterKind::Quest),
            Some(&expected_quest)
        );
        assert_eq!(
            engine.state.players[1].counters.get(&CounterKind::Poison),
            Some(&4)
        );
        assert_eq!(
            engine.state.players[1].counters.get(&CounterKind::Quest),
            Some(&2)
        );
        assert_eq!(
            engine.state.players[2].counters.get(&CounterKind::Poison),
            Some(&2)
        );
        assert_eq!(
            engine.state.players[2].counters.get(&CounterKind::Quest),
            Some(&5)
        );
        assert_eq!(engine.state.players[0].life, 20);
        assert_eq!(engine.state.players[1].life, 19);
        assert_eq!(engine.state.players[2].life, 19);
    }
}

#[test]
fn activated_modal_self_poison_threshold_resolves_tail_before_state_based_loss() {
    let (mut engine, source, _own_vorinclex, opponent_vorinclex) =
        self_counter_replacement_fixture(602_502, 6, 0);
    let activation = raw_modal_activation(
        &engine,
        source,
        vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: Vec::new(),
        }],
        Vec::new(),
    );
    engine
        .apply_command(10, &activation)
        .expect("pay {4} and activate the self-counter mode near the poison threshold");

    let (prompt_id, choice) = resolve_activation_to_counter_replacement_choice(&mut engine);
    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Poison),
        Some(&6)
    );
    assert_eq!(engine.state.players[0].life, 20);
    let halve_application_id = offered_counter_replacement_application_id(
        &choice,
        opponent_vorinclex,
        "halves counters opponents would put, rounded down",
    );
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![halve_application_id],
                        resolution_prompt_id: prompt_id,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("halve first, then let the sole remaining replacement apply");

    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Poison),
        Some(&12)
    );
    assert!(engine.state.players[0].has_lost);
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
    assert!(!engine.state.players[1].has_lost);
    assert!(!engine.state.players[2].has_lost);
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&4)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Quest),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Poison),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Quest),
        Some(&5)
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(!engine.state.is_terminal());
}

#[test]
fn activated_modal_self_counter_uses_captured_controller_after_source_control_changes() {
    let (mut engine, source, own_vorinclex, _) = self_counter_replacement_fixture(602_503, 1, 1);
    let activation = raw_modal_activation(
        &engine,
        source,
        vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: Vec::new(),
        }],
        Vec::new(),
    );
    engine
        .apply_command(10, &activation)
        .expect("activate the self-counter mode before changing source control");

    let item = engine
        .state
        .stack
        .last()
        .expect("the activated ability is on the stack")
        .clone();
    assert_eq!(item.controller, 10);
    assert_eq!(item.source_owner, Some(10));
    assert_eq!(
        item.chosen_modes[0].mode_id,
        tricerules_card_model::ModeId::new("double_your_counters").expect("mode id")
    );

    engine
        .state
        .objects
        .get_mut(&source)
        .expect("source remains on the battlefield")
        .base_controller = 20;
    let mut control_events = Vec::new();
    engine.reindex_battlefield_control(&mut control_events);
    assert_eq!(engine.state.objects[&source].controller, 20);
    assert!(!engine.state.players[0].battlefield.contains(&source));
    assert!(engine.state.players[1].battlefield.contains(&source));

    let (prompt_id, choice) = resolve_activation_to_counter_replacement_choice_for(&mut engine, 10);
    assert_eq!(choice.deciding_player_id, 10);
    let double_application_id = offered_counter_replacement_application_id(
        &choice,
        own_vorinclex,
        "doubles counters this player would put",
    );
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![double_application_id],
                        resolution_prompt_id: prompt_id,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("resolve the captured self-counter mode after source control changes");

    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Poison),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Quest),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&4)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Quest),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Poison),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Quest),
        Some(&5)
    );
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn activated_modal_self_counter_resolves_after_source_leaves_before_resolution() {
    let (mut engine, source, own_vorinclex, _) = self_counter_replacement_fixture(602_504, 1, 1);
    let activation = raw_modal_activation(
        &engine,
        source,
        vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: Vec::new(),
        }],
        Vec::new(),
    );
    engine
        .apply_command(10, &activation)
        .expect("activate the self-counter mode before the source leaves");

    let item = engine
        .state
        .stack
        .last()
        .expect("the activated ability is on the stack")
        .clone();
    assert_eq!(item.controller, 10);
    assert_eq!(item.source_owner, Some(10));
    assert_eq!(
        item.chosen_modes[0].mode_id,
        tricerules_card_model::ModeId::new("double_your_counters").expect("mode id")
    );
    engine
        .apply_dev_command(
            10,
            &rv1::DevCommand {
                target_player_id: 10,
                dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                    card_name: "Synthetic Modal Self Counters With Tail".into(),
                    zone: rv1::DevZone::Graveyard as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("move the physical source through the engine zone-change helper");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);

    let (prompt_id, choice) = resolve_activation_to_counter_replacement_choice_for(&mut engine, 10);
    assert_eq!(choice.deciding_player_id, 10);
    let double_application_id = offered_counter_replacement_application_id(
        &choice,
        own_vorinclex,
        "doubles counters this player would put",
    );
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![double_application_id],
                        resolution_prompt_id: prompt_id,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("resolve the captured self-counter mode after the source leaves");

    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Poison),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Quest),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&4)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Quest),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Poison),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Quest),
        Some(&5)
    );
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn copied_self_counter_survives_source_owner_departure_during_replacement_pause() {
    let (mut engine, source, _source_vorinclex, _) =
        self_counter_replacement_fixture(602_505, 1, 1);
    let activation = raw_modal_activation(
        &engine,
        source,
        vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: Vec::new(),
        }],
        Vec::new(),
    );
    engine
        .apply_command(10, &activation)
        .expect("activate the self-counter mode before making the copy-state fixture");

    let original = engine
        .state
        .stack
        .last()
        .expect("the original activated ability is on the stack")
        .clone();
    assert_eq!(original.controller, 10);
    assert_eq!(original.source_owner, Some(10));
    assert_eq!(
        original.chosen_modes[0].mode_id,
        tricerules_card_model::ModeId::new("double_your_counters").expect("mode id")
    );

    // Consumer-only copied-StackItem fixture: Rings' actual copy producer is covered separately.
    // This synthetic counter bag proves the copied ability's controller/source-owner split.
    let copy_id = engine.state.next_object_id;
    engine.state.next_object_id = copy_id.checked_add(1).expect("fresh stack id");
    let mut copied_ability = original.clone();
    copied_ability.id = copy_id;
    copied_ability.controller = 20;
    copied_ability.is_copy = true;
    copied_ability.ability_text = Some("Copy fixture: Synthetic Self Counter Mode".into());
    engine.state.stack.push(copied_ability.clone());
    assert_ne!(copied_ability.id, original.id);
    assert!(copied_ability.is_copy);
    assert_eq!(copied_ability.controller, 20);
    assert_eq!(copied_ability.source_owner, Some(10));
    assert_eq!(
        copied_ability.ability_text.as_deref(),
        Some("Copy fixture: Synthetic Self Counter Mode")
    );
    assert_eq!(
        copied_ability.chosen_modes.len(),
        original.chosen_modes.len()
    );
    assert_eq!(
        copied_ability.chosen_modes[0].mode_id,
        original.chosen_modes[0].mode_id
    );

    let (prompt_id, choice) = resolve_activation_to_counter_replacement_choice_for(&mut engine, 20);
    assert_eq!(choice.deciding_player_id, 20);
    assert_eq!(choice.resolution_prompt_id, prompt_id);
    assert_eq!(paused_counter_replacement_stack_item_id(&engine), copy_id);
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&4)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Quest),
        Some(&2)
    );

    let _departure = concede(&mut engine, 10);
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.is_terminal());
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.players[2].has_lost);
    assert!(engine.state.stack.iter().all(|item| item.id != original.id));
    assert!(!engine.state.players[0].battlefield.contains(&source));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Poison),
        Some(&1)
    );
    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Quest),
        Some(&1)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&12)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Quest),
        Some(&6)
    );
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.players[2].life, 19);
}

#[test]
fn self_counter_recipient_departure_prunes_paused_placement_and_resumes_each_player_tail() {
    let (mut engine, source, _own_vorinclex, _) =
        self_counter_replacement_fixture_with_tail(602_506, 1, 1, "EachPlayer");
    let activation = raw_modal_activation(
        &engine,
        source,
        vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: Vec::new(),
        }],
        Vec::new(),
    );
    engine
        .apply_command(10, &activation)
        .expect("activate the self-counter mode with an actor-independent tail");

    let item_id = engine
        .state
        .stack
        .last()
        .expect("the activated ability is on the stack")
        .id;
    let (prompt_id, choice) = resolve_activation_to_counter_replacement_choice_for(&mut engine, 10);
    assert_eq!(choice.deciding_player_id, 10);
    assert_eq!(choice.resolution_prompt_id, prompt_id);
    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Poison),
        Some(&1)
    );
    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Quest),
        Some(&1)
    );
    assert_eq!(paused_counter_replacement_stack_item_id(&engine), item_id);

    let departure = concede(&mut engine, 10);
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.is_terminal());
    assert!(engine.state.pending_resolution.is_none());
    let inactive_prompt = departure
        .resolution_prompt_state
        .expect("departure publishes an inactive prompt snapshot to clear the client prompt");
    assert!(!inactive_prompt.active);
    assert!(inactive_prompt.resolution_prompt_id.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Poison),
        Some(&1)
    );
    assert_eq!(
        engine.state.players[0].counters.get(&CounterKind::Quest),
        Some(&1)
    );
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&4)
    );
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Quest),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Poison),
        Some(&2)
    );
    assert_eq!(
        engine.state.players[2].counters.get(&CounterKind::Quest),
        Some(&5)
    );
}

#[test]
fn activated_modal_rejects_invalid_mode_and_target_receipts_before_payment() {
    let (mut engine, source, target) = activated_modal_fixture(602_400);
    let generation = engine.payment_object_ref(target).zone_change_generation;
    let current_target = modal_permanent_target(target, Some(generation));
    let cases = [
        (Vec::new(), Vec::new(), "illegal number of selected modes"),
        (
            vec![rv1::SelectedSpellMode {
                mode_index: 99,
                targets: Vec::new(),
            }],
            Vec::new(),
            "bad activated ability mode index",
        ),
        (
            vec![
                rv1::SelectedSpellMode {
                    mode_index: 1,
                    targets: Vec::new(),
                },
                rv1::SelectedSpellMode {
                    mode_index: 1,
                    targets: Vec::new(),
                },
            ],
            Vec::new(),
            "a mode may be selected only once",
        ),
        (
            vec![
                rv1::SelectedSpellMode {
                    mode_index: 1,
                    targets: Vec::new(),
                },
                rv1::SelectedSpellMode {
                    mode_index: 2,
                    targets: Vec::new(),
                },
            ],
            Vec::new(),
            "illegal number of selected modes",
        ),
        (
            vec![rv1::SelectedSpellMode {
                mode_index: 2,
                targets: Vec::new(),
            }],
            vec![current_target],
            "modal activated abilities use selected_modes targets",
        ),
        (
            vec![rv1::SelectedSpellMode {
                mode_index: 1,
                targets: vec![current_target],
            }],
            Vec::new(),
            "this action takes no targets",
        ),
        (
            vec![rv1::SelectedSpellMode {
                mode_index: 0,
                targets: Vec::new(),
            }],
            Vec::new(),
            "target group cardinality is invalid",
        ),
        (
            vec![rv1::SelectedSpellMode {
                mode_index: 0,
                targets: vec![modal_permanent_target(target, None)],
            }],
            Vec::new(),
            "modal permanent target generation is required",
        ),
        (
            vec![rv1::SelectedSpellMode {
                mode_index: 0,
                targets: vec![modal_permanent_target(
                    target,
                    Some(generation.saturating_add(1)),
                )],
            }],
            Vec::new(),
            "target generation is stale",
        ),
    ];

    for (selected_modes, targets, expected_error) in cases {
        let command = raw_modal_activation(&engine, source, selected_modes, targets);
        assert_modal_rejected_without_cost(&mut engine, command, expected_error);
    }
}

#[test]
fn activated_modal_payment_preview_counts_mode_targets_for_target_tax() {
    let (mut actual_engine, _, actual_target, actual_command) =
        protected_modal_activation_fixture(602_403);
    let actual_generation = actual_engine
        .payment_object_ref(actual_target)
        .zone_change_generation;
    assert_eq!(
        actual_engine.targeting_cost_increase(
            10,
            TargetingCostAction::ActivatedAbilities,
            &[modal_permanent_target(
                actual_target,
                Some(actual_generation)
            )],
        ),
        2,
        "the opponent-owned target contributes its two-mana activation tax"
    );
    actual_engine
        .apply_command(10, &actual_command)
        .expect("the raw modal activation accepts the exact six-mana payment");
    assert_eq!(actual_engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        actual_engine.state.stack[0].chosen_modes[0].targets[0].object_id,
        actual_target
    );

    let (preview_engine, _, _, preview_command) = protected_modal_activation_fixture(602_404);
    let Some(rv1::ruled_command::Cmd::ActivateAbility(preview_activation)) = preview_command.cmd
    else {
        unreachable!("raw modal activation creates an ActivateAbility command");
    };
    let before =
        serde_json::to_value(&preview_engine.state).expect("serialize state before preview");
    let preview = preview_engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            activate_ability: Some(preview_activation),
            ..Default::default()
        },
    );
    assert!(preview.error.is_empty(), "{}", preview.error);
    assert!(preview.complete);
    assert_eq!(
        preview
            .selection
            .as_ref()
            .and_then(|selection| selection.mana.as_ref())
            .map(|mana| mana.c),
        Some(6)
    );
    assert_eq!(
        serde_json::to_value(&preview_engine.state).expect("serialize state after preview"),
        before,
        "payment preview must not mutate game state"
    );
}

#[test]
fn activated_modal_payment_preview_rejects_invalid_mode_receipts_without_mutation() {
    let (engine, source, target) = activated_modal_fixture(602_405);
    let generation = engine.payment_object_ref(target).zone_change_generation;
    let valid_target = modal_permanent_target(target, Some(generation));
    let mut wrong_group_target = valid_target;
    wrong_group_target.group_index = 1;
    let cases = [
        ("missing mode", Vec::new()),
        (
            "duplicate mode",
            vec![
                rv1::SelectedSpellMode {
                    mode_index: 0,
                    targets: vec![valid_target],
                },
                rv1::SelectedSpellMode {
                    mode_index: 0,
                    targets: vec![valid_target],
                },
            ],
        ),
        (
            "out-of-range mode",
            vec![rv1::SelectedSpellMode {
                mode_index: 99,
                targets: Vec::new(),
            }],
        ),
        (
            "missing mode target",
            vec![rv1::SelectedSpellMode {
                mode_index: 0,
                targets: Vec::new(),
            }],
        ),
        (
            "target assigned to untargeted mode",
            vec![rv1::SelectedSpellMode {
                mode_index: 1,
                targets: vec![valid_target],
            }],
        ),
        (
            "wrong target group",
            vec![rv1::SelectedSpellMode {
                mode_index: 0,
                targets: vec![wrong_group_target],
            }],
        ),
        (
            "stale mode target generation",
            vec![rv1::SelectedSpellMode {
                mode_index: 0,
                targets: vec![modal_permanent_target(
                    target,
                    Some(generation.saturating_add(1)),
                )],
            }],
        ),
    ];
    let before = serde_json::to_value(&engine.state).expect("serialize state before previews");

    for (case, selected_modes) in cases {
        let command = raw_modal_activation(&engine, source, selected_modes, Vec::new());
        let Some(rv1::ruled_command::Cmd::ActivateAbility(activation)) = command.cmd else {
            unreachable!("raw modal activation creates an ActivateAbility command");
        };
        let preview = engine.preview_payment(
            10,
            &rv1::PreviewPayment {
                activate_ability: Some(activation),
                ..Default::default()
            },
        );
        assert!(!preview.error.is_empty(), "{case} was accepted");
        assert!(!preview.complete, "{case} produced a complete payment");
        assert_eq!(
            serde_json::to_value(&engine.state).expect("serialize state after preview"),
            before,
            "{case} preview must not mutate game state"
        );
    }
}

#[test]
fn activated_modal_payment_preview_matches_targeted_and_targetless_activations() {
    for (seed, mode_index, target_required) in [(602_406, 0, true), (602_407, 1, false)] {
        let (mut engine, source, target) = activated_modal_fixture(seed);
        let generation = engine.payment_object_ref(target).zone_change_generation;
        let selected_targets = if target_required {
            vec![modal_permanent_target(target, Some(generation))]
        } else {
            Vec::new()
        };
        let command = raw_modal_activation(
            &engine,
            source,
            vec![rv1::SelectedSpellMode {
                mode_index,
                targets: selected_targets,
            }],
            Vec::new(),
        );
        let Some(rv1::ruled_command::Cmd::ActivateAbility(activation)) = command.cmd.as_ref()
        else {
            unreachable!("raw modal activation creates an ActivateAbility command");
        };
        let before =
            serde_json::to_value(&engine.state).expect("serialize state before payment preview");
        let preview = engine.preview_payment(
            10,
            &rv1::PreviewPayment {
                activate_ability: Some(activation.clone()),
                ..Default::default()
            },
        );
        assert!(preview.error.is_empty(), "{}", preview.error);
        assert!(preview.complete);
        assert_eq!(
            preview
                .selection
                .as_ref()
                .and_then(|selection| selection.mana.as_ref())
                .map(|mana| mana.c),
            Some(4)
        );
        assert_eq!(
            serde_json::to_value(&engine.state).expect("serialize state after payment preview"),
            before,
            "payment preview must not mutate game state"
        );

        engine
            .apply_command(10, &command)
            .expect("the same raw modal activation is accepted after preview");
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(
            engine.state.stack[0].chosen_modes[0].targets.len(),
            if target_required { 1 } else { 0 }
        );
    }
}

#[test]
fn nonmodal_payment_preview_rejects_selected_modes_before_payment() {
    let (mut engine, _) = choice_engine();
    let source = place_ready(&mut engine, 10, "Restricted Payment Land");
    engine.state.players[0].mana_pool.colorless = 0;
    let source_ref = engine.payment_object_ref(source);
    let command = rv1::ActivateAbility {
        source_object_id: source,
        ability_index: 0,
        expected_zone_change_generation: source_ref.zone_change_generation,
        selected_modes: vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: Vec::new(),
        }],
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_ref),
            mana: Some(rv1::PaymentMana::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let before = serde_json::to_value(&engine.state).expect("serialize state before preview");
    let preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            activate_ability: Some(command),
            ..Default::default()
        },
    );
    assert_eq!(
        preview.error,
        "illegal command: selected_modes given for a nonmodal activated ability"
    );
    assert!(!preview.complete);
    assert_eq!(
        serde_json::to_value(&engine.state).expect("serialize state after preview"),
        before,
        "rejected preview must not mutate game state"
    );
}

#[test]
fn activated_modal_captures_mode_local_permanent_generation() {
    let (mut engine, source, target) = activated_modal_fixture(602_401);
    let generation = engine.payment_object_ref(target).zone_change_generation;
    let command = raw_modal_activation(
        &engine,
        source,
        vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: vec![modal_permanent_target(target, Some(generation))],
        }],
        Vec::new(),
    );
    engine
        .apply_command(10, &command)
        .expect("valid targeted modal mode is announced");
    let stack_item = engine.state.stack.last().expect("ability is on the stack");
    assert!(
        stack_item.targets.is_empty(),
        "modal mode targets are owned only by the selected mode"
    );
    assert_eq!(
        stack_item.chosen_modes[0].mode_id,
        tricerules_card_model::ModeId::new("double_any_permanent").unwrap()
    );
    assert_eq!(stack_item.chosen_modes[0].targets[0].object_id, target);
    assert_eq!(
        stack_item.chosen_modes[0].targets[0].zone_change_generation,
        Some(generation)
    );
}

#[test]
fn activated_modal_legal_offers_are_private_and_keep_targets_mode_local() {
    let (engine, source, counterless_permanent) = activated_modal_fixture(602_402);
    assert!(engine.state.objects[&counterless_permanent]
        .counters
        .is_empty());
    let mut batch = RuledEventBatch::default();
    super::legal_actions::fill_legal(&mut batch, &engine);

    let key = u64::from(source) << 32;
    let player_modes = batch.legal_by_player[&10]
        .activated_ability_modes
        .get(&key)
        .expect("the active player receives the engine-authored mode offer");
    assert!(
        !batch.legal_by_player[&20]
            .activated_ability_modes
            .contains_key(&key),
        "the opponent cannot see another player's activation offer"
    );
    assert_eq!(player_modes.modes.len(), 3);

    let targeted_mode = &player_modes.modes[0];
    assert!(targeted_mode.needs_target);
    assert!(targeted_mode.selectable);
    let target_group = &targeted_mode
        .targets
        .as_ref()
        .expect("each mode has an engine-owned target table")
        .groups[0];
    let expected_generation = engine
        .payment_object_ref(counterless_permanent)
        .zone_change_generation;
    assert!(target_group
        .valid_permanent_ids
        .contains(&counterless_permanent));
    assert!(target_group
        .valid_permanent_targets
        .iter()
        .any(|candidate| {
            candidate.object_id == counterless_permanent
                && candidate.zone_change_generation == expected_generation
        }));

    for targetless_mode in &player_modes.modes[1..] {
        assert!(targetless_mode.selectable);
        assert!(!targetless_mode.needs_target);
        assert!(targetless_mode
            .targets
            .as_ref()
            .expect("targetless mode has an empty target table")
            .groups
            .is_empty());
    }
}

#[test]
fn nonmodal_activation_rejects_selected_modes_before_mana_cost() {
    let (mut engine, _) = choice_engine();
    let source = place_ready(&mut engine, 10, "Payment Creature");
    let source_generation = engine.payment_object_ref(source).zone_change_generation;
    let before = serde_json::to_value(&engine.state).expect("serialize state before rejection");
    let error = engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: source,
                        ability_index: 0,
                        expected_zone_change_generation: source_generation,
                        selected_modes: vec![rv1::SelectedSpellMode {
                            mode_index: 0,
                            targets: Vec::new(),
                        }],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect_err("nonmodal ability cannot accept selected modes");
    assert_eq!(
        error.to_string(),
        "illegal command: selected_modes given for a nonmodal activated ability"
    );
    assert_eq!(
        serde_json::to_value(&engine.state).expect("serialize state after rejection"),
        before
    );
    assert!(!engine.state.players[0].graveyard.contains(&source));
}

fn concede(engine: &mut GameEngine, player: PlayerId) -> RuledEventBatch {
    engine
        .apply_scenario_command(
            player,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap()
}

#[test]
fn authored_battlefield_chosen_opponent_uses_the_staged_target_and_exact_owner() {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            AUTHORED_OPPONENT_CHOICE_LAND,
        ],
        &[],
    )
    .expect("authored opponent-choice ability compiles");
    let decks = (0..2)
        .map(|_| EngineDeck {
            mainboard: vec!["forest".into(); 40],
            commanders: Vec::new(),
        })
        .collect();
    let mut engine = GameEngine::new_with_registry(
        602_302,
        &[10, 20],
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .expect("engine");
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 10, "Authored Opponent Choice Land");
    let own_creature = place_ready(&mut engine, 10, "Grizzly Bears");
    let opponent_creature = place_ready(&mut engine, 20, "Grizzly Bears");
    engine.state.players[0].mana_pool.colorless = 3;

    let source_generation = engine.payment_object_ref(source).zone_change_generation;
    let own_generation = engine
        .payment_object_ref(own_creature)
        .zone_change_generation;
    let begin = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(
            rv1::BeginAbilityActivation {
                source_object_id: source,
                expected_zone_change_generation: source_generation,
                ability_index: 0,
                source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                own_target: Some(rv1::AbilityActivationTarget {
                    object_id: own_creature,
                    zone_change_generation: own_generation,
                    group_index: 0,
                }),
                opponent_player_id: Some(20),
            },
        )),
    };
    let batch = engine
        .apply_scenario_command(10, &begin)
        .expect("controller announces the authored battlefield ability");
    let pending_for_opponent = batch.legal_by_player[&20]
        .pending_ability_activation
        .as_ref()
        .expect("the chosen opponent receives the target group");
    assert_eq!(
        pending_for_opponent.deciding_player_id, 20,
        "only the chosen opponent decides the authored target group"
    );
    let candidate = pending_for_opponent
        .target_candidates
        .iter()
        .find(|target| target.object_id == opponent_creature && target.group_index == 1)
        .cloned()
        .expect("the authored second target binding is offered in group one");

    let occurrence = engine
        .pending_ability_activation_internal
        .as_ref()
        .expect("the announcement is captured before costs")
        .effective
        .occurrence
        .clone();
    let crate::state::ActivatedAbilityOccurrence::Printed(definition) = &occurrence else {
        panic!("the authored ability must retain its printed exact definition")
    };
    assert_eq!(definition.card_id, "authored_opponent_choice_land");
    assert_eq!(definition.face_id.as_str(), "authored_opponent_choice_land");
    assert_eq!(definition.ability_path[0].as_str(), "activated_01");
    assert!(engine
        .registry
        .compiled_program_for_owner(&tricerules_card_model::ProgramOwnerId::ability(
            definition.clone(),
        ))
        .is_some());

    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    engine
        .apply_scenario_command(
            20,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                    rv1::SubmitAbilityActivationChoice {
                        transaction_id: pending.transaction_id,
                        expected_revision: pending.revision,
                        target: Some(candidate),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("the opponent supplies the second target");
    assert_eq!(
        engine
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .stage(),
        rv1::AbilityActivationStage::Payment
    );

    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    rv1::CommitAbilityActivation {
                        transaction_id: pending.transaction_id,
                        expected_revision: pending.revision,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("the controller pays and commits the announced ability");
    let stack_item = engine.state.stack.last().expect("ability is on the stack");
    assert_eq!(
        stack_item.activated_ability_occurrence.as_ref(),
        Some(&occurrence)
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&source].tapped);

    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(
                actor,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("pass priority to resolve the staged authored ability");
    }
    assert_eq!(engine.state.objects[&own_creature].damage, 1);
    assert_eq!(engine.state.objects[&opponent_creature].damage, 1);
}

#[test]
fn arena_staged_departing_chooser_refreshes_explicit_actor_choice() {
    let (mut engine, legacy) = choice_engine_players(&[10, 20, 30, 40]);
    place_ready(&mut engine, 30, "Grizzly Bears");
    place_ready(&mut engine, 40, "Grizzly Bears");
    engine
        .apply_scenario_command(10, &begin_choice(&engine, &legacy))
        .unwrap();
    let batch = concede(&mut engine, 20);
    let pending = engine
        .state
        .pending_ability_activation
        .as_ref()
        .expect("remaining opponents can complete announcement");
    assert_eq!(pending.stage(), rv1::AbilityActivationStage::ChooseOpponent);
    assert_eq!(pending.deciding_player_id, 10);
    assert_eq!(pending.chosen_opponent_id, None);
    assert_eq!(pending.valid_opponent_ids, vec![30, 40]);
    assert_eq!(pending.announced_targets.len(), 1);
    assert!(batch.legal_by_player[&30]
        .pending_ability_activation
        .as_ref()
        .unwrap()
        .valid_opponent_ids
        .is_empty());
    assert!(!engine.state.objects[&legacy.source_object_id].tapped);
    let choose = rv1::SubmitAbilityActivationChoice {
        transaction_id: pending.transaction_id,
        expected_revision: pending.revision,
        opponent_player_id: Some(30),
        ..Default::default()
    };
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                    choose,
                )),
            },
        )
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .deciding_player_id,
        30
    );
}

#[test]
fn arena_staged_actor_or_source_owner_departure_abandons_without_payment() {
    for departed in [10, 30] {
        let (mut engine, legacy) = choice_engine_players(&[10, 20, 30, 40]);
        engine
            .state
            .objects
            .get_mut(&legacy.source_object_id)
            .unwrap()
            .owner = 30;
        engine
            .apply_scenario_command(10, &begin_choice(&engine, &legacy))
            .unwrap();
        concede(&mut engine, departed);
        assert!(
            engine.state.pending_ability_activation.is_none(),
            "departure {departed}"
        );
        assert!(engine.pending_ability_activation_internal.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    }
}

#[test]
fn arena_staged_answered_opponent_departure_retains_fixed_target_and_payable_cost() {
    let (mut engine, legacy) = choice_engine_players(&[10, 20, 30, 40]);
    let commit = announce_both(&mut engine, &legacy);
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(10)
            .crimes_committed,
        0
    );
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(20)
            .crimes_committed,
        0
    );
    concede(&mut engine, 20);
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(10)
            .crimes_committed,
        0
    );
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(20)
            .crimes_committed,
        0
    );
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    assert_eq!(pending.stage(), rv1::AbilityActivationStage::Payment);
    assert_eq!(pending.chosen_opponent_id, Some(20));
    assert_eq!(
        pending.announced_targets[1].object_id,
        legacy.targets[1].object_id
    );
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
            },
        )
        .unwrap();
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(10)
            .crimes_committed,
        1
    );
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(20)
            .crimes_committed,
        0
    );
    for player in [10, 30, 40] {
        engine
            .apply_scenario_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
    assert!(engine.state.objects[&legacy.targets[0].object_id].tapped);
    assert!(engine.state.players[0]
        .battlefield
        .contains(&legacy.targets[0].object_id));
}

#[test]
fn arena_staged_committed_target_requires_the_chosen_opponents_control() {
    let (mut engine, legacy) = choice_engine_players(&[10, 20, 30, 40]);
    let commit = announce_both(&mut engine, &legacy);
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
            },
        )
        .unwrap();
    let opponent = legacy.targets[1].object_id;
    engine.state.objects.get_mut(&opponent).unwrap().controller = 30;
    engine
        .state
        .objects
        .get_mut(&opponent)
        .unwrap()
        .base_controller = 30;
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != opponent);
    engine.state.players[2].battlefield.push(opponent);
    for player in [10, 20, 30, 40] {
        engine
            .apply_scenario_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
    assert!(engine.state.objects[&legacy.targets[0].object_id].tapped);
    assert!(!engine.state.objects[&opponent].tapped);
    assert_eq!(engine.state.objects[&opponent].damage, 0);
    assert!(engine.state.players[0]
        .battlefield
        .contains(&legacy.targets[0].object_id));
}

#[test]
fn arena_legacy_activation_cannot_spoof_opponents_target() {
    let (mut engine, command) = choice_engine();
    let before = serde_json::to_value(&engine.state).unwrap();
    let error = engine
        .activate_ability(10, &command)
        .expect_err("the actor cannot announce the opponent's target through the legacy command");
    assert!(error.to_string().contains("opponent"), "{error}");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
}

#[test]
fn arena_legacy_payment_preview_requires_opponents_announcement() {
    let (engine, command) = choice_engine();
    let before = serde_json::to_value(&engine.state).unwrap();
    let error = match engine.prepare_activation_payment(10, &command) {
        Ok(_) => panic!("the actor cannot open payment before the opponent's announcement"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("opponent"), "{error}");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
}

fn begin_choice(engine: &GameEngine, legacy: &rv1::ActivateAbility) -> RuledCommand {
    let own = engine.payment_object_ref(legacy.targets[0].object_id);
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(
            rv1::BeginAbilityActivation {
                source_object_id: legacy.source_object_id,
                expected_zone_change_generation: legacy.expected_zone_change_generation,
                ability_index: legacy.ability_index,
                source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                own_target: Some(rv1::AbilityActivationTarget {
                    object_id: own.object_id,
                    zone_change_generation: own.zone_change_generation,
                    group_index: 0,
                }),
                opponent_player_id: Some(20),
            },
        )),
    }
}

fn announce_both(
    engine: &mut GameEngine,
    legacy: &rv1::ActivateAbility,
) -> rv1::CommitAbilityActivation {
    let begin = begin_choice(engine, legacy);
    let batch = engine.apply_scenario_command(10, &begin).unwrap();
    let pending = batch.legal_by_player[&20]
        .pending_ability_activation
        .as_ref()
        .unwrap();
    let reply = rv1::SubmitAbilityActivationChoice {
        transaction_id: pending.transaction_id,
        expected_revision: pending.revision,
        target: Some(pending.target_candidates[0]),
        ..Default::default()
    };
    engine
        .apply_scenario_command(
            20,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                    reply,
                )),
            },
        )
        .unwrap();
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    rv1::CommitAbilityActivation {
        transaction_id: pending.transaction_id,
        expected_revision: pending.revision,
        ..Default::default()
    }
}

#[test]
fn arena_staged_locks_nonzero_tax_and_conditional_reduction() {
    for reduced in [false, true] {
        let (mut engine, mut legacy) = choice_engine();
        let own = legacy.targets[0].object_id;
        engine.state.objects.get_mut(&own).unwrap().card_id = if reduced {
            "payment_creature"
        } else {
            "taxed_payment_creature"
        }
        .into();
        if reduced {
            engine
                .state
                .objects
                .get_mut(&own)
                .unwrap()
                .set_counter(tricerules_card_model::CounterKind::PlusOnePlusOne, 1);
            engine
                .state
                .objects
                .get_mut(&legacy.source_object_id)
                .unwrap()
                .card_id = "reduced_choice_land".into();
        }
        engine.state.players[0].mana_pool.colorless = if reduced { 0 } else { 2 };
        engine.reconcile_activated_ability_slots();
        legacy.ability_index = engine
            .effective_activated_abilities(legacy.source_object_id)
            .into_iter()
            .find(|a| a.definition.requires_opponent_target_choice())
            .unwrap()
            .slot;
        let mut commit = announce_both(&mut engine, &legacy);
        let locked = if reduced { "{1}" } else { "{5}" };
        assert_eq!(
            engine
                .state
                .pending_ability_activation
                .as_ref()
                .unwrap()
                .locked_total_cost,
            locked
        );
        activate_fixture_mana(&mut engine, own);
        assert!(engine.state.players[0].graveyard.contains(&own));
        assert_eq!(
            engine
                .state
                .pending_ability_activation
                .as_ref()
                .unwrap()
                .locked_total_cost,
            locked
        );
        if reduced {
            let effective = engine
                .effective_activated_abilities(legacy.source_object_id)
                .into_iter()
                .find(|a| a.definition.requires_opponent_target_choice())
                .unwrap();
            let reduction = engine
                .activated_mana_reduction(10, legacy.source_object_id, &effective.definition)
                .unwrap();
            let original = ManaCost {
                pips: vec![ManaSymbol::Generic(3)],
            };
            let (live_cost, extra) =
                payment::apply_activated_mana_reduction(&original, 0, reduction).unwrap();
            assert_eq!(live_cost, original);
            assert_eq!(extra, 0);
        } else {
            assert_eq!(
                engine.targeting_cost_increase(
                    10,
                    TargetingCostAction::ActivatedAbilities,
                    &legacy.targets
                ),
                0
            );
        }
        commit.payment = Some(rv1::PaymentSelection {
            mana: Some(rv1::PaymentMana {
                c: if reduced { 1 } else { 5 },
                ..Default::default()
            }),
            ..Default::default()
        });
        let preview = engine.preview_payment(
            10,
            &rv1::PreviewPayment {
                commit_ability_activation: Some(commit.clone()),
                ..Default::default()
            },
        );
        assert!(preview.error.is_empty(), "{}", preview.error);
        assert!(preview.complete);
        commit.payment = preview.selection;
        engine
            .apply_scenario_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
                },
            )
            .unwrap();
        assert_eq!(
            engine.state.players[0].mana_pool.colorless,
            if reduced { 2 } else { 0 }
        );
        assert!(engine.state.objects[&legacy.source_object_id].tapped);
        assert_eq!(engine.state.stack.len(), 1);
    }
}

fn trigger_receipts(
    engine: &GameEngine,
) -> Vec<(
    ObjectId,
    tricerules_card_model::TriggerCondition,
    crate::state::TriggerContext,
)> {
    let mut receipts = Vec::new();
    for trigger in engine
        .state
        .staged_trigger_groups
        .iter()
        .flat_map(|g| g.triggers.iter())
        .chain(
            engine
                .state
                .pending_trigger_order
                .iter()
                .flat_map(|p| p.candidates.iter()),
        )
    {
        receipts.push((
            trigger.source_permanent_id,
            trigger.ability.trigger.clone(),
            trigger.trigger_context,
        ));
    }
    for item in &engine.state.stack {
        if let Some(ability) = &item.triggered_ability {
            receipts.push((
                item.source_permanent_id.unwrap(),
                ability.trigger.clone(),
                item.trigger_context,
            ));
        }
    }
    receipts
}

#[test]
fn arena_staged_snapshots_target_observers_repoints_allocated_id_and_commits_crime_once() {
    for finish in [false, true] {
        let (mut engine, legacy) = choice_engine();
        let own = legacy.targets[0].object_id;
        let opponent = legacy.targets[1].object_id;
        for oid in [own, opponent] {
            engine.state.objects.get_mut(&oid).unwrap().card_id =
                "snapshot_payment_creature".into();
        }
        let watcher = place_ready(&mut engine, 10, "Snapshot Payment Creature");
        engine.state.players[0].mana_pool.colorless = 0;
        engine.reconcile_activated_ability_slots();
        let reserved = engine.state.next_object_id;
        let commit = announce_both(&mut engine, &legacy);
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(10)
                .crimes_committed,
            0
        );
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(20)
                .crimes_committed,
            0
        );
        activate_fixture_mana(&mut engine, own);
        assert!(engine.state.players[0].graveyard.contains(&own));
        assert!(engine.state.next_object_id > reserved);
        assert!(trigger_receipts(&engine)
            .iter()
            .any(|(source, condition, _)| *source == own
                && matches!(
                    condition,
                    tricerules_card_model::TriggerCondition::WhenSelfLeavesBattlefield
                )));
        assert!(trigger_receipts(&engine)
            .iter()
            .all(|(_, condition, _)| !matches!(
                condition,
                tricerules_card_model::TriggerCondition::WheneverSelfBecomesTarget { .. }
            )));
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(10)
                .crimes_committed,
            0
        );
        let command = if finish {
            rv1::ruled_command::Cmd::CommitAbilityActivation(commit.clone())
        } else {
            rv1::ruled_command::Cmd::CancelAbilityActivation(rv1::CancelAbilityActivation {
                transaction_id: commit.transaction_id,
                expected_revision: commit.expected_revision,
            })
        };
        engine
            .apply_scenario_command(10, &RuledCommand { cmd: Some(command) })
            .unwrap();
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(10)
                .crimes_committed,
            u32::from(finish)
        );
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(20)
                .crimes_committed,
            0
        );
        let receipts = trigger_receipts(&engine);
        let mut targets: Vec<_> = receipts
            .iter()
            .filter(|(_, condition, _)| {
                matches!(
                    condition,
                    tricerules_card_model::TriggerCondition::WheneverSelfBecomesTarget { .. }
                )
            })
            .collect();
        targets.sort_by_key(|(source, _, _)| *source);
        if finish {
            let ability = engine
                .state
                .stack
                .iter()
                .find(|item| {
                    !item.is_triggered && item.source_permanent_id == Some(legacy.source_object_id)
                })
                .unwrap();
            assert_ne!(ability.id, reserved);
            assert_eq!(
                targets
                    .iter()
                    .map(|(source, _, _)| *source)
                    .collect::<Vec<_>>(),
                vec![own, opponent]
            );
            for (_, _, context) in targets {
                assert_eq!(
                    context.targeting_stack_object,
                    Some(crate::state::StackObjectRef {
                        object_id: ability.id,
                        zone_change_generation: None
                    })
                );
            }
            assert_eq!(
                receipts
                    .iter()
                    .filter(|(source, condition, _)| *source == watcher
                        && matches!(
                            condition,
                            tricerules_card_model::TriggerCondition::WheneverPlayerCommitsCrime { .. }
                        ))
                    .count(),
                1
            );
            assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        } else {
            assert!(targets.is_empty());
            assert!(receipts.iter().all(|(_, condition, _)| !matches!(
                condition,
                tricerules_card_model::TriggerCondition::WheneverPlayerCommitsCrime { .. }
            )));
            assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
            assert!(!engine.state.objects[&legacy.source_object_id].tapped);
        }
    }
}

#[test]
fn arena_staged_unrelated_departure_refreshes_candidates_without_repicking_receipts() {
    let (mut engine, legacy) = choice_engine_players(&[10, 20, 30, 40]);
    let foreign = place_ready(&mut engine, 20, "Grizzly Bears");
    engine.state.objects.get_mut(&foreign).unwrap().owner = 30;
    engine
        .apply_scenario_command(10, &begin_choice(&engine, &legacy))
        .unwrap();
    let before = engine
        .state
        .pending_ability_activation
        .as_ref()
        .unwrap()
        .clone();
    assert!(before
        .target_candidates
        .iter()
        .any(|t| t.object_id == foreign));
    concede(&mut engine, 30);
    let refreshed = engine
        .state
        .pending_ability_activation
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(
        refreshed.stage(),
        rv1::AbilityActivationStage::OpponentTarget
    );
    assert_eq!(refreshed.transaction_id, before.transaction_id);
    assert!(refreshed.revision > before.revision);
    assert_eq!(refreshed.deciding_player_id, 20);
    assert_eq!(refreshed.chosen_opponent_id, Some(20));
    assert_eq!(refreshed.announced_targets, before.announced_targets);
    assert!(!engine.state.objects.contains_key(&foreign));
    assert_eq!(refreshed.target_candidates.len(), 1);
    let target = refreshed.target_candidates[0];
    assert_eq!(
        &target,
        before
            .target_candidates
            .iter()
            .find(|t| t.object_id == legacy.targets[1].object_id)
            .unwrap()
    );
    let state = format!("{:?}", engine.state);
    engine
        .apply_scenario_command(
            20,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                    rv1::SubmitAbilityActivationChoice {
                        transaction_id: before.transaction_id,
                        expected_revision: before.revision,
                        target: Some(
                            *before
                                .target_candidates
                                .iter()
                                .find(|t| t.object_id == foreign)
                                .unwrap(),
                        ),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect_err("departed candidate and superseded revision");
    assert_eq!(format!("{:?}", engine.state), state);
    engine
        .apply_scenario_command(
            20,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                    rv1::SubmitAbilityActivationChoice {
                        transaction_id: refreshed.transaction_id,
                        expected_revision: refreshed.revision,
                        target: Some(target),
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    let paid = engine
        .state
        .pending_ability_activation
        .as_ref()
        .unwrap()
        .clone();
    concede(&mut engine, 40);
    let retained = engine.state.pending_ability_activation.as_ref().unwrap();
    assert_eq!(retained.stage(), rv1::AbilityActivationStage::Payment);
    assert_eq!(retained.transaction_id, paid.transaction_id);
    assert_eq!(retained.revision, paid.revision);
    assert_eq!(retained.chosen_opponent_id, paid.chosen_opponent_id);
    assert_eq!(retained.announced_targets, paid.announced_targets);
    assert_eq!(retained.locked_total_cost, paid.locked_total_cost);
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    rv1::CommitAbilityActivation {
                        transaction_id: paid.transaction_id,
                        expected_revision: paid.revision,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    for player in [10, 20] {
        engine
            .apply_scenario_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
    assert!(engine.state.players[0]
        .graveyard
        .contains(&legacy.targets[0].object_id));
    assert!(engine.state.players[1]
        .graveyard
        .contains(&legacy.targets[1].object_id));
}

#[test]
fn arena_committed_ability_survives_foreign_source_owner_departure() {
    let (mut engine, legacy) = choice_engine_players(&[10, 20, 30, 40]);
    let source = legacy.source_object_id;
    engine.state.objects.get_mut(&source).unwrap().owner = 30;
    let commit = announce_both(&mut engine, &legacy);
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
            },
        )
        .unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let ability = engine
        .state
        .stack
        .iter()
        .find(|item| !item.is_triggered && item.source_permanent_id == Some(source))
        .unwrap();
    let ability_id = ability.id;
    assert_eq!(ability.controller, 10);
    assert_eq!(ability.source_owner, Some(30));
    assert_ne!(ability_id, source);
    concede(&mut engine, 30);
    assert!(!engine.state.objects.contains_key(&source));
    assert!(engine
        .state
        .players
        .iter()
        .all(|p| !p.battlefield.contains(&source)));
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == ability_id && item.controller == 10));
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    for player in [10, 20, 40] {
        engine
            .apply_scenario_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
    assert!(!engine.state.objects.contains_key(&source));
    assert!(engine.state.players[0]
        .graveyard
        .contains(&legacy.targets[0].object_id));
    assert!(engine.state.players[1]
        .graveyard
        .contains(&legacy.targets[1].object_id));
}

fn activate_fixture_mana(engine: &mut GameEngine, source: ObjectId) -> RuledEventBatch {
    let effective = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|a| a.definition.is_mana_ability())
        .unwrap();
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: source,
                        ability_index: effective.slot,
                        expected_zone_change_generation: engine
                            .payment_object_ref(source)
                            .zone_change_generation,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap()
}

#[test]
fn arena_staged_new_restricted_mana_group_is_refreshed_without_unlocking_costs() {
    for (name, permitted) in [
        ("Restricted Payment Land", true),
        ("Spell Payment Land", false),
    ] {
        let (mut engine, legacy) = choice_engine();
        let mana_source = place_ready(&mut engine, 10, name);
        engine.state.players[0].mana_pool.colorless = 0;
        let mut commit = announce_both(&mut engine, &legacy);
        assert!(engine
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .eligible_restricted_mana_group_ids
            .is_empty());
        let batch = activate_fixture_mana(&mut engine, mana_source);
        let group = engine.state.players[0]
            .restricted_mana
            .last()
            .unwrap()
            .restriction_group_id;
        let offer = batch.legal_by_player[&10]
            .pending_ability_activation
            .as_ref()
            .unwrap();
        assert_eq!(
            offer.eligible_restricted_mana_group_ids.contains(&group),
            permitted
        );
        assert_eq!(offer.locked_total_cost, "{3}");
        commit.restricted_mana = vec![rv1::ManaSpendSelection {
            restriction_group_id: group,
            c: 3,
            ..Default::default()
        }];
        let before = serde_json::to_value(&engine.state).unwrap();
        let outcome = engine.apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
            },
        );
        if permitted {
            outcome.expect("newly floated permitted group is payable");
            assert!(engine.state.players[0].restricted_mana.is_empty());
            assert!(engine.state.objects[&legacy.source_object_id].tapped);
        } else {
            outcome.expect_err("creature-spell mana cannot pay an ability");
            assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
            assert!(engine.state.objects[&mana_source].tapped);
        }
    }
}

#[test]
fn arena_staged_nested_mana_damage_choice_resumes_same_transaction() {
    let (mut engine, legacy) = choice_engine();
    let source = place_ready(&mut engine, 10, "Pain Payment Land");
    engine.state.players[0].mana_pool.colorless = 0;
    engine.state.add_damage_prevention_shield(10, 1);
    engine.state.add_damage_prevention_shield(10, 1);
    let commit = announce_both(&mut engine, &legacy);
    let batch = activate_fixture_mana(&mut engine, source);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("prevention choice precedes activation payment");
    let choice = pending.presentation.candidates[0];
    assert!(batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .unwrap()
        .payment_preview
        .is_none());
    let before = serde_json::to_value(&engine.state).unwrap();
    for cmd in [
        rv1::ruled_command::Cmd::CommitAbilityActivation(commit.clone()),
        rv1::ruled_command::Cmd::CancelAbilityActivation(rv1::CancelAbilityActivation {
            transaction_id: commit.transaction_id,
            expected_revision: commit.expected_revision,
        }),
    ] {
        engine
            .apply_scenario_command(10, &RuledCommand { cmd: Some(cmd) })
            .expect_err("nested work must finish before outer action");
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    }
    let batch = engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![choice],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(
        batch.legal_by_player[&10]
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .transaction_id,
        commit.transaction_id
    );
    assert!(batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .unwrap()
        .payment_preview
        .is_some());
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
            },
        )
        .unwrap();
    assert_eq!(engine.state.stack.len(), 1);
}

#[test]
fn arena_staged_cancel_retains_completed_mana_and_failed_commit_preserves_undo() {
    let (mut engine, legacy) = choice_engine();
    let source = place_ready(&mut engine, 10, "Forest");
    engine.state.players[0].mana_pool.colorless = 0;
    let commit = announce_both(&mut engine, &legacy);
    activate_fixture_mana(&mut engine, source);
    assert_eq!(engine.state.undoable_mana_abilities.len(), 1);
    let before = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    commit.clone(),
                )),
            },
        )
        .expect_err("one mana is insufficient");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CancelAbilityActivation(
                    rv1::CancelAbilityActivation {
                        transaction_id: commit.transaction_id,
                        expected_revision: commit.expected_revision,
                    },
                )),
            },
        )
        .unwrap();
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert_eq!(engine.state.undoable_mana_abilities.len(), 1);
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::UndoManaAbility(
                    rv1::UndoManaAbility {
                        attack_transaction_id: 0,
                        activation_command_index: 0,
                    },
                )),
            },
        )
        .unwrap();
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
}

#[test]
fn arena_staged_magus_source_sickness_self_target_and_definition_snapshot() {
    for lose_definition in [false, true] {
        let (mut engine, mut legacy) = choice_engine();
        let source = legacy.source_object_id;
        engine.state.objects.get_mut(&source).unwrap().card_id = "opponent_choice_magus".into();
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .summoning_sick = true;
        engine.reconcile_activated_ability_slots();
        legacy.ability_index = engine
            .effective_activated_abilities(source)
            .into_iter()
            .find(|a| a.definition.requires_opponent_target_choice())
            .unwrap()
            .slot;
        legacy.targets[0].object_id = source;
        let before = serde_json::to_value(&engine.state).unwrap();
        engine
            .apply_scenario_command(10, &begin_choice(&engine, &legacy))
            .expect_err("Magus tap cost requires haste or nonsick source");
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .summoning_sick = false;
        let commit = announce_both(&mut engine, &legacy);
        if lose_definition {
            engine.state.objects.get_mut(&source).unwrap().card_id = "forest".into();
            engine.reconcile_activated_ability_slots();
        }
        engine
            .apply_scenario_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
                },
            )
            .unwrap();
        assert_eq!(engine.state.stack[0].card_id, "opponent_choice_magus");
        assert_eq!(engine.state.stack[0].targets[0].object_id, source);
        assert!(engine.state.objects[&source].tapped);
        assert!(engine.state.stack[0]
            .activated_ability
            .as_ref()
            .unwrap()
            .requires_opponent_target_choice());
        if !lose_definition {
            for player in [10, 20] {
                engine
                    .apply_scenario_command(
                        player,
                        &RuledCommand {
                            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                        },
                    )
                    .unwrap();
            }
            assert_eq!(engine.state.objects[&source].damage, 2);
            assert!(engine.state.players[1]
                .graveyard
                .contains(&legacy.targets[1].object_id));
        }
    }
}

#[test]
fn arena_staged_paid_commit_and_preview_preserve_rejected_state() {
    let (mut engine, legacy) = choice_engine();
    let mut commit = announce_both(&mut engine, &legacy);
    commit.payment = Some(rv1::PaymentSelection {
        mana: Some(rv1::PaymentMana {
            c: 3,
            ..Default::default()
        }),
        ..Default::default()
    });
    let preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            commit_ability_activation: Some(commit.clone()),
            ..Default::default()
        },
    );
    assert!(preview.error.is_empty(), "{}", preview.error);
    assert!(preview.complete);
    let before = serde_json::to_value(&engine.state).unwrap();
    let cmd = |commit| RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
    };
    engine
        .apply_scenario_command(20, &cmd(commit.clone()))
        .expect_err("only the payer commits");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    let mut stale = commit.clone();
    stale.expected_revision += 1;
    engine
        .apply_scenario_command(10, &cmd(stale))
        .expect_err("stale revision");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    commit.payment = preview.selection;
    let batch = engine
        .apply_scenario_command(10, &cmd(commit))
        .expect("pay and finish the announcement");
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
    assert!(engine.state.objects[&legacy.source_object_id].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].targets.len(), 2);
    assert!(batch
        .legal_by_player
        .values()
        .all(|legal| legal.pending_ability_activation.is_none()));
    for player in [10, 20] {
        engine
            .apply_scenario_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.players[0]
        .graveyard
        .contains(&legacy.targets[0].object_id));
    assert!(engine.state.players[1]
        .graveyard
        .contains(&legacy.targets[1].object_id));
}

#[test]
fn arena_staged_mana_sacrifice_keeps_announced_incarnation_and_partial_resolution() {
    let (mut engine, legacy) = choice_engine();
    let own = legacy.targets[0].object_id;
    engine.state.objects.get_mut(&own).unwrap().card_id = "payment_creature".into();
    engine.state.players[0].mana_pool.colorless = 0;
    engine.reconcile_activated_ability_slots();
    let generation = engine.payment_object_ref(own).zone_change_generation;
    let mana_slot = engine
        .effective_activated_abilities(own)
        .into_iter()
        .find(|ability| ability.definition.is_mana_ability())
        .unwrap()
        .slot;
    let mana = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
            rv1::ActivateAbility {
                source_object_id: own,
                ability_index: mana_slot,
                expected_zone_change_generation: generation,
                ..Default::default()
            },
        )),
    };
    engine
        .apply_scenario_command(10, &begin_choice(&engine, &legacy))
        .unwrap();
    let before = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_scenario_command(10, &mana)
        .expect_err("no mana window before opponent answer");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    let reply = rv1::SubmitAbilityActivationChoice {
        transaction_id: pending.transaction_id,
        expected_revision: pending.revision,
        target: Some(pending.target_candidates[0]),
        ..Default::default()
    };
    let batch = engine
        .apply_scenario_command(
            20,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                    reply,
                )),
            },
        )
        .unwrap();
    let key = (own as u64) << 32 | u64::from(mana_slot);
    assert!(
        batch.legal_by_player[&10]
            .mana_payment_by_ability
            .contains_key(&key),
        "payer sees legal mana actions"
    );
    assert!(batch.legal_by_player[&20]
        .mana_payment_by_ability
        .is_empty());
    engine
        .apply_scenario_command(10, &mana)
        .expect("sacrifice chosen own creature for mana after cost lock");
    assert!(engine.state.players[0].graveyard.contains(&own));
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert!(engine.state.stack.is_empty());
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    let commit = rv1::CommitAbilityActivation {
        transaction_id: pending.transaction_id,
        expected_revision: pending.revision,
        ..Default::default()
    };
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
            },
        )
        .unwrap();
    assert_eq!(
        engine.state.stack[0].targets[0].zone_change_generation,
        Some(generation)
    );
    assert_ne!(
        engine.payment_object_ref(own).zone_change_generation,
        generation
    );
    for player in [10, 20] {
        engine
            .apply_scenario_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
    let opponent = legacy.targets[1].object_id;
    assert!(engine.state.players[1].battlefield.contains(&opponent));
    assert!(engine.state.objects[&opponent].tapped);
    assert_eq!(engine.state.objects[&opponent].damage, 0);
}

fn pending_staged_loyalty_cost_replacement() -> (
    GameEngine,
    ObjectId,
    u64,
    u64,
    usize,
    crate::state::ManaPool,
) {
    let (mut engine, legacy) = choice_engine();
    let walker = place_ready(&mut engine, 10, "Loyalty Choice Walker");
    place_ready(&mut engine, 10, "Lae'zel, Vlaakith's Champion");
    for _ in 0..3 {
        place_ready(&mut engine, 10, "Doubling Season");
    }
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .counters
        .insert(CounterKind::Loyalty, 5);
    let generation = engine.payment_object_ref(walker).zone_change_generation;
    let begin = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(
            rv1::BeginAbilityActivation {
                source_object_id: walker,
                expected_zone_change_generation: generation,
                ability_index: 0,
                source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                own_target: Some(rv1::AbilityActivationTarget {
                    object_id: legacy.targets[0].object_id,
                    zone_change_generation: engine
                        .payment_object_ref(legacy.targets[0].object_id)
                        .zone_change_generation,
                    group_index: 0,
                }),
                opponent_player_id: Some(20),
            },
        )),
    };
    engine
        .apply_scenario_command(10, &begin)
        .expect("announce ability");
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    let opponent_choice = rv1::SubmitAbilityActivationChoice {
        transaction_id: pending.transaction_id,
        expected_revision: pending.revision,
        target: Some(pending.target_candidates[0]),
        ..Default::default()
    };
    engine
        .apply_scenario_command(
            20,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                    opponent_choice,
                )),
            },
        )
        .expect("opponent selects the second target");

    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    let transaction_id = pending.transaction_id;
    let expected_revision = pending.revision;
    let mut commit = rv1::CommitAbilityActivation {
        transaction_id,
        expected_revision,
        payment: Some(rv1::PaymentSelection {
            mana: Some(rv1::PaymentMana {
                c: 3,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            commit_ability_activation: Some(commit.clone()),
            ..Default::default()
        },
    );
    assert!(preview.error.is_empty(), "{}", preview.error);
    assert!(preview.complete);
    commit.payment = preview.selection;
    let stack_len = engine.state.stack.len();
    let mana_before = engine.state.players[0].mana_pool;
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    commit.clone(),
                )),
            },
        )
        .expect("open replacement choice before payment");
    assert!(engine.state.pending_resolution.is_some());
    (
        engine,
        walker,
        transaction_id,
        expected_revision,
        stack_len,
        mana_before,
    )
}

#[test]
fn staged_positive_loyalty_cost_waits_for_replacement_order_before_payment_commit() {
    let (mut engine, walker, transaction_id, expected_revision, stack_len, mana_before) =
        pending_staged_loyalty_cost_replacement();
    let commit = rv1::CommitAbilityActivation {
        transaction_id,
        expected_revision,
        ..Default::default()
    };

    for expected_choices in [3, 2] {
        let (deciding_player, choice_kind, candidate_count, selected) = {
            let choice = engine.state.pending_resolution.as_ref().unwrap();
            (
                choice.deciding_player,
                choice.presentation.choice_kind,
                choice.presentation.candidates.len(),
                choice.presentation.candidates[0],
            )
        };
        assert_eq!(deciding_player, 10);
        assert_eq!(choice_kind, rv1::ChoiceKind::ReplacementEffect);
        assert_eq!(candidate_count, expected_choices);
        assert!(!engine.state.objects[&walker].tapped);
        assert_eq!(
            engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
            5
        );
        assert_eq!(engine.state.players[0].mana_pool, mana_before);
        assert_eq!(engine.state.stack.len(), stack_len);
        assert!(engine.state.pending_ability_activation.is_some());
        assert!(engine.pending_ability_activation_internal.is_some());
        serde_json::to_value(&engine.state).expect("the parked cost choice stays serializable");

        let before_invalid = serde_json::to_value(&engine.state).unwrap();
        engine
            .apply_scenario_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CancelAbilityActivation(
                        rv1::CancelAbilityActivation {
                            transaction_id,
                            expected_revision,
                        },
                    )),
                },
            )
            .expect_err("a staged payment cannot be canceled during replacement order");
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before_invalid);
        engine
            .apply_scenario_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                        commit.clone(),
                    )),
                },
            )
            .expect_err("a staged payment cannot be committed twice");
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before_invalid);
        engine
            .apply_scenario_command(
                20,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![selected],
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect_err("only the affected player's replacement choice is accepted");
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before_invalid);
        engine
            .apply_scenario_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![selected],
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("choose the next replacement effect");
    }

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
    assert!(engine.pending_counter_cost_internal.is_none());
    assert!(engine.state.objects[&walker].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        21
    );
    assert_eq!(engine.state.stack.len(), stack_len + 1);
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(walker));
}

#[test]
fn failed_staged_loyalty_payment_does_not_commit_counter_tap_or_stack() {
    let (mut engine, walker, _transaction_id, _expected_revision, stack_len, _) =
        pending_staged_loyalty_cost_replacement();
    let choice = engine.state.pending_resolution.as_ref().unwrap();
    let selected = choice.presentation.candidates[0];
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![selected],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("first replacement application remains staged");
    engine.state.players[0].mana_pool.colorless = 2;
    let choice = engine.state.pending_resolution.as_ref().unwrap();
    let selected = choice.presentation.candidates[0];
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![selected],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect_err("revalidated mana payment must fail before any cost is committed");
    assert!(!engine.state.objects[&walker].tapped);
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert_eq!(engine.state.stack.len(), stack_len);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
    assert!(engine.pending_counter_cost_internal.is_none());
}

#[test]
fn arena_staged_locked_tap_cost_rejects_changed_source_and_failed_payment_atomically() {
    for fault in 0..5 {
        let (mut engine, legacy) = choice_engine();
        let commit = announce_both(&mut engine, &legacy);
        let source = engine
            .state
            .objects
            .get_mut(&legacy.source_object_id)
            .unwrap();
        match fault {
            0 => engine.state.players[0].mana_pool.colorless = 2,
            1 => source.tapped = true,
            2 => source.controller = 20,
            3 => {
                source.card_id = "grizzly_bears".into();
                source.summoning_sick = true;
            }
            _ => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(source.id)
                    .or_default() += 1
            }
        }
        let before = serde_json::to_value(&engine.state).unwrap();
        engine
            .apply_scenario_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
                },
            )
            .expect_err("invalid cost cannot debit anything");
        assert_eq!(
            serde_json::to_value(&engine.state).unwrap(),
            before,
            "fault {fault}"
        );
        assert!(engine.pending_ability_activation_internal.is_some());
    }
}

#[test]
fn arena_staged_announcement_hands_target_choice_to_opponent_before_payment() {
    let (mut engine, legacy) = choice_engine();
    let begin = begin_choice(&engine, &legacy);
    let batch = engine
        .apply_scenario_command(10, &begin)
        .expect("begin announced ability");
    let actor = batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .unwrap();
    let opponent = batch.legal_by_player[&20]
        .pending_ability_activation
        .as_ref()
        .unwrap();
    assert_eq!(
        opponent.stage(),
        rv1::AbilityActivationStage::OpponentTarget
    );
    assert_eq!(actor.deciding_player_id, 20);
    assert_eq!(actor.actor_player_id, 10);
    assert_eq!(actor.announced_targets.len(), 1);
    assert_eq!(
        actor.announced_targets[0].object_id,
        legacy.targets[0].object_id
    );
    assert!(actor.target_candidates.is_empty());
    assert!(actor.target_group.is_none());
    assert!(actor.payment_preview.is_none());
    assert_eq!(opponent.target_candidates.len(), 1);
    assert_eq!(
        opponent.target_candidates[0].object_id,
        legacy.targets[1].object_id
    );
    assert!(opponent.target_group.as_ref().unwrap().chosen_by_opponent);
    assert!(!engine.state.objects[&legacy.source_object_id].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert!(engine.state.stack.is_empty());
    assert!(!batch
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::StackPushed(_)))));

    let answer = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
            rv1::SubmitAbilityActivationChoice {
                transaction_id: opponent.transaction_id,
                expected_revision: opponent.revision,
                opponent_player_id: None,
                target: Some(opponent.target_candidates[0]),
            },
        )),
    };
    let before = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_scenario_command(10, &answer)
        .expect_err("actor cannot answer for opponent");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
            },
        )
        .expect_err("the ability announcement has no priority window");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);

    let batch = engine
        .apply_scenario_command(20, &answer)
        .expect("opponent announces their target");
    let actor = batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .unwrap();
    let opponent = batch.legal_by_player[&20]
        .pending_ability_activation
        .as_ref()
        .unwrap();
    assert_eq!(actor.stage(), rv1::AbilityActivationStage::Payment);
    assert_eq!(actor.deciding_player_id, 10);
    assert_eq!(actor.announced_targets.len(), 2);
    assert_eq!(
        actor.announced_targets[1].object_id,
        legacy.targets[1].object_id
    );
    assert!(!actor.locked_total_cost.is_empty());
    assert!(opponent.locked_total_cost.is_empty());
    assert!(opponent.target_candidates.is_empty());
    assert!(!engine.state.objects[&legacy.source_object_id].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn arena_staged_choice_has_no_normal_action_offers() {
    let (mut engine, legacy) = choice_engine();
    let batch = engine
        .apply_scenario_command(10, &begin_choice(&engine, &legacy))
        .unwrap();
    for legal in batch.legal_by_player.values() {
        assert!(legal.hand_actions.is_empty());
        assert!(legal.zone_cast_actions.is_empty());
        assert!(legal.permanent_actions.is_empty());
        assert!(
            legal.valid_targets_by_ability.is_empty(),
            "normal ability offers must yield to the exclusive target choice"
        );
        assert!(legal.cost_choices_by_ability.is_empty());
        assert!(legal.mana_payment_by_ability.is_empty());
    }
}

#[test]
fn arena_staged_terminal_game_abandons_announcement() {
    let (mut engine, legacy) = choice_engine();
    engine
        .apply_scenario_command(10, &begin_choice(&engine, &legacy))
        .unwrap();
    let batch = engine
        .apply_scenario_command(
            20,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    assert!(engine.state.is_terminal());
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
    assert!(batch.legal_by_player.is_empty());
    assert!(!engine.state.objects[&legacy.source_object_id].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
}

#[test]
fn arena_staged_invalid_target_answers_are_atomic_and_actor_can_cancel() {
    let (mut engine, legacy) = choice_engine();
    let batch = engine
        .apply_scenario_command(10, &begin_choice(&engine, &legacy))
        .unwrap();
    let pending = batch.legal_by_player[&20]
        .pending_ability_activation
        .as_ref()
        .unwrap();
    let valid = rv1::SubmitAbilityActivationChoice {
        transaction_id: pending.transaction_id,
        expected_revision: pending.revision,
        target: Some(pending.target_candidates[0]),
        opponent_player_id: None,
    };
    let mut invalid = Vec::new();
    let mut own = valid;
    own.target.as_mut().unwrap().object_id = legacy.targets[0].object_id;
    invalid.push(own);
    let mut group = valid;
    group.target.as_mut().unwrap().group_index = 0;
    invalid.push(group);
    let mut generation = valid;
    generation.target.as_mut().unwrap().zone_change_generation += 1;
    invalid.push(generation);
    let mut transaction = valid;
    transaction.transaction_id += 1;
    invalid.push(transaction);
    let mut revision = valid;
    revision.expected_revision += 1;
    invalid.push(revision);
    let mut change_opponent = valid;
    change_opponent.opponent_player_id = Some(10);
    invalid.push(change_opponent);
    let before = serde_json::to_value(&engine.state).unwrap();
    for reply in invalid {
        engine
            .apply_scenario_command(
                20,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                        reply,
                    )),
                },
            )
            .expect_err("invalid replies cannot change the announcement or costs");
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    }
    let cancel = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CancelAbilityActivation(
            rv1::CancelAbilityActivation {
                transaction_id: valid.transaction_id,
                expected_revision: valid.expected_revision,
            },
        )),
    };
    engine
        .apply_scenario_command(20, &cancel)
        .expect_err("only the actor may cancel");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    let batch = engine.apply_scenario_command(10, &cancel).unwrap();
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.state.blocking_choice().is_none());
    assert!(batch
        .legal_by_player
        .values()
        .all(|legal| legal.pending_ability_activation.is_none()));
    assert!(!engine.state.objects[&legacy.source_object_id].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
}
