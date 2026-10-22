//! Duplicant's entry-linked exile and copiable characteristic continuous effect.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
};
use tricerules_cards::CounterKind;
use tricerules_core::{AffectedScope, ContinuousEffect, EngineDeck, TurnStep, Zone};
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;
use tricerules_proto::ruled::v1::{
    dev_command::Dev, ruled_command::Cmd, ruled_event::Ev, ChooseTriggerTarget, DevCommand,
    DevMoveCard, DevZone, RuledCommand, RuledEventBatch,
};

fn engine() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        508_001,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    grant_pool(&mut engine, 0);
    engine
}

fn three_player_engine() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        508_003,
        &[0, 1, 2],
        20,
        None,
        true,
    )
    .expect("three-player engine");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    engine
}

fn four_player_engine() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        508_004,
        &[0, 1, 2, 3],
        20,
        None,
        true,
    )
    .expect("four-player engine");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    engine
}

fn select_branch(index: u32) -> RuledCommand {
    let mut command = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() {
        answer.selected_branch_index = index;
    }
    command
}

fn stack_resolved_count(batch: &RuledEventBatch, stack_id: u32) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(event.ev.as_ref(), Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id)
        })
        .count()
}

fn choose_trigger_target(object_id: Option<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: object_id.map(target_object).unwrap_or_default(),
        })),
    }
}

fn cast_duplicant_and_resolve_imprint(engine: &mut GameEngine, target: Option<u32>) -> u32 {
    cast_duplicant_with_decision(engine, target, ResolutionChoiceDecision::SelectBranch)
}

fn cast_duplicant_with_decision(
    engine: &mut GameEngine,
    target: Option<u32>,
    decision: ResolutionChoiceDecision,
) -> u32 {
    let duplicant = inject_card_into_hand(engine, 0, "duplicant");
    let slot = hand_index_for_card(engine, 0, "duplicant");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Duplicant");
    pass_both_players(engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(target))
        .expect("choose Duplicant's optional ETB target");
    pass_both_players(engine);
    answer_optional_triggered_ability_choice(engine, decision);
    duplicant
}

fn move_owned_card(
    engine: &mut GameEngine,
    actor: i32,
    owner: i32,
    card_name: &str,
    zone: DevZone,
) {
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: card_name.to_string(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap_or_else(|error| panic!("move P{owner}'s {card_name} to {zone:?}: {error:?}"));
}

fn engine_with_giant_count_cda() -> tricerules_core::GameEngine {
    const GIANT_COUNTER: &str = r#"(
        id: "giant_counter",
        name: "Giant Counter",
        face_id: "giant_counter",
        types: ["Creature", "Giant"],
        power: 1,
        toughness: 1,
        characteristic_defining_abilities: [(
            ability_id: "characteristic_01",
            presentation: Fallback,
            definition: CountScaledPowerToughness(
                count: BattlefieldCreatures(filter: (
                    controllers: Controller,
                    subtype: Some("Giant"),
                    exclude_source: true,
                )),
                power_per_match: 1,
                toughness_per_match: 1,
            ),
        )],
    )"#;
    const VANILLA_GIANT: &str = r#"(
        id: "vanilla_giant",
        name: "Vanilla Giant",
        face_id: "vanilla_giant",
        types: ["Creature", "Giant"],
        power: 2,
        toughness: 2,
    )"#;
    const FILLER: &str = r#"(
        id: "filler",
        name: "Filler",
        face_id: "filler",
        types: ["Land"],
    )"#;
    let duplicant = include_str!("../../../tricerules-cards/data/duplicant.ron");
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[duplicant, GIANT_COUNTER, VANILLA_GIANT, FILLER],
            &[],
        )
        .expect("Duplicant layer-cycle fixture is valid card data"),
    ));
    let deck = vec!["filler".to_string(); 12];
    let mut engine = tricerules_core::GameEngine::new(
        registry,
        508_002,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("engine with synthetic Giant CDA");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    engine
}

fn engine_with_kokusho_commander() -> (tricerules_core::GameEngine, u32) {
    let decks = Some(vec![
        EngineDeck {
            mainboard: vec!["mountain".to_string(); 12],
            commanders: Vec::new(),
        },
        EngineDeck {
            mainboard: vec!["island".to_string(); 12],
            commanders: vec!["kokusho,_the_evening_star".to_string()],
        },
    ]);
    let mut engine = tricerules_core::GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        508_003,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Commander engine with Kokusho");
    advance_to_main1_from_game_start(&mut engine);
    let commander = engine.state.players[1].command_zone[0];
    let actor = engine.state.priority_player_id();
    move_owned_card(
        &mut engine,
        actor,
        1,
        "Kokusho, the Evening Star",
        DevZone::Battlefield,
    );
    grant_pool(&mut engine, 0);
    (engine, commander)
}

#[test]
fn duplicant_imprints_the_exact_exiled_creature_characteristics() {
    let mut engine = engine();
    let target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");

    let duplicant = cast_duplicant_and_resolve_imprint(&mut engine, Some(target));

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    let characteristics = engine.characteristics(duplicant).expect("Duplicant");
    assert!(characteristics.has_type("Artifact"));
    assert!(characteristics.has_type("Creature"));
    assert!(characteristics.has_type("Giant"));
    assert!(characteristics.has_type("Shapeshifter"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(3), Some(3))
    );

    let batch = engine.initial_response_batch();
    let view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("updated public battlefield view");
    let public_duplicant = view
        .per_player
        .iter()
        .flat_map(|player| &player.battlefield_objects)
        .find(|object| object.object_id == duplicant)
        .expect("public zone view includes each controller's battlefield");
    assert_eq!(public_duplicant.controller_player_id, Some(0));
    assert_eq!((public_duplicant.power, public_duplicant.toughness), (3, 3));
    assert!(public_duplicant
        .rules_annotation_labels
        .contains(&"Creature types: Giant, Shapeshifter".to_string()));

    engine
        .state
        .objects
        .get_mut(&duplicant)
        .expect("Duplicant object")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let modified = engine
        .characteristics(duplicant)
        .expect("modified Duplicant");
    assert_eq!(
        (modified.power, modified.toughness),
        (Some(4), Some(4)),
        "the layer-7b copied base applies before +1/+1 counters"
    );
}

#[test]
fn duplicant_declined_imprint_keeps_its_printed_characteristics() {
    let mut engine = engine();
    let target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");

    let duplicant =
        cast_duplicant_with_decision(&mut engine, Some(target), ResolutionChoiceDecision::Decline);

    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    let characteristics = engine.characteristics(duplicant).expect("Duplicant");
    assert!(!characteristics.has_type("Giant"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(2), Some(4))
    );
}

#[test]
fn duplicant_rejects_a_token_target_without_consuming_the_trigger() {
    let mut engine = engine();
    let token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    let legal_target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    assert!(engine.state.objects[&token].is_token());

    let duplicant = inject_card_into_hand(&mut engine, 0, "duplicant");
    let slot = hand_index_for_card(&engine, 0, "duplicant");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Duplicant");
    pass_both_players(&mut engine);

    let pending_trigger_id = engine
        .state
        .pending_triggers
        .front()
        .expect("Duplicant trigger awaits target selection")
        .object_id;
    let revision_before_rejection = engine.state.command_index;
    let rejected = engine.apply_scenario_command(0, &choose_trigger_target(Some(token)));
    assert!(rejected.is_err(), "a token is not a legal Duplicant target");
    assert_eq!(engine.state.command_index, revision_before_rejection);
    assert_eq!(
        engine
            .state
            .pending_triggers
            .front()
            .map(|trigger| trigger.object_id),
        Some(pending_trigger_id),
        "rejecting the token preserves the pending trigger for a legal answer"
    );
    assert_eq!(engine.state.objects[&token].zone, Zone::Battlefield);

    engine
        .apply_scenario_command(0, &choose_trigger_target(Some(legal_target)))
        .expect("select the legal nontoken creature after the rejected answer");
    pass_both_players(&mut engine);
    answer_optional_triggered_ability_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);

    assert_eq!(engine.state.objects[&legal_target].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&token].zone, Zone::Battlefield);
    assert!(engine.characteristics(duplicant).unwrap().has_type("Giant"));
}

#[test]
fn duplicant_does_not_copy_a_card_that_was_only_creature_on_the_battlefield() {
    let mut engine = engine();
    let target = inject_creature_on_battlefield(&mut engine, 1, "island");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(target),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            ..Default::default()
        }),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    assert!(engine.characteristics(target).unwrap().is_creature());

    let duplicant = cast_duplicant_and_resolve_imprint(&mut engine, Some(target));

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(!engine.characteristics(target).unwrap().is_creature());
    let characteristics = engine.characteristics(duplicant).expect("Duplicant");
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(2), Some(4))
    );
    assert!(!characteristics.has_type("Island"));
}

#[test]
fn duplicant_exile_uses_the_existing_aura_and_equipment_zone_change_sbas() {
    let mut engine = engine();
    let target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
    let equipment = inject_permanent_on_battlefield(&mut engine, 1, "bonesplitter");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::AttachmentRecipient::Object(target));
    engine
        .state
        .objects
        .get_mut(&equipment)
        .unwrap()
        .attached_to = Some(tricerules_core::AttachmentRecipient::Object(target));

    cast_duplicant_and_resolve_imprint(&mut engine, Some(target));

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&equipment].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
}

#[test]
fn duplicant_link_tracks_commander_acceptance_and_decline() {
    for (branch, expected_zone) in [(0, Zone::Command), (1, Zone::Exile)] {
        let (mut engine, commander) = engine_with_kokusho_commander();
        let duplicant = cast_duplicant_and_resolve_imprint(&mut engine, Some(commander));
        assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
        let linked = engine.characteristics(duplicant).expect("linked Duplicant");
        assert!(linked.has_type("Dragon"));
        assert_eq!((linked.power, linked.toughness), (Some(5), Some(5)));

        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("commander state-based-action choice");
        assert_eq!(pending.deciding_player, 1);
        let result = engine
            .apply_scenario_command(
                1,
                &RuledCommand {
                    cmd: Some(Cmd::SubmitResolutionChoice(
                        tricerules_proto::ruled::v1::SubmitResolutionChoice {
                            resolution_prompt_id: 0,
                            decision: ResolutionChoiceDecision::SelectBranch as i32,
                            selected_branch_index: branch,
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("answer the commander state-based-action choice");
        assert_eq!(engine.state.objects[&commander].zone, expected_zone);
        let after = engine
            .characteristics(duplicant)
            .expect("updated Duplicant");
        if branch == 0 {
            assert!(!after.has_type("Dragon"));
            assert_eq!((after.power, after.toughness), (Some(2), Some(4)));
            let view = result
                .events
                .iter()
                .find_map(|event| match &event.ev {
                    Some(Ev::ZoneView(view)) => Some(view),
                    _ => None,
                })
                .expect("accepted commander move publishes the linked characteristic change");
            assert!(!view.battlefields_unchanged);
        } else {
            assert!(after.has_type("Dragon"));
            assert_eq!((after.power, after.toughness), (Some(5), Some(5)));
            let next_actor = engine.state.priority_player_id();
            let batch = engine
                .apply_scenario_command(next_actor, &pass())
                .expect("the declined zone-change generation does not prompt again");
            assert!(!batch.events.iter().any(|event| matches!(
                event.ev.as_ref(),
                Some(Ev::ResolutionChoiceRequired(choice))
                    if choice.prompt_text.contains("command zone")
            )));
        }
    }
}

#[test]
fn duplicant_does_not_exile_a_new_zone_incarnation_of_its_target() {
    let mut engine = engine();
    let target = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    let duplicant = inject_card_into_hand(&mut engine, 0, "duplicant");
    let slot = hand_index_for_card(&engine, 0, "duplicant");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Duplicant");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(Some(target)))
        .expect("choose Hill Giant as the ETB target");

    move_owned_card(&mut engine, 0, 0, "Hill Giant", DevZone::Hand);
    move_owned_card(&mut engine, 0, 0, "Hill Giant", DevZone::Battlefield);
    let returned_generation = engine.state.zone_change_generation[&target];
    assert!(returned_generation > original_generation);
    pass_both_players(&mut engine);
    if engine.state.pending_resolution.is_some() {
        answer_optional_triggered_ability_choice(
            &mut engine,
            ResolutionChoiceDecision::SelectBranch,
        );
    }

    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&target],
        returned_generation
    );
    let characteristics = engine.characteristics(duplicant).expect("Duplicant");
    assert!(!characteristics.has_type("Giant"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(2), Some(4))
    );
}

#[test]
fn duplicant_delegate_applies_to_the_exact_original_target_without_a_second_fizzle() {
    let mut engine = three_player_engine();
    let target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let other_creature = inject_creature_on_battlefield(&mut engine, 2, "hill_giant");
    let target_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    let other_generation = engine
        .state
        .zone_change_generation
        .get(&other_creature)
        .copied()
        .unwrap_or(0);
    let duplicant = inject_card_into_hand(&mut engine, 0, "duplicant");
    let slot = hand_index_for_card(&engine, 0, "duplicant");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Duplicant");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(Some(target)))
        .expect("choose the exact original Hill Giant incarnation");
    pass_priority_round(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Duplicant begins resolving its optional targeted ability");
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("Duplicant uses the optional-trigger continuation");
    };
    let original = stack.item.clone();
    assert_eq!(original.controller, 0);
    assert_eq!(original.source_permanent_id, Some(duplicant));
    assert_eq!(original.targets.len(), 1);
    assert_eq!(original.targets[0].object_id, target);
    assert_eq!(
        original.targets[0].zone_change_generation,
        Some(target_generation)
    );

    engine
        .apply_scenario_command(0, &concede())
        .expect("the original optional-trigger controller concedes");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the begun targeted ability survives its controller's departure");
    let selector = pending.deciding_player;
    assert_ne!(selector, 0);
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("the targeted ability keeps its original continuation");
    };
    assert_eq!(stack.item.id, original.id);
    assert_eq!(stack.item.controller, original.controller);
    assert_eq!(stack.item.source_permanent_id, original.source_permanent_id);
    assert_eq!(stack.item.source_zone_change, original.source_zone_change);
    assert_eq!(stack.item.targets[0].object_id, target);
    assert_eq!(
        stack.item.targets[0].zone_change_generation,
        Some(target_generation)
    );

    engine
        .apply_scenario_command(selector, &select_branch(0))
        .expect("a survivor selects the delegate for the targeted ability");
    let delegate = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the delegated Apply/Decline stage remains on the same trigger")
        .deciding_player;
    let applied = engine
        .apply_scenario_command(
            delegate,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("the delegate applies Duplicant's original target effect");

    assert_eq!(stack_resolved_count(&applied, original.id), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(
        engine.state.zone_change_generation[&target],
        target_generation + 1,
        "only the originally targeted object incarnation is exiled"
    );
    assert_eq!(
        engine.state.objects[&other_creature].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&other_creature)
            .copied()
            .unwrap_or(0),
        other_generation
    );
}

#[test]
fn duplicant_departed_target_does_not_rebind_during_delegated_resolution() {
    let mut engine = four_player_engine();
    let departed_target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let surviving_same_name = inject_creature_on_battlefield(&mut engine, 3, "hill_giant");
    let target_generation = engine
        .state
        .zone_change_generation
        .get(&departed_target)
        .copied()
        .unwrap_or(0);
    let surviving_generation = engine
        .state
        .zone_change_generation
        .get(&surviving_same_name)
        .copied()
        .unwrap_or(0);
    let duplicant = inject_card_into_hand(&mut engine, 0, "duplicant");
    let slot = hand_index_for_card(&engine, 0, "duplicant");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Duplicant");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(Some(departed_target)))
        .expect("choose the exact Hill Giant owned by P1");
    pass_priority_round(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Duplicant begins its targeted optional ability");
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("the begun ability uses the optional-trigger continuation");
    };
    let original = stack.item.clone();
    assert_eq!(original.source_permanent_id, Some(duplicant));
    assert_eq!(original.targets.len(), 1);
    assert_eq!(original.targets[0].object_id, departed_target);
    assert_eq!(
        original.targets[0].zone_change_generation,
        Some(target_generation)
    );

    engine
        .apply_scenario_command(1, &concede())
        .expect("the targeted creature's owner leaves during the begun choice");
    assert!(engine.state.players[1].has_lost);
    assert!(engine
        .state
        .objects
        .get(&departed_target)
        .is_none_or(|object| object.zone != Zone::Battlefield));
    let still_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the live controller keeps the same started ability");
    assert_eq!(still_pending.deciding_player, 0);
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } =
        &still_pending.continuation
    else {
        panic!("the first departure does not replace the targeted continuation");
    };
    assert_eq!(stack.item.id, original.id);
    assert_eq!(stack.item.targets[0].object_id, departed_target);
    assert_eq!(
        stack.item.targets[0].zone_change_generation,
        Some(target_generation)
    );

    let delegation_offer = engine
        .apply_scenario_command(0, &concede())
        .expect("the original controller leaves after target cleanup");
    let selector = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P2 selects the delegate for the same root")
        .deciding_player;
    assert_eq!(selector, 2);
    let _delegate_choice = find_resolution_choice(&delegation_offer)
        .expect("P2 receives the actual delegate-selection offer");
    engine
        .apply_scenario_command(selector, &select_branch(1))
        .expect("P2 selects P3 as delegate for the same root");
    let delegate = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P3 receives the original targeted ability")
        .deciding_player;
    assert_eq!(delegate, 3);
    let applied = engine
        .apply_scenario_command(
            delegate,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("the delegate applies the original target effect");

    assert_eq!(stack_resolved_count(&applied, original.id), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine
        .state
        .objects
        .get(&departed_target)
        .is_none_or(|object| object.zone != Zone::Battlefield));
    assert_eq!(
        engine.state.objects[&surviving_same_name].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&surviving_same_name)
            .copied()
            .unwrap_or(0),
        surviving_generation,
        "the effect never substitutes a surviving same-name incarnation"
    );
}

#[test]
fn duplicant_new_incarnation_cannot_read_an_older_etb_pair() {
    let mut engine = engine();
    let target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let duplicant = inject_card_into_hand(&mut engine, 0, "duplicant");
    let slot = hand_index_for_card(&engine, 0, "duplicant");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Duplicant");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(Some(target)))
        .expect("choose Hill Giant as the ETB target");
    let triggering_generation = engine.state.zone_change_generation[&duplicant];

    // The trigger retains the first incarnation's pair identity while its source is away.
    move_owned_card(&mut engine, 0, 0, "Duplicant", DevZone::Hand);
    pass_both_players(&mut engine);
    if engine.state.pending_resolution.is_some() {
        answer_optional_triggered_ability_choice(
            &mut engine,
            ResolutionChoiceDecision::SelectBranch,
        );
    }
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&duplicant].zone, Zone::Hand);

    move_owned_card(&mut engine, 0, 0, "Duplicant", DevZone::Battlefield);
    let returned_generation = engine.state.zone_change_generation[&duplicant];
    assert!(returned_generation > triggering_generation);
    let characteristics = engine.characteristics(duplicant).expect("new Duplicant");
    assert!(!characteristics.has_type("Giant"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(2), Some(4)),
        "the new incarnation cannot consume the earlier trigger's linked exile record"
    );
}

#[test]
fn copied_duplicant_gets_its_own_linked_exile_pair() {
    let mut engine = engine();
    let first_target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let first_duplicant = cast_duplicant_and_resolve_imprint(&mut engine, Some(first_target));
    let second_target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    let clone = inject_card_into_hand(&mut engine, 0, "clone");
    let clone_slot = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_scenario_command(0, &cast_spell(clone_slot, Vec::new()))
        .expect("cast Clone");
    pass_both_players(&mut engine);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Clone copy-source choice")
        .presentation
        .candidates
        .contains(&first_duplicant));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![first_duplicant]))
        .expect("enter as a copy of Duplicant");
    engine
        .apply_scenario_command(0, &choose_trigger_target(Some(second_target)))
        .expect("choose the second creature for the copied trigger");
    pass_both_players(&mut engine);
    answer_optional_triggered_ability_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);

    assert_eq!(engine.state.objects[&first_target].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&second_target].zone, Zone::Exile);
    let original = engine
        .characteristics(first_duplicant)
        .expect("original Duplicant");
    let copied = engine.characteristics(clone).expect("copied Duplicant");
    assert!(original.has_type("Giant"));
    assert_eq!((original.power, original.toughness), (Some(3), Some(3)));
    assert!(copied.has_type("Bear"));
    assert!(copied.has_type("Shapeshifter"));
    assert_eq!((copied.power, copied.toughness), (Some(2), Some(2)));
}

#[test]
fn duplicant_layer_four_types_feed_the_exiled_creatures_layer_seven_cda() {
    let mut engine = engine_with_giant_count_cda();
    inject_creature_on_battlefield(&mut engine, 0, "vanilla_giant");
    let target = inject_creature_on_battlefield(&mut engine, 0, "giant_counter");
    assert_eq!(engine.characteristics(target).unwrap().power, Some(1));

    let duplicant = cast_duplicant_and_resolve_imprint(&mut engine, Some(target));

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    let characteristics = engine.characteristics(duplicant).expect("Duplicant");
    assert!(characteristics.has_type("Giant"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(2), Some(2)),
        "the exiled Giant CDA counts Duplicant after Duplicant gains Giant in layer 4"
    );
}

#[test]
fn duplicant_later_layer_four_type_setting_follows_timestamp_order() {
    let mut engine = engine();
    let target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let duplicant = cast_duplicant_and_resolve_imprint(&mut engine, Some(target));

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(duplicant),
        kind: ContinuousEffectKind::Layer4SetCreatureTypes(vec!["Frog".to_string()]),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index + 1,
    });

    let characteristics = engine.characteristics(duplicant).expect("Duplicant");
    assert!(characteristics.has_type("Frog"));
    assert!(!characteristics.has_type("Giant"));
    assert!(!characteristics.has_type("Shapeshifter"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(3), Some(3))
    );
}

#[test]
fn duplicant_later_all_creature_types_setting_updates_public_representation() {
    let mut engine = engine();
    let target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let duplicant = cast_duplicant_and_resolve_imprint(&mut engine, Some(target));

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(duplicant),
        kind: ContinuousEffectKind::Layer4SetAllCreatureTypes,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index + 1,
    });

    let characteristics = engine.characteristics(duplicant).expect("Duplicant");
    assert!(characteristics.all_creature_types);
    assert!(characteristics.has_type("Dragon"));
    let view = engine.initial_response_batch();
    let public_duplicant = view
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("updated zone view")
        .per_player
        .iter()
        .flat_map(|player| &player.battlefield_objects)
        .find(|object| object.object_id == duplicant)
        .expect("public Duplicant view");
    assert!(public_duplicant
        .rules_annotation_labels
        .contains(&"Creature types: All creature types".to_string()));
}

#[test]
fn duplicant_layer_seven_setting_continues_after_layer_six_removes_abilities() {
    let mut engine = engine();
    let target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let duplicant = cast_duplicant_and_resolve_imprint(&mut engine, Some(target));

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(duplicant),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index + 1,
    });

    let characteristics = engine.characteristics(duplicant).expect("Duplicant");
    assert!(characteristics.has_type("Giant"));
    assert!(characteristics.has_type("Shapeshifter"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(3), Some(3)),
        "the layer-7b part continues after the ability-removal effect in layer 6"
    );
}

#[test]
fn duplicant_refreshes_linked_hand_count_and_public_view_snapshot() {
    let mut engine = engine();
    engine.enable_dev_commands();
    let target = inject_creature_on_battlefield(&mut engine, 0, "psychosis_crawler");
    let duplicant = cast_duplicant_and_resolve_imprint(&mut engine, Some(target));
    let before_hand_count = engine.state.players[0].hand.len();
    let before = engine
        .characteristics(duplicant)
        .and_then(|characteristics| characteristics.power)
        .expect("Duplicant's imprinted power");
    assert_eq!(before as usize, before_hand_count);

    // Prime the serialized battlefield cache, then change only the exiled CDA's owner hand.
    engine.initial_response_batch();
    inject_library_card(&mut engine, 0, "island");
    engine.initial_response_batch();
    let batch = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: "Island".to_string(),
                        zone: DevZone::Hand as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the exact island from library to hand");

    let public_view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("hand-count change publishes a zone view");
    assert!(!public_view.battlefields_unchanged);
    let public_duplicant = public_view
        .per_player
        .iter()
        .flat_map(|player| &player.battlefield_objects)
        .find(|object| object.object_id == duplicant)
        .expect("updated public battlefield includes Duplicant");
    assert_eq!(
        (public_duplicant.power, public_duplicant.toughness),
        (before + 1, before + 1)
    );
    assert!(public_duplicant
        .rules_annotation_labels
        .contains(&"Creature types: Phyrexian, Horror, Shapeshifter".to_string()));
}
