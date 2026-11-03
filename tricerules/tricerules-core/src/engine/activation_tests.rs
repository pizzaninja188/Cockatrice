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

const SYNTHETIC_ACTIVATED_REVEAL_RECEIPT_SOURCE: &str = r#"(
    id: "synthetic_activated_reveal_receipt_source",
    name: "Synthetic Activated Reveal Receipt Source",
    face_id: "synthetic_activated_reveal_receipt_source",
    types: ["Artifact"],
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
        costs: [], effect: [],
        program: Some((body: (steps: [
            (id: "reveal_top", bind: ["revealed_card"],
                kind: Perform(RevealTopCardToHand)),
            (id: "lose_life", input: "revealed_card",
                kind: Perform(LoseLife(amount: RevealedCardManaValue, who: EachOpponent)))
        ], yields: []), target_bindings: []))
    )],
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

const CAPTURED_ARTIFACT_PAYMENT_LAND: &str = r#"(
    id: "captured_artifact_payment_land", name: "Captured Artifact Payment Land",
    face_id: "captured_artifact_payment_land", types: ["Land"],
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
        costs: [Tap], effect: [ProduceMana(options: [(c: 1)], restriction: Some((
            restriction_id: "restriction_01", presentation: Fallback,
            activate_ability: [(card_type: Some(Artifact))],
        )))],
    )],
)"#;

const CAPTURED_ARTIFACT_SPELL_PAYMENT_LAND: &str = r#"(
    id: "captured_artifact_spell_payment_land", name: "Captured Artifact Spell Payment Land",
    face_id: "captured_artifact_spell_payment_land", types: ["Land"],
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
        costs: [Tap], effect: [ProduceMana(options: [(c: 1)], restriction: Some((
            restriction_id: "restriction_01", presentation: Fallback,
            cast_spell: [(card_type: Some(Artifact))],
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

const LOYALTY_X_TEST_WALKER: &str = r#"(
    id: "loyalty_x_test_walker", name: "Loyalty X Test Walker",
    face_id: "loyalty_x_test_walker", supertypes: ["Legendary"],
    types: ["Planeswalker"], loyalty: 6,
    activated_abilities: [(ability_id: "minus_x", presentation: Fallback,
        costs: [LoyaltyX], effect: [GainLife(amount: "X", who: Controller)])],
)"#;

const LOYALTY_X_EXILE_TEST_WALKER: &str = r#"(
    id: "loyalty_x_exile_test_walker", name: "Loyalty X Exile Test Walker",
    face_id: "loyalty_x_exile_test_walker", supertypes: ["Legendary"],
    types: ["Planeswalker"], loyalty: 1,
    activated_abilities: [(ability_id: "minus_x", presentation: Fallback,
        costs: [LoyaltyX], effect: [ExileAll(
            kind: (kind: AnyPermanent),
            max_mana_value_limit: Some(ActivatedAbilityX)
        )])],
)"#;

const LOYALTY_X_CARTH_STATIC_FIXTURE: &str = r#"(
    id: "loyalty_x_carth_static_fixture", name: "Carth Loyalty Cost Fixture",
    face_id: "loyalty_x_carth_static_fixture", types: ["Artifact"],
    static_abilities: [(ability_id: "static_01", presentation: Fallback,
        definition: ControllerPlaneswalkerLoyaltyCostsAdditionalOne)],
)"#;

const LOYALTY_X_SECOND_STATIC_FIXTURE: &str = r#"(
    id: "loyalty_x_second_static_fixture", name: "Second Loyalty Cost Fixture",
    face_id: "loyalty_x_second_static_fixture", types: ["Artifact"],
    static_abilities: [(ability_id: "static_01", presentation: Fallback,
        definition: ControllerPlaneswalkerLoyaltyCostsAdditionalOne)],
)"#;

fn loyalty_x_carth_offer_fixture(seed: u64, prohibit_counter_gain: bool) -> (GameEngine, ObjectId) {
    let walker = if prohibit_counter_gain {
        LOYALTY_X_TEST_WALKER.replace(
            "activated_abilities:",
            r#"static_abilities: [(ability_id: "prohibit_counters", presentation: Fallback,
                definition: ProhibitCounters(affected: Self_))], activated_abilities:"#,
        )
    } else {
        LOYALTY_X_TEST_WALKER.to_string()
    };
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            LOYALTY_X_CARTH_STATIC_FIXTURE,
            walker.as_str(),
        ],
        &[],
    )
    .expect("the LoyaltyX and Carth runtime fixture compiles");
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
    place_ready(&mut engine, 10, "Carth Loyalty Cost Fixture");
    let source = place_ready(&mut engine, 10, "Loyalty X Test Walker");
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("planeswalker")
        .set_counter(CounterKind::Loyalty, 6);
    // The fixture uses Carth's exact static cost modifier without its unrelated enter trigger.
    (engine, source)
}

fn loyalty_x_prepared_replacement_replay_fixture(
    seed: u64,
) -> (GameEngine, ObjectId, ObjectId, ObjectId) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/laezel,_vlaakiths_champion.ron"),
            include_str!("../../../tricerules-cards/data/vorinclex,_monstrous_raider.ron"),
            LOYALTY_X_CARTH_STATIC_FIXTURE,
            LOYALTY_X_SECOND_STATIC_FIXTURE,
            LOYALTY_X_TEST_WALKER,
        ],
        &[],
    )
    .expect("prepared LoyaltyX replacement fixture compiles");
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
    .expect("two-player prepared replacement engine");
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    // Put the walker in first so the replacement sources do not park its intrinsic entry loyalty.
    let source = place_ready(&mut engine, 10, "Loyalty X Test Walker");
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("planeswalker")
        .set_counter(CounterKind::Loyalty, 6);
    place_ready(&mut engine, 10, "Carth Loyalty Cost Fixture");
    place_ready(&mut engine, 10, "Second Loyalty Cost Fixture");
    let laezel = place_ready(&mut engine, 10, "Lae'zel, Vlaakith's Champion");
    let opponent_vorinclex = place_ready(&mut engine, 20, "Vorinclex, Monstrous Raider");
    (engine, source, laezel, opponent_vorinclex)
}

fn loyalty_x_command(engine: &GameEngine, source: ObjectId, x_value: u32) -> rv1::ActivateAbility {
    rv1::ActivateAbility {
        source_object_id: source,
        ability_index: 0,
        expected_zone_change_generation: engine.payment_object_ref(source).zone_change_generation,
        x_value,
        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
        ..Default::default()
    }
}

fn activate_loyalty_x(
    engine: &mut GameEngine,
    source: ObjectId,
    x_value: u32,
) -> Result<RuledEventBatch, EngineError> {
    let command = loyalty_x_command(engine, source, x_value);
    engine.apply_scenario_command(
        10,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(command)),
        },
    )
}

fn assert_public_loyalty_x_announcement(batch: &RuledEventBatch, x_value: u32) {
    let expected = format!("X = {x_value}");
    let pushed = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::StackPushed(pushed)) => Some(pushed),
            _ => None,
        })
        .expect("activated LoyaltyX publishes StackPushed");
    assert!(
        pushed.ability_annotation.contains(&expected),
        "stack annotation announces {expected}: {}",
        pushed.ability_annotation
    );
    let log = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::Log(log)) => Some(log),
            _ => None,
        })
        .expect("activated LoyaltyX publishes its public ability log");
    assert!(
        log.text.contains(&expected),
        "ability log announces {expected}"
    );
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

const B2_AGGREGATE_COUNTER_COST_FIXTURE: &str = r#"(
    id: "b2_aggregate_counter_cost_fixture",
    name: "B2 Aggregate Counter Cost Fixture",
    face_id: "b2_aggregate_counter_cost_fixture",
    types: ["Artifact"],
    activated_abilities: [
        (ability_id: "aggregate_01", presentation: Fallback,
            costs: [RemoveCountersFromPermanents(
                count: 3, counter: None,
                filter: (kind: AnyPermanent, controller: You,
                    permanent_types: [Artifact, Creature, Planeswalker],
                    excluded_objects: [Source]))],
            effect: [Draw(who: Controller, count: 1)]),
        (ability_id: "singleton_control_02", presentation: Fallback,
            costs: [RemoveCounters(counter: Some(Charge), count: 1,
                payment_source: SelectedPermanent((kind: AnyPermanent,
                    controller: You,
                    permanent_types: [Artifact, Creature, Planeswalker],
                    excluded_objects: [Source])))],
            effect: [Draw(who: Controller, count: 1)])
    ]
)"#;

fn aggregate_counter_cost_fixture(seed: u64) -> (GameEngine, ObjectId, ObjectId, ObjectId) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/ornithopter.ron"),
            include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            B2_AGGREGATE_COUNTER_COST_FIXTURE,
        ],
        &[],
    )
    .expect("the local aggregate counter-cost fixture validates");
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
    .expect("aggregate counter-cost engine");
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine.state.players[0].mana_pool.colorless = 10;
    let source = place_ready(&mut engine, 10, "B2 Aggregate Counter Cost Fixture");
    let artifact = place_ready(&mut engine, 10, "Ornithopter");
    let creature = place_ready(&mut engine, 10, "Grizzly Bears");
    engine
        .state
        .objects
        .get_mut(&artifact)
        .expect("artifact contributor")
        .add_counters(CounterKind::Charge, 2, 0);
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("creature contributor")
        .add_counters(CounterKind::Night, 1, 0);
    (engine, source, artifact, creature)
}

const C2_AGGREGATE_COUNTER_PAYMENT_FIXTURE: &str = r#"(
    id: "c2_aggregate_counter_payment_fixture",
    name: "C2 Aggregate Counter Payment Fixture",
    face_id: "c2_aggregate_counter_payment_fixture",
    types: ["Creature"],
    power: Some(1),
    toughness: Some(1),
    activated_abilities: [
        (ability_id: "aggregate_payment_01", presentation: Fallback,
            costs: [Mana("{1}{U/P}{U/P}"), RemoveCountersFromPermanents(count: 3, counter: None,
                    filter: (kind: AnyPermanent, controller: You,
                        permanent_types: [Artifact, Creature, Planeswalker],
                        excluded_objects: [Source]))],
            effect: [PutCounters(subject: Source, counter: PlusOnePlusOne, count: 1)])
    ]
)"#;

const C2_AGGREGATE_TRANSFORM_PAYMENT_FIXTURE: &str = r#"(
    id: "c2_aggregate_transform_payment_fixture_front_c2_aggregate_transform_payment_fixture_back",
    name: "C2 Aggregate Transform Payment Fixture",
    layout: Transform,
    faces: [
        (
            name: "C2 Aggregate Transform Payment Fixture",
            face_id: "c2_aggregate_transform_payment_fixture_front",
            types: ["Creature"],
            power: Some(1),
            toughness: Some(1),
            activated_abilities: [
                (ability_id: "aggregate_payment_01", presentation: Fallback,
                    costs: [Mana("{1}{U/P}{U/P}"), RemoveCountersFromPermanents(count: 3, counter: None,
                            filter: (kind: AnyPermanent, controller: You,
                                permanent_types: [Artifact, Creature, Planeswalker],
                                excluded_objects: [Source]))],
                    effect: [PutCounters(subject: Source, counter: PlusOnePlusOne, count: 1)])
            ],
        ),
        (
            name: "C2 Aggregate Transform Payment Fixture Back",
            face_id: "c2_aggregate_transform_payment_fixture_back",
            types: ["Creature"],
            power: Some(2),
            toughness: Some(3),
        ),
    ],
)"#;

fn aggregate_counter_payment_fixture(seed: u64) -> (GameEngine, ObjectId, ObjectId, ObjectId) {
    aggregate_counter_payment_fixture_with_definition(
        seed,
        C2_AGGREGATE_COUNTER_PAYMENT_FIXTURE,
        false,
    )
}

fn aggregate_counter_payment_fixture_with_definition(
    seed: u64,
    source_definition: &str,
    source_in_hand: bool,
) -> (GameEngine, ObjectId, ObjectId, ObjectId) {
    aggregate_counter_payment_fixture_with_definition_and_name(
        seed,
        source_definition,
        "C2 Aggregate Counter Payment Fixture",
        source_in_hand,
    )
}

fn aggregate_counter_payment_fixture_with_definition_and_name(
    seed: u64,
    source_definition: &str,
    source_name: &str,
    source_in_hand: bool,
) -> (GameEngine, ObjectId, ObjectId, ObjectId) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/ornithopter.ron"),
            include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            include_str!("../../../tricerules-cards/data/liquimetal_torque.ron"),
            include_str!("../../../tricerules-cards/data/krark-clan_ironworks.ron"),
            include_str!("../../../tricerules-cards/data/mycosynth_lattice.ron"),
            PAIN_PAYMENT_LAND,
            RESTRICTED_PAYMENT_LAND,
            SPELL_PAYMENT_LAND,
            CAPTURED_ARTIFACT_PAYMENT_LAND,
            CAPTURED_ARTIFACT_SPELL_PAYMENT_LAND,
            source_definition,
        ],
        &[],
    )
    .expect("the local C2 aggregate counter payment fixture validates");
    let decks = (0..2)
        .map(|_| EngineDeck {
            mainboard: vec!["forest".into(); 40],
            commanders: Vec::new(),
        })
        .collect();
    let source_card_id = registry
        .id_for_name(source_name)
        .expect("the selected local C2 source name resolves in its fixture registry")
        .to_string();
    let mut engine = GameEngine::new_with_registry(
        seed,
        &[10, 20],
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .expect("C2 aggregate counter payment engine");
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = if source_in_hand {
        engine
            .apply_dev_command(
                10,
                &rv1::DevCommand {
                    target_player_id: 10,
                    dev: Some(rv1::dev_command::Dev::PutCardInZone(
                        rv1::DevPutCardInZone {
                            card_name: source_name.into(),
                            zone: rv1::DevZone::Hand as i32,
                            ready: false,
                        },
                    )),
                },
            )
            .expect("local C2 fixture enters hand");
        engine.state.players[0]
            .hand
            .iter()
            .copied()
            .find(|object| engine.state.objects[object].card_id == source_card_id)
            .expect("local C2 source is in the actor's hand")
    } else {
        place_ready(&mut engine, 10, source_name)
    };
    let artifact = place_ready(&mut engine, 10, "Ornithopter");
    let creature = place_ready(&mut engine, 10, "Grizzly Bears");
    engine
        .state
        .objects
        .get_mut(&artifact)
        .expect("artifact contributor")
        .add_counters(CounterKind::Charge, 2, 0);
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("creature contributor")
        .add_counters(CounterKind::Night, 1, 0);
    (engine, source, artifact, creature)
}

fn aggregate_counter_group_selection(
    engine: &GameEngine,
    artifact: ObjectId,
    creature: ObjectId,
    cost_index: u32,
) -> rv1::CostSelection {
    rv1::CostSelection {
        cost_index,
        selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
            rv1::CounterRemovalGroupSelection {
                contributions: vec![
                    rv1::CounterRemovalContribution {
                        object: Some(engine.payment_object_ref(artifact)),
                        option_id: crate::engine::counters::counter_option_id(CounterKind::Charge),
                        count: 2,
                    },
                    rv1::CounterRemovalContribution {
                        object: Some(engine.payment_object_ref(creature)),
                        option_id: crate::engine::counters::counter_option_id(CounterKind::Night),
                        count: 1,
                    },
                ],
            },
        )),
    }
}

fn aggregate_phyrexian_life_choices(pay_life: [bool; 2]) -> Vec<rv1::FlexPipPayment> {
    pay_life
        .into_iter()
        .enumerate()
        .map(|(index, pay_life)| rv1::FlexPipPayment {
            pip_index: index as u32 + 1,
            pay_life,
        })
        .collect()
}

fn aggregate_counter_begin_command(
    engine: &GameEngine,
    source: ObjectId,
    artifact: ObjectId,
    creature: ObjectId,
    flex_payments: Vec<rv1::FlexPipPayment>,
) -> rv1::BeginAbilityActivation {
    let effective = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("the aggregate payment ability is active");
    let group_cost_index = effective
        .definition
        .costs
        .iter()
        .position(|cost| matches!(cost, AbilityCost::RemoveCountersFromPermanents { .. }))
        .expect("the aggregate payment ability has its split counter cost");
    let source_ref = engine.payment_object_ref(source);
    rv1::BeginAbilityActivation {
        source_object_id: source,
        expected_zone_change_generation: source_ref.zone_change_generation,
        ability_index: effective.slot,
        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
        cost_selections: vec![aggregate_counter_group_selection(
            engine,
            artifact,
            creature,
            group_cost_index as u32,
        )],
        flex_payments,
        ..Default::default()
    }
}

fn begin_aggregate_counter_payment_window(
    engine: &mut GameEngine,
    source: ObjectId,
    artifact: ObjectId,
    creature: ObjectId,
    pay_life: [bool; 2],
) -> (rv1::PendingAbilityActivation, rv1::CostObjectRef) {
    let source_ref = engine.payment_object_ref(source);
    let command = aggregate_counter_begin_command(
        engine,
        source,
        artifact,
        creature,
        aggregate_phyrexian_life_choices(pay_life),
    );
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(command)),
            },
        )
        .expect("the validated split-counter Begin opens the locked payment window");
    (
        engine
            .state
            .pending_ability_activation
            .clone()
            .expect("the accepted activation remains pending"),
        source_ref,
    )
}

fn assert_aggregate_commit_rejected_unchanged(
    engine: &mut GameEngine,
    player: PlayerId,
    command: rv1::CommitAbilityActivation,
    description: &str,
) {
    let before = format!("{:?}", engine.state);
    let result = engine.apply_command(
        player,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(command)),
        },
    );
    assert!(
        matches!(result, Err(EngineError::Illegal(_))),
        "{description} must reject as Illegal"
    );
    assert_eq!(
        format!("{:?}", engine.state),
        before,
        "{description} preserves the pending transaction and all resources"
    );
    assert!(engine.state.pending_ability_activation.is_some());
}

fn aggregate_counter_shape_definition(shape: &str) -> String {
    let mut definition: tricerules_card_model::card_def::RawCardDefinition =
        ron::from_str(C2_AGGREGATE_COUNTER_PAYMENT_FIXTURE)
            .expect("the labeled C2 payment fixture parses as a raw definition");
    let ability = definition
        .activated_abilities
        .iter_mut()
        .find(|ability| ability.ability_id.as_str() == "aggregate_payment_01")
        .expect("the named fixture ability exists");
    match shape {
        "source_tap" => ability.costs.insert(1, AbilityCost::Tap),
        "fixed_life" => ability.costs.insert(1, AbilityCost::PayLife { amount: 3 }),
        "sorcery_speed" => ability.timing = tricerules_card_model::ActivationTiming::SorcerySpeed,
        "per_turn_limit" => {
            ability.activation_limit = Some(
                tricerules_card_model::primitives::ActivationLimit::PerTurn { max_activations: 1 },
            )
        }
        "hand" => ability.source_zone = AbilitySourceZone::Hand,
        "modal" => {
            ability.effect.clear();
            ability.modal = Some(
                ron::from_str::<tricerules_card_model::ModalDef>(
                    r#"(min_modes: 1, max_modes: 1, modes: [
                        (mode_id: "draw_one", presentation: Fallback,
                            effects: [Draw(who: Controller, count: 1)]),
                        (mode_id: "draw_two", presentation: Fallback,
                            effects: [Draw(who: Controller, count: 2)])
                    ])"#,
                )
                .expect("the local modal sibling is a typed model definition"),
            );
        }
        "targeted" => {
            ability.effect = vec![ron::from_str::<SpellEffectKind>(
                "Tap(subject: Chosen((kind: Creature, controller: You)))",
            )
            .expect("the local target sibling uses a model-valid chosen effect")];
            ability.targeting = Some(
                ron::from_str::<tricerules_card_model::primitives::TargetingDef>(
                    r#"(groups: [(min: 1, max: 1, prompt: "Choose your creature",
                        effect_indices: [0])])"#,
                )
                .expect("the local target sibling uses a model-valid target group"),
            );
        }
        "variable" => {
            ability.costs[0] = AbilityCost::Mana(
                tricerules_card_model::ManaCost::parse("{X}").expect("model-valid X mana cost"),
            );
        }
        "colored" => {
            ability.costs[0] = AbilityCost::Mana(
                tricerules_card_model::ManaCost::parse("{R}{U/P}{U/P}")
                    .expect("model-valid colored and Phyrexian mana cost"),
            );
        }
        "hybrid" => {
            ability.costs[0] = AbilityCost::Mana(
                tricerules_card_model::ManaCost::parse("{1}{U/R}")
                    .expect("model-valid hybrid mana cost"),
            );
        }
        "mono_hybrid" => {
            ability.costs[0] = AbilityCost::Mana(
                tricerules_card_model::ManaCost::parse("{2/U}")
                    .expect("model-valid monohybrid mana cost"),
            );
        }
        other => panic!("unknown aggregate shape fixture: {other}"),
    }
    ron::ser::to_string(&definition).expect("typed local fixture serializes for the registry")
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
    let parked = crate::state::ParkedStackResolution::for_flat_effects(
        item.clone(),
        Some(1),
        Default::default(),
    );
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
    engine
        .reindex_battlefield_control(&mut control_events)
        .expect("reindex after control effect");
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
fn aggregate_counter_public_affordability_enables_only_supported_accepted_shape() {
    let (mut engine, source, artifact, creature) = aggregate_counter_payment_fixture(602_500);
    let effective = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("the aggregate payment ability is active");

    let event = engine.ev_zone_view_sync_tracked();
    let Some(rv1::ruled_event::Ev::ZoneView(zone_view)) = event.ev else {
        panic!("the actual public state event includes a zone view");
    };
    let actor_view = zone_view
        .per_player
        .iter()
        .find(|view| view.player_id == 10)
        .expect("the actor receives its battlefield view");
    let source_view = actor_view
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == source)
        .expect("the source is present in the public battlefield view");
    let ability_info = source_view
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_index == effective.slot)
        .expect("the supported aggregate ability has public AbilityInfo");
    assert!(
        ability_info.activatable,
        "the supported fixed targetless aggregate shape is publicly activatable when its complete costs are payable"
    );

    let mut batch = RuledEventBatch::default();
    super::legal_actions::fill_legal(&mut batch, &engine);
    let public_legal = batch
        .legal_by_player
        .get(&10)
        .expect("the actor receives legal action data");
    let ability_key = (u64::from(source) << 32) | u64::from(effective.slot);
    let choices = public_legal
        .cost_choices_by_ability
        .get(&ability_key)
        .expect("the aggregate ability has public cost choices");
    assert!(choices.non_mana_costs_payable);
    let group = choices
        .choices
        .iter()
        .find_map(|choice| choice.counter_removal_group.as_ref())
        .expect("the complete payable aggregate cost is offered as a group");
    assert_eq!(group.required_count, 3);
    assert!(group.candidates.iter().any(|candidate| {
        candidate
            .object
            .as_ref()
            .is_some_and(|object| object.object_id == artifact)
    }));
    assert!(group.candidates.iter().any(|candidate| {
        candidate
            .object
            .as_ref()
            .is_some_and(|object| object.object_id == creature)
    }));
}

#[test]
fn aggregate_counter_public_competition_preserves_a_fixed_kind_for_a_second_group() {
    let mut definition: tricerules_card_model::card_def::RawCardDefinition =
        ron::from_str(C2_AGGREGATE_COUNTER_PAYMENT_FIXTURE)
            .expect("the local aggregate definition parses");
    let ability = definition
        .activated_abilities
        .iter_mut()
        .find(|ability| ability.ability_id.as_str() == "aggregate_payment_01")
        .expect("the aggregate ability exists");
    let filter = match &ability.costs[1] {
        AbilityCost::RemoveCountersFromPermanents { filter, .. } => filter.clone(),
        other => panic!("the first counter cost is aggregate: {other:?}"),
    };
    ability
        .costs
        .push(AbilityCost::RemoveCountersFromPermanents {
            count: 3,
            counter: Some(CounterKind::Charge),
            filter,
        });
    let definition =
        ron::ser::to_string(&definition).expect("the typed two-group definition serializes");
    let (mut engine, source, artifact, creature) =
        aggregate_counter_payment_fixture_with_definition(602_516, &definition, false);
    engine.state.players[0].mana_pool.colorless = 1;
    let effective = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("the two-group aggregate ability is active");

    let ability_info_activatable = |engine: &mut GameEngine| {
        let event = engine.ev_zone_view_sync_tracked();
        let Some(rv1::ruled_event::Ev::ZoneView(zone_view)) = event.ev else {
            panic!("the public state event includes a zone view");
        };
        zone_view
            .per_player
            .iter()
            .find(|view| view.player_id == 10)
            .and_then(|view| {
                view.battlefield_objects
                    .iter()
                    .find(|object| object.object_id == source)
            })
            .and_then(|object| {
                object
                    .activated_abilities
                    .iter()
                    .find(|ability| ability.ability_index == effective.slot)
            })
            .expect("the aggregate ability has a public AbilityInfo")
            .activatable
    };
    assert!(!engine.ability_activatable(source, effective.slot as usize, &effective.definition));
    assert!(
        !ability_info_activatable(&mut engine),
        "the public AbilityInfo is false when shared inventory cannot pay both groups"
    );
    let mut insufficient_batch = RuledEventBatch::default();
    super::legal_actions::fill_legal(&mut insufficient_batch, &engine);
    let insufficient_choices = insufficient_batch.legal_by_player[&10]
        .cost_choices_by_ability
        .get(&((u64::from(source) << 32) | u64::from(effective.slot)))
        .expect("the structurally supported two-group cost offer remains keyed");
    assert!(
        !insufficient_choices.non_mana_costs_payable,
        "the public keyed aggregate offer also uses the shared allocator"
    );

    let other_artifact = place_ready(&mut engine, 10, "Ornithopter");
    engine
        .state
        .objects
        .get_mut(&artifact)
        .unwrap()
        .add_counters(CounterKind::Charge, 1, 0);
    engine
        .state
        .objects
        .get_mut(&other_artifact)
        .unwrap()
        .add_counters(CounterKind::Charge, 2, 0);
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        3
    );
    assert_eq!(
        engine.state.objects[&other_artifact].counter_count(CounterKind::Charge),
        2
    );
    assert!(
        engine.ability_activatable(source, effective.slot as usize, &effective.definition),
        "the shared public allocator must reserve the Charge-three slot for the constrained demand"
    );
    assert!(ability_info_activatable(&mut engine));
    let mut batch = RuledEventBatch::default();
    super::legal_actions::fill_legal(&mut batch, &engine);
    let choices = batch.legal_by_player[&10]
        .cost_choices_by_ability
        .get(&((u64::from(source) << 32) | u64::from(effective.slot)))
        .expect("the public ability publishes both group costs");
    assert!(choices.non_mana_costs_payable);
    let groups = choices
        .choices
        .iter()
        .filter_map(|choice| {
            choice
                .counter_removal_group
                .as_ref()
                .map(|group| (choice.cost_index, group))
        })
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), 2);
    assert!(groups.iter().all(|(_, group)| group.required_count == 3));
    let fixed_charge_group = groups
        .iter()
        .find(|(cost_index, _)| *cost_index == 2)
        .map(|(_, group)| *group)
        .expect("the fixed-kind group retains its cost index");
    let charge_capacity = |object_id| {
        fixed_charge_group
            .candidates
            .iter()
            .find(|candidate| {
                candidate
                    .object
                    .as_ref()
                    .is_some_and(|object| object.object_id == object_id)
            })
            .and_then(|candidate| {
                candidate.options.iter().find(|option| {
                    option.option_id
                        == crate::engine::counters::counter_option_id(CounterKind::Charge)
                })
            })
            .map(|option| option.available_count)
            .expect("the offered Charge candidate reports its available contribution count")
    };
    assert_eq!(charge_capacity(artifact), 3);
    assert_eq!(charge_capacity(other_artifact), 2);

    let source_ref = engine.payment_object_ref(source);
    let begin = rv1::BeginAbilityActivation {
        source_object_id: source,
        expected_zone_change_generation: source_ref.zone_change_generation,
        ability_index: effective.slot,
        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
        cost_selections: vec![
            aggregate_counter_group_selection(&engine, other_artifact, creature, 1),
            rv1::CostSelection {
                cost_index: 2,
                selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
                    rv1::CounterRemovalGroupSelection {
                        contributions: vec![rv1::CounterRemovalContribution {
                            object: Some(engine.payment_object_ref(artifact)),
                            option_id: crate::engine::counters::counter_option_id(
                                CounterKind::Charge,
                            ),
                            count: 3,
                        }],
                    },
                )),
            },
        ],
        flex_payments: aggregate_phyrexian_life_choices([true, true]),
        ..Default::default()
    };
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
            },
        )
        .expect("Begin accepts both groups using disjoint rows after shared competition");
    let pending = engine.state.pending_ability_activation.clone().unwrap();
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        3
    );
    assert_eq!(
        engine.state.objects[&other_artifact].counter_count(CounterKind::Charge),
        2
    );
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    rv1::CommitAbilityActivation {
                        transaction_id: pending.transaction_id,
                        expected_revision: pending.revision,
                        payment: Some(rv1::PaymentSelection {
                            expected_state_revision: engine.state.command_index,
                            source: Some(source_ref),
                            mana: Some(rv1::PaymentMana {
                                c: 1,
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("both split groups debit atomically");
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        engine.state.objects[&other_artifact].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        0
    );
    assert_eq!(engine.state.players[0].life, 16);
    assert_eq!(engine.state.stack.len(), 1);
}

#[test]
fn aggregate_counter_begin_locks_group_and_both_phyrexian_choices() {
    let (mut engine, source, artifact, creature) = aggregate_counter_payment_fixture(602_501);
    let effective = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("the aggregate payment ability is active");
    let group_selection = rv1::CostSelection {
        cost_index: 1,
        selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
            rv1::CounterRemovalGroupSelection {
                contributions: vec![
                    rv1::CounterRemovalContribution {
                        object: Some(engine.payment_object_ref(artifact)),
                        option_id: crate::engine::counters::counter_option_id(CounterKind::Charge),
                        count: 2,
                    },
                    rv1::CounterRemovalContribution {
                        object: Some(engine.payment_object_ref(creature)),
                        option_id: crate::engine::counters::counter_option_id(CounterKind::Night),
                        count: 1,
                    },
                ],
            },
        )),
    };
    let begin = rv1::BeginAbilityActivation {
        source_object_id: source,
        expected_zone_change_generation: engine.payment_object_ref(source).zone_change_generation,
        ability_index: effective.slot,
        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
        cost_selections: vec![group_selection],
        flex_payments: vec![
            rv1::FlexPipPayment {
                pip_index: 1,
                pay_life: true,
            },
            rv1::FlexPipPayment {
                pip_index: 2,
                pay_life: true,
            },
        ],
        ..Default::default()
    };
    let before = serde_json::to_value(&engine.state)
        .expect("serialize complete state before accepted aggregate Begin");
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
            },
        )
        .expect("a supported payable aggregate Begin accepts both fixed Phyrexian choices");

    let pending = engine
        .state
        .pending_ability_activation
        .as_ref()
        .expect("accepted aggregate Begin publishes a pending activation");
    assert_eq!(
        pending.stage(),
        rv1::AbilityActivationStage::Payment,
        "the supported targetless ability begins directly in its payment window"
    );
    assert_eq!(pending.actor_player_id, 10);
    assert_eq!(pending.source_object_id, source);
    assert_eq!(
        engine.state.players[0].life, 20,
        "Begin reserves but does not pay life"
    );
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        1
    );
    assert!(
        serde_json::to_value(&engine.state).expect("serialize state after Begin") != before,
        "accepted Begin publishes the payment window without debiting announced costs"
    );
    assert!(engine.pending_ability_activation_internal.is_some());
}

#[test]
fn aggregate_counter_begin_combines_fixed_and_phyrexian_life_before_publishing() {
    let definition = aggregate_counter_shape_definition("fixed_life");
    let (mut engine, source, artifact, creature) =
        aggregate_counter_payment_fixture_with_definition(602_511, &definition, false);
    engine.state.players[0].life = 6;
    let effective = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("fixed-life aggregate capability is active");
    assert!(
        engine.ability_activatable(source, effective.slot as usize, &effective.definition),
        "the test begins with payable counters and no announcement-time gate on future mana"
    );
    let group_cost_index = effective
        .definition
        .costs
        .iter()
        .position(|cost| matches!(cost, AbilityCost::RemoveCountersFromPermanents { .. }))
        .expect("the fixed-life ability retains its aggregate group");
    let begin = rv1::BeginAbilityActivation {
        source_object_id: source,
        expected_zone_change_generation: engine.payment_object_ref(source).zone_change_generation,
        ability_index: effective.slot,
        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
        cost_selections: vec![rv1::CostSelection {
            cost_index: group_cost_index as u32,
            selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
                rv1::CounterRemovalGroupSelection {
                    contributions: vec![
                        rv1::CounterRemovalContribution {
                            object: Some(engine.payment_object_ref(artifact)),
                            option_id: crate::engine::counters::counter_option_id(
                                CounterKind::Charge,
                            ),
                            count: 2,
                        },
                        rv1::CounterRemovalContribution {
                            object: Some(engine.payment_object_ref(creature)),
                            option_id: crate::engine::counters::counter_option_id(
                                CounterKind::Night,
                            ),
                            count: 1,
                        },
                    ],
                },
            )),
        }],
        flex_payments: vec![
            rv1::FlexPipPayment {
                pip_index: 1,
                pay_life: true,
            },
            rv1::FlexPipPayment {
                pip_index: 2,
                pay_life: true,
            },
        ],
        ..Default::default()
    };
    let before = format!("{:?}", engine.state);
    let result = engine.apply_command(
        10,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
        },
    );
    assert!(
        matches!(result, Err(EngineError::Illegal(_))),
        "fixed PayLife 3 plus both Phyrexian life choices costs 7, which the actor at 6 life cannot pay"
    );
    assert_eq!(format!("{:?}", engine.state), before);
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
}

#[test]
fn aggregate_counter_accepted_fixed_and_phyrexian_life_preview_commit_retries_at_exact_total() {
    let definition = aggregate_counter_shape_definition("fixed_life");
    let (mut engine, source, artifact, creature) =
        aggregate_counter_payment_fixture_with_definition(602_516, &definition, false);
    engine.state.players[0].life = 8;
    engine.state.players[0].mana_pool.colorless = 1;

    // This is a prepared-state boundary check: Begin is accepted at 8 life and
    // locks PayLife 3 plus two Phyrexian life choices without paying them.
    let (pending, source_ref) = begin_aggregate_counter_payment_window(
        &mut engine,
        source,
        artifact,
        creature,
        [true, true],
    );
    assert_eq!(
        pending.flex_payments,
        aggregate_phyrexian_life_choices([true, true])
    );
    assert_eq!(engine.state.players[0].life, 8);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        1
    );

    engine.state.players[0].life = 6;
    let make_commit = |expected_state_revision, mana| rv1::CommitAbilityActivation {
        transaction_id: pending.transaction_id,
        expected_revision: pending.revision,
        payment: Some(rv1::PaymentSelection {
            expected_state_revision,
            source: Some(source_ref),
            mana: Some(mana),
            ..Default::default()
        }),
        ..Default::default()
    };

    let before_incomplete_preview = format!("{:?}", engine.state);
    let incomplete_preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            transaction_id: pending.transaction_id,
            revision: engine.state.command_index,
            commit_ability_activation: Some(make_commit(
                engine.state.command_index,
                rv1::PaymentMana::default(),
            )),
            ..Default::default()
        },
    );
    assert!(!incomplete_preview.complete);
    assert!(
        !incomplete_preview.error.is_empty(),
        "the incomplete-mana Preview still rejects the combined life total"
    );
    assert_eq!(format!("{:?}", engine.state), before_incomplete_preview);
    assert_eq!(
        engine.state.pending_ability_activation.as_ref(),
        Some(&pending),
        "the incomplete Preview retains the accepted choices for retry"
    );

    let complete_mana = rv1::PaymentMana {
        c: 1,
        ..Default::default()
    };
    let before_complete_preview = format!("{:?}", engine.state);
    let complete_preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            transaction_id: pending.transaction_id,
            revision: engine.state.command_index,
            commit_ability_activation: Some(make_commit(engine.state.command_index, complete_mana)),
            ..Default::default()
        },
    );
    assert!(!complete_preview.complete);
    assert!(
        !complete_preview.error.is_empty(),
        "complete mana cannot make the combined PayLife 3 plus Phyrexian life 4 payable at 6"
    );
    assert_eq!(format!("{:?}", engine.state), before_complete_preview);
    assert_eq!(
        engine.state.pending_ability_activation.as_ref(),
        Some(&pending),
        "the complete Preview retains the accepted choices for retry"
    );

    let before_rejected_commit = format!("{:?}", engine.state);
    let rejected_commit = engine.apply_command(
        10,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                make_commit(engine.state.command_index, complete_mana),
            )),
        },
    );
    assert!(
        matches!(rejected_commit, Err(EngineError::Illegal(_))),
        "explicit complete-mana Commit rejects the combined life total at 6"
    );
    assert_eq!(format!("{:?}", engine.state), before_rejected_commit);
    assert_eq!(
        engine.state.pending_ability_activation.as_ref(),
        Some(&pending),
        "rejected Commit leaves the accepted payment available for retry"
    );

    // Restore the prepared actor to exactly 7: all life components are paid
    // once, after which normal state-based loss at 0 life is allowed to end
    // the game and remove the activation.
    engine.state.players[0].life = 7;
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    make_commit(engine.state.command_index, complete_mana),
                )),
            },
        )
        .expect("the same locked choices succeed at their exact life total");

    assert_eq!(engine.state.players[0].life, 0);
    assert!(engine.state.players[0].has_lost);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        0
    );
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
}

#[test]
fn aggregate_counter_begin_locks_each_phyrexian_choice_and_rejects_malformed_lists() {
    let (base, source, artifact, creature) = aggregate_counter_payment_fixture(602_515);
    let make_choices = |choices: [bool; 2]| {
        choices
            .into_iter()
            .enumerate()
            .map(|(index, pay_life)| rv1::FlexPipPayment {
                pip_index: index as u32 + 1,
                pay_life,
            })
            .collect::<Vec<_>>()
    };
    let missing = vec![rv1::FlexPipPayment {
        pip_index: 1,
        pay_life: true,
    }];
    let malformed = [
        ("missing pip", missing),
        (
            "duplicate pip",
            vec![
                rv1::FlexPipPayment {
                    pip_index: 1,
                    pay_life: true,
                },
                rv1::FlexPipPayment {
                    pip_index: 1,
                    pay_life: false,
                },
            ],
        ),
        (
            "generic pip index",
            vec![
                rv1::FlexPipPayment {
                    pip_index: 0,
                    pay_life: true,
                },
                rv1::FlexPipPayment {
                    pip_index: 2,
                    pay_life: false,
                },
            ],
        ),
        (
            "out of range pip index",
            vec![
                rv1::FlexPipPayment {
                    pip_index: 1,
                    pay_life: true,
                },
                rv1::FlexPipPayment {
                    pip_index: 3,
                    pay_life: false,
                },
            ],
        ),
    ];
    for (label, flex_payments) in malformed {
        let mut engine = base.clone();
        let mut begin = aggregate_counter_begin_command(
            &engine,
            source,
            artifact,
            creature,
            make_choices([true, true]),
        );
        begin.flex_payments = flex_payments;
        let before = serde_json::to_value(&engine.state)
            .expect("serialize state before malformed Phyrexian choices");
        assert!(
            matches!(
                engine.apply_command(
                    10,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
                    },
                ),
                Err(EngineError::Illegal(_))
            ),
            "{label} must reject Begin"
        );
        assert_eq!(
            serde_json::to_value(&engine.state)
                .expect("serialize state after malformed Phyrexian choices"),
            before,
            "{label} must not allocate a transaction or change resources"
        );
        assert!(engine.pending_ability_activation_internal.is_none());
    }

    for (label, pay_life, blue, expected_life) in [
        ("UU", [false, false], 2, 20),
        ("U then life", [false, true], 1, 18),
        ("life then U", [true, false], 1, 18),
        ("four life", [true, true], 0, 16),
    ] {
        let mut engine = base.clone();
        engine.state.players[0].mana_pool.colorless = 1;
        engine.state.players[0].mana_pool.blue = blue;
        let source_ref = engine.payment_object_ref(source);
        let begin = aggregate_counter_begin_command(
            &engine,
            source,
            artifact,
            creature,
            make_choices(pay_life),
        );
        engine
            .apply_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
                },
            )
            .expect("the complete explicit Phyrexian list is accepted");
        let pending = engine.state.pending_ability_activation.clone().unwrap();
        assert_eq!(pending.flex_payments, make_choices(pay_life), "{label}");
        assert_eq!(
            engine.state.players[0].life, 20,
            "Begin locks {label} without spending"
        );
        assert_eq!(engine.state.players[0].mana_pool.blue, blue);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
        engine
            .apply_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                        rv1::CommitAbilityActivation {
                            transaction_id: pending.transaction_id,
                            expected_revision: pending.revision,
                            payment: Some(rv1::PaymentSelection {
                                expected_state_revision: engine.state.command_index,
                                source: Some(source_ref),
                                mana: Some(rv1::PaymentMana {
                                    u: blue,
                                    c: 1,
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("the selected blue and life alternatives complete the fixed cost");
        assert_eq!(engine.state.players[0].life, expected_life, "{label}");
        assert_eq!(engine.state.players[0].mana_pool.blue, 0, "{label}");
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0, "{label}");
        assert_eq!(
            engine.state.objects[&artifact].counter_count(CounterKind::Charge),
            0
        );
        assert_eq!(
            engine.state.objects[&creature].counter_count(CounterKind::Night),
            0
        );
        assert_eq!(engine.state.stack.len(), 1, "{label}");
    }
}

#[test]
fn aggregate_counter_commit_rejects_changed_flex_stale_context_and_retries_once() {
    let (mut engine, source, artifact, creature) = aggregate_counter_payment_fixture(602_512);
    engine.state.players[0].mana_pool.colorless = 1;
    let (pending, source_ref) = begin_aggregate_counter_payment_window(
        &mut engine,
        source,
        artifact,
        creature,
        [true, true],
    );
    let valid = rv1::CommitAbilityActivation {
        transaction_id: pending.transaction_id,
        expected_revision: pending.revision,
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_ref),
            mana: Some(rv1::PaymentMana {
                c: 1,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_aggregate_commit_rejected_unchanged(
        &mut engine,
        20,
        valid.clone(),
        "a different player attempting the accepted payment",
    );
    let mut wrong_transaction = valid.clone();
    wrong_transaction.transaction_id = wrong_transaction.transaction_id.saturating_add(1);
    assert_aggregate_commit_rejected_unchanged(
        &mut engine,
        10,
        wrong_transaction,
        "a mismatched activation transaction id",
    );
    let mut stale_pending_revision = valid.clone();
    stale_pending_revision.expected_revision =
        stale_pending_revision.expected_revision.saturating_add(1);
    assert_aggregate_commit_rejected_unchanged(
        &mut engine,
        10,
        stale_pending_revision,
        "a stale pending activation revision",
    );
    let mut stale_payment_revision = valid.clone();
    stale_payment_revision
        .payment
        .as_mut()
        .expect("explicit payment exists")
        .expected_state_revision = engine.state.command_index.saturating_sub(1);
    assert_aggregate_commit_rejected_unchanged(
        &mut engine,
        10,
        stale_payment_revision,
        "a stale current payment state revision",
    );
    let mut wrong_source_receipt = valid.clone();
    wrong_source_receipt
        .payment
        .as_mut()
        .expect("explicit payment exists")
        .source
        .as_mut()
        .expect("the original source receipt is selected")
        .zone_change_generation = source_ref.zone_change_generation.saturating_add(1);
    assert_aggregate_commit_rejected_unchanged(
        &mut engine,
        10,
        wrong_source_receipt,
        "a payment naming a different source incarnation",
    );
    let mut forged_restricted_group = valid.clone();
    forged_restricted_group.restricted_mana = vec![rv1::ManaSpendSelection {
        restriction_group_id: 17,
        c: 1,
        ..Default::default()
    }];
    forged_restricted_group
        .payment
        .as_mut()
        .expect("explicit payment exists")
        .mana = Some(rv1::PaymentMana::default());
    assert_aggregate_commit_rejected_unchanged(
        &mut engine,
        10,
        forged_restricted_group,
        "an unavailable restricted mana group",
    );
    let mut identical_flex_override = valid.clone();
    identical_flex_override.flex_payments = aggregate_phyrexian_life_choices([true, true]);
    assert_aggregate_commit_rejected_unchanged(
        &mut engine,
        10,
        identical_flex_override,
        "a nonempty Commit flex list that repeats the accepted choices",
    );
    let mut changed_flex_override = valid.clone();
    changed_flex_override.flex_payments = aggregate_phyrexian_life_choices([false, true]);
    assert_aggregate_commit_rejected_unchanged(
        &mut engine,
        10,
        changed_flex_override,
        "a nonempty Commit flex list that changes an accepted choice",
    );

    let mut missing_internal = engine.clone();
    missing_internal.pending_ability_activation_internal = None;
    assert_aggregate_commit_rejected_unchanged(
        &mut missing_internal,
        10,
        valid.clone(),
        "missing engine-owned accepted aggregate state",
    );
    let mut mismatched_pending = engine.clone();
    mismatched_pending
        .state
        .pending_ability_activation
        .as_mut()
        .expect("accepted pending activation exists")
        .flex_payments
        .pop();
    assert_aggregate_commit_rejected_unchanged(
        &mut mismatched_pending,
        10,
        valid.clone(),
        "a Pending flex list that disagrees with the engine-owned snapshot",
    );

    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    valid.clone(),
                )),
            },
        )
        .expect("the unchanged accepted choices remain available for retry");
    assert!(engine.state.pending_ability_activation.is_none());
    assert_eq!(engine.state.players[0].life, 16);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        0
    );
    assert_eq!(engine.state.stack.len(), 1);

    let before_duplicate = format!("{:?}", engine.state);
    assert!(matches!(
        engine.apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(valid)),
            },
        ),
        Err(EngineError::Illegal(_))
    ));
    assert_eq!(format!("{:?}", engine.state), before_duplicate);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "duplicate Commit creates no second ability"
    );
}

#[test]
fn aggregate_counter_incomplete_preview_rejects_invalid_live_contributors_first() {
    #[derive(Clone, Copy)]
    enum ContributorChange {
        Control,
        Type,
        Phase,
        Generation,
        Quantity,
    }

    for (offset, (label, change)) in [
        ("control", ContributorChange::Control),
        ("type", ContributorChange::Type),
        ("phase", ContributorChange::Phase),
        ("generation", ContributorChange::Generation),
        ("quantity", ContributorChange::Quantity),
    ]
    .into_iter()
    .enumerate()
    {
        let (mut engine, source, artifact, creature) =
            aggregate_counter_payment_fixture(602_517 + offset as u64);
        engine.state.players[0].mana_pool.colorless = 1;
        let (pending, source_ref) = begin_aggregate_counter_payment_window(
            &mut engine,
            source,
            artifact,
            creature,
            [true, true],
        );
        let complete_mana_commit = |engine: &GameEngine| rv1::CommitAbilityActivation {
            transaction_id: pending.transaction_id,
            expected_revision: pending.revision,
            payment: Some(rv1::PaymentSelection {
                expected_state_revision: engine.state.command_index,
                source: Some(source_ref),
                mana: Some(rv1::PaymentMana {
                    c: 1,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let original_artifact_ref = engine.payment_object_ref(artifact);
        let before_valid_preview = serde_json::to_value(&engine.state)
            .expect("serialize state before valid incomplete Preview");
        let valid_incomplete = engine.preview_payment(
            10,
            &rv1::PreviewPayment {
                transaction_id: pending.transaction_id,
                revision: engine.state.command_index,
                commit_ability_activation: Some(rv1::CommitAbilityActivation {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    payment: Some(rv1::PaymentSelection {
                        expected_state_revision: engine.state.command_index,
                        source: Some(source_ref),
                        mana: Some(rv1::PaymentMana::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        assert!(
            valid_incomplete.error.is_empty(),
            "{label}: a valid row set with no generic mana is a normal incomplete Preview: {}",
            valid_incomplete.error
        );
        assert!(!valid_incomplete.complete);
        assert_eq!(
            serde_json::to_value(&engine.state)
                .expect("serialize state after valid incomplete Preview"),
            before_valid_preview
        );
        match change {
            ContributorChange::Control => {
                engine
                    .state
                    .continuous_effects
                    .push(crate::state::ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: None,
                        affected: crate::state::AffectedScope::Single(artifact),
                        kind: tricerules_card_model::ContinuousEffectKind::Layer2Control {
                            controller: tricerules_card_model::ControllerReference::Fixed(20),
                        },
                        condition: None,
                        duration: tricerules_card_model::EffectDuration::UntilEndOfTurn,
                        timestamp: engine.state.command_index.saturating_add(1),
                    });
                assert_eq!(engine.controller_of(artifact), Some(20));
            }
            ContributorChange::Type => {
                engine
                    .state
                    .continuous_effects
                    .push(crate::state::ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: None,
                        affected: crate::state::AffectedScope::Single(creature),
                        kind: tricerules_card_model::ContinuousEffectKind::Layer4RemoveCreature,
                        condition: None,
                        duration: tricerules_card_model::EffectDuration::UntilEndOfTurn,
                        timestamp: engine.state.command_index.saturating_add(1),
                    });
                assert!(!engine.characteristics(creature).unwrap().is_creature());
            }
            ContributorChange::Phase => {
                engine
                    .state
                    .objects
                    .get_mut(&artifact)
                    .unwrap()
                    .phase_status = Some(crate::state::PhasedOutStatus {
                    phase_out_under: 0,
                    phase_in_due_after_departed_turn: false,
                    phase_in_with: None,
                    direct_attachment_snapshot: None,
                });
            }
            ContributorChange::Generation => {
                crate::engine::resolution::move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    artifact,
                    Zone::Graveyard,
                    None,
                )
                .expect("contributor leaves after the accepted Begin");
                crate::engine::resolution::move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    artifact,
                    Zone::Battlefield,
                    None,
                )
                .expect("contributor returns as a new generation");
                engine
                    .state
                    .objects
                    .get_mut(&artifact)
                    .unwrap()
                    .add_counters(CounterKind::Charge, 2, 0);
                assert_ne!(
                    engine.payment_object_ref(artifact).zone_change_generation,
                    original_artifact_ref.zone_change_generation
                );
            }
            ContributorChange::Quantity => {
                engine
                    .state
                    .objects
                    .get_mut(&artifact)
                    .unwrap()
                    .counters
                    .insert(CounterKind::Charge, 1);
            }
        }

        let before_preview = format!("{:?}", engine.state);
        let preview = engine.preview_payment(
            10,
            &rv1::PreviewPayment {
                transaction_id: pending.transaction_id,
                revision: engine.state.command_index,
                commit_ability_activation: Some(rv1::CommitAbilityActivation {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    payment: Some(rv1::PaymentSelection {
                        expected_state_revision: engine.state.command_index,
                        source: Some(source_ref),
                        mana: Some(rv1::PaymentMana::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        assert!(
            !preview.complete,
            "{label}: absent generic mana still leaves an incomplete selected payment"
        );
        assert!(
            !preview.error.is_empty(),
            "{label}: unlike the valid incomplete preview, changed contributors invalidate the locked rows"
        );
        assert_ne!(preview.error, valid_incomplete.error, "{label}");
        assert_eq!(
            format!("{:?}", engine.state),
            before_preview,
            "{label}: failed Preview is read-only"
        );
        assert_eq!(
            engine.state.pending_ability_activation.as_ref(),
            Some(&pending),
            "{label}: the accepted transaction and rows remain available for retry"
        );
        assert!(engine.pending_ability_activation_internal.is_some());

        let rejected_complete_mana_commit = complete_mana_commit(&engine);
        assert_aggregate_commit_rejected_unchanged(
            &mut engine,
            10,
            rejected_complete_mana_commit,
            &format!("{label}: complete-mana Commit after the contributor changed"),
        );
        assert!(engine.pending_ability_activation_internal.is_some());
        assert_eq!(engine.state.players[0].life, 20, "{label}: no life debit");
        assert_eq!(
            engine.state.players[0].mana_pool.colorless, 1,
            "{label}: no mana debit"
        );
        assert_eq!(
            engine.state.objects[&artifact].counter_count(CounterKind::Charge),
            if matches!(change, ContributorChange::Quantity) {
                1
            } else {
                2
            },
            "{label}: no artifact counter debit"
        );
        assert_eq!(
            engine.state.objects[&creature].counter_count(CounterKind::Night),
            1,
            "{label}: no creature counter debit"
        );

        match change {
            ContributorChange::Control => {
                engine.state.continuous_effects.retain(|effect| {
                    !(matches!(
                        &effect.affected,
                        crate::state::AffectedScope::Single(id) if *id == artifact
                    ) && matches!(
                        &effect.kind,
                        tricerules_card_model::ContinuousEffectKind::Layer2Control {
                            controller: tricerules_card_model::ControllerReference::Fixed(20)
                        }
                    ))
                });
                assert_eq!(engine.controller_of(artifact), Some(10));
            }
            ContributorChange::Type => {
                engine.state.continuous_effects.retain(|effect| {
                    !(matches!(
                        &effect.affected,
                        crate::state::AffectedScope::Single(id) if *id == creature
                    ) && matches!(
                        &effect.kind,
                        tricerules_card_model::ContinuousEffectKind::Layer4RemoveCreature
                    ))
                });
                assert!(engine.characteristics(creature).unwrap().is_creature());
            }
            ContributorChange::Phase => {
                engine
                    .state
                    .objects
                    .get_mut(&artifact)
                    .unwrap()
                    .phase_status = None;
                assert!(engine.state.objects[&artifact].phase_status.is_none());
            }
            ContributorChange::Generation => {
                // A new zone-change generation permanently stales the accepted
                // object receipt. Do not pretend a generation rewind is a
                // lawful retry path; the repairable cases below exercise the
                // unchanged accepted rows and choices.
                continue;
            }
            ContributorChange::Quantity => {
                engine
                    .state
                    .objects
                    .get_mut(&artifact)
                    .unwrap()
                    .add_counters(CounterKind::Charge, 1, 0);
                assert_eq!(
                    engine.state.objects[&artifact].counter_count(CounterKind::Charge),
                    2
                );
            }
        }

        let retry = complete_mana_commit(&engine);
        engine
            .apply_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(retry)),
                },
            )
            .expect("repairing the live contributor allows the same accepted rows to commit");
        assert!(engine.state.pending_ability_activation.is_none(), "{label}");
        assert!(
            engine.pending_ability_activation_internal.is_none(),
            "{label}"
        );
        assert_eq!(engine.state.players[0].life, 16, "{label}: life debit once");
        assert_eq!(
            engine.state.players[0].mana_pool.colorless, 0,
            "{label}: mana debit once"
        );
        assert_eq!(
            engine.state.objects[&artifact].counter_count(CounterKind::Charge),
            0,
            "{label}: artifact counter debit once"
        );
        assert_eq!(
            engine.state.objects[&creature].counter_count(CounterKind::Night),
            0,
            "{label}: creature counter debit once"
        );
        assert_eq!(engine.state.stack.len(), 1, "{label}: one activation");
    }
}

#[test]
fn aggregate_counter_activation_does_not_require_an_untapped_or_sickness_free_source() {
    for (offset, (tapped, summoning_sick)) in [(true, false), (false, true), (true, true)]
        .into_iter()
        .enumerate()
    {
        let (mut engine, source, artifact, creature) =
            aggregate_counter_payment_fixture(602_590 + offset as u64);
        engine.state.players[0].mana_pool.colorless = 1;
        let source_state = engine.state.objects.get_mut(&source).unwrap();
        source_state.tapped = tapped;
        source_state.summoning_sick = summoning_sick;

        let effective = engine
            .effective_activated_abilities(source)
            .into_iter()
            .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
            .expect("the aggregate ability remains available on a tapped or summoning-sick source");
        assert!(
            !effective
                .definition
                .costs
                .iter()
                .any(|cost| matches!(cost, AbilityCost::Tap)),
            "the aggregate ability has no source Tap cost"
        );
        assert!(
            engine.ability_activatable(source, effective.slot as usize, &effective.definition),
            "the exact tapped/summoning-sick combination ({tapped}, {summoning_sick}) is legal without a Tap cost"
        );

        let (pending, source_ref) = begin_aggregate_counter_payment_window(
            &mut engine,
            source,
            artifact,
            creature,
            [true, true],
        );
        let commit = rv1::CommitAbilityActivation {
            transaction_id: pending.transaction_id,
            expected_revision: pending.revision,
            payment: Some(rv1::PaymentSelection {
                expected_state_revision: engine.state.command_index,
                source: Some(source_ref),
                mana: Some(rv1::PaymentMana {
                    c: 1,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        engine
            .apply_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
                },
            )
            .expect("the tapped or summoning-sick source can pay the no-Tap aggregate ability");

        assert_eq!(engine.state.objects[&source].tapped, tapped);
        assert_eq!(engine.state.objects[&source].summoning_sick, summoning_sick);
        assert_eq!(engine.state.players[0].life, 16);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        assert_eq!(
            engine.state.objects[&artifact].counter_count(CounterKind::Charge),
            0
        );
        assert_eq!(
            engine.state.objects[&creature].counter_count(CounterKind::Night),
            0
        );
        assert_eq!(engine.state.stack.len(), 1);
    }
}

#[test]
fn aggregate_counter_begin_rejects_prohibited_activated_ability_without_removing_it() {
    let (mut engine, source, artifact, creature) = aggregate_counter_payment_fixture(602_593);
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .state
        .continuous_effects
        .push(crate::state::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: crate::state::AffectedScope::Single(source),
            kind: tricerules_card_model::ContinuousEffectKind::ProhibitActivatedAbilities,
            condition: None,
            duration: tricerules_card_model::EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index.saturating_add(1),
        });
    let effective = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("the prohibition leaves the activated ability present");
    assert!(
        !engine.ability_activatable(source, effective.slot as usize, &effective.definition),
        "the live prohibition blocks activation while the ability remains effective"
    );
    let begin = aggregate_counter_begin_command(
        &engine,
        source,
        artifact,
        creature,
        aggregate_phyrexian_life_choices([true, true]),
    );
    let before = format!("{:?}", engine.state);
    let result = engine.apply_command(
        10,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
        },
    );
    assert!(
        matches!(result, Err(EngineError::Illegal(_))),
        "Begin rejects the otherwise supported, payable ability under its live prohibition"
    );
    assert_eq!(format!("{:?}", engine.state), before);
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
}

#[test]
fn aggregate_counter_begin_enforces_sorcery_timing_during_upkeep() {
    let definition = aggregate_counter_shape_definition("sorcery_speed");
    let (mut engine, source, artifact, creature) =
        aggregate_counter_payment_fixture_with_definition(602_594, &definition, false);
    engine.state.players[0].mana_pool.colorless = 1;
    engine.state.turn_step = TurnStep::Upkeep;
    let effective = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("the typed SorcerySpeed sibling remains present during Upkeep");
    assert_eq!(
        effective.definition.timing,
        tricerules_card_model::ActivationTiming::SorcerySpeed
    );
    assert_eq!(engine.state.priority_idx, 0, "actor 10 retains priority");
    assert!(
        !engine.ability_activatable(source, effective.slot as usize, &effective.definition),
        "the ability's stricter timing rejects Upkeep despite actor priority"
    );
    let begin = aggregate_counter_begin_command(
        &engine,
        source,
        artifact,
        creature,
        aggregate_phyrexian_life_choices([true, true]),
    );
    let before = format!("{:?}", engine.state);
    let result = engine.apply_command(
        10,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
        },
    );
    assert!(
        matches!(result, Err(EngineError::Illegal(_))),
        "Begin enforces the typed SorcerySpeed restriction during Upkeep"
    );
    assert_eq!(format!("{:?}", engine.state), before);
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
}

#[test]
fn aggregate_counter_begin_enforces_per_turn_limit_after_a_real_payment() {
    let definition = aggregate_counter_shape_definition("per_turn_limit");
    let (mut engine, source, artifact, creature) =
        aggregate_counter_payment_fixture_with_definition(602_595, &definition, false);
    engine.state.players[0].mana_pool.colorless = 2;
    engine
        .state
        .objects
        .get_mut(&artifact)
        .unwrap()
        .add_counters(CounterKind::Charge, 2, 0);
    engine
        .state
        .objects
        .get_mut(&creature)
        .unwrap()
        .add_counters(CounterKind::Night, 1, 0);
    let effective = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("the typed one-per-turn ability is present");
    assert_eq!(
        effective.definition.activation_limit,
        Some(tricerules_card_model::primitives::ActivationLimit::PerTurn { max_activations: 1 })
    );
    assert!(engine.ability_activatable(source, effective.slot as usize, &effective.definition));

    let (pending, source_ref) = begin_aggregate_counter_payment_window(
        &mut engine,
        source,
        artifact,
        creature,
        [true, true],
    );
    let first_commit = rv1::CommitAbilityActivation {
        transaction_id: pending.transaction_id,
        expected_revision: pending.revision,
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_ref),
            mana: Some(rv1::PaymentMana {
                c: 1,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    first_commit,
                )),
            },
        )
        .expect("the first activation pays its complete mana and counter costs");
    assert_eq!(engine.state.players[0].life, 16);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        1
    );

    let effective = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("the spent ability remains present for its second Begin check");
    assert!(
        !engine.ability_activatable(source, effective.slot as usize, &effective.definition),
        "the per-turn limit is the remaining blocker after a complete first payment"
    );
    let second_begin = aggregate_counter_begin_command(
        &engine,
        source,
        artifact,
        creature,
        aggregate_phyrexian_life_choices([true, true]),
    );
    let before = format!("{:?}", engine.state);
    let result = engine.apply_command(
        10,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(
                second_begin,
            )),
        },
    );
    assert!(
        matches!(result, Err(EngineError::Illegal(_))),
        "the second otherwise payable Begin rejects at the per-turn limit"
    );
    assert_eq!(format!("{:?}", engine.state), before);
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
}

#[test]
fn aggregate_counter_begin_rejects_only_a_stale_source_generation_atomically() {
    let (mut engine, source, artifact, creature) = aggregate_counter_payment_fixture(602_596);
    engine.state.players[0].mana_pool.colorless = 1;
    let mut begin = aggregate_counter_begin_command(
        &engine,
        source,
        artifact,
        creature,
        aggregate_phyrexian_life_choices([true, true]),
    );
    begin.expected_zone_change_generation = begin.expected_zone_change_generation.saturating_add(1);
    let before = format!("{:?}", engine.state);
    let result = engine.apply_command(
        10,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
        },
    );
    assert!(
        matches!(result, Err(EngineError::Illegal(_))),
        "Begin rejects only because its source zone-change generation is stale"
    );
    assert_eq!(format!("{:?}", engine.state), before);
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
}

#[test]
fn aggregate_counter_begin_uses_the_derived_controller_only_for_the_new_route() {
    let (mut base, source, artifact10, creature10) = aggregate_counter_payment_fixture(602_513);
    let artifact20 = place_ready(&mut base, 20, "Ornithopter");
    let creature20 = place_ready(&mut base, 20, "Grizzly Bears");
    base.state
        .objects
        .get_mut(&artifact20)
        .expect("the derived controller artifact contributor")
        .add_counters(CounterKind::Charge, 2, 0);
    base.state
        .objects
        .get_mut(&creature20)
        .expect("the derived controller creature contributor")
        .add_counters(CounterKind::Night, 1, 0);
    base.state
        .continuous_effects
        .push(crate::state::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: crate::state::AffectedScope::Single(source),
            kind: tricerules_card_model::ContinuousEffectKind::Layer2Control {
                controller: tricerules_card_model::ControllerReference::Fixed(20),
            },
            condition: None,
            duration: tricerules_card_model::EffectDuration::UntilEndOfTurn,
            timestamp: 100,
        });
    assert_eq!(base.state.objects[&source].controller, 10);
    assert_eq!(base.controller_of(source), Some(20));
    let effective = base
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("the derived controller sees the aggregate ability");

    let mut raw_controller = base.clone();
    raw_controller.state.priority_idx = 0;
    let raw_command = aggregate_counter_begin_command(
        &raw_controller,
        source,
        artifact10,
        creature10,
        aggregate_phyrexian_life_choices([true, true]),
    );
    let before_raw = format!("{:?}", raw_controller.state);
    let raw_result = raw_controller.apply_command(
        10,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(raw_command)),
        },
    );
    let raw_actor_was_rejected_atomically = matches!(raw_result, Err(EngineError::Illegal(_)))
        && format!("{:?}", raw_controller.state) == before_raw;

    let mut derived_controller = base;
    derived_controller.state.priority_idx = 1;
    let derived_command = aggregate_counter_begin_command(
        &derived_controller,
        source,
        artifact20,
        creature20,
        aggregate_phyrexian_life_choices([true, true]),
    );
    let derived_result = derived_controller.apply_command(
        20,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(
                derived_command,
            )),
        },
    );
    assert!(
        raw_actor_was_rejected_atomically
            && derived_result.is_ok()
            && derived_controller
                .state
                .pending_ability_activation
                .as_ref()
                .is_some_and(|pending| pending.actor_player_id == 20),
        "raw controller 10 must be rejected atomically while derived controller 20 is accepted; raw_rejected_atomically={raw_actor_was_rejected_atomically}, derived_result={derived_result:?}, ability_slot={}",
        effective.slot
    );
}

#[test]
fn aggregate_counter_accepted_begin_source_sacrifice_preview_commit_is_atomic() {
    let (mut engine, source, artifact, creature) = aggregate_counter_payment_fixture(602_502);
    let torque = place_ready(&mut engine, 10, "Liquimetal Torque");
    let ironworks = place_ready(&mut engine, 10, "Krark-Clan Ironworks");
    let captured_artifact_land = place_ready(&mut engine, 10, "Captured Artifact Payment Land");
    let captured_spell_land = place_ready(&mut engine, 10, "Captured Artifact Spell Payment Land");
    engine.state.players[0].mana_pool = Default::default();

    let torque_ability = engine
        .effective_activated_abilities(torque)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "activated_02")
        .expect("the actual Liquimetal Torque AddTypes ability is offered");
    let source_receipt = engine.payment_object_ref(source);
    let torque_receipt = engine.payment_object_ref(torque);
    let torque_activation_batch = engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: torque,
                        ability_index: torque_ability.slot,
                        expected_zone_change_generation: torque_receipt.zone_change_generation,
                        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                        targets: vec![rv1::TargetRef {
                            object_id: source,
                            group_index: 0,
                            kind: rv1::TargetRefKind::Permanent as i32,
                            expected_zone_change_generation: Some(
                                source_receipt.zone_change_generation,
                            ),
                            ..Default::default()
                        }],
                        payment: Some(rv1::PaymentSelection {
                            expected_state_revision: engine.state.command_index,
                            source: Some(torque_receipt),
                            mana: Some(rv1::PaymentMana::default()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("actual Torque AddTypes activation is announced");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "actual Torque activation must put one nonmana ability on the stack before passing; batch={:#?}; stack={:#?}",
        torque_activation_batch,
        engine.state.stack
    );
    for _ in 0..2 {
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(
                priority,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("both players pass priority to resolve Liquimetal Torque");
    }
    assert!(
        engine.state.stack.is_empty(),
        "the actual Torque ability resolves before Begin; remaining stack: {:#?}",
        engine.state.stack
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine.state.players[0].battlefield.contains(&source));
    assert_eq!(
        engine.payment_object_ref(source).zone_change_generation,
        source_receipt.zone_change_generation,
        "the stable local creature remains the same battlefield incarnation through Torque resolution"
    );
    assert!(
        engine
            .characteristics(source)
            .is_some_and(|characteristics| characteristics.has_type("Artifact")),
        "the derived Artifact type is present before the accepted activation begins; source={:#?}; effects={:#?}",
        engine.characteristics(source),
        engine.state.continuous_effects
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);

    let aggregate = engine
        .effective_activated_abilities(source)
        .into_iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
        .expect("the local aggregate payment capability is active");
    let group_selection = rv1::CostSelection {
        cost_index: 1,
        selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
            rv1::CounterRemovalGroupSelection {
                contributions: vec![
                    rv1::CounterRemovalContribution {
                        object: Some(engine.payment_object_ref(artifact)),
                        option_id: crate::engine::counters::counter_option_id(CounterKind::Charge),
                        count: 2,
                    },
                    rv1::CounterRemovalContribution {
                        object: Some(engine.payment_object_ref(creature)),
                        option_id: crate::engine::counters::counter_option_id(CounterKind::Night),
                        count: 1,
                    },
                ],
            },
        )),
    };
    let begin = rv1::BeginAbilityActivation {
        source_object_id: source,
        expected_zone_change_generation: source_receipt.zone_change_generation,
        ability_index: aggregate.slot,
        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
        cost_selections: vec![group_selection],
        flex_payments: vec![
            rv1::FlexPipPayment {
                pip_index: 1,
                pay_life: true,
            },
            rv1::FlexPipPayment {
                pip_index: 2,
                pay_life: true,
            },
        ],
        ..Default::default()
    };
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
            },
        )
        .expect("accepted aggregate Begin opens the Payment stage without spending costs");
    let pending = engine
        .state
        .pending_ability_activation
        .as_ref()
        .expect("accepted aggregate payment stage remains pending");
    let transaction_id = pending.transaction_id;
    let pending_revision = pending.revision;

    let mut cancelled_before_mana = engine.clone();
    cancelled_before_mana
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CancelAbilityActivation(
                    rv1::CancelAbilityActivation {
                        transaction_id,
                        expected_revision: pending_revision,
                    },
                )),
            },
        )
        .expect("Cancel is available before independent mana actions");
    assert!(cancelled_before_mana
        .state
        .pending_ability_activation
        .is_none());
    assert!(cancelled_before_mana
        .pending_ability_activation_internal
        .is_none());
    assert_eq!(cancelled_before_mana.state.players[0].life, 20);
    assert_eq!(
        cancelled_before_mana.state.players[0].mana_pool.colorless,
        0
    );
    assert_eq!(
        cancelled_before_mana.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        cancelled_before_mana.state.objects[&creature].counter_count(CounterKind::Night),
        1
    );
    let next_begin = aggregate_counter_begin_command(
        &cancelled_before_mana,
        source,
        artifact,
        creature,
        aggregate_phyrexian_life_choices([true, false]),
    );
    cancelled_before_mana
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(next_begin)),
            },
        )
        .expect("a fresh Begin after Cancel may choose a different Phyrexian payment");
    let next_pending = cancelled_before_mana
        .state
        .pending_ability_activation
        .as_ref()
        .expect("the new choice has its own accepted transaction");
    assert_ne!(next_pending.transaction_id, transaction_id);
    assert_eq!(
        next_pending.flex_payments,
        aggregate_phyrexian_life_choices([true, false])
    );
    assert_eq!(cancelled_before_mana.state.players[0].life, 20);
    assert_eq!(
        cancelled_before_mana.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        cancelled_before_mana.state.objects[&creature].counter_count(CounterKind::Night),
        1
    );

    let ironworks_ability = engine
        .effective_activated_abilities(ironworks)
        .into_iter()
        .find(|ability| {
            ability.definition.is_mana_ability()
                && ability
                    .definition
                    .costs
                    .iter()
                    .any(|cost| matches!(cost, AbilityCost::SacrificePermanent { .. }))
        })
        .expect("the actual KCI mana ability is offered");
    let mut legal = RuledEventBatch::default();
    super::legal_actions::fill_legal(&mut legal, &engine);
    let actor_legal = legal
        .legal_by_player
        .get(&10)
        .expect("the paying actor receives the actual legal mana-ability offer");
    let ironworks_key = (u64::from(ironworks) << 32) | u64::from(ironworks_ability.slot);
    let ironworks_choices = actor_legal
        .cost_choices_by_ability
        .get(&ironworks_key)
        .expect("KCI has actual offered battlefield cost choices during Payment");
    let source_candidate = ironworks_choices
        .choices
        .iter()
        .flat_map(|choice| &choice.candidate_objects)
        .find(|candidate| {
            candidate
                .object
                .as_ref()
                .is_some_and(|object| object.object_id == source)
        })
        .expect("the actually offered KCI artifact sacrifice candidate is the original source");
    let source_cost_ref = source_candidate
        .object
        .expect("offered KCI sacrifice candidate is generation-bound");
    let kci_cost_index = ironworks_choices
        .choices
        .iter()
        .find(|choice| {
            choice
                .candidate_objects
                .iter()
                .any(|candidate| candidate.object.as_ref() == Some(&source_cost_ref))
        })
        .expect("the source is offered for one actual KCI cost")
        .cost_index;
    let ironworks_receipt = engine.payment_object_ref(ironworks);
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: ironworks,
                        ability_index: ironworks_ability.slot,
                        expected_zone_change_generation: ironworks_receipt.zone_change_generation,
                        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                        cost_selections: vec![rv1::CostSelection {
                            cost_index: kci_cost_index,
                            selection: Some(rv1::cost_selection::Selection::PermanentId(
                                source_cost_ref.object_id,
                            )),
                        }],
                        payment: Some(rv1::PaymentSelection {
                            expected_state_revision: engine.state.command_index,
                            source: Some(ironworks_receipt),
                            mana: Some(rv1::PaymentMana::default()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("KCI sacrifices the derived artifact source and produces two colorless mana");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_ne!(
        engine.payment_object_ref(source).zone_change_generation,
        source_receipt.zone_change_generation,
        "the original source's departure advances its generation"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    let pending = engine
        .state
        .pending_ability_activation
        .as_ref()
        .expect("the accepted aggregate Payment window survives source sacrifice");
    assert_eq!(pending.transaction_id, transaction_id);
    assert_eq!(pending.revision, pending_revision);
    assert_eq!(pending.stage(), rv1::AbilityActivationStage::Payment);

    let mut cancelled_after_kci = engine.clone();
    let mana_actions_before_cancel = engine.state.undoable_mana_abilities.clone();
    cancelled_after_kci
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CancelAbilityActivation(
                    rv1::CancelAbilityActivation {
                        transaction_id,
                        expected_revision: pending_revision,
                    },
                )),
            },
        )
        .expect("Cancel closes the accepted cost window after KCI already sacrificed the source");
    assert!(cancelled_after_kci
        .state
        .pending_ability_activation
        .is_none());
    assert!(cancelled_after_kci
        .pending_ability_activation_internal
        .is_none());
    assert_eq!(
        cancelled_after_kci.state.objects[&source].zone,
        Zone::Graveyard
    );
    assert!(cancelled_after_kci.state.players[0]
        .graveyard
        .contains(&source));
    assert_eq!(cancelled_after_kci.state.players[0].mana_pool.colorless, 2);
    assert_eq!(
        format!("{:?}", cancelled_after_kci.state.undoable_mana_abilities),
        format!("{:?}", mana_actions_before_cancel),
        "Cancel leaves the independently recorded KCI mana action intact"
    );
    assert_eq!(cancelled_after_kci.state.players[0].life, 20);
    assert_eq!(
        cancelled_after_kci.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        cancelled_after_kci.state.objects[&creature].counter_count(CounterKind::Night),
        1
    );
    assert!(cancelled_after_kci.state.stack.is_empty());

    let pending_before_restricted_mana = pending.clone();
    let mut artifact_permission = engine.clone();
    let restricted_batch = activate_fixture_mana(&mut artifact_permission, captured_artifact_land);
    let artifact_restriction_group = artifact_permission.state.players[0]
        .restricted_mana
        .last()
        .expect("the actual restricted land created a current mana group")
        .restriction_group_id;
    let artifact_offer = restricted_batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .expect("the post-Begin mana action refreshes the pending offer");
    assert!(artifact_offer
        .eligible_restricted_mana_group_ids
        .contains(&artifact_restriction_group));
    assert_eq!(artifact_offer.transaction_id, transaction_id);
    assert_eq!(artifact_offer.revision, pending_revision);
    assert_eq!(
        artifact_offer.cost_selections,
        pending_before_restricted_mana.cost_selections
    );
    assert_eq!(
        artifact_offer.flex_payments,
        pending_before_restricted_mana.flex_payments
    );
    assert_eq!(
        artifact_offer.locked_total_cost,
        pending_before_restricted_mana.locked_total_cost
    );
    assert_eq!(artifact_permission.state.players[0].mana_pool.colorless, 2);
    let artifact_commit = rv1::CommitAbilityActivation {
        transaction_id,
        expected_revision: pending_revision,
        restricted_mana: vec![rv1::ManaSpendSelection {
            restriction_group_id: artifact_restriction_group,
            c: 1,
            ..Default::default()
        }],
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: artifact_permission.state.command_index,
            source: Some(source_receipt),
            mana: Some(rv1::PaymentMana::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let artifact_before_preview = format!("{:?}", artifact_permission.state);
    let artifact_preview = artifact_permission.preview_payment(
        10,
        &rv1::PreviewPayment {
            transaction_id,
            revision: artifact_permission.state.command_index,
            commit_ability_activation: Some(artifact_commit.clone()),
            ..Default::default()
        },
    );
    assert!(
        artifact_preview.error.is_empty(),
        "{}",
        artifact_preview.error
    );
    assert!(artifact_preview.complete);
    assert_eq!(
        format!("{:?}", artifact_permission.state),
        artifact_before_preview
    );
    artifact_permission
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    artifact_commit,
                )),
            },
        )
        .expect("captured Artifact purpose can spend one newly created restricted mana");
    assert!(artifact_permission
        .state
        .pending_ability_activation
        .is_none());
    assert!(artifact_permission.state.players[0]
        .restricted_mana
        .is_empty());
    assert_eq!(artifact_permission.state.players[0].mana_pool.colorless, 2);
    assert_eq!(artifact_permission.state.players[0].life, 16);
    assert_eq!(
        artifact_permission.state.objects[&artifact].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        artifact_permission.state.objects[&creature].counter_count(CounterKind::Night),
        0
    );
    assert_eq!(artifact_permission.state.stack.len(), 1);

    let mut spell_only = engine.clone();
    let spell_batch = activate_fixture_mana(&mut spell_only, captured_spell_land);
    let spell_restriction_group = spell_only.state.players[0]
        .restricted_mana
        .last()
        .expect("the spell-only land created a current mana group")
        .restriction_group_id;
    let spell_offer = spell_batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .expect("the current Payment offer refreshes after the spell-only mana action");
    assert!(!spell_offer
        .eligible_restricted_mana_group_ids
        .contains(&spell_restriction_group));
    assert_eq!(spell_only.state.players[0].mana_pool.colorless, 2);
    let spell_only_commit = rv1::CommitAbilityActivation {
        transaction_id,
        expected_revision: pending_revision,
        restricted_mana: vec![rv1::ManaSpendSelection {
            restriction_group_id: spell_restriction_group,
            c: 1,
            ..Default::default()
        }],
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: spell_only.state.command_index,
            source: Some(source_receipt),
            mana: Some(rv1::PaymentMana::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let before_spell_rejection = format!("{:?}", spell_only.state);
    assert!(
        matches!(
            spell_only.apply_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                        spell_only_commit,
                    )),
                },
            ),
            Err(EngineError::Illegal(_))
        ),
        "spell-only mana cannot pay an activated ability even when ordinary mana is available"
    );
    assert_eq!(format!("{:?}", spell_only.state), before_spell_rejection);
    assert_eq!(spell_only.state.players[0].mana_pool.colorless, 2);
    assert_eq!(spell_only.state.players[0].life, 20);
    assert_eq!(
        spell_only.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        spell_only.state.objects[&creature].counter_count(CounterKind::Night),
        1
    );
    let spell_pending = spell_only
        .state
        .pending_ability_activation
        .as_ref()
        .expect("the rejected payment keeps the same accepted transaction pending");
    assert_eq!(
        spell_pending.transaction_id,
        pending_before_restricted_mana.transaction_id
    );
    assert_eq!(
        spell_pending.revision,
        pending_before_restricted_mana.revision
    );
    assert_eq!(
        spell_pending.cost_selections,
        pending_before_restricted_mana.cost_selections
    );
    assert_eq!(
        spell_pending.flex_payments,
        pending_before_restricted_mana.flex_payments
    );
    assert_eq!(
        spell_pending.locked_total_cost,
        pending_before_restricted_mana.locked_total_cost
    );

    let payment = rv1::PaymentSelection {
        expected_state_revision: engine.state.command_index,
        source: Some(source_receipt),
        mana: Some(rv1::PaymentMana {
            c: 1,
            ..Default::default()
        }),
        ..Default::default()
    };
    let commit = rv1::CommitAbilityActivation {
        transaction_id,
        expected_revision: pending_revision,
        payment: Some(payment),
        ..Default::default()
    };
    let before_preview = format!("{:?}", engine.state);
    let preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            transaction_id,
            revision: engine.state.command_index,
            commit_ability_activation: Some(commit.clone()),
            ..Default::default()
        },
    );
    assert!(preview.error.is_empty(), "{}", preview.error);
    assert!(
        preview.complete,
        "the reserved 2C can pay generic1 while both Phyrexian pips pay life"
    );
    assert_eq!(
        preview
            .selection
            .as_ref()
            .and_then(|selection| selection.source.as_ref()),
        Some(&source_receipt),
        "Preview retains the original source receipt after the source leaves"
    );
    assert_eq!(
        format!("{:?}", engine.state),
        before_preview,
        "Preview is read-only"
    );
    let commit = rv1::CommitAbilityActivation {
        payment: preview.selection,
        ..commit
    };
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
            },
        )
        .expect("explicit Commit uses the original source receipt and locked group and Phyrexian choices");
    assert!(engine.state.pending_ability_activation.is_none());
    assert_eq!(engine.state.players[0].life, 16);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        0
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one captured activation is put on the stack"
    );
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(source));

    for _ in 0..2 {
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(
                priority,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("both players pass to resolve the captured activation");
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "the absent source does not receive the captured self-counter effect"
    );
}

#[test]
fn aggregate_counter_accepted_route_survives_prepared_source_changes() {
    #[derive(Clone, Copy)]
    enum SourceChange {
        ReturnAsNewGeneration,
        DerivedControl,
        PhaseOut,
        LoseAbilityMenu,
    }

    for (offset, (label, change)) in [
        (
            "return as new generation",
            SourceChange::ReturnAsNewGeneration,
        ),
        ("derived control", SourceChange::DerivedControl),
        ("phase out", SourceChange::PhaseOut),
        ("remove all abilities", SourceChange::LoseAbilityMenu),
    ]
    .into_iter()
    .enumerate()
    {
        let (mut engine, source, artifact, creature) =
            aggregate_counter_payment_fixture(602_530 + offset as u64);
        engine.state.players[0].mana_pool.colorless = 1;
        let (accepted, source_receipt) = begin_aggregate_counter_payment_window(
            &mut engine,
            source,
            artifact,
            creature,
            [true, true],
        );
        assert_eq!(
            accepted.flex_payments,
            aggregate_phyrexian_life_choices([true, true])
        );
        assert_eq!(accepted.cost_selections.len(), 1);
        let accepted_rows = accepted.cost_selections.clone();
        let accepted_flex = accepted.flex_payments.clone();

        match change {
            SourceChange::ReturnAsNewGeneration => {
                crate::engine::resolution::move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    source,
                    Zone::Graveyard,
                    None,
                )
                .expect("the prepared source leaves the battlefield");
                crate::engine::resolution::move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    source,
                    Zone::Battlefield,
                    None,
                )
                .expect("the same physical source object returns as a new incarnation");
                assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
                assert!(engine.state.players[0].battlefield.contains(&source));
                assert_ne!(
                    engine.payment_object_ref(source).zone_change_generation,
                    source_receipt.zone_change_generation,
                    "leave and return keeps the ObjectId but advances its generation"
                );
            }
            SourceChange::DerivedControl => {
                engine
                    .state
                    .continuous_effects
                    .push(crate::state::ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: None,
                        affected: crate::state::AffectedScope::Single(source),
                        kind: tricerules_card_model::ContinuousEffectKind::Layer2Control {
                            controller: tricerules_card_model::ControllerReference::Fixed(20),
                        },
                        condition: None,
                        duration: tricerules_card_model::EffectDuration::UntilEndOfTurn,
                        timestamp: engine.state.command_index.saturating_add(1),
                    });
                let mut control_events = Vec::new();
                engine
                    .reindex_battlefield_control(&mut control_events)
                    .expect("materialize the prepared derived-control change");
                assert_eq!(engine.state.objects[&source].controller, 20);
                assert!(engine.state.players[1].battlefield.contains(&source));
            }
            SourceChange::PhaseOut => {
                engine.state.objects.get_mut(&source).unwrap().phase_status =
                    Some(crate::state::PhasedOutStatus {
                        phase_out_under: 10,
                        phase_in_due_after_departed_turn: false,
                        phase_in_with: None,
                        direct_attachment_snapshot: None,
                    });
                let mut phase_events = Vec::new();
                engine
                    .reindex_battlefield_control(&mut phase_events)
                    .expect("materialize the prepared phasing change");
                assert!(engine.state.objects[&source].phase_status.is_some());
            }
            SourceChange::LoseAbilityMenu => {
                engine
                    .state
                    .continuous_effects
                    .push(crate::state::ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: None,
                        affected: crate::state::AffectedScope::Single(source),
                        kind: tricerules_card_model::ContinuousEffectKind::Layer6RemoveAllAbilities,
                        condition: None,
                        duration: tricerules_card_model::EffectDuration::UntilEndOfTurn,
                        timestamp: engine.state.command_index.saturating_add(1),
                    });
                assert!(
                    engine.effective_activated_abilities(source).is_empty(),
                    "the live source menu is removed while the accepted occurrence remains"
                );
            }
        }

        let transition_snapshot = format!("{:?}", engine.state);
        let mut refreshed = RuledEventBatch::default();
        super::legal_actions::fill_legal(&mut refreshed, &engine);
        let actor_offer = refreshed.legal_by_player[&10]
            .pending_ability_activation
            .as_ref()
            .expect("fresh LegalActions reconstructs the accepted offer from its captured route");
        assert_eq!(
            actor_offer.transaction_id, accepted.transaction_id,
            "{label}"
        );
        assert_eq!(actor_offer.revision, accepted.revision, "{label}");
        assert_eq!(actor_offer.cost_selections, accepted_rows, "{label}");
        assert_eq!(actor_offer.flex_payments, accepted_flex, "{label}");
        assert_eq!(
            actor_offer.locked_total_cost, accepted.locked_total_cost,
            "{label}"
        );
        if matches!(change, SourceChange::LoseAbilityMenu) {
            assert!(actor_offer.payment_preview.is_some());
            assert_eq!(
                format!("{:?}", engine.state),
                transition_snapshot,
                "a fresh offer does not mutate the state after ability-menu loss"
            );
        }

        let commit = rv1::CommitAbilityActivation {
            transaction_id: accepted.transaction_id,
            expected_revision: accepted.revision,
            payment: Some(rv1::PaymentSelection {
                expected_state_revision: engine.state.command_index,
                source: Some(source_receipt),
                mana: Some(rv1::PaymentMana {
                    c: 1,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let before_preview = format!("{:?}", engine.state);
        let preview = engine.preview_payment(
            10,
            &rv1::PreviewPayment {
                transaction_id: accepted.transaction_id,
                revision: engine.state.command_index,
                commit_ability_activation: Some(commit.clone()),
                ..Default::default()
            },
        );
        assert!(preview.error.is_empty(), "{label}: {}", preview.error);
        assert!(preview.complete, "{label}");
        assert_eq!(
            preview
                .selection
                .as_ref()
                .and_then(|selection| selection.source.as_ref()),
            Some(&source_receipt),
            "{label}: Preview retains the original source receipt"
        );
        assert_eq!(
            format!("{:?}", engine.state),
            before_preview,
            "{label}: Preview is read-only after the direct source transition"
        );
        engine
            .apply_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                        rv1::CommitAbilityActivation {
                            payment: preview.selection,
                            ..commit
                        },
                    )),
                },
            )
            .unwrap_or_else(|error| {
                panic!("{label}: original actor commits captured route: {error}")
            });
        assert!(engine.state.pending_ability_activation.is_none(), "{label}");
        assert!(
            engine.pending_ability_activation_internal.is_none(),
            "{label}"
        );
        assert_eq!(engine.state.players[0].life, 16, "{label}");
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0, "{label}");
        assert_eq!(
            engine.state.objects[&artifact].counter_count(CounterKind::Charge),
            0,
            "{label}"
        );
        assert_eq!(
            engine.state.objects[&creature].counter_count(CounterKind::Night),
            0,
            "{label}"
        );
        assert_eq!(
            engine.state.stack.len(),
            1,
            "{label}: one original occurrence is finished"
        );
        assert_eq!(
            engine.state.stack[0].controller, 10,
            "{label}: original actor controls the stack object"
        );
        assert_eq!(
            engine.state.stack[0].source_zone_change, source_receipt.zone_change_generation,
            "{label}"
        );
        if matches!(change, SourceChange::ReturnAsNewGeneration) {
            assert_ne!(
                engine.payment_object_ref(source).zone_change_generation,
                engine.state.stack[0].source_zone_change,
                "{label}: the stack occurrence remains bound to the old source generation"
            );
        }
        for _ in 0..2 {
            let priority = engine.state.priority_player_id();
            engine
                .apply_command(
                    priority,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .unwrap_or_else(|error| {
                    panic!("{label}: pass to resolve the captured ability: {error}")
                });
        }
        assert!(
            engine.state.stack.is_empty(),
            "{label}: captured ability resolves"
        );
        let expected_source_counters = match change {
            SourceChange::ReturnAsNewGeneration | SourceChange::PhaseOut => 0,
            SourceChange::DerivedControl | SourceChange::LoseAbilityMenu => 1,
        };
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            expected_source_counters,
            "{label}: the captured effect applies only to its still-live source incarnation"
        );
    }
}

#[test]
fn aggregate_counter_actor_concede_abandons_without_payment() {
    let (mut engine, source, artifact, creature) = aggregate_counter_payment_fixture(602_534);
    engine.state.players[0].mana_pool.colorless = 1;
    let (pending, source_receipt) = begin_aggregate_counter_payment_window(
        &mut engine,
        source,
        artifact,
        creature,
        [true, true],
    );
    assert_eq!(pending.actor_player_id, 10);
    assert_eq!(
        pending.flex_payments,
        aggregate_phyrexian_life_choices([true, true])
    );
    assert_eq!(
        engine.payment_object_ref(source).zone_change_generation,
        source_receipt.zone_change_generation
    );

    concede(&mut engine, 10);

    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[0].life, 20,
        "Concede spends neither locked Phyrexian pip"
    );
    assert_eq!(
        engine.state.players[0].mana_pool.colorless, 1,
        "the offered mana remains untouched"
    );
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        1
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn aggregate_counter_nested_mana_choice_masks_and_restores_accepted_payment() {
    let (mut engine, source, artifact, creature) = aggregate_counter_payment_fixture(602_535);
    let pain_land = place_ready(&mut engine, 10, "Pain Payment Land");
    engine.state.players[0].mana_pool = Default::default();
    engine.state.add_damage_prevention_shield(10, 1);
    engine.state.add_damage_prevention_shield(10, 1);
    let (accepted, source_receipt) = begin_aggregate_counter_payment_window(
        &mut engine,
        source,
        artifact,
        creature,
        [true, true],
    );
    let accepted_rows = accepted.cost_selections.clone();
    let accepted_flex = accepted.flex_payments.clone();
    assert_eq!(
        accepted_flex,
        aggregate_phyrexian_life_choices([true, true])
    );

    let mana_batch = activate_fixture_mana(&mut engine, pain_land);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the real Pain Payment Land damage opens the two-shield order choice")
        .presentation
        .candidates[0];
    let canonical_pending = engine
        .state
        .pending_ability_activation
        .as_ref()
        .expect("the original aggregate Payment route remains canonical under nested work");
    assert_eq!(canonical_pending.transaction_id, accepted.transaction_id);
    assert_eq!(canonical_pending.revision, accepted.revision);
    assert_eq!(canonical_pending.cost_selections, accepted_rows);
    assert_eq!(canonical_pending.flex_payments, accepted_flex);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    let actor_nested_offer = mana_batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .expect("the mana action response contains the masked actor projection");
    assert!(actor_nested_offer.cost_selections.is_empty());
    assert!(actor_nested_offer.flex_payments.is_empty());
    assert!(actor_nested_offer.payment_preview.is_none());

    let nested_snapshot = format!("{:?}", engine.state);
    for viewer in [10, 20, -1] {
        let projected = engine
            .pending_ability_activation_for(viewer)
            .expect("every viewer receives the still-pending public activation");
        assert!(projected.cost_selections.is_empty(), "viewer {viewer}");
        assert!(projected.flex_payments.is_empty(), "viewer {viewer}");
        assert!(projected.payment_preview.is_none(), "viewer {viewer}");
        assert!(projected.locked_total_cost.is_empty(), "viewer {viewer}");
        assert!(
            pending_public_projection_matches(&projected, canonical_pending),
            "viewer {viewer}"
        );
    }
    assert_eq!(
        format!("{:?}", engine.state),
        nested_snapshot,
        "nested projections cannot mutate canonical rows or flex choices"
    );
    assert_eq!(
        engine
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .cost_selections,
        accepted_rows,
        "canonical group rows stay present while every viewer is masked"
    );
    assert_eq!(
        engine
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .flex_payments,
        accepted_flex,
        "canonical fixed Phyrexian choices stay present while every viewer is masked"
    );

    let resumed_batch = engine
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
        .expect("the actual actor answers the nested prevention-shield choice");
    assert!(engine.state.pending_resolution.is_none());
    let resumed_canonical = engine
        .state
        .pending_ability_activation
        .as_ref()
        .expect("the same aggregate Payment route resumes after nested resolution");
    assert_eq!(resumed_canonical.transaction_id, accepted.transaction_id);
    assert_eq!(resumed_canonical.revision, accepted.revision);
    assert_eq!(resumed_canonical.cost_selections, accepted_rows);
    assert_eq!(resumed_canonical.flex_payments, accepted_flex);
    let actor_offer = resumed_batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .expect("resumption returns the actor offer");
    assert_eq!(actor_offer.transaction_id, accepted.transaction_id);
    assert_eq!(actor_offer.cost_selections, accepted_rows);
    assert_eq!(actor_offer.flex_payments, accepted_flex);
    let payment_preview = actor_offer
        .payment_preview
        .as_ref()
        .expect("the actor regains its payment preview after nested work");
    assert!(
        payment_preview.error.is_empty(),
        "{}",
        payment_preview.error
    );
    assert!(
        !payment_preview.complete,
        "the restored candidate preview stays incomplete until the actor supplies a mana selection"
    );

    let resumed_snapshot = format!("{:?}", engine.state);
    let actor_projection = engine
        .pending_ability_activation_for(10)
        .expect("actor projection resumes with the same accepted payment");
    assert_eq!(actor_projection.transaction_id, accepted.transaction_id);
    assert_eq!(actor_projection.cost_selections, accepted_rows);
    assert_eq!(actor_projection.flex_payments, accepted_flex);
    let actor_payment_preview = actor_projection
        .payment_preview
        .as_ref()
        .expect("actor projection restores its candidate payment preview");
    assert!(
        actor_payment_preview.error.is_empty(),
        "{}",
        actor_payment_preview.error
    );
    assert!(!actor_payment_preview.complete);
    for viewer in [20, -1] {
        let projected = engine
            .pending_ability_activation_for(viewer)
            .expect("nonactor resumes the public activation projection");
        assert!(projected.cost_selections.is_empty(), "viewer {viewer}");
        assert!(projected.flex_payments.is_empty(), "viewer {viewer}");
        assert!(projected.payment_preview.is_none(), "viewer {viewer}");
    }
    assert_eq!(
        format!("{:?}", engine.state),
        resumed_snapshot,
        "resumed projections are read-only"
    );

    let commit = rv1::CommitAbilityActivation {
        transaction_id: accepted.transaction_id,
        expected_revision: accepted.revision,
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_receipt),
            mana: Some(rv1::PaymentMana {
                c: 1,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let before_preview = format!("{:?}", engine.state);
    let preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            transaction_id: accepted.transaction_id,
            revision: engine.state.command_index,
            commit_ability_activation: Some(commit.clone()),
            ..Default::default()
        },
    );
    assert!(preview.error.is_empty(), "{}", preview.error);
    assert!(preview.complete);
    assert_eq!(
        preview
            .selection
            .as_ref()
            .and_then(|selection| selection.source.as_ref()),
        Some(&source_receipt)
    );
    assert_eq!(format!("{:?}", engine.state), before_preview);
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    rv1::CommitAbilityActivation {
                        payment: preview.selection,
                        ..commit
                    },
                )),
            },
        )
        .expect("the original actor commits the same payment after nested choice resumes");
    assert!(engine.state.pending_ability_activation.is_none());
    assert_eq!(engine.state.players[0].life, 16);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        0
    );
    assert_eq!(engine.state.stack.len(), 1);
}

#[test]
fn aggregate_counter_uses_live_lattice_permission_after_begin() {
    let (mut engine, source, artifact, creature) = aggregate_counter_payment_fixture(602_536);
    let lattice = place_ready(&mut engine, 10, "Mycosynth Lattice");
    engine.state.players[0].mana_pool = Default::default();
    engine.state.players[0].mana_pool.red = 3;
    assert!(engine.may_spend_mana_as_any_color(10));
    let (accepted, source_receipt) = begin_aggregate_counter_payment_window(
        &mut engine,
        source,
        artifact,
        creature,
        [false, false],
    );
    assert_eq!(
        accepted.flex_payments,
        aggregate_phyrexian_life_choices([false, false])
    );

    engine
        .state
        .continuous_effects
        .push(crate::state::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: crate::state::AffectedScope::Single(lattice),
            kind: tricerules_card_model::ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: tricerules_card_model::EffectDuration::Indefinite,
            timestamp: engine.state.command_index.saturating_add(1),
        });
    assert!(!engine.may_spend_mana_as_any_color(10));

    let unavailable_permission_commit = rv1::CommitAbilityActivation {
        transaction_id: accepted.transaction_id,
        expected_revision: accepted.revision,
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_receipt),
            mana: Some(rv1::PaymentMana {
                r: 3,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let before_invalid_preview = format!("{:?}", engine.state);
    let invalid_preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            transaction_id: accepted.transaction_id,
            revision: engine.state.command_index,
            commit_ability_activation: Some(unavailable_permission_commit.clone()),
            ..Default::default()
        },
    );
    assert!(
        !invalid_preview.complete,
        "off-color red cannot satisfy the two locked blue Phyrexian pips after Lattice loses its ability"
    );
    assert_eq!(format!("{:?}", engine.state), before_invalid_preview);
    assert_aggregate_commit_rejected_unchanged(
        &mut engine,
        10,
        unavailable_permission_commit.clone(),
        "off-color payment after the live Lattice permission is removed",
    );

    engine.state.continuous_effects.pop();
    assert!(engine.may_spend_mana_as_any_color(10));
    let before_valid_preview = format!("{:?}", engine.state);
    let valid_preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            transaction_id: accepted.transaction_id,
            revision: engine.state.command_index,
            commit_ability_activation: Some(unavailable_permission_commit.clone()),
            ..Default::default()
        },
    );
    assert!(valid_preview.error.is_empty(), "{}", valid_preview.error);
    assert!(valid_preview.complete);
    assert_eq!(format!("{:?}", engine.state), before_valid_preview);
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    rv1::CommitAbilityActivation {
                        payment: valid_preview.selection,
                        ..unavailable_permission_commit
                    },
                )),
            },
        )
        .expect("restoring the live Lattice ability permits retrying the unchanged payment");
    assert!(engine.state.pending_ability_activation.is_none());
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        0
    );
    assert_eq!(engine.state.stack.len(), 1);
}

#[test]
fn aggregate_counter_accepted_route_survives_source_transform_face_change() {
    let (mut engine, source, artifact, creature) =
        aggregate_counter_payment_fixture_with_definition_and_name(
            602_537,
            C2_AGGREGATE_TRANSFORM_PAYMENT_FIXTURE,
            "C2 Aggregate Transform Payment Fixture",
            false,
        );
    engine.state.players[0].mana_pool.colorless = 1;
    assert_eq!(engine.state.objects[&source].face_up_index, 0);
    let (accepted, source_receipt) = begin_aggregate_counter_payment_window(
        &mut engine,
        source,
        artifact,
        creature,
        [true, true],
    );
    let accepted_rows = accepted.cost_selections.clone();
    let accepted_flex = accepted.flex_payments.clone();
    let old_face_generation = engine
        .state
        .face_change_generation
        .get(&source)
        .copied()
        .unwrap_or_default();

    let mut face_events = Vec::new();
    assert!(engine
        .change_permanent_face(source, FaceChangeAction::Transform, &mut face_events)
        .expect("prepared Transform changes the source face"));
    assert_eq!(engine.state.objects[&source].face_up_index, 1);
    assert_eq!(
        engine.payment_object_ref(source).zone_change_generation,
        source_receipt.zone_change_generation,
        "Transform preserves the same battlefield object incarnation"
    );
    let transformed_face_generation = engine.state.face_change_generation[&source];
    assert!(transformed_face_generation > old_face_generation);
    assert!(
        engine.effective_activated_abilities(source).is_empty(),
        "the transformed back face has no aggregate ability menu"
    );
    let transformed_characteristics = engine.characteristics(source).unwrap();
    assert_eq!(transformed_characteristics.power, Some(2));
    assert_eq!(transformed_characteristics.toughness, Some(3));

    let transformed_snapshot = format!("{:?}", engine.state);
    let mut refreshed = RuledEventBatch::default();
    super::legal_actions::fill_legal(&mut refreshed, &engine);
    let actor_offer = refreshed.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .expect(
            "fresh LegalActions reconstructs the accepted offer after Transform removes the menu",
        );
    assert_eq!(actor_offer.transaction_id, accepted.transaction_id);
    assert_eq!(actor_offer.revision, accepted.revision);
    assert_eq!(actor_offer.cost_selections, accepted_rows);
    assert_eq!(actor_offer.flex_payments, accepted_flex);
    assert_eq!(actor_offer.locked_total_cost, accepted.locked_total_cost);
    assert_eq!(format!("{:?}", engine.state), transformed_snapshot);

    let commit = rv1::CommitAbilityActivation {
        transaction_id: accepted.transaction_id,
        expected_revision: accepted.revision,
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_receipt),
            mana: Some(rv1::PaymentMana {
                c: 1,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let before_preview = format!("{:?}", engine.state);
    let preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            transaction_id: accepted.transaction_id,
            revision: engine.state.command_index,
            commit_ability_activation: Some(commit.clone()),
            ..Default::default()
        },
    );
    assert!(preview.error.is_empty(), "{}", preview.error);
    assert!(preview.complete);
    assert_eq!(
        preview
            .selection
            .as_ref()
            .and_then(|selection| selection.source.as_ref()),
        Some(&source_receipt),
        "Preview retains the original source receipt through face change"
    );
    assert_eq!(format!("{:?}", engine.state), before_preview);
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    rv1::CommitAbilityActivation {
                        payment: preview.selection,
                        ..commit
                    },
                )),
            },
        )
        .expect("the actor commits the captured front-face ability from the same object");
    assert!(engine.state.pending_ability_activation.is_none());
    let stack_item = engine
        .state
        .stack
        .last()
        .expect("the captured ability is on the stack");
    assert_eq!(stack_item.source_permanent_id, Some(source));
    assert_eq!(
        stack_item.source_zone_change,
        source_receipt.zone_change_generation
    );
    assert_eq!(stack_item.source_face_change, old_face_generation);
    assert_ne!(stack_item.source_face_change, transformed_face_generation);
    assert_eq!(
        stack_item
            .activated_ability
            .as_ref()
            .map(|ability| ability.ability_id.as_str()),
        Some("aggregate_payment_01"),
        "the stack retains the front-face activated ability definition"
    );

    for _ in 0..2 {
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(
                priority,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("both players pass to resolve the captured front-face ability");
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the same battlefield incarnation receives its captured effect after transforming"
    );
}

#[test]
fn aggregate_counter_unsupported_shapes_stay_unoffered_and_begin_atomic() {
    for (case_index, shape) in ["source_tap", "hand", "modal", "targeted", "variable"]
        .into_iter()
        .enumerate()
    {
        let source_in_hand = shape == "hand";
        let definition = aggregate_counter_shape_definition(shape);
        let (mut engine, source, artifact, creature) =
            aggregate_counter_payment_fixture_with_definition(
                602_520 + case_index as u64,
                &definition,
                source_in_hand,
            );
        {
            let pool = &mut engine.state.players[0].mana_pool;
            pool.white = 20;
            pool.blue = 20;
            pool.black = 20;
            pool.red = 20;
            pool.green = 20;
            pool.colorless = 20;
        }

        let (ability_index, ability) = if source_in_hand {
            engine
                .authored_zone_activated_abilities(source, AbilitySourceZone::Hand)
                .into_iter()
                .find(|(_, ability, _)| ability.ability_id.as_str() == "aggregate_payment_01")
                .map(|(index, ability, _)| (index as u32, ability))
                .expect("the typed hand fixture exposes its authored hand ability")
        } else {
            let effective = engine
                .effective_activated_abilities(source)
                .into_iter()
                .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
                .expect("the typed battlefield fixture exposes its activated ability");
            (effective.slot, effective.definition)
        };
        let group_cost_index = ability
            .costs
            .iter()
            .position(|cost| matches!(cost, AbilityCost::RemoveCountersFromPermanents { .. }))
            .expect("the shape sibling retains its aggregate counter cost");
        let group_selection = rv1::CostSelection {
            cost_index: group_cost_index as u32,
            selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
                rv1::CounterRemovalGroupSelection {
                    contributions: vec![
                        rv1::CounterRemovalContribution {
                            object: Some(engine.payment_object_ref(artifact)),
                            option_id: crate::engine::counters::counter_option_id(
                                CounterKind::Charge,
                            ),
                            count: 2,
                        },
                        rv1::CounterRemovalContribution {
                            object: Some(engine.payment_object_ref(creature)),
                            option_id: crate::engine::counters::counter_option_id(
                                CounterKind::Night,
                            ),
                            count: 1,
                        },
                    ],
                },
            )),
        };
        let flex_payments = if shape == "variable" {
            Vec::new()
        } else {
            vec![
                rv1::FlexPipPayment {
                    pip_index: 1,
                    pay_life: true,
                },
                rv1::FlexPipPayment {
                    pip_index: 2,
                    pay_life: true,
                },
            ]
        };
        let mut legal = RuledEventBatch::default();
        super::legal_actions::fill_legal(&mut legal, &engine);
        let actor_legal = legal
            .legal_by_player
            .get(&10)
            .expect("the actor receives public legal action data");
        if source_in_hand {
            let hand_action = actor_legal
                .zone_ability_actions
                .iter()
                .find(|action| action.object_id == source)
                .expect("the actual hand ability is publicly projected");
            assert!(
                !hand_action
                    .ability
                    .as_ref()
                    .is_some_and(|info| info.activatable),
                "hand aggregate sibling must not be offered as activatable"
            );
            let key = (u64::from(source) << 32) | u64::from(ability_index);
            if let Some(choices) = actor_legal.cost_choices_by_ability.get(&key) {
                assert!(!choices.non_mana_costs_payable);
                assert!(
                    choices
                        .choices
                        .iter()
                        .all(|choice| choice.counter_removal_group.is_none()),
                    "hand aggregate sibling has no grouped payable offer"
                );
            }
        } else {
            let event = engine.ev_zone_view_sync_tracked();
            let Some(rv1::ruled_event::Ev::ZoneView(zone_view)) = event.ev else {
                panic!("the actual public state event includes a zone view");
            };
            let actor_view = zone_view
                .per_player
                .iter()
                .find(|view| view.player_id == 10)
                .expect("the actor receives its battlefield view");
            let source_view = actor_view
                .battlefield_objects
                .iter()
                .find(|object| object.object_id == source)
                .expect("the source is present in the public battlefield view");
            let ability_info = source_view
                .activated_abilities
                .iter()
                .find(|info| info.ability_index == ability_index)
                .expect("the unsupported sibling has public AbilityInfo");
            assert!(
                !ability_info.activatable,
                "{shape} sibling stays unactivatable"
            );
            let key = (u64::from(source) << 32) | u64::from(ability_index);
            let choices = actor_legal
                .cost_choices_by_ability
                .get(&key)
                .expect("the unsupported battlefield sibling has keyed public costs");
            assert!(
                choices
                    .choices
                    .iter()
                    .all(|choice| choice.counter_removal_group.is_none()),
                "{shape} sibling has no grouped payable offer"
            );
        }

        let source_zone = if source_in_hand {
            rv1::AbilitySourceZone::Hand as i32
        } else {
            rv1::AbilitySourceZone::Battlefield as i32
        };
        let begin = rv1::BeginAbilityActivation {
            source_object_id: source,
            expected_zone_change_generation: engine
                .payment_object_ref(source)
                .zone_change_generation,
            ability_index,
            source_zone,
            cost_selections: vec![group_selection],
            flex_payments,
            own_target: (shape == "targeted").then_some(rv1::AbilityActivationTarget {
                object_id: creature,
                zone_change_generation: engine.payment_object_ref(creature).zone_change_generation,
                group_index: 0,
            }),
            ..Default::default()
        };
        let before = serde_json::to_value(&engine.state)
            .expect("serialize complete state before unsupported Begin");
        let result = engine.apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
            },
        );
        assert!(
            matches!(result, Err(EngineError::Illegal(_))),
            "{shape} aggregate sibling must fail Begin as Illegal"
        );
        assert_eq!(
            serde_json::to_value(&engine.state).expect("serialize state after rejected Begin"),
            before,
            "{shape} unsupported Begin preserves state, pending data, history and allocators"
        );
        assert!(engine.pending_ability_activation_internal.is_none());
    }
}

#[test]
fn aggregate_counter_hybrid_mana_shape_is_unoffered_and_begin_atomic_illegal() {
    for (case_index, shape) in ["hybrid", "mono_hybrid"].into_iter().enumerate() {
        let definition = aggregate_counter_shape_definition(shape);
        let (mut engine, source, artifact, creature) =
            aggregate_counter_payment_fixture_with_definition(
                602_530 + case_index as u64,
                &definition,
                false,
            );
        {
            let pool = &mut engine.state.players[0].mana_pool;
            pool.white = 20;
            pool.blue = 20;
            pool.black = 20;
            pool.red = 20;
            pool.green = 20;
            pool.colorless = 20;
        }
        let effective = engine
            .effective_activated_abilities(source)
            .into_iter()
            .find(|ability| ability.definition.ability_id.as_str() == "aggregate_payment_01")
            .expect("the model-valid flexible-mana sibling is active");
        let event = engine.ev_zone_view_sync_tracked();
        let Some(rv1::ruled_event::Ev::ZoneView(zone_view)) = event.ev else {
            panic!("the actual public state event includes a zone view");
        };
        let actor_view = zone_view
            .per_player
            .iter()
            .find(|view| view.player_id == 10)
            .expect("the actor receives its battlefield view");
        let source_view = actor_view
            .battlefield_objects
            .iter()
            .find(|object| object.object_id == source)
            .expect("the source is present in the public battlefield view");
        let ability_info = source_view
            .activated_abilities
            .iter()
            .find(|ability| ability.ability_index == effective.slot)
            .expect("hybrid sibling has public AbilityInfo");
        assert!(
            !ability_info.activatable,
            "{shape} mana is not lockable at Begin"
        );

        let mut legal = RuledEventBatch::default();
        super::legal_actions::fill_legal(&mut legal, &engine);
        let actor_legal = legal
            .legal_by_player
            .get(&10)
            .expect("the actor receives public legal action data");
        let key = (u64::from(source) << 32) | u64::from(effective.slot);
        let choices = actor_legal
            .cost_choices_by_ability
            .get(&key)
            .expect("the sibling retains keyed structural cost status");
        assert!(
            choices
                .choices
                .iter()
                .all(|choice| choice.counter_removal_group.is_none()),
            "{shape} mana must not expose a grouped payable offer"
        );

        let group_cost_index = effective
            .definition
            .costs
            .iter()
            .position(|cost| matches!(cost, AbilityCost::RemoveCountersFromPermanents { .. }))
            .expect("hybrid sibling retains its group cost");
        let begin = rv1::BeginAbilityActivation {
            source_object_id: source,
            expected_zone_change_generation: engine
                .payment_object_ref(source)
                .zone_change_generation,
            ability_index: effective.slot,
            source_zone: rv1::AbilitySourceZone::Battlefield as i32,
            cost_selections: vec![rv1::CostSelection {
                cost_index: group_cost_index as u32,
                selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
                    rv1::CounterRemovalGroupSelection {
                        contributions: vec![
                            rv1::CounterRemovalContribution {
                                object: Some(engine.payment_object_ref(artifact)),
                                option_id: crate::engine::counters::counter_option_id(
                                    CounterKind::Charge,
                                ),
                                count: 2,
                            },
                            rv1::CounterRemovalContribution {
                                object: Some(engine.payment_object_ref(creature)),
                                option_id: crate::engine::counters::counter_option_id(
                                    CounterKind::Night,
                                ),
                                count: 1,
                            },
                        ],
                    },
                )),
            }],
            ..Default::default()
        };
        let before = serde_json::to_value(&engine.state)
            .expect("serialize state before flexible-mana Begin");
        let result = engine.apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
            },
        );
        assert!(
            matches!(result, Err(EngineError::Illegal(_))),
            "{shape} aggregate Begin must reject before allocating pending state"
        );
        assert_eq!(
            serde_json::to_value(&engine.state).expect("serialize state after rejected Begin"),
            before
        );
        assert!(engine.pending_ability_activation_internal.is_none());
    }
}

#[test]
fn aggregate_counter_public_offer_keeps_legacy_direct_activation_atomic_illegal() {
    let (mut engine, source, artifact, creature) = aggregate_counter_cost_fixture(602_420);
    let effective = engine.effective_activated_abilities(source);
    assert_eq!(
        effective.len(),
        2,
        "both printed fixture abilities are active"
    );
    let aggregate = effective
        .iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_01")
        .expect("aggregate-cost ability");
    let singleton = effective
        .iter()
        .find(|ability| ability.definition.ability_id.as_str() == "singleton_control_02")
        .expect("supported singleton control ability");
    let (aggregate_count, filter) = match &aggregate.definition.costs[0] {
        AbilityCost::RemoveCountersFromPermanents {
            count,
            counter,
            filter,
        } => {
            assert_eq!(*count, 3);
            assert!(counter.is_none());
            (*count, filter)
        }
        other => panic!("fixture aggregate cost has expected B1 shape: {other:?}"),
    };
    assert_eq!(aggregate_count, 3);

    for (object_id, kind, count) in [
        (artifact, CounterKind::Charge, 2),
        (creature, CounterKind::Night, 1),
    ] {
        let object = &engine.state.objects[&object_id];
        assert_eq!(object.zone, Zone::Battlefield);
        assert_eq!(object.controller, 10);
        assert!(engine.state.players[0].battlefield.contains(&object_id));
        assert_eq!(object.counter_count(kind), count);
        assert!(engine.counter_payment_permanent_matches(10, source, object_id, filter));
    }
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge)
            + engine.state.objects[&creature].counter_count(CounterKind::Night),
        aggregate_count,
        "distinct eligible counter kinds across two eligible permanents cover the cost"
    );
    assert!(
        !engine.counter_payment_permanent_matches(10, source, source, filter),
        "the filter excludes the activation source"
    );
    assert!(!engine.counter_costs_payable(source, &aggregate.definition.costs));
    assert!(engine.counter_costs_payable(source, &singleton.definition.costs));

    let aggregate_info = super::legal_actions::activated_ability_info(&engine, source, aggregate);
    assert!(aggregate_info.activatable);
    assert!(!aggregate_info.cost_label.contains("unsupported"));
    let singleton_info = super::legal_actions::activated_ability_info(&engine, source, singleton);
    assert!(
        singleton_info.activatable,
        "the singleton positive control is legal"
    );

    let mut batch = RuledEventBatch::default();
    super::legal_actions::fill_legal(&mut batch, &engine);
    let public_legal = batch
        .legal_by_player
        .get(&10)
        .expect("the active player receives legal action data");
    let aggregate_key = (u64::from(source) << 32) | u64::from(aggregate.slot);
    let aggregate_choices = public_legal
        .cost_choices_by_ability
        .get(&aggregate_key)
        .expect("the supported aggregate ability has keyed cost data");
    assert!(aggregate_choices.non_mana_costs_payable);
    let group = aggregate_choices
        .choices
        .iter()
        .find_map(|choice| choice.counter_removal_group.as_ref())
        .expect("the supported public route exposes grouped counter-removal choices");
    assert_eq!(group.required_count, aggregate_count);
    assert!(group.candidates.iter().any(|candidate| {
        candidate
            .object
            .as_ref()
            .is_some_and(|object| object.object_id == artifact)
    }));
    assert!(group.candidates.iter().any(|candidate| {
        candidate
            .object
            .as_ref()
            .is_some_and(|object| object.object_id == creature)
    }));
    assert!(aggregate_choices.cast_cost_groups.is_empty());

    let singleton_key = (u64::from(source) << 32) | u64::from(singleton.slot);
    let singleton_choices = public_legal
        .cost_choices_by_ability
        .get(&singleton_key)
        .expect("supported control has its ordinary cost offer");
    assert!(singleton_choices.non_mana_costs_payable);
    assert_eq!(singleton_choices.choices.len(), 1);
    assert!(singleton_choices.choices[0]
        .candidate_ids
        .contains(&artifact));
    let singleton_counter_choice = singleton_choices.choices[0]
        .counter_removal
        .as_ref()
        .expect("legacy singleton counter choice remains available");
    assert!(singleton_counter_choice.source.is_none());
    assert_eq!(singleton_counter_choice.count, 1);
    assert_eq!(
        singleton_counter_choice.options[0].option_id,
        crate::engine::counters::counter_option_id(CounterKind::Charge)
    );

    let source_ref = engine.payment_object_ref(source);
    let artifact_ref = engine.payment_object_ref(artifact);
    let singleton_selection = rv1::CostSelection {
        cost_index: 0,
        selection: Some(rv1::cost_selection::Selection::CounterRemoval(
            rv1::CounterRemovalSelection {
                source: Some(artifact_ref),
                option_id: crate::engine::counters::counter_option_id(CounterKind::Charge),
            },
        )),
    };
    let assert_no_pending_activation = |engine: &GameEngine| {
        assert!(engine.state.pending_ability_activation.is_none());
        assert!(engine.pending_ability_activation_internal.is_none());
    };

    for cost_selections in [Vec::new(), vec![singleton_selection.clone()]] {
        let proposal = rv1::ActivateAbility {
            source_object_id: source,
            ability_index: aggregate.slot,
            expected_zone_change_generation: source_ref.zone_change_generation,
            source_zone: rv1::AbilitySourceZone::Battlefield as i32,
            cost_selections,
            payment: Some(rv1::PaymentSelection {
                expected_state_revision: engine.state.command_index,
                source: Some(source_ref),
                mana: Some(rv1::PaymentMana::default()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let before_command =
            serde_json::to_value(&engine.state).expect("serialize state before direct activation");
        let result = engine.apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(proposal.clone())),
            },
        );
        match result {
            Err(EngineError::Illegal(_)) => {}
            Err(error) => panic!("aggregate activation rejects as Illegal: {error:?}"),
            Ok(_) => panic!("unsupported aggregate activation must not execute"),
        }
        assert_eq!(
            serde_json::to_value(&engine.state).expect("serialize state after direct activation"),
            before_command,
            "direct legacy activation rejection is atomic"
        );
        assert_no_pending_activation(&engine);

        let before_preview =
            serde_json::to_value(&engine.state).expect("serialize state before preview");
        let preview = engine.preview_payment(
            10,
            &rv1::PreviewPayment {
                activate_ability: Some(proposal),
                ..Default::default()
            },
        );
        assert!(!preview.error.is_empty());
        assert!(!preview.complete);
        assert_eq!(
            serde_json::to_value(&engine.state).expect("serialize state after preview"),
            before_preview,
            "failed payment preview is read-only"
        );
        assert_no_pending_activation(&engine);
    }

    let singleton_proposal = rv1::ActivateAbility {
        source_object_id: source,
        ability_index: singleton.slot,
        expected_zone_change_generation: source_ref.zone_change_generation,
        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
        cost_selections: vec![singleton_selection],
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_ref),
            mana: Some(rv1::PaymentMana::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let before_control_preview =
        serde_json::to_value(&engine.state).expect("serialize state before control preview");
    let control_preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            activate_ability: Some(singleton_proposal),
            ..Default::default()
        },
    );
    assert!(
        control_preview.error.is_empty(),
        "{}",
        control_preview.error
    );
    assert!(control_preview.complete);
    assert_eq!(
        serde_json::to_value(&engine.state).expect("serialize state after control preview"),
        before_control_preview,
        "successful preview plans but does not debit the singleton control"
    );
    assert_no_pending_activation(&engine);
}

#[test]
fn aggregate_counter_c2_public_route_is_guarded_while_direct_route_stays_closed() {
    let (mut engine, source, artifact, creature) = aggregate_counter_cost_fixture(602_432);
    let effective = engine.effective_activated_abilities(source);
    let aggregate = effective
        .iter()
        .find(|ability| ability.definition.ability_id.as_str() == "aggregate_01")
        .expect("aggregate-cost ability");
    let aggregate_cost = &aggregate.definition.costs[0];
    let required_count = match aggregate_cost {
        AbilityCost::RemoveCountersFromPermanents { count, .. } => *count,
        other => panic!("fixture retains the approved aggregate shape: {other:?}"),
    };
    assert_eq!(required_count, 3);
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        1
    );
    assert!(!engine.counter_costs_payable(source, &aggregate.definition.costs));

    let event = engine.ev_zone_view_sync_tracked();
    let Some(rv1::ruled_event::Ev::ZoneView(zone_view)) = event.ev else {
        panic!("the actual public state event includes a zone view");
    };
    let actor_view = zone_view
        .per_player
        .iter()
        .find(|view| view.player_id == 10)
        .expect("the actor receives the public battlefield AbilityInfo");
    let source_view = actor_view
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == source)
        .expect("the source is present in the public battlefield view");
    let ability_info = source_view
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_index == aggregate.slot)
        .expect("the public AbilityInfo for the aggregate cost");
    assert!(ability_info.activatable);

    let mut batch = RuledEventBatch::default();
    super::legal_actions::fill_legal(&mut batch, &engine);
    let public_legal = batch
        .legal_by_player
        .get(&10)
        .expect("the actor receives legal action data");
    let aggregate_key = (u64::from(source) << 32) | u64::from(aggregate.slot);
    let choices = public_legal
        .cost_choices_by_ability
        .get(&aggregate_key)
        .expect("the aggregate ability keeps its keyed structural status");
    assert!(choices.non_mana_costs_payable);
    let offered_group = choices
        .choices
        .iter()
        .find_map(|choice| choice.counter_removal_group.as_ref())
        .expect("the supported aggregate cost is publicly offered as one group");
    assert_eq!(offered_group.required_count, 3);
    assert!(offered_group.candidates.iter().any(|candidate| {
        candidate
            .object
            .as_ref()
            .is_some_and(|object| object.object_id == artifact)
    }));
    assert!(offered_group.candidates.iter().any(|candidate| {
        candidate
            .object
            .as_ref()
            .is_some_and(|object| object.object_id == creature)
    }));

    let source_ref = engine.payment_object_ref(source);
    let group_selection = rv1::CostSelection {
        cost_index: 0,
        selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
            rv1::CounterRemovalGroupSelection {
                contributions: vec![
                    rv1::CounterRemovalContribution {
                        object: Some(engine.payment_object_ref(artifact)),
                        option_id: crate::engine::counters::counter_option_id(CounterKind::Charge),
                        count: 2,
                    },
                    rv1::CounterRemovalContribution {
                        object: Some(engine.payment_object_ref(creature)),
                        option_id: crate::engine::counters::counter_option_id(CounterKind::Night),
                        count: 1,
                    },
                ],
            },
        )),
    };
    let begin = rv1::BeginAbilityActivation {
        source_object_id: source,
        expected_zone_change_generation: source_ref.zone_change_generation,
        ability_index: aggregate.slot,
        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
        cost_selections: vec![group_selection.clone()],
        ..Default::default()
    };
    let begin_command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
    };
    engine
        .apply_command(10, &begin_command)
        .expect("the guarded accepted aggregate route supports the validated group");
    let pending = engine
        .state
        .pending_ability_activation
        .as_ref()
        .expect("supported aggregate Begin opens the Payment stage");
    let transaction_id = pending.transaction_id;
    let expected_revision = pending.revision;
    assert_eq!(pending.stage(), rv1::AbilityActivationStage::Payment);
    engine
        .apply_command(
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
        .expect("Cancel closes the accepted window without debiting its group");
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());

    let direct = rv1::ActivateAbility {
        source_object_id: source,
        ability_index: aggregate.slot,
        expected_zone_change_generation: source_ref.zone_change_generation,
        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
        cost_selections: vec![group_selection],
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_ref),
            mana: Some(rv1::PaymentMana::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let direct_command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(direct.clone())),
    };
    let before_direct = serde_json::to_value(&engine.state)
        .expect("serialize complete state and allocators before direct activation");
    assert!(matches!(
        engine.apply_command(10, &direct_command),
        Err(EngineError::Illegal(_))
    ));
    assert_eq!(
        serde_json::to_value(&engine.state)
            .expect("serialize complete state and allocators after direct activation"),
        before_direct
    );
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());

    let before_preview = serde_json::to_value(&engine.state)
        .expect("serialize complete state and allocators before ordinary Preview");
    let preview = engine.preview_payment(
        10,
        &rv1::PreviewPayment {
            activate_ability: Some(direct),
            ..Default::default()
        },
    );
    assert!(!preview.error.is_empty());
    assert!(!preview.complete);
    assert_eq!(
        serde_json::to_value(&engine.state)
            .expect("serialize complete state and allocators after ordinary Preview"),
        before_preview
    );
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Night),
        1
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
                ..Default::default()
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
fn activated_program_reveals_exact_top_card_and_loses_its_captured_pre_move_mana_value() {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/endless_one.ron"),
            include_str!("../../../tricerules-cards/data/blaze.ron"),
            SYNTHETIC_ACTIVATED_REVEAL_RECEIPT_SOURCE,
        ],
        &[],
    )
    .expect("the synthetic reveal receipt program compiles with existing card fixtures");
    let decks = (0..3)
        .map(|_| {
            let mut mainboard = vec!["forest".to_string(); 24];
            mainboard.extend(vec!["endless_one".to_string(); 8]);
            mainboard.extend(vec!["blaze".to_string(); 8]);
            EngineDeck {
                mainboard,
                commanders: Vec::new(),
            }
        })
        .collect();
    let mut engine = GameEngine::new_with_registry(
        602_302,
        &[10, 20, 30],
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .expect("three-player reveal receipt engine");
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 10, "Synthetic Activated Reveal Receipt Source");

    for (card_id, expected_mana_value) in [("endless_one", 0_i32), ("blaze", 1_i32)] {
        let controller_index = engine.state.player_idx(10).expect("controller is live");
        let object_id = engine.state.players[controller_index]
            .library
            .iter()
            .copied()
            .find(|object_id| engine.state.objects[object_id].card_id == card_id)
            .expect("eight copies leave a fixture card in the library after the opening hand");
        let position = engine.state.players[controller_index]
            .library
            .iter()
            .position(|candidate| *candidate == object_id)
            .expect("selected card remains in the library");
        engine.state.players[controller_index]
            .library
            .remove(position);
        engine.state.players[controller_index]
            .library
            .insert(0, object_id);
        let pre_move_generation = engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        let hand_before = engine.state.players[controller_index].hand.len();
        let library_before = engine.state.players[controller_index].library.len();
        let life_before = [engine.state.players[1].life, engine.state.players[2].life];
        let draws_before =
            [10, 20, 30].map(|player| engine.state.turn_history.current.player(player).cards_drawn);

        let source_ref = engine.payment_object_ref(source);
        engine
            .apply_scenario_command(
                10,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                        rv1::ActivateAbility {
                            source_object_id: source,
                            ability_index: 0,
                            expected_zone_change_generation: source_ref.zone_change_generation,
                            source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("controller activates the captured reveal receipt ability");
        assert_eq!(engine.state.stack.len(), 1);

        let mut resolution_batch = None;
        for _ in 0..engine.state.players.len() {
            let actor = engine.state.priority_player_id();
            let batch = engine
                .apply_scenario_command(
                    actor,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .expect("all players pass to resolve the reveal receipt ability");
            if engine.state.stack.is_empty() {
                resolution_batch = Some(batch);
                break;
            }
        }
        let batch = resolution_batch.expect("all players passed and the ability resolved");

        let reveal_index = batch
            .events
            .iter()
            .position(|event| {
                matches!(
                    event.ev.as_ref(),
                    Some(rv1::ruled_event::Ev::CardsRevealed(reveal))
                        if reveal.cards.iter().any(|card| card.object_id == object_id)
                )
            })
            .expect("the top library object is publicly revealed before it moves");
        let reveal = match batch.events[reveal_index].ev.as_ref() {
            Some(rv1::ruled_event::Ev::CardsRevealed(reveal)) => reveal,
            _ => unreachable!("the indexed event is CardsRevealed"),
        };
        assert_eq!(
            reveal.source_zone,
            rv1::ChoiceCandidateSourceZone::Library as i32
        );
        assert_eq!(reveal.cards.len(), 1);
        assert_eq!(reveal.cards[0].object_id, object_id);
        assert_eq!(reveal.cards[0].card_id, card_id);
        assert_eq!(reveal.cards[0].zone_change_generation, pre_move_generation);

        let move_index = batch
            .events
            .iter()
            .position(|event| {
                matches!(
                    event.ev.as_ref(),
                    Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                        if moved.object_id == object_id
                            && moved.destination
                                == rv1::permanent_moved::Destination::Hand as i32
                )
            })
            .expect("the exact revealed object makes an ordinary move to hand");
        assert!(
            reveal_index < move_index,
            "public reveal precedes hand movement"
        );
        assert_eq!(
            engine.state.objects[&object_id].zone,
            crate::state::Zone::Hand
        );
        assert_eq!(
            engine.state.zone_change_generation[&object_id],
            pre_move_generation + 1
        );
        assert_eq!(
            engine.state.players[controller_index].hand.len(),
            hand_before + 1
        );
        assert_eq!(
            engine.state.players[controller_index].library.len(),
            library_before - 1
        );
        assert_eq!(
            engine.state.players[1].life,
            life_before[0] - expected_mana_value
        );
        assert_eq!(
            engine.state.players[2].life,
            life_before[1] - expected_mana_value
        );
        for (player, draws) in [10, 20, 30].into_iter().zip(draws_before) {
            assert_eq!(
                engine.state.turn_history.current.player(player).cards_drawn,
                draws,
                "revealing and moving a card is not drawing it"
            );
        }
    }

    let controller_index = engine
        .state
        .player_idx(10)
        .expect("controller remains live");
    let library_objects = engine.state.players[controller_index].library.clone();
    for object_id in library_objects {
        super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            object_id,
            crate::state::Zone::Hand,
            None,
        )
        .expect("move every remaining card out to exercise a genuinely empty library");
    }
    assert!(engine.state.players[controller_index].library.is_empty());
    let hand_before = engine.state.players[controller_index].hand.len();
    let library_before = engine.state.players[controller_index].library.len();
    let life_before = [engine.state.players[1].life, engine.state.players[2].life];
    let draws_before =
        [10, 20, 30].map(|player| engine.state.turn_history.current.player(player).cards_drawn);
    let source_ref = engine.payment_object_ref(source);
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: source,
                        ability_index: 0,
                        expected_zone_change_generation: source_ref.zone_change_generation,
                        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("controller activates with an empty library");
    let mut resolution_batch = None;
    for _ in 0..engine.state.players.len() {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_scenario_command(
                actor,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("all players pass to resolve the empty-library receipt ability");
        if engine.state.stack.is_empty() {
            resolution_batch = Some(batch);
            break;
        }
    }
    let batch = resolution_batch.expect("the empty-library ability resolves");
    assert!(batch.events.iter().all(|event| {
        !matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::CardsRevealed(_))
        )
    }));
    assert!(batch.events.iter().all(|event| {
        !matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                if moved.destination == rv1::permanent_moved::Destination::Hand as i32
        )
    }));
    assert!(engine.state.players[controller_index].library.is_empty());
    assert_eq!(
        engine.state.players[controller_index].library.len(),
        library_before
    );
    assert_eq!(
        engine.state.players[controller_index].hand.len(),
        hand_before
    );
    assert_eq!(engine.state.players[1].life, life_before[0]);
    assert_eq!(engine.state.players[2].life, life_before[1]);
    assert!(!engine.state.players[controller_index].has_lost);
    for (player, draws) in [10, 20, 30].into_iter().zip(draws_before) {
        assert_eq!(
            engine.state.turn_history.current.player(player).cards_drawn,
            draws,
            "an empty-library reveal is not a draw attempt"
        );
    }
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
                ..Default::default()
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
                ..Default::default()
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

fn urban_retreat_hand_activation_engine(
) -> (GameEngine, rv1::BeginAbilityActivation, ObjectId, ObjectId) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        602_302,
        &[10, 20],
        20,
        Some(vec![
            vec!["forest".to_string(); 40],
            vec!["forest".to_string(); 40],
        ]),
        true,
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine
        .apply_dev_command(
            10,
            &rv1::DevCommand {
                target_player_id: 10,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: "Forest".into(),
                        zone: rv1::DevZone::Hand as i32,
                        ready: false,
                    },
                )),
            },
        )
        .expect("put an actual card object in the actor's hand");
    let source = engine.state.next_object_id - 1;
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("the hand source object exists")
        .card_id = "urban_retreat".into();
    let creature = place_ready(&mut engine, 10, "Grizzly Bears");
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("the payment creature exists")
        .tapped = true;
    engine.state.players[0].mana_pool.colorless = 2;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine.reconcile_activated_ability_slots();
    let command = rv1::BeginAbilityActivation {
        source_object_id: source,
        expected_zone_change_generation: engine.payment_object_ref(source).zone_change_generation,
        ability_index: 1,
        source_zone: rv1::AbilitySourceZone::Hand as i32,
        ..Default::default()
    };
    (engine, command, source, creature)
}

#[test]
fn aggregate_counter_begin_payloads_reject_without_state_or_resource_change() {
    let (battlefield_engine, legacy) = choice_engine();
    let battlefield_command = begin_choice(&battlefield_engine, &legacy);
    let mut battlefield_control = battlefield_engine.clone();
    battlefield_control
        .apply_scenario_command(10, &battlefield_command)
        .expect("empty lists preserve the valid Battlefield route");
    assert_eq!(
        battlefield_control
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .stage(),
        rv1::AbilityActivationStage::OpponentTarget
    );
    assert_eq!(battlefield_control.state.players[0].mana_pool.colorless, 3);

    let (hand_engine, hand_begin, source, creature) = urban_retreat_hand_activation_engine();
    let hand_command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(hand_begin)),
    };
    let mut hand_control = hand_engine.clone();
    let hand_batch = hand_control
        .apply_scenario_command(10, &hand_command)
        .expect("empty lists preserve the valid Urban Retreat hand route");
    assert_eq!(
        hand_control
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .stage(),
        rv1::AbilityActivationStage::Payment
    );
    assert_eq!(hand_control.state.players[0].mana_pool.colorless, 2);
    assert_eq!(hand_control.state.objects[&source].zone, Zone::Hand);
    assert!(hand_control.state.objects[&creature].tapped);
    assert!(hand_batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .and_then(|pending| pending.payment_preview.as_ref())
        .is_some_and(|preview| preview.valid));

    let group_cost = rv1::CostSelection {
        cost_index: 0,
        selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
            rv1::CounterRemovalGroupSelection::default(),
        )),
    };
    let payloads = [
        (
            "cost-only-default-element",
            vec![rv1::CostSelection::default()],
            vec![],
        ),
        (
            "flex-only-default-element",
            vec![],
            vec![rv1::FlexPipPayment::default()],
        ),
        (
            "both-group-and-flex",
            vec![group_cost],
            vec![rv1::FlexPipPayment {
                pip_index: 1,
                pay_life: true,
            }],
        ),
    ];
    let routes = [
        ("battlefield", battlefield_engine, battlefield_command),
        ("hand", hand_engine, hand_command),
    ];
    let mut failures = Vec::new();
    for (route, base_engine, base_command) in routes {
        for (payload, cost_selections, flex_payments) in &payloads {
            let mut engine = base_engine.clone();
            let mut command = base_command.clone();
            let Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)) = command.cmd.as_mut()
            else {
                panic!("the fixed {route} command is a BeginAbilityActivation");
            };
            begin.cost_selections = cost_selections.clone();
            begin.flex_payments = flex_payments.clone();
            let before = serde_json::to_value(&engine.state).unwrap();
            let result = engine.apply_scenario_command(10, &command);
            if result.is_ok() {
                failures.push(format!("{route} {payload} Begin was accepted"));
            }
            if serde_json::to_value(&engine.state).unwrap() != before {
                failures.push(format!(
                    "{route} {payload} changed engine state or resources"
                ));
            }
            if engine.pending_ability_activation_internal.is_some() {
                failures.push(format!(
                    "{route} {payload} allocated pending activation state"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "nonempty Begin lists must reject before Battlefield/Hand dispatch without changing full state: {failures:#?}"
    );
}

fn synthetic_counter_group_announcements(
    object: rv1::CostObjectRef,
) -> (Vec<rv1::CostSelection>, Vec<rv1::FlexPipPayment>) {
    (
        vec![rv1::CostSelection {
            cost_index: 0,
            selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
                rv1::CounterRemovalGroupSelection {
                    contributions: vec![rv1::CounterRemovalContribution {
                        object: Some(object),
                        option_id: 7,
                        count: 1,
                    }],
                },
            )),
        }],
        vec![rv1::FlexPipPayment {
            pip_index: 1,
            pay_life: true,
        }],
    )
}

fn pending_public_projection_matches(
    projected: &rv1::PendingAbilityActivation,
    canonical: &rv1::PendingAbilityActivation,
) -> bool {
    projected.transaction_id == canonical.transaction_id
        && projected.revision == canonical.revision
        && projected.stage == canonical.stage
        && projected.actor_player_id == canonical.actor_player_id
        && projected.deciding_player_id == canonical.deciding_player_id
        && projected.source_object_id == canonical.source_object_id
        && projected.source_zone_change_generation == canonical.source_zone_change_generation
        && projected.ability_index == canonical.ability_index
        && projected.reserved_object_id == canonical.reserved_object_id
        && projected.chosen_opponent_id == canonical.chosen_opponent_id
        && projected.announced_targets == canonical.announced_targets
        && projected.source_description == canonical.source_description
        && projected.source_zone == canonical.source_zone
        && projected.return_tapped_creature_cost_index
            == canonical.return_tapped_creature_cost_index
}

#[test]
fn aggregate_pending_projection_keeps_announcements_actor_private() {
    let mut failures = Vec::new();

    let (mut engine, legacy) = choice_engine_players(&[10, 20, 30]);
    let begin = begin_choice(&engine, &legacy);
    let begin_batch = engine
        .apply_scenario_command(10, &begin)
        .expect("the existing Battlefield route reaches its opponent target stage");
    assert_eq!(
        begin_batch.legal_by_player[&20]
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .stage(),
        rv1::AbilityActivationStage::OpponentTarget
    );
    let own_identity = engine.payment_object_ref(legacy.targets[0].object_id);
    let (announcements, flex_announcements) = synthetic_counter_group_announcements(own_identity);
    {
        let canonical = engine
            .state
            .pending_ability_activation
            .as_mut()
            .expect("the actual opponent-target activation remains pending");
        canonical.cost_selections = announcements.clone();
        canonical.flex_payments = flex_announcements.clone();
    }
    let canonical = engine.state.pending_ability_activation.clone().unwrap();
    let before_projection = serde_json::to_value(&engine.state).unwrap();
    for (viewer, retains_new_fields) in [(10, true), (20, false), (30, false), (-1, false)] {
        let projected = engine
            .pending_ability_activation_for(viewer)
            .expect("the canonical pending activation has a projection");
        if !pending_public_projection_matches(&projected, &canonical) {
            failures.push(format!("viewer {viewer} changed public pending fields"));
        }
        if retains_new_fields {
            if projected.cost_selections != announcements
                || projected.flex_payments != flex_announcements
            {
                failures.push("actor 10 lost its announcements while opponent 20 decides".into());
            }
            if !projected.target_candidates.is_empty() || projected.target_group.is_some() {
                failures.push("actor 10 saw the opponent's target offer".into());
            }
        } else if !projected.cost_selections.is_empty() || !projected.flex_payments.is_empty() {
            failures.push(format!(
                "nonactor viewer {viewer} received actor-private lists"
            ));
        }
        if viewer == 20
            && (projected.target_candidates.len() != 1
                || !projected
                    .target_group
                    .as_ref()
                    .is_some_and(|group| group.chosen_by_opponent))
        {
            failures.push("deciding opponent 20 lost the existing target offer".into());
        }
    }
    if serde_json::to_value(&engine.state).unwrap() != before_projection {
        failures.push("pending projections mutated canonical Battlefield state".into());
    }

    let (mut nested, legacy) = choice_engine();
    let mana_source = place_ready(&mut nested, 10, "Pain Payment Land");
    nested.state.players[0].mana_pool.colorless = 0;
    nested.state.add_damage_prevention_shield(10, 1);
    nested.state.add_damage_prevention_shield(10, 1);
    let commit = announce_both(&mut nested, &legacy);
    let nested_batch = activate_fixture_mana(&mut nested, mana_source);
    let choice = nested
        .state
        .pending_resolution
        .as_ref()
        .expect("the real shield/order choice parks the outer activation")
        .presentation
        .candidates[0];
    if nested_batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .and_then(|pending| pending.payment_preview.as_ref())
        .is_some()
    {
        failures.push("nested resolution exposed the actor payment preview".into());
    }
    let nested_identity = nested.payment_object_ref(legacy.targets[0].object_id);
    let (nested_announcements, nested_flex) =
        synthetic_counter_group_announcements(nested_identity);
    {
        let canonical = nested
            .state
            .pending_ability_activation
            .as_mut()
            .expect("the outer activation remains pending under nested resolution");
        canonical.cost_selections = nested_announcements.clone();
        canonical.flex_payments = nested_flex.clone();
    }
    let canonical = nested.state.pending_ability_activation.clone().unwrap();
    let nested_before_projection = serde_json::to_value(&nested.state).unwrap();
    for viewer in [10, 20, -1] {
        let projected = nested
            .pending_ability_activation_for(viewer)
            .expect("the outer pending activation remains projectable");
        if !projected.cost_selections.is_empty() || !projected.flex_payments.is_empty() {
            failures.push(format!(
                "nested resolution exposed actor lists to viewer {viewer}"
            ));
        }
        if projected.payment_preview.is_some() {
            failures.push(format!(
                "nested resolution exposed payment preview to viewer {viewer}"
            ));
        }
        if !pending_public_projection_matches(&projected, &canonical) {
            failures.push(format!(
                "nested projection changed public fields for viewer {viewer}"
            ));
        }
    }
    if serde_json::to_value(&nested.state).unwrap() != nested_before_projection {
        failures.push("nested pending projections mutated canonical state".into());
    }

    let batch = nested
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
        .expect("answer the actual nested shield choice");
    if nested.state.pending_resolution.is_some() {
        failures.push("answering the nested choice did not resume the outer activation".into());
    }
    let resumed = nested
        .state
        .pending_ability_activation
        .clone()
        .expect("the same outer activation resumes after the nested choice");
    if resumed.transaction_id != commit.transaction_id
        || resumed.cost_selections != nested_announcements
        || resumed.flex_payments != nested_flex
    {
        failures.push("nested answer changed the canonical announcements or transaction".into());
    }
    let actor_batch = batch.legal_by_player[&10]
        .pending_ability_activation
        .as_ref()
        .expect("resumed response includes the actor pending activation");
    if actor_batch.transaction_id != commit.transaction_id
        || actor_batch.cost_selections != nested_announcements
        || actor_batch.flex_payments != nested_flex
        || actor_batch.payment_preview.is_none()
    {
        failures
            .push("actor did not regain its lists and payment preview after nested choice".into());
    }
    let resumed_before_projection = serde_json::to_value(&nested.state).unwrap();
    let actor_after = nested
        .pending_ability_activation_for(10)
        .expect("actor projection returns when nested resolution clears");
    if actor_after.cost_selections != nested_announcements
        || actor_after.flex_payments != nested_flex
        || actor_after.payment_preview.is_none()
        || !pending_public_projection_matches(&actor_after, &resumed)
    {
        failures.push("direct actor projection did not restore its lists".into());
    }
    for viewer in [20, -1] {
        let projected = nested
            .pending_ability_activation_for(viewer)
            .expect("nonactor projection remains available after nested choice");
        if !projected.cost_selections.is_empty()
            || !projected.flex_payments.is_empty()
            || projected.payment_preview.is_some()
            || !pending_public_projection_matches(&projected, &resumed)
        {
            failures.push(format!(
                "resumed nonactor viewer {viewer} received private data"
            ));
        }
    }
    if serde_json::to_value(&nested.state).unwrap() != resumed_before_projection {
        failures.push("resumed projections mutated canonical announcement state".into());
    }

    assert!(
        failures.is_empty(),
        "new pending announcements must be actor-private independently of decider and suppressed during nested resolution: {failures:#?}"
    );
}

#[test]
fn phased_out_source_has_no_effective_activated_abilities() {
    let (mut engine, command) = choice_engine();
    let source_id = command.source_object_id;
    assert!(
        !engine.effective_activated_abilities(source_id).is_empty(),
        "the fixture source has a live activated ability before phasing"
    );

    engine
        .state
        .objects
        .get_mut(&source_id)
        .unwrap()
        .phase_status = Some(crate::state::PhasedOutStatus {
        phase_out_under: 10,
        phase_in_due_after_departed_turn: false,
        phase_in_with: None,
        direct_attachment_snapshot: None,
    });

    assert!(
        engine.effective_activated_abilities(source_id).is_empty(),
        "a phased-out permanent has no live activated abilities"
    );
}

#[test]
fn loyalty_x_offer_uses_carth_adjusted_interval_and_remains_activatable() {
    let (mut engine, walker) = loyalty_x_carth_offer_fixture(602_310, false);
    let effective = engine
        .effective_activated_abilities(walker)
        .into_iter()
        .next()
        .expect("the fixture has one activated LoyaltyX ability");
    let offer = super::legal_actions::activated_ability_info(&engine, walker, &effective);
    let range = offer
        .loyalty_x_choice
        .as_ref()
        .expect("engine publishes the inclusive LoyaltyX range");

    assert_eq!((range.min_x, range.max_x), (0, 7));
    assert!(offer.activatable);

    let before_invalid = serde_json::to_value(&engine.state).unwrap();
    activate_loyalty_x(&mut engine, walker, 8).expect_err("X above the offered maximum rejects");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before_invalid);

    let batch =
        activate_loyalty_x(&mut engine, walker, 7).expect("Carth's modified X=7 cost is payable");
    assert_public_loyalty_x_announcement(&batch, 7);
    assert!(!engine.state.players[0].battlefield.contains(&walker));
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack.last().unwrap().chosen_x, 7);
    assert!(engine
        .state
        .stack
        .last()
        .unwrap()
        .ability_text
        .as_deref()
        .unwrap()
        .contains("X = 7"));
}

#[test]
fn loyalty_x_counter_prevention_keeps_nonzero_range_activatable() {
    let (mut engine, walker) = loyalty_x_carth_offer_fixture(602_311, true);
    let effective = engine
        .effective_activated_abilities(walker)
        .into_iter()
        .next()
        .expect("the fixture has one activated LoyaltyX ability");
    let offer = super::legal_actions::activated_ability_info(&engine, walker, &effective);
    let range = offer
        .loyalty_x_choice
        .as_ref()
        .expect("counter prevention retains legal LoyaltyX values");

    assert_eq!((range.min_x, range.max_x), (1, 7));
    assert!(offer.activatable);

    let before_invalid = serde_json::to_value(&engine.state).unwrap();
    activate_loyalty_x(&mut engine, walker, 0)
        .expect_err("X=0's positive modified loyalty cost is prohibited");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before_invalid);
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6
    );

    activate_loyalty_x(&mut engine, walker, 1).expect("zero net loyalty cost is legal");
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(engine.state.stack.last().unwrap().chosen_x, 1);
    engine.state.stack.clear();
    let effective = engine
        .effective_activated_abilities(walker)
        .into_iter()
        .next()
        .unwrap();
    assert!(!engine.ability_activatable(walker, effective.slot as usize, &effective.definition));
}

#[test]
fn loyalty_x_zero_is_payable_with_carth_and_consumes_the_loyalty_use() {
    let (mut engine, walker) = loyalty_x_carth_offer_fixture(602_312, false);
    let batch =
        activate_loyalty_x(&mut engine, walker, 0).expect("Carth changes X=0 into a +1 cost");
    assert_public_loyalty_x_announcement(&batch, 0);

    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        7
    );
    assert_eq!(engine.state.stack.last().unwrap().chosen_x, 0);
    engine.state.stack.clear();
    let effective = engine
        .effective_activated_abilities(walker)
        .into_iter()
        .next()
        .unwrap();
    assert!(!engine.ability_activatable(walker, effective.slot as usize, &effective.definition));
}

#[test]
fn loyalty_x_exile_uses_captured_x_after_cost_sba_removes_its_source() {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            LOYALTY_X_EXILE_TEST_WALKER,
        ],
        &[],
    )
    .expect("LoyaltyX ExileAll fixture compiles");
    let decks = (0..2)
        .map(|_| EngineDeck {
            mainboard: vec!["forest".into(); 40],
            commanders: Vec::new(),
        })
        .collect();
    let mut engine = GameEngine::new_with_registry(
        602_315,
        &[10, 20],
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .expect("two-player LoyaltyX ExileAll engine");
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 10, "Loyalty X Exile Test Walker");
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("planeswalker source")
        .set_counter(CounterKind::Loyalty, 1);
    let forest = place_ready(&mut engine, 10, "Forest");
    let bears = place_ready(&mut engine, 20, "Grizzly Bears");

    let activation = activate_loyalty_x(&mut engine, source, 1)
        .expect("the source can pay its last loyalty counter for X=1");
    assert_public_loyalty_x_announcement(&activation, 1);
    assert_eq!(engine.state.stack.last().unwrap().chosen_x, 1);
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Graveyard,
        "state-based actions remove the source after its loyalty cost"
    );

    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(
                actor,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("pass priority to resolve the announced ability");
    }

    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&forest].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&bears].zone,
        Zone::Battlefield,
        "the resolving effect reads the stored X=1 after its source leaves"
    );
}

#[test]
fn loyalty_x_prepared_positive_cost_replay_preserves_nonzero_x_and_commits_once() {
    // Two +1 modifiers keep X=1's final counter placement positive after a replay. This prepared
    // fixture exercises the existing cost-replacement continuation without claiming normal-session
    // reachability for two applicable loyalty-cost modifiers.
    let (mut engine, walker, laezel, opponent_vorinclex) =
        loyalty_x_prepared_replacement_replay_fixture(602_314);
    let effective = engine
        .effective_activated_abilities(walker)
        .into_iter()
        .next()
        .expect("the prepared walker has one LoyaltyX ability");
    let offer = super::legal_actions::activated_ability_info(&engine, walker, &effective);
    assert_eq!(
        offer
            .loyalty_x_choice
            .as_ref()
            .map(|range| (range.min_x, range.max_x)),
        Some((0, 8))
    );
    assert!(offer.activatable);

    let parked = activate_loyalty_x(&mut engine, walker, 1)
        .expect("X=1 has a positive modified loyalty placement and parks for replacement order");
    let choice = parked
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)
                if choice.choice_kind == rv1::ChoiceKind::ReplacementEffect as i32 =>
            {
                Some(choice)
            }
            _ => None,
        })
        .expect("Lae'zel and opponent Vorinclex require a replacement-order choice");
    assert_eq!(choice.deciding_player_id, 10);
    assert_eq!(choice.replacement_options.len(), 2);
    let laezel_application_id =
        offered_counter_replacement_application_id(choice, laezel, "adds one of each counter kind");
    let vorinclex_application_id = offered_counter_replacement_application_id(
        choice,
        opponent_vorinclex,
        "halves counters opponents would put, rounded down",
    );
    assert_ne!(laezel_application_id, vorinclex_application_id);
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine.pending_counter_cost_internal.is_some());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6,
        "the cost remains uncommitted while the ordering choice is parked"
    );

    let completed = engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![vorinclex_application_id],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("the selected counter replacement resumes the original activation");
    assert_public_loyalty_x_announcement(&completed, 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.pending_counter_cost_internal.is_none());
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.pending_ability_activation_internal.is_none());
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the replay creates one ability item"
    );
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(walker));
    assert_eq!(engine.state.stack[0].chosen_x, 1);
    assert!(engine.state.stack[0]
        .ability_text
        .as_deref()
        .expect("the replayed stack item retains its public annotation")
        .contains("X = 1"));
    assert!(
        engine
            .state
            .turn_history
            .current
            .player(10)
            .activated_planeswalker_loyalty
    );
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6,
        "opponent Vorinclex halves the prepared +1 placement to zero; a reset to X=0 would retain +2"
    );
}

#[test]
fn loyalty_x_offer_uses_signed_domain_at_high_loyalty_and_rejects_overflow() {
    let (mut engine, walker) = loyalty_x_carth_offer_fixture(602_313, false);
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, u32::MAX);
    let effective = engine
        .effective_activated_abilities(walker)
        .into_iter()
        .next()
        .unwrap();
    let offer = super::legal_actions::activated_ability_info(&engine, walker, &effective);
    let range = offer.loyalty_x_choice.as_ref().unwrap();
    assert_eq!((range.min_x, range.max_x), (0, i32::MAX as u32));

    let before_invalid = serde_json::to_value(&engine.state).unwrap();
    activate_loyalty_x(&mut engine, walker, i32::MAX as u32 + 1)
        .expect_err("X above i32::MAX cannot be truncated or wrapped");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before_invalid);

    let mut stale = loyalty_x_command(&engine, walker, 1);
    stale.expected_zone_change_generation += 1;
    let before_stale = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(stale)),
            },
        )
        .expect_err("stale source generations reject before payment");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before_stale);

    activate_loyalty_x(&mut engine, walker, 0)
        .expect("the positive cost retains saturation behavior");
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        u32::MAX
    );
    assert_eq!(engine.state.stack.last().unwrap().chosen_x, 0);
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
