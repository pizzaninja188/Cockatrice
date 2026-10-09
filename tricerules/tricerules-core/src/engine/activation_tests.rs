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
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            OPPONENT_CHOICE_LAND,
            PAYMENT_CREATURE,
            RESTRICTED_PAYMENT_LAND,
            SPELL_PAYMENT_LAND,
            PAIN_PAYMENT_LAND,
            &magus,
            &taxed,
            &reduced,
            &snapshot,
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
            .apply_dev_command(&rv1::DevCommand {
                target_player_id: player,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: card.into(),
                        zone: rv1::DevZone::Battlefield as i32,
                        ready: true,
                    },
                )),
            })
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
        .apply_dev_command(&rv1::DevCommand {
            target_player_id: player,
            dev: Some(rv1::dev_command::Dev::PutCardInZone(
                rv1::DevPutCardInZone {
                    card_name: name.into(),
                    zone: rv1::DevZone::Battlefield as i32,
                    ready: true,
                },
            )),
        })
        .unwrap();
    engine.reconcile_activated_ability_slots();
    engine.state.next_object_id - 1
}

fn concede(engine: &mut GameEngine, player: PlayerId) -> RuledEventBatch {
    engine
        .apply_command(
            player,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap()
}

#[test]
fn arena_staged_departing_chooser_refreshes_explicit_actor_choice() {
    let (mut engine, legacy) = choice_engine_players(&[10, 20, 30, 40]);
    place_ready(&mut engine, 30, "Grizzly Bears");
    place_ready(&mut engine, 40, "Grizzly Bears");
    engine
        .apply_command(10, &begin_choice(&engine, &legacy))
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
        .apply_command(
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
            .apply_command(10, &begin_choice(&engine, &legacy))
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
        .apply_command(
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
            .apply_command(
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
        .apply_command(
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
            .apply_command(
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
    let batch = engine.apply_command(10, &begin).unwrap();
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
        .apply_command(
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
                .set_counter(tricerules_cards::CounterKind::PlusOnePlusOne, 1);
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
            .apply_command(
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
    tricerules_cards::TriggerCondition,
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
                    tricerules_cards::TriggerCondition::WhenSelfLeavesBattlefield
                )));
        assert!(trigger_receipts(&engine)
            .iter()
            .all(|(_, condition, _)| !matches!(
                condition,
                tricerules_cards::TriggerCondition::WheneverSelfBecomesTarget { .. }
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
            .apply_command(10, &RuledCommand { cmd: Some(command) })
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
                    tricerules_cards::TriggerCondition::WheneverSelfBecomesTarget { .. }
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
                            tricerules_cards::TriggerCondition::WheneverPlayerCommitsCrime { .. }
                        ))
                    .count(),
                1
            );
            assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        } else {
            assert!(targets.is_empty());
            assert!(receipts.iter().all(|(_, condition, _)| !matches!(
                condition,
                tricerules_cards::TriggerCondition::WheneverPlayerCommitsCrime { .. }
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
        .apply_command(10, &begin_choice(&engine, &legacy))
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
        .apply_command(
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
        .apply_command(
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
        .apply_command(
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
            .apply_command(
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
        .apply_command(
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
            .apply_command(
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
        .apply_command(
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
        let outcome = engine.apply_command(
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
            .apply_command(10, &RuledCommand { cmd: Some(cmd) })
            .expect_err("nested work must finish before outer action");
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    }
    let batch = engine
        .apply_command(
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
        .apply_command(
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
        .apply_command(
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
        .apply_command(
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
        .apply_command(
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
            .apply_command(10, &begin_choice(&engine, &legacy))
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
            .apply_command(
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
                    .apply_command(
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
        .apply_command(20, &cmd(commit.clone()))
        .expect_err("only the payer commits");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    let mut stale = commit.clone();
    stale.expected_revision += 1;
    engine
        .apply_command(10, &cmd(stale))
        .expect_err("stale revision");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    commit.payment = preview.selection;
    let batch = engine
        .apply_command(10, &cmd(commit))
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
            .apply_command(
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
        .apply_command(10, &begin_choice(&engine, &legacy))
        .unwrap();
    let before = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_command(10, &mana)
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
        .apply_command(
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
        .apply_command(10, &mana)
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
        .apply_command(
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
            .apply_command(
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
            .apply_command(
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
        .apply_command(10, &begin)
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
        .apply_command(10, &answer)
        .expect_err("actor cannot answer for opponent");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    engine
        .apply_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
            },
        )
        .expect_err("the ability announcement has no priority window");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);

    let batch = engine
        .apply_command(20, &answer)
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
        .apply_command(10, &begin_choice(&engine, &legacy))
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
        .apply_command(10, &begin_choice(&engine, &legacy))
        .unwrap();
    let batch = engine
        .apply_command(
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
        .apply_command(10, &begin_choice(&engine, &legacy))
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
            .apply_command(
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
        .apply_command(20, &cancel)
        .expect_err("only the actor may cancel");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    let batch = engine.apply_command(10, &cancel).unwrap();
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.state.blocking_choice().is_none());
    assert!(batch
        .legal_by_player
        .values()
        .all(|legal| legal.pending_ability_activation.is_none()));
    assert!(!engine.state.objects[&legacy.source_object_id].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
}
