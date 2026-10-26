use super::*;
use crate::state::{ChosenMode, StackTarget};
use tricerules_card_model::primitives::EffectSubject;
use tricerules_proto::ruled::v1 as rv1;

#[derive(Clone, Copy, Debug)]
enum ChosenAction {
    Fight,
    Destroy,
    Sacrifice,
    Attach,
    Shuffle,
    Tap,
    Regenerate,
    Exchange,
    Exile,
}

const ACTIONS: [ChosenAction; 8] = [
    ChosenAction::Fight,
    ChosenAction::Destroy,
    ChosenAction::Sacrifice,
    ChosenAction::Attach,
    ChosenAction::Shuffle,
    ChosenAction::Tap,
    ChosenAction::Regenerate,
    ChosenAction::Exchange,
];

fn fixture_engine() -> GameEngine {
    let deck = [
        "grizzly_bears",
        "hill_giant",
        "short_sword",
        "thran_dynamo",
        "the_mycosynth_gardens",
        "forest",
    ]
    .into_iter()
    .cycle()
    .take(32)
    .map(str::to_string)
    .collect::<Vec<_>>();
    GameEngine::new(
        tricerules_cards::registry::global(),
        60_608,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("target receipt action fixture")
}

#[derive(Clone, Copy)]
enum CopyAction {
    Gardens,
    ChosenCreatureToken,
}

fn invoke_copy_action(
    engine: &mut GameEngine,
    top: &StackItem,
    target_ids: &[ObjectId],
    receipts: &[StackTarget],
    action: CopyAction,
) -> Result<(EffectOutcome, Vec<rv1::RuledEvent>, EffectResult), EngineError> {
    let role_targets = vec![target_ids.to_vec()];
    let role_group_indices = [0];
    let previous_effect_result = EffectResult::default();
    let mut effect_result = EffectResult::default();
    let mut events = Vec::new();
    let outcome = {
        let mut cx = EffectCx {
            engine,
            events: &mut events,
            targets: target_ids,
            targets_by_role: &role_targets,
            target_receipts: receipts,
            target_role_group_indices: &role_group_indices,
            top,
            controller: 0,
            affected_player: 0,
            spell_label: "Copy receipt fixture",
            previous_effect_result: &previous_effect_result,
            effect_result: &mut effect_result,
            effect_index: 0,
            resume_previous_result: None,
            program_state: None,
        };
        match action {
            CopyAction::Gardens => misc::copy_target_artifact_with_mana_value_x(
                &mut cx,
                SpellEffectKind::CopyTargetArtifactWithManaValueX {
                    target: TargetFilter::default(),
                },
            ),
            CopyAction::ChosenCreatureToken => tokens::create_token_copies(
                &mut cx,
                SpellEffectKind::CreateTokenCopies {
                    count: tricerules_card_model::primitives::Amount::Fixed(1),
                    source: tricerules_card_model::primitives::TokenCopySource::Chosen(Box::new(
                        TargetFilter::default_creature(),
                    )),
                },
            ),
        }
    }?;
    Ok((outcome, events, effect_result))
}

fn set_copy_values_from(engine: &mut GameEngine, target: ObjectId, donor: ObjectId) -> String {
    let values = engine
        .copiable_values_for(donor)
        .expect("fixture donor has copiable values");
    let source_card_id = values.source_card_id.clone();
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("fixture target")
        .copiable_values = Some(values);
    source_card_id
}

fn gardens_stack_item(engine: &GameEngine, gardens: ObjectId, target: ObjectId) -> StackItem {
    let mut top = item(vec![receipt(engine, target, 0)]);
    top.source_permanent_id = Some(gardens);
    top.source_zone_change = engine
        .state
        .zone_change_generation
        .get(&gardens)
        .copied()
        .unwrap_or(0);
    top
}

fn card_in_zone(engine: &mut GameEngine, owner: PlayerId, card_id: &str, zone: Zone) -> ObjectId {
    let player_index = engine.state.player_idx(owner).expect("fixture owner");
    let object_id = engine.state.players[player_index]
        .library
        .iter()
        .chain(&engine.state.players[player_index].hand)
        .copied()
        .find(|id| engine.state.objects[id].card_id == card_id)
        .expect("fixture card");
    move_object_to_zone(&mut engine.state, engine.registry, object_id, zone, None)
        .expect("fixture move");
    object_id
}

fn receipt(engine: &GameEngine, object_id: ObjectId, group_index: u32) -> StackTarget {
    let kind = match engine.state.objects[&object_id].zone {
        Zone::Battlefield => rv1::TargetRefKind::Permanent,
        Zone::Graveyard => rv1::TargetRefKind::Graveyard,
        _ => rv1::TargetRefKind::Unspecified,
    };
    StackTarget {
        object_id,
        group_index,
        damage_amount: 0,
        kind: kind as i32,
        zone_change_generation: Some(
            engine
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
        ),
        required_controller: None,
    }
}

fn item(targets: Vec<StackTarget>) -> StackItem {
    StackItem {
        prototyped: false,
        mana_colors_spent_to_cast: Default::default(),
        id: u32::MAX,
        controller: 0,
        card_id: "target_receipt_fixture".into(),
        targets,
        ability_text: None,
        source_permanent_id: None,
        source_owner: None,
        source_zone_change: 0,
        trigger_use_key: None,
        source_face_change: 0,
        ability_index: None,
        activated_ability: None,
        activated_ability_occurrence: None,
        triggered_ability: None,
        is_triggered: false,
        is_copy: false,
        face_index: 0,
        cast_method: SpellCastMethod::Normal,
        returned_attacker_assignment: None,
        chosen_x: 0,
        chosen_modes: Vec::new(),
        cast_condition_results: Vec::new(),
        cast_occurrence: None,
        cast_by: None,
        cast_cost_receipts: Vec::new(),
        payment_result: CardResultCohort::default(),
        exiled_cohorts: Default::default(),
        chaos_warp_owner_instructions: Default::default(),
        resolution_branch_choices: Default::default(),
        blight_receipts: Vec::new(),
        trigger_context: TriggerContext::default(),
    }
}

fn invoke(
    engine: &mut GameEngine,
    top: &StackItem,
    receipts: &[StackTarget],
    role_group_indices: &[u32],
    action: ChosenAction,
) -> Result<(EffectOutcome, Vec<rv1::RuledEvent>, EffectResult), EngineError> {
    let targets = receipts
        .iter()
        .map(|target| target.object_id)
        .collect::<Vec<_>>();
    let targets_by_role = role_group_indices
        .iter()
        .map(|group| {
            receipts
                .iter()
                .filter(|target| target.group_index == *group)
                .map(|target| target.object_id)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let previous_effect_result = EffectResult::default();
    let mut effect_result = EffectResult::default();
    let mut events = Vec::new();
    let outcome = {
        let mut cx = EffectCx {
            engine,
            events: &mut events,
            targets: &targets,
            targets_by_role: &targets_by_role,
            target_receipts: receipts,
            target_role_group_indices: role_group_indices,
            top,
            controller: 0,
            affected_player: 0,
            spell_label: "Target receipt fixture",
            previous_effect_result: &previous_effect_result,
            effect_result: &mut effect_result,
            effect_index: 0,
            resume_previous_result: None,
            program_state: None,
        };
        match action {
            ChosenAction::Fight => damage::fight(
                &mut cx,
                SpellEffectKind::Fight {
                    first: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    second: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
            ),
            ChosenAction::Destroy => misc::destroy(
                &mut cx,
                SpellEffectKind::Destroy {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
            ),
            ChosenAction::Sacrifice => misc::sacrifice(
                &mut cx,
                SpellEffectKind::Sacrifice {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
            ),
            ChosenAction::Attach => misc::attach_equipment(
                &mut cx,
                SpellEffectKind::AttachEquipment {
                    equipment: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    creature: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
            ),
            ChosenAction::Shuffle => zones::shuffle_permanents_into_owners_libraries(
                &mut cx,
                SpellEffectKind::ShufflePermanentsIntoOwnersLibraries {
                    subjects: vec![EffectSubject::Chosen(Box::new(
                        TargetFilter::default_creature(),
                    ))],
                },
            ),
            ChosenAction::Exile => zones::exile(
                &mut cx,
                SpellEffectKind::Exile {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    linked_exile_id: None,
                },
            ),
            ChosenAction::Tap => misc::tap(
                &mut cx,
                SpellEffectKind::Tap {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
            ),
            ChosenAction::Regenerate => misc::regenerate(
                &mut cx,
                SpellEffectKind::Regenerate {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
            ),
            ChosenAction::Exchange => zones::exchange_artifact_with_graveyard(&mut cx),
        }
    }?;
    Ok((outcome, events, effect_result))
}

fn action_fixture(action: ChosenAction) -> (GameEngine, Vec<ObjectId>, Vec<u32>) {
    let mut engine = fixture_engine();
    match action {
        ChosenAction::Fight => {
            let first = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
            let second = card_in_zone(&mut engine, 1, "hill_giant", Zone::Battlefield);
            (engine, vec![first, second], vec![0, 1])
        }
        ChosenAction::Destroy
        | ChosenAction::Sacrifice
        | ChosenAction::Shuffle
        | ChosenAction::Tap
        | ChosenAction::Regenerate => {
            let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
            (engine, vec![target], vec![0])
        }
        ChosenAction::Attach => {
            let equipment = card_in_zone(&mut engine, 0, "short_sword", Zone::Battlefield);
            let creature = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
            (engine, vec![equipment, creature], vec![0, 1])
        }
        ChosenAction::Exchange => {
            let departure = card_in_zone(&mut engine, 0, "short_sword", Zone::Battlefield);
            let incoming = card_in_zone(&mut engine, 0, "short_sword", Zone::Graveyard);
            (engine, vec![departure, incoming], vec![0, 1])
        }
        ChosenAction::Exile => unreachable!("Exile uses the mode-local fixture"),
    }
}

fn is_noop(
    action: ChosenAction,
    engine: &GameEngine,
    ids: &[ObjectId],
    result: &EffectResult,
) -> bool {
    match action {
        ChosenAction::Fight => {
            engine.state.objects[&ids[0]].damage == 0 && engine.state.objects[&ids[1]].damage == 0
        }
        ChosenAction::Destroy | ChosenAction::Sacrifice | ChosenAction::Shuffle => {
            engine.state.objects[&ids[0]].zone == Zone::Battlefield && result.cards.is_empty()
        }
        ChosenAction::Attach => engine.state.objects[&ids[0]].attached_to.is_none(),
        ChosenAction::Tap => !engine.state.objects[&ids[0]].tapped,
        ChosenAction::Regenerate => engine.state.objects[&ids[0]].regeneration_shields == 0,
        ChosenAction::Exchange => {
            engine.state.objects[&ids[0]].zone == Zone::Battlefield
                && engine.state.objects[&ids[1]].zone == Zone::Graveyard
        }
        ChosenAction::Exile => engine.state.objects[&ids[0]].zone == Zone::Graveyard,
    }
}

#[test]
fn chosen_action_receipts_reject_reentered_incarnations() {
    let mut still_acted = Vec::new();
    for action in ACTIONS {
        let stale_roles: &[usize] = match action {
            ChosenAction::Fight | ChosenAction::Attach => &[1],
            ChosenAction::Exchange => &[0, 1],
            ChosenAction::Exile => unreachable!(),
            _ => &[0],
        };
        for &stale_role in stale_roles {
            let (mut engine, ids, groups) = action_fixture(action);
            let receipts = ids
                .iter()
                .zip(&groups)
                .map(|(&id, &group)| receipt(&engine, id, group))
                .collect::<Vec<_>>();
            let stale_target_id = ids[stale_role];
            let original_zone = engine.state.objects[&stale_target_id].zone;
            let away_zone = match original_zone {
                Zone::Battlefield => Zone::Hand,
                Zone::Graveyard => Zone::Exile,
                _ => unreachable!("fixture targets a supported zone"),
            };
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                stale_target_id,
                away_zone,
                None,
            )
            .expect("leave the original target zone");
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                stale_target_id,
                original_zone,
                None,
            )
            .expect("return to the target zone as a new incarnation");

            let top = item(receipts.clone());
            let (_, _, result) = invoke(&mut engine, &top, &receipts, &groups, action)
                .expect("resolve chosen action");
            if !is_noop(action, &engine, &ids, &result) {
                still_acted.push(format!("{action:?} role {stale_role}"));
            }
        }
    }
    assert!(
        still_acted.is_empty(),
        "stale resolution-start receipts must not act on a returned incarnation: {still_acted:?}"
    );
}

fn action_applied(
    action: ChosenAction,
    engine: &GameEngine,
    ids: &[ObjectId],
    result: &EffectResult,
) -> bool {
    match action {
        ChosenAction::Fight => {
            engine.state.objects[&ids[0]].damage > 0 && engine.state.objects[&ids[1]].damage > 0
        }
        ChosenAction::Destroy | ChosenAction::Sacrifice => {
            engine.state.objects[&ids[0]].zone == Zone::Graveyard && !result.cards.is_empty()
        }
        ChosenAction::Attach => {
            engine.state.objects[&ids[0]].attached_to == Some(AttachmentRecipient::Object(ids[1]))
        }
        ChosenAction::Shuffle => engine.state.objects[&ids[0]].zone == Zone::Library,
        ChosenAction::Tap => engine.state.objects[&ids[0]].tapped,
        ChosenAction::Regenerate => engine.state.objects[&ids[0]].regeneration_shields > 0,
        ChosenAction::Exchange => {
            engine.state.objects[&ids[0]].zone == Zone::Graveyard
                && engine.state.objects[&ids[1]].zone == Zone::Battlefield
        }
        ChosenAction::Exile => engine.state.objects[&ids[0]].zone == Zone::Exile,
    }
}

#[test]
fn current_receipts_still_apply_each_chosen_action() {
    for action in ACTIONS {
        let (mut engine, ids, groups) = action_fixture(action);
        let receipts = ids
            .iter()
            .zip(&groups)
            .map(|(&id, &group)| receipt(&engine, id, group))
            .collect::<Vec<_>>();
        let top = item(receipts.clone());
        let (_, _, result) = invoke(&mut engine, &top, &receipts, &groups, action)
            .expect("resolve chosen action with current receipts");
        assert!(
            action_applied(action, &engine, &ids, &result),
            "fresh receipt did not apply {action:?}"
        );
    }
}

#[test]
fn regenerate_uses_a_current_target_even_after_it_loses_creature_type() {
    let (mut engine, ids, groups) = action_fixture(ChosenAction::Regenerate);
    let target = ids[0];
    let target_receipt = receipt(&engine, target, groups[0]);
    let artifact = card_in_zone(&mut engine, 0, "short_sword", Zone::Battlefield);
    let artifact_values = engine
        .copiable_values_for(artifact)
        .expect("artifact fixture has copiable values");
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("regeneration target")
        .copiable_values = Some(artifact_values);
    assert!(
        !engine
            .characteristics(target)
            .expect("target remains a permanent")
            .is_creature(),
        "the target loses its creature type after the target snapshot"
    );

    let top = item(vec![target_receipt]);
    let (_, _, _) = invoke(
        &mut engine,
        &top,
        &[target_receipt],
        &groups,
        ChosenAction::Regenerate,
    )
    .expect("resolve regeneration on the same permanent incarnation");

    assert_eq!(
        engine.state.objects[&target].regeneration_shields, 1,
        "a same-incarnation permanent target receives its regeneration shield"
    );
}

#[test]
fn mode_local_exile_uses_exact_receipt_for_owner_zero_and_ignores_sibling_group_zero() {
    let mut engine = fixture_engine();
    let selected = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let sibling = card_in_zone(&mut engine, 1, "hill_giant", Zone::Battlefield);
    assert_eq!(engine.state.objects[&selected].owner, 0);
    let selected_receipt = receipt(&engine, selected, 0);
    let sibling_receipt = receipt(&engine, sibling, 0);
    let mut top = item(Vec::new());
    top.chosen_modes = vec![
        ChosenMode {
            mode_id: tricerules_card_model::ModeId::new("sibling_mode").unwrap(),
            targets: vec![sibling_receipt],
        },
        ChosenMode {
            mode_id: tricerules_card_model::ModeId::new("selected_mode").unwrap(),
            targets: vec![selected_receipt],
        },
    ];

    let (_, _, _) = invoke(
        &mut engine,
        &top,
        &[selected_receipt],
        &[0],
        ChosenAction::Exile,
    )
    .expect("resolve mode-local Exile");

    assert_eq!(engine.state.objects[&selected].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&sibling].zone, Zone::Battlefield);
}

#[test]
fn gardens_copy_uses_only_retained_target_receipts_and_captured_incarnation_values() {
    let mut engine = fixture_engine();
    let gardens = card_in_zone(&mut engine, 0, "the_mycosynth_gardens", Zone::Battlefield);
    let target = card_in_zone(&mut engine, 0, "short_sword", Zone::Battlefield);
    let donor = card_in_zone(&mut engine, 0, "thran_dynamo", Zone::Battlefield);
    let old_source = set_copy_values_from(&mut engine, target, donor);
    let target_receipt = receipt(&engine, target, 0);
    let top = gardens_stack_item(&engine, gardens, target);

    invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::Gardens,
    )
    .expect("copy the current target incarnation");
    assert_eq!(
        engine.state.objects[&gardens]
            .copiable_values
            .as_ref()
            .map(|values| values.source_card_id.as_str()),
        Some(old_source.as_str()),
        "a current receipt copies the live copiable values"
    );

    for return_to_battlefield in [true, false] {
        let mut engine = fixture_engine();
        let gardens = card_in_zone(&mut engine, 0, "the_mycosynth_gardens", Zone::Battlefield);
        let target = card_in_zone(&mut engine, 0, "short_sword", Zone::Battlefield);
        let donor = card_in_zone(&mut engine, 0, "thran_dynamo", Zone::Battlefield);
        let old_source = set_copy_values_from(&mut engine, target, donor);
        let target_receipt = receipt(&engine, target, 0);
        let top = gardens_stack_item(&engine, gardens, target);
        move_object_to_zone(&mut engine.state, engine.registry, target, Zone::Hand, None)
            .expect("leave the target zone");
        if return_to_battlefield {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                target,
                Zone::Battlefield,
                None,
            )
            .expect("return as a new incarnation");
            assert_eq!(
                engine.copiable_values_for(target).unwrap().source_card_id,
                "short_sword",
                "the new incarnation has different live copiable values"
            );
        }

        invoke_copy_action(
            &mut engine,
            &top,
            &[target],
            &[target_receipt],
            CopyAction::Gardens,
        )
        .expect("resolve using the retained target incarnation");
        assert_eq!(
            engine.state.objects[&gardens]
                .copiable_values
                .as_ref()
                .map(|values| values.source_card_id.as_str()),
            Some(old_source.as_str()),
            "a legal-at-start target uses its captured copiable values after departure"
        );
    }

    let mut engine = fixture_engine();
    let gardens = card_in_zone(&mut engine, 0, "the_mycosynth_gardens", Zone::Battlefield);
    let target = card_in_zone(&mut engine, 0, "short_sword", Zone::Battlefield);
    let donor = card_in_zone(&mut engine, 0, "thran_dynamo", Zone::Battlefield);
    set_copy_values_from(&mut engine, target, donor);
    let top = gardens_stack_item(&engine, gardens, target);
    move_object_to_zone(&mut engine.state, engine.registry, target, Zone::Hand, None)
        .expect("leave the target zone");

    invoke_copy_action(&mut engine, &top, &[target], &[], CopyAction::Gardens)
        .expect("resolve without a retained target receipt");
    assert!(
        engine.state.objects[&gardens].copiable_values.is_none(),
        "an absent legal-at-start receipt cannot use saved LKI"
    );
}

#[test]
fn chosen_token_copy_uses_only_retained_target_receipts_and_captured_incarnation_values() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let donor = card_in_zone(&mut engine, 0, "hill_giant", Zone::Battlefield);
    let old_source = set_copy_values_from(&mut engine, target, donor);
    let target_receipt = receipt(&engine, target, 0);
    let top = item(vec![target_receipt]);

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::ChosenCreatureToken,
    )
    .expect("copy the current chosen creature incarnation");
    let token = result
        .produced_objects
        .first()
        .expect("current target creates a token")
        .object_id;
    assert_eq!(
        engine.state.objects[&token]
            .token_origin
            .as_ref()
            .map(|values| values.source_card_id.as_str()),
        Some(old_source.as_str()),
        "a current receipt copies the live copiable values"
    );

    for return_to_battlefield in [true, false] {
        let mut engine = fixture_engine();
        let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
        let donor = card_in_zone(&mut engine, 0, "hill_giant", Zone::Battlefield);
        let old_source = set_copy_values_from(&mut engine, target, donor);
        let target_receipt = receipt(&engine, target, 0);
        let top = item(vec![target_receipt]);
        move_object_to_zone(&mut engine.state, engine.registry, target, Zone::Hand, None)
            .expect("leave the target zone");
        if return_to_battlefield {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                target,
                Zone::Battlefield,
                None,
            )
            .expect("return as a new incarnation");
            assert_eq!(
                engine.copiable_values_for(target).unwrap().source_card_id,
                "grizzly_bears",
                "the new incarnation has different live copiable values"
            );
        }

        let (_, _, result) = invoke_copy_action(
            &mut engine,
            &top,
            &[target],
            &[target_receipt],
            CopyAction::ChosenCreatureToken,
        )
        .expect("resolve using the retained target incarnation");
        let token = result
            .produced_objects
            .first()
            .expect("legal-at-start target creates a token from its LKI")
            .object_id;
        assert_eq!(
            engine.state.objects[&token]
                .token_origin
                .as_ref()
                .map(|values| values.source_card_id.as_str()),
            Some(old_source.as_str()),
            "a departed target uses its captured copiable values"
        );
    }

    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let donor = card_in_zone(&mut engine, 0, "hill_giant", Zone::Battlefield);
    set_copy_values_from(&mut engine, target, donor);
    let top = item(Vec::new());
    move_object_to_zone(&mut engine.state, engine.registry, target, Zone::Hand, None)
        .expect("leave the target zone");

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[],
        CopyAction::ChosenCreatureToken,
    )
    .expect("resolve without a retained target receipt");
    assert!(
        result.produced_objects.is_empty(),
        "an absent legal-at-start receipt cannot use saved LKI"
    );
}
