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

fn fixture_engine_for(player_ids: &[PlayerId]) -> GameEngine {
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
        player_ids,
        20,
        Some(vec![deck; player_ids.len()]),
        true,
    )
    .expect("target receipt action fixture")
}

fn fixture_engine() -> GameEngine {
    fixture_engine_for(&[0, 1])
}

#[derive(Clone, Copy)]
enum CopyAction {
    Gardens,
    ChosenCreatureToken,
    ChosenCreatureTokenWithNoOverrides,
    ChosenCreatureTokenWithLegendaryRemoval,
    ChosenCreatureTokenWithColorlessOverride,
    ChosenCreatureTokenWithKayaException,
    ChosenCreatureTokenWithReplaceException,
    KayaMinusThree,
}

fn kaya_copy_exception(
    mode: tricerules_card_model::TokenCopySubtypeMode,
) -> tricerules_card_model::TokenCopyException {
    tricerules_card_model::TokenCopyException {
        colors: Some(vec![tricerules_card_model::primitives::Color::White]),
        power: Some(1),
        toughness: Some(1),
        remove_legendary: false,
        added_card_types: vec![tricerules_card_model::primitives::PermanentTypeFilter::Creature],
        creature_subtypes: Some(tricerules_card_model::TokenCopyCreatureSubtypeChange {
            mode,
            subtypes: vec!["Spirit".into()],
        }),
        added_keywords: vec![tricerules_card_model::primitives::Keyword::Flying],
    }
}

fn optional_copy_exception(
    colors: Option<Vec<tricerules_card_model::primitives::Color>>,
    power: Option<i32>,
    toughness: Option<i32>,
    remove_legendary: bool,
) -> tricerules_card_model::TokenCopyException {
    tricerules_card_model::TokenCopyException {
        colors,
        power,
        toughness,
        remove_legendary,
        added_card_types: vec![],
        creature_subtypes: None,
        added_keywords: vec![],
    }
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
                    copy_exception: None,
                },
            ),
            CopyAction::ChosenCreatureTokenWithNoOverrides
            | CopyAction::ChosenCreatureTokenWithLegendaryRemoval
            | CopyAction::ChosenCreatureTokenWithColorlessOverride => {
                let exception = match action {
                    CopyAction::ChosenCreatureTokenWithNoOverrides => {
                        optional_copy_exception(None, None, None, false)
                    }
                    CopyAction::ChosenCreatureTokenWithLegendaryRemoval => {
                        optional_copy_exception(None, None, None, true)
                    }
                    CopyAction::ChosenCreatureTokenWithColorlessOverride => {
                        optional_copy_exception(Some(vec![]), None, None, false)
                    }
                    _ => unreachable!("grouped copy-exception action"),
                };
                tokens::create_token_copies(
                    &mut cx,
                    SpellEffectKind::CreateTokenCopies {
                        count: tricerules_card_model::primitives::Amount::Fixed(1),
                        source: tricerules_card_model::primitives::TokenCopySource::Chosen(
                            Box::new(TargetFilter::default_creature()),
                        ),
                        copy_exception: Some(exception),
                    },
                )
            }
            CopyAction::ChosenCreatureTokenWithKayaException => tokens::create_token_copies(
                &mut cx,
                SpellEffectKind::CreateTokenCopies {
                    count: tricerules_card_model::primitives::Amount::Fixed(1),
                    source: tricerules_card_model::primitives::TokenCopySource::Chosen(Box::new(
                        TargetFilter::default_creature(),
                    )),
                    copy_exception: Some(kaya_copy_exception(
                        tricerules_card_model::TokenCopySubtypeMode::Add,
                    )),
                },
            ),
            CopyAction::ChosenCreatureTokenWithReplaceException => tokens::create_token_copies(
                &mut cx,
                SpellEffectKind::CreateTokenCopies {
                    count: tricerules_card_model::primitives::Amount::Fixed(1),
                    source: tricerules_card_model::primitives::TokenCopySource::Chosen(Box::new(
                        TargetFilter::default_creature(),
                    )),
                    copy_exception: Some(kaya_copy_exception(
                        tricerules_card_model::TokenCopySubtypeMode::Replace,
                    )),
                },
            ),
            CopyAction::KayaMinusThree => dispatch::execute_effect(
                &mut cx,
                SpellEffectKind::ExileTargetAndCopyUnlessAura {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
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

fn install_copy_exception_source_values(engine: &mut GameEngine, target: ObjectId) {
    let mut values = engine
        .copiable_values_for(target)
        .expect("fixture target has copiable values");
    values.face.types = vec!["Creature".into(), "Human".into()];
    values.face.color_indicator = Some(vec![tricerules_card_model::primitives::Color::Blue]);
    values.face.colors_override = Some(vec![tricerules_card_model::primitives::Color::Blue]);

    let cdas = vec![
        tricerules_card_model::IdentifiedAbility::fallback(
            "copy_test_devoid",
            tricerules_card_model::CharacteristicDefiningAbility::Devoid,
        )
        .expect("test Devoid CDA identity"),
        tricerules_card_model::IdentifiedAbility::fallback(
            "copy_test_colors",
            tricerules_card_model::CharacteristicDefiningAbility::DefinesColors {
                colors: vec![tricerules_card_model::primitives::Color::Blue],
            },
        )
        .expect("test color CDA identity"),
        tricerules_cards::registry::global()
            .get("lumbering_worldwagon")
            .expect("P/T CDA fixture is registered")
            .primary_face()
            .characteristic_defining_abilities[0]
            .clone(),
        tricerules_card_model::IdentifiedAbility::fallback(
            "copy_test_changeling",
            tricerules_card_model::CharacteristicDefiningAbility::Changeling,
        )
        .expect("test Changeling CDA identity"),
    ];
    values.face.characteristic_defining_abilities = cdas;
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("fixture target")
        .copiable_values = Some(values);
}

fn install_legendary_copy_exception_source_values(engine: &mut GameEngine, target: ObjectId) {
    install_copy_exception_source_values(engine, target);
    let mut values = engine.state.objects[&target]
        .copiable_values
        .clone()
        .expect("fixture source has owned copiable values");
    values.face.supertypes = vec!["Snow".into(), "Legendary".into()];
    values.face.power = Some(4);
    values.face.toughness = Some(6);
    values.face.loyalty = Some(3);
    values.face.keywords = vec![tricerules_card_model::primitives::Keyword::Flying];
    values.face.activated_abilities = engine
        .registry
        .get("lumbering_worldwagon")
        .expect("Lumbering Worldwagon fixture is registered")
        .primary_face()
        .activated_abilities
        .clone();
    values.face.derive_type_flags();
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("fixture source")
        .copiable_values = Some(values);
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

fn invoke_explore(
    engine: &mut GameEngine,
    top: &StackItem,
    receipts: &[StackTarget],
) -> Result<(EffectOutcome, Vec<rv1::RuledEvent>, EffectResult), EngineError> {
    let targets = receipts
        .iter()
        .map(|target| target.object_id)
        .collect::<Vec<_>>();
    let role_targets = vec![targets.clone()];
    let role_group_indices = [0];
    let previous_effect_result = EffectResult::default();
    let mut effect_result = EffectResult::default();
    let mut events = Vec::new();
    let outcome = {
        let mut cx = EffectCx {
            engine,
            events: &mut events,
            targets: &targets,
            targets_by_role: &role_targets,
            target_receipts: receipts,
            target_role_group_indices: &role_group_indices,
            top,
            controller: 0,
            affected_player: 0,
            spell_label: "Target receipt Explore fixture",
            previous_effect_result: &previous_effect_result,
            effect_result: &mut effect_result,
            effect_index: 0,
            resume_previous_result: None,
            program_state: None,
        };
        zones::explore(
            &mut cx,
            SpellEffectKind::Explore {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
        )
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

#[test]
fn chosen_exile_then_explore_uses_the_retained_receipt_lki() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let land = engine.state.players[0]
        .library
        .iter()
        .copied()
        .find(|id| engine.state.objects[id].card_id == "forest")
        .expect("fixture Forest");
    engine.state.players[0]
        .library
        .retain(|candidate| *candidate != land);
    engine.state.players[0].library.push_front(land);
    let receipts = vec![receipt(&engine, target, 0)];
    let top = item(receipts.clone());
    let old_generation = receipts[0]
        .zone_change_generation
        .expect("target receipt generation");

    let (outcome, _, _) = invoke(&mut engine, &top, &receipts, &[0], ChosenAction::Exile)
        .expect("resolve ExileChosen");
    assert_eq!(outcome, EffectOutcome::Continue);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_ne!(engine.state.zone_change_generation[&target], old_generation);

    let (outcome, events, _) = invoke_explore(&mut engine, &top, &receipts)
        .expect("resolve ExploreChosen using its retained target receipt");
    assert_eq!(outcome, EffectOutcome::Continue);
    assert_eq!(engine.state.objects[&land].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert!(events.iter().any(|event| matches!(
        &event.ev,
        Some(rv1::ruled_event::Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == land)
    )));
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

#[test]
fn kaya_copy_exception_changes_only_the_new_token_snapshot() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    install_copy_exception_source_values(&mut engine, target);
    let target_receipt = receipt(&engine, target, 0);
    let top = item(vec![target_receipt]);
    let original = engine
        .characteristics(target)
        .expect("source characteristics before copying");

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::ChosenCreatureTokenWithKayaException,
    )
    .expect("copy the target with Kaya's fixed exception");
    let token = result
        .produced_objects
        .first()
        .expect("copy exception creates one token")
        .object_id;
    let copy = engine.state.objects[&token]
        .token_origin
        .as_ref()
        .expect("token retains its copiable origin");
    assert_eq!(
        copy.face.colors(),
        vec![tricerules_card_model::primitives::Color::White]
    );
    assert_eq!((copy.face.power, copy.face.toughness), (Some(1), Some(1)));
    assert!(copy.face.types.iter().any(|kind| kind == "Creature"));
    assert!(copy.face.types.iter().any(|kind| kind == "Human"));
    assert!(copy.face.types.iter().any(|kind| kind == "Spirit"));
    assert!(copy
        .face
        .keywords
        .contains(&tricerules_card_model::primitives::Keyword::Flying));
    assert_eq!(copy.face.color_indicator, None);
    assert!(copy
        .face
        .characteristic_defining_abilities
        .iter()
        .any(|ability| matches!(
            &ability.definition,
            tricerules_card_model::CharacteristicDefiningAbility::Changeling
        )));
    assert!(!copy.face.characteristic_defining_abilities.iter().any(
        |ability| matches!(
            &ability.definition,
            tricerules_card_model::CharacteristicDefiningAbility::Devoid
                | tricerules_card_model::CharacteristicDefiningAbility::DefinesColors { .. }
                | tricerules_card_model::CharacteristicDefiningAbility::CountScaledPowerToughness { .. }
        )
    ));
    assert_eq!(engine.characteristics(target), Some(original));

    let token_receipt = receipt(&engine, token, 0);
    let top = item(vec![token_receipt]);
    let (_, _, second_result) = invoke_copy_action(
        &mut engine,
        &top,
        &[token],
        &[token_receipt],
        CopyAction::ChosenCreatureToken,
    )
    .expect("copy the already modified token without another exception");
    let second_token = second_result
        .produced_objects
        .first()
        .expect("later copy inherits the exception characteristics")
        .object_id;
    let second_copy = engine.state.objects[&second_token]
        .token_origin
        .as_ref()
        .expect("later copy retains its copiable values");
    assert_eq!(
        second_copy.face.colors(),
        vec![tricerules_card_model::primitives::Color::White]
    );
    assert_eq!(
        (second_copy.face.power, second_copy.face.toughness),
        (Some(1), Some(1))
    );
    assert!(second_copy.face.types.iter().any(|kind| kind == "Human"));
    assert!(second_copy.face.types.iter().any(|kind| kind == "Spirit"));
    assert!(second_copy
        .face
        .keywords
        .contains(&tricerules_card_model::primitives::Keyword::Flying));
}

#[test]
fn remove_legendary_copy_exception_preserves_other_owned_values() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    install_legendary_copy_exception_source_values(&mut engine, target);
    let source_values = engine.state.objects[&target]
        .copiable_values
        .clone()
        .expect("fixture owns copiable values");
    let original = engine
        .characteristics(target)
        .expect("source characteristics before copying");
    let target_receipt = receipt(&engine, target, 0);

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &item(vec![target_receipt]),
        &[target],
        &[target_receipt],
        CopyAction::ChosenCreatureTokenWithLegendaryRemoval,
    )
    .expect("copy the target after removing only Legendary");
    let token = result
        .produced_objects
        .first()
        .expect("copy exception creates a token")
        .object_id;
    let copy = engine.state.objects[&token]
        .token_origin
        .as_ref()
        .expect("token retains its owned copiable values");

    assert_eq!(copy.face.supertypes, vec!["Snow"]);
    assert!(!copy.face.is_legendary);
    assert_eq!(copy.face.types, source_values.face.types);
    assert_eq!(
        copy.face.color_indicator,
        source_values.face.color_indicator
    );
    assert_eq!(
        copy.face.colors_override,
        source_values.face.colors_override
    );
    assert_eq!((copy.face.power, copy.face.toughness), (Some(4), Some(6)));
    assert_eq!(copy.face.loyalty, Some(3));
    assert_eq!(copy.face.keywords, source_values.face.keywords);
    assert_eq!(
        copy.face.activated_abilities,
        source_values.face.activated_abilities
    );
    assert_eq!(
        copy.face.characteristic_defining_abilities,
        source_values.face.characteristic_defining_abilities
    );
    assert_eq!(copy.source_card_id, source_values.source_card_id);
    assert_eq!(copy.source_face_index, source_values.source_face_index);
    assert_eq!(copy.display_name, source_values.display_name);
    assert_eq!(copy.prototyped, source_values.prototyped);
    assert_eq!(
        copy.room_faces.is_some(),
        source_values.room_faces.is_some()
    );
    assert_eq!(engine.characteristics(target), Some(original));
}

#[test]
fn copy_exception_without_color_and_pt_overrides_preserves_copy_cdas() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    install_copy_exception_source_values(&mut engine, target);
    let source_values = engine.state.objects[&target]
        .copiable_values
        .clone()
        .expect("fixture owns copiable values");
    let target_receipt = receipt(&engine, target, 0);

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &item(vec![target_receipt]),
        &[target],
        &[target_receipt],
        CopyAction::ChosenCreatureTokenWithNoOverrides,
    )
    .expect("copy without color or P/T overrides");
    let token = result
        .produced_objects
        .first()
        .expect("copy exception creates a token")
        .object_id;
    let copy = engine.state.objects[&token]
        .token_origin
        .as_ref()
        .expect("token retains its owned copiable values");

    assert_eq!(
        copy.face.color_indicator,
        source_values.face.color_indicator
    );
    assert_eq!(
        copy.face.colors_override,
        source_values.face.colors_override
    );
    assert_eq!(
        (copy.face.power, copy.face.toughness),
        (source_values.face.power, source_values.face.toughness)
    );
    assert_eq!(
        copy.face.characteristic_defining_abilities,
        source_values.face.characteristic_defining_abilities
    );
}

#[test]
fn copy_exception_explicit_colorless_override_removes_only_color_cdas() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    install_copy_exception_source_values(&mut engine, target);
    let target_receipt = receipt(&engine, target, 0);

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &item(vec![target_receipt]),
        &[target],
        &[target_receipt],
        CopyAction::ChosenCreatureTokenWithColorlessOverride,
    )
    .expect("apply an explicit colorless override");
    let token = result
        .produced_objects
        .first()
        .expect("copy exception creates a token")
        .object_id;
    let copy = engine.state.objects[&token]
        .token_origin
        .as_ref()
        .expect("token retains its owned copiable values");

    assert_eq!(copy.face.colors(), Vec::new());
    assert_eq!(copy.face.colors_override, Some(Vec::new()));
    assert_eq!(copy.face.color_indicator, None);
    assert!(!copy
        .face
        .characteristic_defining_abilities
        .iter()
        .any(|ability| matches!(
            &ability.definition,
            tricerules_card_model::CharacteristicDefiningAbility::Devoid
                | tricerules_card_model::CharacteristicDefiningAbility::DefinesColors { .. }
        )));
    assert!(copy
        .face
        .characteristic_defining_abilities
        .iter()
        .any(|ability| matches!(
            &ability.definition,
            tricerules_card_model::CharacteristicDefiningAbility::CountScaledPowerToughness { .. }
        )));
    assert!(copy
        .face
        .characteristic_defining_abilities
        .iter()
        .any(|ability| matches!(
            &ability.definition,
            tricerules_card_model::CharacteristicDefiningAbility::Changeling
        )));
}

#[test]
fn remove_legendary_copy_exception_updates_active_and_intrinsic_faces() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    {
        let object = engine
            .state
            .objects
            .get_mut(&target)
            .expect("fixture source");
        object.card_id = "reckless_waif_merciless_predator".into();
        object.face_up_index = 1;
    }
    install_legendary_copy_exception_source_values(&mut engine, target);
    let target_receipt = receipt(&engine, target, 0);

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &item(vec![target_receipt]),
        &[target],
        &[target_receipt],
        CopyAction::ChosenCreatureTokenWithLegendaryRemoval,
    )
    .expect("copy both intrinsic faces after removing Legendary");
    let token = result
        .produced_objects
        .first()
        .expect("copy exception creates a token")
        .object_id;
    let object = &engine.state.objects[&token];
    assert_eq!(object.face_up_index, 1);
    assert_eq!(
        object
            .token_origin
            .as_ref()
            .expect("token retains its active copied face")
            .face
            .supertypes,
        vec!["Snow"]
    );
    let faces = object
        .token_faces
        .as_ref()
        .expect("token retains both intrinsic faces");
    assert_eq!(faces.layout, tricerules_card_model::Layout::Transform);
    for face in &faces.faces {
        assert_eq!(face.face.supertypes, vec!["Snow"]);
        assert!(!face.face.is_legendary);
    }
}

#[test]
fn token_copy_exception_replace_suppresses_changeling_and_existing_creature_subtypes() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    install_copy_exception_source_values(&mut engine, target);
    let target_receipt = receipt(&engine, target, 0);
    let top = item(vec![target_receipt]);
    let original = engine
        .characteristics(target)
        .expect("source characteristics before copying");

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::ChosenCreatureTokenWithReplaceException,
    )
    .expect("replace copied creature subtypes");
    let token = result
        .produced_objects
        .first()
        .expect("replacement exception creates a token")
        .object_id;
    let copy = engine.state.objects[&token]
        .token_origin
        .as_ref()
        .expect("token retains its copiable origin");
    assert!(!copy.face.types.iter().any(|kind| kind == "Human"));
    assert!(copy.face.types.iter().any(|kind| kind == "Spirit"));
    assert!(!copy
        .face
        .characteristic_defining_abilities
        .iter()
        .any(|ability| matches!(
            &ability.definition,
            tricerules_card_model::CharacteristicDefiningAbility::Changeling
        )));
    assert_eq!(engine.characteristics(target), Some(original));
}

#[test]
fn token_copy_exception_updates_both_double_faced_faces_and_keeps_the_active_face() {
    let mut engine = fixture_engine();
    let expected_layout = engine
        .registry
        .get("reckless_waif_merciless_predator")
        .expect("double-faced fixture is registered")
        .layout;
    assert_eq!(expected_layout, tricerules_card_model::Layout::Transform);
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    {
        let object = engine
            .state
            .objects
            .get_mut(&target)
            .expect("fixture target");
        object.card_id = "reckless_waif_merciless_predator".into();
        object.face_up_index = 1;
    }
    let target_receipt = receipt(&engine, target, 0);
    let top = item(vec![target_receipt]);

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::ChosenCreatureTokenWithKayaException,
    )
    .expect("copy the double-faced target with Kaya's exception");
    let token = result
        .produced_objects
        .first()
        .expect("double-faced copy creates a token")
        .object_id;
    let object = &engine.state.objects[&token];
    let faces = object
        .token_faces
        .as_ref()
        .expect("copy token retains both intrinsic faces");
    assert_eq!(faces.layout, expected_layout);
    assert_eq!(object.face_up_index, 1);
    for (index, face) in faces.faces.iter().enumerate() {
        assert_eq!(face.source_face_index, index);
        assert_eq!(
            face.face.colors(),
            vec![tricerules_card_model::primitives::Color::White]
        );
        assert_eq!((face.face.power, face.face.toughness), (Some(1), Some(1)));
        assert!(face.face.is_creature);
        assert!(face.face.types.iter().any(|kind| kind == "Spirit"));
        assert!(face
            .face
            .keywords
            .contains(&tricerules_card_model::primitives::Keyword::Flying));
    }
    assert_eq!(engine.state.objects[&target].face_up_index, 1);

    let mut face_events = Vec::new();
    assert!(engine
        .change_permanent_face(
            token,
            tricerules_card_model::primitives::FaceChangeAction::Transform,
            &mut face_events,
        )
        .expect("transform the copied token through the engine face-change path"));
    assert_eq!(engine.state.objects[&token].face_up_index, 0);
    assert!(face_events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::FaceChanged(_))
    )));

    let token_receipt = receipt(&engine, token, 0);
    let top = item(vec![token_receipt]);
    let (_, _, later_result) = invoke_copy_action(
        &mut engine,
        &top,
        &[token],
        &[token_receipt],
        CopyAction::ChosenCreatureToken,
    )
    .expect("copy the transformed token without another exception");
    let later_token = later_result
        .produced_objects
        .first()
        .expect("transformed copy produces a later token")
        .object_id;
    let later_object = &engine.state.objects[&later_token];
    assert_eq!(later_object.face_up_index, 0);
    let later_faces = later_object
        .token_faces
        .as_ref()
        .expect("later copy retains both transformed faces");
    for face in &later_faces.faces {
        assert_eq!(
            face.face.colors(),
            vec![tricerules_card_model::primitives::Color::White]
        );
        assert_eq!((face.face.power, face.face.toughness), (Some(1), Some(1)));
        assert!(face.face.types.iter().any(|kind| kind == "Spirit"));
        assert!(face
            .face
            .keywords
            .contains(&tricerules_card_model::primitives::Keyword::Flying));
    }
}

#[test]
fn kaya_minus_three_exiles_a_non_aura_then_creates_the_exception_token() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let target_receipt = receipt(&engine, target, 0);
    let top = item(vec![target_receipt]);

    let (outcome, events, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::KayaMinusThree,
    )
    .expect("resolve Kaya's compound exile-and-copy effect");
    assert_eq!(outcome, EffectOutcome::Continue);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::Log(message)) if message.text.contains("exiles")
    )));
    let token = result
        .produced_objects
        .last()
        .expect("non-Aura target creates one copy token")
        .object_id;
    let copy = engine.state.objects[&token]
        .token_origin
        .as_ref()
        .expect("Kaya token retains copied characteristics");
    assert_eq!(
        copy.face.colors(),
        vec![tricerules_card_model::primitives::Color::White]
    );
    assert_eq!((copy.face.power, copy.face.toughness), (Some(1), Some(1)));
    assert!(copy.face.types.iter().any(|kind| kind == "Spirit"));
}

#[test]
fn kaya_copy_source_prepared_state_uses_current_derived_characteristics_for_same_generation() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let donor = card_in_zone(&mut engine, 0, "hill_giant", Zone::Battlefield);
    set_copy_values_from(&mut engine, target, donor);
    let generation = engine.state.zone_change_generation[&target];

    let (types, snapshot) = zones::token_copy_source_after_move(&engine, target, generation)
        .expect("same-generation current battlefield values are available");
    assert!(types.iter().any(|kind| kind == "Giant"));
    assert!(!types.iter().any(|kind| kind == "Bear"));
    assert_eq!(snapshot.values.source_card_id, "hill_giant");
}

#[test]
fn kaya_minus_three_prepared_state_uses_old_generation_lki_not_a_returned_permanent() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let donor = card_in_zone(&mut engine, 0, "hill_giant", Zone::Battlefield);
    let old_source = set_copy_values_from(&mut engine, target, donor);
    let target_receipt = receipt(&engine, target, 0);
    let old_generation = target_receipt.zone_change_generation.unwrap();
    let top = item(vec![target_receipt]);
    move_object_to_zone(&mut engine.state, engine.registry, target, Zone::Hand, None)
        .expect("leave the captured battlefield incarnation");
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        target,
        Zone::Battlefield,
        None,
    )
    .expect("return the physical card as a newer incarnation");
    let new_generation = engine.state.zone_change_generation[&target];
    assert_ne!(new_generation, old_generation);

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::KayaMinusThree,
    )
    .expect("resolve with the retained old-generation receipt");
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.zone_change_generation[&target], new_generation);
    let token = result
        .produced_objects
        .first()
        .expect("old-generation LKI still supplies the copy source")
        .object_id;
    let copy = engine.state.objects[&token]
        .token_origin
        .as_ref()
        .expect("token retains old-generation values");
    assert_eq!(copy.source_card_id, old_source);
    assert_eq!(
        copy.face.colors(),
        vec![tricerules_card_model::primitives::Color::White]
    );
}

#[test]
fn kaya_minus_three_prepared_state_fails_closed_when_exact_copy_lki_is_missing() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let target_receipt = receipt(&engine, target, 0);
    let old_generation = target_receipt.zone_change_generation.unwrap();
    let top = item(vec![target_receipt]);
    move_object_to_zone(&mut engine.state, engine.registry, target, Zone::Hand, None)
        .expect("leave the captured battlefield incarnation");
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        target,
        Zone::Battlefield,
        None,
    )
    .expect("return the physical card as a newer incarnation");
    engine
        .state
        .last_known_copy_by_generation
        .remove(&(target, old_generation));

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::KayaMinusThree,
    )
    .expect("missing old-generation copy LKI is a no-op after Exile attempt");
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert!(result.produced_objects.is_empty());
}

#[test]
fn kaya_minus_three_prepared_state_uses_derived_aura_types_not_the_copy_face_flag() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let target_receipt = receipt(&engine, target, 0);
    let old_generation = target_receipt.zone_change_generation.unwrap();
    let top = item(vec![target_receipt]);
    move_object_to_zone(&mut engine.state, engine.registry, target, Zone::Hand, None)
        .expect("leave the captured battlefield incarnation");
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        target,
        Zone::Battlefield,
        None,
    )
    .expect("return the physical card as a newer incarnation");
    engine.state.last_known_types_by_generation.insert(
        (target, old_generation),
        vec!["Creature".into(), "Aura".into()],
    );
    assert!(
        !engine.state.last_known_copy_by_generation[&(target, old_generation)]
            .values
            .face
            .is_aura
    );

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::KayaMinusThree,
    )
    .expect("derived Aura LKI suppresses token creation");
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert!(result.produced_objects.is_empty());
}

#[test]
fn kaya_minus_three_copies_face_down_battlefield_values_before_exile_reveals_the_card() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    engine.state.objects.get_mut(&target).unwrap().face_down = true;
    let target_receipt = receipt(&engine, target, 0);
    let top = item(vec![target_receipt]);
    let old_generation = target_receipt.zone_change_generation.unwrap();
    let face_down_snapshot = super::copying::token_copy_snapshot_for_incarnation(
        &engine.state,
        engine.registry,
        target,
        old_generation,
    )
    .expect("face-down battlefield values are copiable");
    assert_eq!(face_down_snapshot.token_id, "anonymous_creature_token");
    assert!(face_down_snapshot.values.source_card_id.is_empty());

    let (_, _, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::KayaMinusThree,
    )
    .expect("use the face-down battlefield copy snapshot after ordinary Exile");
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    let token = result
        .produced_objects
        .first()
        .expect("face-down non-Aura source creates a token")
        .object_id;
    let copy = engine.state.objects[&token]
        .token_origin
        .as_ref()
        .expect("token retains anonymous face-down copiable values");
    assert!(copy.source_card_id.is_empty());
    assert_eq!(copy.display_name, "Face-down creature");
    assert_eq!(
        copy.face.colors(),
        vec![tricerules_card_model::primitives::Color::White]
    );
}

#[test]
fn kaya_minus_three_real_token_entry_choice_resumes_the_tail_once() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let donor = card_in_zone(&mut engine, 0, "hill_giant", Zone::Battlefield);
    let source = card_in_zone(&mut engine, 0, "the_mycosynth_gardens", Zone::Battlefield);
    let mut copied_values = engine
        .copiable_values_for(target)
        .expect("target copy source has current values");
    copied_values.face.static_abilities = engine
        .registry
        .get("clone")
        .expect("Clone replacement fixture is registered")
        .primary_face()
        .static_abilities
        .clone();
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .copiable_values = Some(copied_values);

    let target_receipt = receipt(&engine, target, 0);
    let mut top = item(vec![target_receipt]);
    let mut activated_ability = engine
        .registry
        .get("the_mycosynth_gardens")
        .expect("ability fixture is registered")
        .primary_face()
        .activated_abilities[0]
        .clone();
    activated_ability.effect = vec![
        SpellEffectKind::ExileTargetAndCopyUnlessAura {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        },
        SpellEffectKind::Draw {
            who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            count: tricerules_card_model::primitives::Amount::Fixed(1),
        },
    ];
    activated_ability.program = None;
    activated_ability.modal = None;
    activated_ability.targeting = None;
    top.id = u32::MAX;
    top.card_id = "the_mycosynth_gardens".into();
    top.ability_text = Some("Kaya −3 continuation fixture".into());
    top.source_permanent_id = Some(source);
    top.source_owner = Some(0);
    top.source_zone_change = engine.state.zone_change_generation[&source];
    top.ability_index = Some(0);
    top.activated_ability = Some(activated_ability);
    engine.state.stack.push(top);

    let hand_before = engine.state.players[0].hand.len();
    let mut initial_events = Vec::new();
    assert_eq!(
        engine
            .resolve_top_of_stack(&mut initial_events)
            .expect("resolve into the token copy-source choice"),
        ResolutionProgress::Parked
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    let exiled_generation = engine.state.zone_change_generation[&target];
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Clone as-enters choice parks token entry");
    assert_eq!(
        pending.presentation.choice_kind,
        rv1::ChoiceKind::CopySource
    );
    assert_eq!(
        pending
            .continuation
            .stack()
            .expect("entry choice retains the resolving ability")
            .resume_effect_index,
        Some(1),
        "the continuation starts after the compound exile/copy instruction"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before);

    engine
        .publish_resolution_prompt_ids(&mut initial_events)
        .expect("publish the entry copy-source choice");
    let choice = initial_events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)
                if choice.choice_kind == rv1::ChoiceKind::CopySource as i32 =>
            {
                Some(choice.clone())
            }
            _ => None,
        })
        .expect("published Clone copy-source prompt");
    assert!(choice.candidate_object_ids.contains(&donor));

    let resumed = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: choice.resolution_prompt_id,
                        chosen_object_ids: vec![donor],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("finish token entry and resume after the compound effect");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(
        engine.state.zone_change_generation[&target],
        exiled_generation
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(
        engine
            .state
            .objects
            .values()
            .filter(|object| object.is_token() && object.zone == Zone::Battlefield)
            .count(),
        1,
        "resuming the entry choice does not repeat token creation"
    );
    let events = initial_events.iter().chain(resumed.events.iter());
    assert_eq!(
        events
            .clone()
            .filter(|event| matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::TokenCreated(_))
            ))
            .count(),
        1
    );
    assert_eq!(
        events
            .filter(|event| matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::Log(log)) if log.text.contains("exiles")
            ))
            .count(),
        1,
        "the target is exiled exactly once across suspension and resume"
    );
}

#[test]
fn kaya_minus_three_concede_during_parked_clone_entry_finishes_outer_resolution_once() {
    let mut engine = fixture_engine_for(&[0, 1, 2]);
    let target = card_in_zone(&mut engine, 1, "grizzly_bears", Zone::Battlefield);
    let donor = card_in_zone(&mut engine, 1, "hill_giant", Zone::Battlefield);
    let source = card_in_zone(&mut engine, 0, "the_mycosynth_gardens", Zone::Battlefield);
    let mut copied_values = engine
        .copiable_values_for(target)
        .expect("target copy source has current values");
    copied_values.face.static_abilities = engine
        .registry
        .get("clone")
        .expect("Clone replacement fixture is registered")
        .primary_face()
        .static_abilities
        .clone();
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .copiable_values = Some(copied_values);

    let target_receipt = receipt(&engine, target, 0);
    let mut top = item(vec![target_receipt]);
    let mut activated_ability = engine
        .registry
        .get("the_mycosynth_gardens")
        .expect("ability fixture is registered")
        .primary_face()
        .activated_abilities[0]
        .clone();
    activated_ability.effect = vec![
        SpellEffectKind::ExileTargetAndCopyUnlessAura {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        },
        SpellEffectKind::Draw {
            who: tricerules_card_model::primitives::PlayerRecipient::EachOpponent,
            count: tricerules_card_model::primitives::Amount::Fixed(1),
        },
    ];
    activated_ability.program = None;
    activated_ability.modal = None;
    activated_ability.targeting = None;
    top.id = u32::MAX;
    top.card_id = "the_mycosynth_gardens".into();
    top.ability_text = Some("Kaya −3 departure continuation fixture".into());
    top.source_permanent_id = Some(source);
    top.source_owner = Some(0);
    top.source_zone_change = engine.state.zone_change_generation[&source];
    top.ability_index = Some(0);
    top.activated_ability = Some(activated_ability);
    engine.state.stack.push(top);

    let hand_sizes_before = [
        engine.state.players[1].hand.len(),
        engine.state.players[2].hand.len(),
    ];
    let cards_drawn_before = [
        engine.state.turn_history.current.player(1).cards_drawn,
        engine.state.turn_history.current.player(2).cards_drawn,
    ];
    let mut initial_events = Vec::new();
    assert_eq!(
        engine
            .resolve_top_of_stack(&mut initial_events)
            .expect("resolve into the token copy-source choice"),
        ResolutionProgress::Parked
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    let exiled_generation = engine.state.zone_change_generation[&target];
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Clone as-enters choice parks token entry");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(
        pending.presentation.choice_kind,
        rv1::ChoiceKind::CopySource
    );

    engine
        .publish_resolution_prompt_ids(&mut initial_events)
        .expect("publish the entry copy-source choice");
    let choice = initial_events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)
                if choice.choice_kind == rv1::ChoiceKind::CopySource as i32 =>
            {
                Some(choice.clone())
            }
            _ => None,
        })
        .expect("published Clone copy-source prompt");
    assert!(choice.candidate_object_ids.contains(&donor));

    let conceded = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .expect("a player may concede while the token-entry choice is parked");

    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[1].has_lost);
    assert!(!engine.state.players[2].has_lost);
    assert!(
        !engine.state.is_terminal(),
        "two players remain in the game"
    );
    assert!(
        engine.state.pending_resolution.is_none(),
        "the departed chooser leaves no stuck prompt"
    );
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(
        engine.state.stack.is_empty(),
        "the begun outer resolution completed"
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(
        engine.state.zone_change_generation[&target],
        exiled_generation
    );
    assert_eq!(engine.state.players[1].hand.len(), hand_sizes_before[0] + 1);
    assert_eq!(engine.state.players[2].hand.len(), hand_sizes_before[1] + 1);
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        cards_drawn_before[0] + 1
    );
    assert_eq!(
        engine.state.turn_history.current.player(2).cards_drawn,
        cards_drawn_before[1] + 1
    );

    let events = initial_events.iter().chain(conceded.events.iter());
    assert_eq!(
        events
            .clone()
            .filter(|event| matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                    if moved.object_id == target
                        && moved.destination == rv1::permanent_moved::Destination::Exile as i32
            ))
            .count(),
        1,
        "the target's begun exile move is not repeated after departure"
    );
    assert_eq!(
        events
            .clone()
            .filter(|event| matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::TokenCreated(_))
            ))
            .count(),
        0,
        "a departed original controller receives no token"
    );
    assert!(engine
        .state
        .objects
        .values()
        .all(|object| !object.is_token() || object.owner != 0));
}

#[test]
fn kaya_minus_three_does_not_mint_for_a_departed_original_controller() {
    let mut engine = fixture_engine();
    let target = card_in_zone(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    let target_receipt = receipt(&engine, target, 0);
    let top = item(vec![target_receipt]);
    engine.state.players[0].has_lost = true;

    let (outcome, _, result) = invoke_copy_action(
        &mut engine,
        &top,
        &[target],
        &[target_receipt],
        CopyAction::KayaMinusThree,
    )
    .expect("begun resolution finishes after its controller leaves");
    assert_eq!(outcome, EffectOutcome::Continue);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(result.produced_objects.is_empty());
    assert!(engine
        .state
        .objects
        .values()
        .all(|object| !object.is_token() || object.zone != Zone::Battlefield));
}
