use super::*;

// Cuombajj Witches' target announcement shape, extended in individual tests to exercise
// optional cardinality and a multi-group opponent suffix without adding fictional cards.
const ANY_TARGET_CHOICE: &str = r#"(
    id: "choice_witch", name: "Choice Witch", face_id: "choice_witch",
    types: ["Creature"], power: Some(1), toughness: Some(3),
    activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
        costs: [Tap], effect: [
            DamageTarget(amount: 1, target: (kind: AnyTarget)),
            DamageTarget(amount: 1, target: (kind: AnyTarget))
        ], targeting: Some((groups: [
            (min: 1, max: 1, effect_indices: [0]),
            (min: 1, max: 1, effect_indices: [1], chooser: ChosenOpponent)
        ]))
    )]
)"#;

fn fixture(definition: &str) -> (GameEngine, ObjectId) {
    fixture_with_setup(definition, |_, _| {})
}

fn fixture_with_setup(
    definition: &str,
    setup: impl FnOnce(&mut GameEngine, ObjectId),
) -> (GameEngine, ObjectId) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
            definition,
        ],
        &[],
    )
    .unwrap();
    let decks = (0..4)
        .map(|_| EngineDeck {
            mainboard: vec!["forest".into(); 40],
            commanders: Vec::new(),
        })
        .collect();
    let mut engine = GameEngine::new_with_registry(
        602_399,
        &[10, 20, 30, 40],
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 10, "Choice Witch");
    setup(&mut engine, source);
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(
                    rv1::BeginAbilityActivation {
                        source_object_id: source,
                        expected_zone_change_generation: engine
                            .payment_object_ref(source)
                            .zone_change_generation,
                        opponent_player_id: Some(20),
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    (engine, source)
}

fn reply(engine: &mut GameEngine, targets: &[(rv1::TargetRefKind, ObjectId)]) -> RuledCommand {
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
            rv1::SubmitAbilityActivationChoice {
                transaction_id: pending.transaction_id,
                expected_revision: pending.revision,
                targets: targets
                    .iter()
                    .map(|(kind, oid)| {
                        *pending
                            .target_candidates
                            .iter()
                            .find(|c| c.kind == *kind as i32 && c.object_id == *oid)
                            .expect("typed offered target")
                    })
                    .collect(),
                ..Default::default()
            },
        )),
    };
    engine
        .apply_scenario_command(pending.deciding_player_id, &command)
        .unwrap();
    command
}

#[test]
fn shared_announcement_supports_tap_only_player_targets_and_repeat_across_groups() {
    let (mut engine, source) = fixture(ANY_TARGET_CHOICE);
    let before = engine.state.players[2].life;
    reply(&mut engine, &[(rv1::TargetRefKind::Player, 30)]);
    assert!(
        !engine
            .pending_ability_activation_for(10)
            .unwrap()
            .can_cancel
    );
    assert!(engine
        .pending_ability_activation_for(10)
        .unwrap()
        .target_candidates
        .is_empty());
    assert!(engine
        .pending_ability_activation_for(30)
        .unwrap()
        .target_candidates
        .is_empty());
    reply(&mut engine, &[(rv1::TargetRefKind::Player, 30)]);
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    assert_eq!(pending.stage(), rv1::AbilityActivationStage::Payment);
    assert_eq!(pending.announced_targets.len(), 2);
    assert!(!engine.state.objects[&source].tapped);
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
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.stack[0]
            .targets
            .iter()
            .map(|t| (t.kind, t.object_id))
            .collect::<Vec<_>>(),
        vec![(rv1::TargetRefKind::Player as i32, 30); 2]
    );
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
    assert_eq!(engine.state.players[2].life, before - 2);
}

#[test]
fn shared_announcement_optional_group_accepts_empty_and_rejects_duplicate_multi_target() {
    let definition = ANY_TARGET_CHOICE
        .replace(
            "min: 1, max: 1, effect_indices: [0]",
            "min: 0, max: 2, effect_indices: [0]",
        )
        .replacen(
            "DamageTarget(amount: 1, target: (kind: AnyTarget))",
            "DamageTargets(amount: 1, target: (kind: AnyTarget), division: PerTarget)",
            1,
        );
    let (mut engine, _) = fixture(&definition);
    let pending = engine.state.pending_ability_activation.clone().unwrap();
    let target = pending.target_candidates[0];
    let invalid = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
            rv1::SubmitAbilityActivationChoice {
                transaction_id: pending.transaction_id,
                expected_revision: pending.revision,
                targets: vec![target, target],
                ..Default::default()
            },
        )),
    };
    let before = serde_json::to_value(&engine.state).unwrap();
    assert!(engine.apply_scenario_command(10, &invalid).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    reply(&mut engine, &[]);
    assert_eq!(
        engine
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .active_target_group_index,
        1
    );
    reply(&mut engine, &[(rv1::TargetRefKind::Player, 10)]);
    assert_eq!(
        engine
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .announced_targets
            .len(),
        1
    );
}

#[test]
fn shared_announcement_departed_suffix_chooser_preserves_completed_groups() {
    let definition = ANY_TARGET_CHOICE.replace(
        "DamageTarget(amount: 1, target: (kind: AnyTarget))\n        ]", 
        "DamageTarget(amount: 1, target: (kind: AnyTarget)), DamageTarget(amount: 1, target: (kind: AnyTarget))\n        ]"
    ).replace("effect_indices: [1], chooser: ChosenOpponent)",
        "effect_indices: [1], chooser: ChosenOpponent), (min: 1, max: 1, effect_indices: [2], chooser: ChosenOpponent)");
    let (mut engine, _) = fixture(&definition);
    reply(&mut engine, &[(rv1::TargetRefKind::Player, 30)]);
    let old_reply = reply(&mut engine, &[(rv1::TargetRefKind::Player, 40)]);
    let before = engine.state.pending_ability_activation.clone().unwrap();
    assert_eq!(before.active_target_group_index, 2);
    concede(&mut engine, 20);
    let pending = engine.state.pending_ability_activation.clone().unwrap();
    assert_eq!(pending.transaction_id, before.transaction_id);
    assert_eq!(pending.announced_targets, before.announced_targets);
    assert_eq!(pending.active_target_group_index, 2);
    assert_eq!(pending.stage(), rv1::AbilityActivationStage::ChooseOpponent);
    assert!(
        !engine
            .pending_ability_activation_for(10)
            .unwrap()
            .can_cancel
    );
    assert!(engine.apply_scenario_command(20, &old_reply).is_err());
    engine
        .apply_scenario_command(
            10,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                    rv1::SubmitAbilityActivationChoice {
                        transaction_id: pending.transaction_id,
                        expected_revision: pending.revision,
                        opponent_player_id: Some(30),
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    reply(&mut engine, &[(rv1::TargetRefKind::Player, 10)]);
    let final_view = engine.state.pending_ability_activation.as_ref().unwrap();
    assert_eq!(final_view.announced_targets[..2], before.announced_targets);
    assert_eq!(final_view.stage(), rv1::AbilityActivationStage::Payment);
}

#[test]
fn shared_announcement_non_tap_cost_accepts_a_tapped_source() {
    let (mut engine, source) = fixture(&ANY_TARGET_CHOICE.replace("costs: [Tap]", "costs: []"));
    engine.state.objects.get_mut(&source).unwrap().tapped = true;
    reply(&mut engine, &[(rv1::TargetRefKind::Player, 30)]);
    reply(&mut engine, &[(rv1::TargetRefKind::Player, 40)]);
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
        .expect("a tap symbol is absent, so being tapped must not prohibit payment");
}

#[test]
fn shared_announcement_collision_does_not_offer_a_land_as_any_target() {
    let (mut engine, source) = fixture(ANY_TARGET_CHOICE);
    // A battlefield Forest shares player 30's number.
    let oid = 30;
    for player in &mut engine.state.players {
        player.library.retain(|id| *id != oid);
    }
    engine.state.players[1].battlefield.push(oid);
    let mut object = engine.state.objects[&source].clone();
    object.id = oid;
    object.card_id = "forest".into();
    object.zone = Zone::Battlefield;
    object.controller = 20;
    engine.state.objects.insert(oid, object);
    engine.state.zone_change_generation.insert(oid, 1);
    engine.reconcile_pending_ability_activation(&mut Vec::new());
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    assert!(pending
        .target_candidates
        .iter()
        .any(|c| c.object_id == oid && c.kind == rv1::TargetRefKind::Player as i32));
    assert!(!pending
        .target_candidates
        .iter()
        .any(|c| c.object_id == oid && c.kind == rv1::TargetRefKind::Permanent as i32));
}

#[test]
fn shared_announcement_collision_damage_resolves_to_each_typed_recipient() {
    let (mut engine, source) = fixture(ANY_TARGET_CHOICE);
    let mut object = engine.state.objects[&source].clone();
    object.id = 30;
    object.card_id = "grizzly_bears".into();
    object.controller = 20;
    engine.state.objects.insert(30, object);
    engine.state.zone_change_generation.insert(30, 1);
    engine.state.players[1].battlefield.push(30);
    engine.reconcile_pending_ability_activation(&mut Vec::new());
    reply(&mut engine, &[(rv1::TargetRefKind::Permanent, 30)]);
    reply(&mut engine, &[(rv1::TargetRefKind::Player, 30)]);
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
    assert_eq!(engine.state.players[2].life, 19);
    assert_eq!(engine.state.objects[&30].damage, 1);
}

#[test]
fn shared_announcement_multiple_targets_resolve_each_receipt_and_replay() {
    use prost::Message;
    let definition = ANY_TARGET_CHOICE
        .replace(
            "min: 1, max: 1, effect_indices: [0]",
            "min: 0, max: 2, effect_indices: [0]",
        )
        .replacen(
            "DamageTarget(amount: 1, target: (kind: AnyTarget))",
            "DamageTargets(amount: 1, target: (kind: AnyTarget), division: PerTarget)",
            1,
        );
    let (mut engine, _) = fixture(&definition);
    let (mut replay, _) = fixture(&definition);
    let command = reply(
        &mut engine,
        &[
            (rv1::TargetRefKind::Player, 30),
            (rv1::TargetRefKind::Player, 40),
        ],
    );
    replay
        .apply_scenario_command(
            10,
            &RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap(),
        )
        .unwrap();
    let command = reply(&mut engine, &[(rv1::TargetRefKind::Player, 10)]);
    replay
        .apply_scenario_command(
            20,
            &RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap(),
        )
        .unwrap();
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    let commit = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
            rv1::CommitAbilityActivation {
                transaction_id: pending.transaction_id,
                expected_revision: pending.revision,
                ..Default::default()
            },
        )),
    };
    let commands = std::iter::once((10, commit)).chain([10, 20, 30, 40].map(|player| {
        (
            player,
            RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
            },
        )
    }));
    for (player, command) in commands {
        engine.apply_scenario_command(player, &command).unwrap();
        replay
            .apply_scenario_command(
                player,
                &RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap(),
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(&engine.state).unwrap(),
            serde_json::to_value(&replay.state).unwrap()
        );
    }
    assert_eq!(engine.state.players[0].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
    assert_eq!(engine.state.players[3].life, 19);
}

#[test]
fn shared_announcement_keeps_original_chooser_controller_receipt_until_payment() {
    let definition = r#"(
        id: "choice_witch", name: "Choice Witch", face_id: "choice_witch",
        types: ["Creature"], power: Some(1), toughness: Some(3),
        activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
            costs: [Tap], effect: [
                DamageTarget(amount: 1, target: (kind: AnyTarget)),
                DamageTarget(amount: 1, target: (kind: Creature, controller: Opponent)),
                DamageTarget(amount: 1, target: (kind: AnyTarget))
            ], targeting: Some((groups: [
                (min: 1, max: 1, effect_indices: [0]),
                (min: 1, max: 1, effect_indices: [1], chooser: ChosenOpponent, controlled_by_chooser: true),
                (min: 1, max: 1, effect_indices: [2], chooser: ChosenOpponent)
            ]))
        )]
    )"#;
    let (mut engine, source) = fixture_with_setup(definition, |engine, source| {
        let mut borrowed = engine.state.objects[&source].clone();
        borrowed.id = 30;
        borrowed.base_controller = 20;
        borrowed.controller = 20;
        engine.state.objects.insert(30, borrowed);
        engine.state.zone_change_generation.insert(30, 1);
        engine.state.players[1].battlefield.push(30);
    });
    reply(&mut engine, &[(rv1::TargetRefKind::Player, 40)]);
    reply(&mut engine, &[(rv1::TargetRefKind::Permanent, 30)]);
    assert_eq!(
        engine
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .active_target_group_index,
        2
    );
    // A control effect ending may leave the same incarnation under another opponent.
    engine.state.objects.get_mut(&30).unwrap().base_controller = 30;
    engine.state.objects.get_mut(&30).unwrap().controller = 30;
    engine.state.players[1].battlefield.retain(|oid| *oid != 30);
    engine.state.players[2].battlefield.push(30);
    engine.reconcile_pending_ability_activation(&mut Vec::new());
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(!engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn shared_announcement_typed_filters_keep_hexproof_and_disjunction_constraints() {
    for definition in [
        ANY_TARGET_CHOICE.replace(
            "types: [\"Creature\"]",
            "keywords: [Hexproof], types: [\"Creature\"]",
        ),
        ANY_TARGET_CHOICE.replace(
            "target: (kind: AnyTarget)",
            "target: (any_of: [(kind: AnyPlayer), (kind: Creature, controller: You)])",
        ),
    ] {
        let (mut engine, source) = fixture(&definition);
        let mut opponent = engine.state.objects[&source].clone();
        opponent.id = 30;
        opponent.base_controller = 20;
        opponent.controller = 20;
        engine.state.objects.insert(30, opponent);
        engine.state.zone_change_generation.insert(30, 1);
        engine.state.players[1].battlefield.push(30);
        engine.reconcile_pending_ability_activation(&mut Vec::new());
        let pending = engine.state.pending_ability_activation.as_ref().unwrap();
        assert!(pending
            .target_candidates
            .iter()
            .any(|c| c.object_id == 30 && c.kind == rv1::TargetRefKind::Player as i32));
        assert!(!pending
            .target_candidates
            .iter()
            .any(|c| c.object_id == 30 && c.kind == rv1::TargetRefKind::Permanent as i32));
    }
}
