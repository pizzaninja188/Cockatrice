//! Hideaway 4 identity, source-linked privacy, land play, and cast-and-return rules.
use crate::helpers::ScenarioCommandExt;
use prost::Message;

use super::helpers::*;
use tricerules_cards::registry;
use tricerules_cards::{
    primitives::*, AbilityDefinitionId, AbilityId, CardFaceId, Keyword, Layout, ProgramOwnerId,
};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CastSource, CostObjectRef, CostObjectRefs,
    CostSelection, LandSource, PaymentMana, PaymentSelection, ResolutionChoiceDecision,
};

const MOSSWORT: &str = "mosswort_bridge";
const BUGGY: &str = "smugglers_buggy";

fn game(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[MOSSWORT, BUGGY, "opt", "grizzly_bears"]);
    let mut engine = GameEngine::new(
        registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_hideaway_look(engine: &mut GameEngine, chooser: i32, chosen_card: u32) {
    answer_trigger_order_in_engine_order(engine);
    pass_priority_round(engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Hideaway offers a private top-card choice");
    assert_eq!(pending.deciding_player, chooser);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryLook);
    assert_eq!(pending.presentation.min, 1);
    assert_eq!(pending.presentation.max, 1);
    assert!(pending.presentation.candidates.contains(&chosen_card));
    engine
        .apply_scenario_command(chooser, &submit_resolution_choice(vec![chosen_card]))
        .expect("exile the selected Hideaway card face down");
    assert_eq!(engine.state.objects[&chosen_card].zone, Zone::Exile);
    assert!(engine.state.objects[&chosen_card].face_down);
}

fn put_hideaway_card_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    engine.state.players[player].library.push_front(object);
    object
}

fn one_creature_cost(object_id: u32, generation: u64) -> CostSelection {
    CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id,
                zone_change_generation: generation,
            }],
        })),
    }
}

fn submit_resolution_choice_at(prompt_id: u64, chosen: Vec<u32>) -> RuledCommand {
    let mut command = submit_resolution_choice(chosen);
    let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = prompt_id;
    command
}

fn submit_resolution_decision_at(
    prompt_id: u64,
    decision: ResolutionChoiceDecision,
) -> RuledCommand {
    let mut command = submit_resolution_decision(decision);
    let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = prompt_id;
    command
}

fn linked_exile_cast_announcement(
    engine: &mut GameEngine,
    player: i32,
    object_id: u32,
    target_object_id: u32,
) -> tricerules_proto::ruled::v1::SpellCastAnnouncement {
    let generation = engine.state.zone_change_generation[&object_id];
    let action = engine.initial_response_batch().legal_by_player[&player]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == object_id)
        .cloned()
        .expect("the current linked-play offer publishes this exact exile spell");
    tricerules_proto::ruled::v1::SpellCastAnnouncement {
        targets: target_object(target_object_id),
        face_index: action.face_index,
        source: Some(exile_cast_source(object_id, generation)),
        cast_method: action.cast_method,
        casting_permission_id: action.casting_permission_id,
        ..Default::default()
    }
}

fn submit_resolution_cast_at(
    prompt_id: u64,
    announcement: tricerules_proto::ruled::v1::SpellCastAnnouncement,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(
            tricerules_proto::ruled::v1::SubmitResolutionChoice {
                resolution_prompt_id: prompt_id,
                decision: ResolutionChoiceDecision::CastSpell as i32,
                spell_cast_announcement: Some(announcement),
                ..Default::default()
            },
        )),
    }
}

fn rejects_without_state_change(engine: &mut GameEngine, player: i32, command: &RuledCommand) {
    let state_before = engine
        .diagnostic_snapshot()
        .expect("pre-rejection snapshot");
    let seed_before = engine.state.seed;
    let command_index_before = engine.state.command_index;
    assert!(engine.apply_scenario_command(player, command).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("post-rejection snapshot"),
        state_before
    );
    assert_eq!(engine.state.seed, seed_before);
    assert_eq!(engine.state.command_index, command_index_before);
}

fn assert_linked_play_root_retired_once_without_moving_source(
    events: &[tricerules_proto::ruled::v1::RuledEvent],
    stack_object_id: u32,
    source_object_id: u32,
) {
    assert_ne!(stack_object_id, source_object_id);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackResolved(resolved))
                    if resolved.object_id == stack_object_id
            ))
            .count(),
        1,
        "the linked-play completion retires its exact root stack item once"
    );
    assert!(!events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::PermanentMoved(moved))
            if moved.object_id == source_object_id
    )));
    assert!(!events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(revealed))
            if revealed.cards.iter().any(|card| card.object_id == source_object_id)
    )));
}

fn apply_serialized_accepted(
    engine: &mut GameEngine,
    actor: i32,
    command: RuledCommand,
    accepted: &mut Vec<(i32, Vec<u8>, Vec<u8>)>,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let command_bytes = command.encode_to_vec();
    let decoded = RuledCommand::decode(command_bytes.as_slice())
        .expect("serialize and decode the accepted RuledCommand");
    let batch = engine
        .apply_scenario_command(actor, &decoded)
        .expect("apply the accepted Mosswort command");
    accepted.push((actor, command_bytes, batch.encode_to_vec()));
    batch
}

#[test]
fn mosswort_bridge_and_smugglers_buggy_register_their_exact_rules_shapes() {
    let bridge = registry::global()
        .get(MOSSWORT)
        .expect("Mosswort Bridge definition");
    assert_eq!(bridge.name, "Mosswort Bridge");
    assert_eq!(bridge.layout, Layout::Normal);
    let bridge_face = bridge.primary_face();
    assert_eq!(bridge_face.types, ["Land"]);
    assert!(matches!(
        bridge_face.static_abilities[0].definition,
        StaticAbilityDef::EntersTapped { .. }
    ));
    assert!(matches!(
        bridge_face.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::Hideaway {
            count: 4,
            ref linked_exile_id,
        }] if linked_exile_id.as_str() == "exiled_cards"
    ));
    assert!(matches!(
        bridge_face.activated_abilities[0].effect.as_slice(),
        [SpellEffectKind::ProduceMana { .. }]
    ));
    let bridge_activated = &bridge_face.activated_abilities[1];
    assert!(bridge_activated.effect.is_empty());
    let bridge_program = bridge_activated
        .program
        .as_ref()
        .expect("Mosswort Bridge's activated_02 is an authored Program");
    assert!(bridge_program.target_bindings.is_empty());
    assert!(bridge_program.body.yields.is_empty());
    let [choose] = bridge_program.body.steps.as_slice() else {
        panic!("Mosswort's Program is one terminal optional Choose");
    };
    assert!(choose.bind.is_empty() && choose.input.is_none());
    let InstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        panic!("Mosswort's Program root is a Choose");
    };
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("Mosswort's optional Choose has one branch");
    };
    assert_eq!(branch.cost, ResolutionCost::None);
    assert!(matches!(
        &branch.requirement,
        ResolutionBranchRequirement::GameCondition(GameCondition::BattlefieldAggregate {
            filter,
            aggregate: BattlefieldAggregate::TotalPower,
            min: Some(10),
            max: None,
            exclude_observed_object: false,
        }) if filter.controllers == RelativePlayerSet::Controller
            && filter.card_type == Some(CardTypeFilter::Creature)
            && filter.required_supertypes.is_empty()
            && filter.token.is_none()
            && filter.any_of.is_none()
            && filter.color.is_none()
            && filter.name.is_none()
            && filter.required_subtypes.is_empty()
            && !filter.exclude_source
    ));
    assert!(branch.body.yields.is_empty());
    let [play] = branch.body.steps.as_slice() else {
        panic!("the qualifying branch ends in one linked-Play operation");
    };
    assert!(play.bind.is_empty() && play.input.is_none());
    assert!(matches!(
        &play.kind,
        InstructionKind::Perform(effect)
            if matches!(
                effect.as_ref(),
                SpellEffectKind::PlayLinkedExiledCard {
                    producer_ability_id,
                    linked_exile_id,
                    filter: None,
                    play_kind: LinkedExiledCardPlayKind::Play,
                    return_source_to_owner_hand_if_cast: false,
                } if producer_ability_id.as_str() == "hideaway"
                    && linked_exile_id.as_str() == "exiled_cards"
            )
    ));
    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: MOSSWORT.into(),
        face_id: CardFaceId::new(MOSSWORT).unwrap(),
        ability_path: vec![AbilityId::new("activated_02").unwrap()],
    });
    assert!(registry::global()
        .compiled_program_for_owner(&owner)
        .is_some());

    let buggy = registry::global()
        .get(BUGGY)
        .expect("Smuggler's Buggy definition");
    assert_eq!(buggy.name, "Smuggler's Buggy");
    assert_eq!(buggy.layout, Layout::Normal);
    let buggy_face = buggy.primary_face();
    assert_eq!(buggy_face.mana_cost.to_string(), "{4}");
    assert_eq!(buggy_face.types, ["Artifact", "Vehicle"]);
    assert_eq!((buggy_face.power, buggy_face.toughness), (Some(5), Some(5)));
    assert_eq!(buggy_face.triggered_abilities.len(), 2);
    assert_eq!(buggy_face.keywords, Vec::<Keyword>::new());
    assert!(matches!(
        buggy_face.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::Hideaway { count: 4, .. }]
    ));
    assert!(matches!(
        buggy_face.triggered_abilities[1].effect.as_slice(),
        [SpellEffectKind::PlayLinkedExiledCard {
            play_kind: LinkedExiledCardPlayKind::Cast,
            return_source_to_owner_hand_if_cast: true,
            ..
        }]
    ));
    assert!(matches!(
        buggy_face.activated_abilities[0].costs.as_slice(),
        [AbilityCost::TapPermanents {
            constraint: ObjectPaymentConstraint::AggregateMinimum {
                minimum: 2,
                contribution: ObjectContributionKind::CurrentPower,
            },
            ..
        }]
    ));
}

#[test]
fn mosswort_bridge_checks_power_when_its_ability_resolves_then_plays_the_linked_land() {
    let mut engine = game(20_261_011);
    let bridge = inject_card_into_hand(&mut engine, 0, MOSSWORT);
    let entered = move_ready_to_battlefield(&mut engine, 0, MOSSWORT);
    assert_eq!(entered, bridge);
    assert!(engine.state.objects[&bridge].tapped);

    let linked_land = put_hideaway_card_on_top(&mut engine, 0, "mountain");
    resolve_hideaway_look(&mut engine, 0, linked_land);
    assert_eq!(engine.state.objects[&linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_land].face_down);

    // Set up eight power when activating. The threshold is evaluated as the ability resolves,
    // so a fifth Bear entering while it is on the stack makes the linked play available.
    engine.state.objects.get_mut(&bridge).unwrap().tapped = false;
    let initial_bears = (0..4)
        .map(|_| inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears"))
        .collect::<Vec<_>>();
    assert_eq!(
        initial_bears
            .iter()
            .map(|object| engine.characteristics(*object).unwrap().power.unwrap() as i64)
            .sum::<i64>(),
        8
    );
    engine.state.players[0].mana_pool.green = 1;
    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, bridge, 1, Vec::new()))
        .expect("activate Mosswort Bridge with one green and tap it");
    let fifth_bear = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(
        initial_bears
            .iter()
            .copied()
            .chain([fifth_bear])
            .map(|object| engine.characteristics(object).unwrap().power.unwrap() as i64)
            .sum::<i64>(),
        10
    );

    // Replay starts from the same seeded state with the activation and resolution-time power
    // change already established. The accepted stream below carries the exact branch, linked
    // selection and PlayLand command bytes plus their event batches.
    let mut replay = engine.clone();
    let mut accepted = Vec::new();

    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        apply_serialized_accepted(&mut engine, actor, pass(), &mut accepted);
    }
    let threshold_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Mosswort's qualifying branch asks whether to play the linked card");
    assert_eq!(
        threshold_choice.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    let ability_stack_item_id = match &threshold_choice.continuation {
        tricerules_core::state::ResolutionContinuation::ProgramChoose { stack, .. } => {
            assert_eq!(stack.item.source_permanent_id, Some(bridge));
            stack.item.id
        }
        _ => panic!("the qualifying branch retains the exact Mosswort Program stack item"),
    };
    let branch_prompt_id = threshold_choice.resolution_prompt_id.get();
    assert_ne!(
        branch_prompt_id, 0,
        "resolution prompts are explicitly identified"
    );
    let stale_branch =
        submit_resolution_decision_at(branch_prompt_id + 1, ResolutionChoiceDecision::SelectBranch);
    rejects_without_state_change(&mut engine, 0, &stale_branch);
    apply_serialized_accepted(
        &mut engine,
        0,
        submit_resolution_decision_at(branch_prompt_id, ResolutionChoiceDecision::SelectBranch),
        &mut accepted,
    );

    let linked_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("choose the exact linked Hideaway card");
    assert_eq!(
        linked_choice.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert_eq!(linked_choice.presentation.candidates, [linked_land]);
    let linked_prompt_id = linked_choice.resolution_prompt_id.get();
    assert_ne!(linked_prompt_id, 0);
    rejects_without_state_change(
        &mut engine,
        0,
        &submit_resolution_choice_at(linked_prompt_id + 1, vec![linked_land]),
    );
    apply_serialized_accepted(
        &mut engine,
        0,
        submit_resolution_choice_at(linked_prompt_id, vec![linked_land]),
        &mut accepted,
    );
    let exile_generation = engine.state.zone_change_generation[&linked_land];
    let special_cast_prompt_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("linked Play offers its live SpecialCast decision")
        .resolution_prompt_id
        .get();
    assert_ne!(special_cast_prompt_id, 0);
    let land_action = RuledCommand {
        cmd: Some(Cmd::PlayLand(tricerules_proto::ruled::v1::PlayLand {
            resolution_prompt_id: special_cast_prompt_id,
            source: Some(LandSource {
                location: Some(
                    tricerules_proto::ruled::v1::land_source::Location::ExileObjectId(linked_land),
                ),
                expected_zone_change_generation: Some(exile_generation),
            }),
            face_index: 0,
        })),
    };
    let mut stale_prompt_action = land_action.clone();
    let Some(Cmd::PlayLand(stale_prompt)) = stale_prompt_action.cmd.as_mut() else {
        unreachable!()
    };
    stale_prompt.resolution_prompt_id = special_cast_prompt_id + 1;
    rejects_without_state_change(&mut engine, 0, &stale_prompt_action);
    let mut stale_generation_action = land_action.clone();
    let Some(Cmd::PlayLand(stale_generation)) = stale_generation_action.cmd.as_mut() else {
        unreachable!()
    };
    stale_generation
        .source
        .as_mut()
        .unwrap()
        .expected_zone_change_generation = Some(exile_generation + 1);
    rejects_without_state_change(&mut engine, 0, &stale_generation_action);
    assert_eq!(engine.state.objects[&linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_land].face_down);

    let land_batch = apply_serialized_accepted(&mut engine, 0, land_action.clone(), &mut accepted);

    assert_eq!(engine.state.objects[&linked_land].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&linked_land].controller, 0);
    assert!(
        !engine.state.objects[&linked_land].face_down,
        "CR 406.3a turns the face-down exiled card face up before it is played"
    );
    let characteristics = engine.characteristics(linked_land).unwrap();
    assert!(characteristics.has_type("Land"));
    assert!(!characteristics.is_creature());
    let reveal_index = land_batch
        .events
        .iter()
        .position(|event| matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
                if reveal.source_zone == tricerules_proto::ruled::v1::ChoiceCandidateSourceZone::Exile as i32
                    && reveal.cards.iter().any(|card| card.object_id == linked_land && card.card_id == "mountain")
        ))
        .expect("the played card is publicly revealed from exile");
    let moved_index = land_batch
        .events
        .iter()
        .position(|event| matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::PermanentMoved(moved))
                if moved.object_id == linked_land
                    && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield as i32
                    && moved.card_id == "mountain"
                    && !moved.face_down
        ))
        .expect("the battlefield event identifies the face-up Mountain");
    assert!(
        reveal_index < moved_index,
        "reveal precedes battlefield entry"
    );
    assert_eq!(engine.state.lands_played_this_turn, 1);
    assert!(engine.state.pending_resolution.is_none());
    let completed = engine.diagnostic_snapshot().unwrap();
    assert!(completed["state"]["pending_linked_exile_play"].is_null());
    assert_eq!(completed["state"]["stack"], serde_json::json!([]));
    assert_linked_play_root_retired_once_without_moving_source(
        &land_batch.events,
        ability_stack_item_id,
        bridge,
    );
    assert_eq!(engine.state.objects[&bridge].zone, Zone::Battlefield);
    rejects_without_state_change(&mut engine, 0, &land_action);

    for (actor, command_bytes, expected_batch) in accepted {
        let command = RuledCommand::decode(command_bytes.as_slice())
            .expect("decode the saved accepted Mosswort command");
        let replayed = replay
            .apply_scenario_command(actor, &command)
            .expect("replay a serialized accepted Mosswort command");
        assert_eq!(
            replayed.encode_to_vec(),
            expected_batch,
            "the same-seed accepted command emits the same serialized events"
        );
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap(),
        "the accepted branch, linked selection and PlayLand stream replays to identical state"
    );
}

#[test]
fn mosswort_linked_cast_survives_source_bounce_and_cancelled_proposal() {
    let mut engine = game(20_261_017);
    let bridge = move_ready_to_battlefield(&mut engine, 0, MOSSWORT);
    let linked_spell = put_hideaway_card_on_top(&mut engine, 0, "lightning_bolt");
    resolve_hideaway_look(&mut engine, 0, linked_spell);
    let bridge_generation = engine.state.zone_change_generation[&bridge];

    engine.state.objects.get_mut(&bridge).unwrap().tapped = false;
    let target_bear = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    for _ in 1..5 {
        inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    }
    engine.state.players[0].mana_pool.green = 1;
    inject_card_into_hand(&mut engine, 1, "boomerang");
    engine.state.players[1].mana_pool.blue = 2;

    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, bridge, 1, Vec::new()))
        .expect("activate Mosswort Bridge");
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass priority to the opponent after the activation");
    assert_eq!(engine.state.priority_player_id(), 1);
    let activated = engine
        .state
        .stack
        .iter()
        .find(|item| item.source_permanent_id == Some(bridge))
        .expect("the activation retains its exact source incarnation");
    assert_eq!(activated.source_zone_change, bridge_generation);

    let boomerang = hand_index_for_card(&engine, 1, "boomerang");
    engine
        .apply_scenario_command(1, &cast_spell(boomerang, target_object(bridge)))
        .expect("cast the registered Boomerang at Mosswort Bridge");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&bridge].zone, Zone::Hand);
    assert_eq!(
        engine.state.zone_change_generation[&bridge],
        bridge_generation + 1
    );
    let activated = engine
        .state
        .stack
        .iter()
        .find(|item| item.source_permanent_id == Some(bridge))
        .expect("the original activation survives the source's bounce");
    assert_eq!(activated.source_zone_change, bridge_generation);

    pass_priority_round(&mut engine);
    let branch = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the activated ability resolves using its captured source")
        .clone();
    assert_eq!(branch.deciding_player, 0);
    assert_eq!(
        branch.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    let branch_prompt_id = branch.resolution_prompt_id.get();
    assert_ne!(branch_prompt_id, 0);
    let tricerules_core::state::ResolutionContinuation::ProgramChoose { stack, .. } =
        &branch.continuation
    else {
        panic!("the qualifying branch is the migrated Mosswort Program");
    };
    assert_eq!(stack.item.source_permanent_id, Some(bridge));
    assert_eq!(stack.item.source_zone_change, bridge_generation);
    let ability_stack_item_id = stack.item.id;
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision_at(
                branch_prompt_id,
                ResolutionChoiceDecision::SelectBranch,
            ),
        )
        .expect("choose the optional linked-play branch after the source left");

    let linked_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("choose the exact Hideaway card")
        .clone();
    assert_eq!(linked_choice.presentation.candidates, [linked_spell]);
    let linked_prompt_id = linked_choice.resolution_prompt_id.get();
    assert_ne!(linked_prompt_id, 0);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_choice_at(linked_prompt_id, vec![linked_spell]),
        )
        .expect("select the linked face-down instant");

    let special = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the linked instant is offered for a free cast")
        .clone();
    assert_eq!(special.presentation.choice_kind, ChoiceKind::SpecialCast);
    let first_special_prompt_id = special.resolution_prompt_id.get();
    assert_ne!(first_special_prompt_id, 0);
    let tricerules_core::state::ResolutionContinuation::SpecialCast { stack, .. } =
        &special.continuation
    else {
        panic!("the linked cast offer retains the resolving Program");
    };
    assert_eq!(stack.item.source_permanent_id, Some(bridge));
    assert_eq!(stack.item.source_zone_change, bridge_generation);
    let exile_generation = engine.state.zone_change_generation[&linked_spell];
    let pending_link_snapshot = engine.diagnostic_snapshot().unwrap();
    let pending_linked_play = pending_link_snapshot["state"]["pending_linked_exile_play"].clone();
    assert!(!pending_linked_play.is_null());
    let linked_records = pending_link_snapshot["state"]["linked_exile_records"].clone();

    let announcement = linked_exile_cast_announcement(&mut engine, 0, linked_spell, target_bear);
    let begun = engine
        .apply_scenario_command(
            0,
            &submit_resolution_cast_at(first_special_prompt_id, announcement),
        )
        .expect("begin the exact linked spell proposal");
    assert_eq!(engine.state.objects[&linked_spell].zone, Zone::Stack);
    assert!(
        !engine.state.objects[&linked_spell].face_down,
        "the accepted linked-cast announcement turns its face-down Exile source face up"
    );
    assert_eq!(
        begun
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
                    if reveal.cards.iter().any(|card| card.object_id == linked_spell && card.card_id == "lightning_bolt")
            ))
            .count(),
        1,
        "the accepted linked-cast announcement publishes one identity receipt"
    );
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the linked spell awaits an explicit payment commit")
        .transaction_id;
    assert_eq!(engine.state.objects[&linked_spell].id, linked_spell);

    let cancelled = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(
                    tricerules_proto::ruled::v1::CancelSpellCast { transaction_id },
                )),
            },
        )
        .expect("cancel the uncommitted linked-cast proposal");
    assert!(!cancelled.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == linked_spell)
    )));
    assert_eq!(engine.state.objects[&linked_spell].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_spell].face_down);
    assert_eq!(
        engine.state.zone_change_generation[&linked_spell],
        exile_generation
    );
    assert_eq!(engine.state.objects[&bridge].zone, Zone::Hand);
    assert_eq!(
        engine.state.zone_change_generation[&bridge],
        bridge_generation + 1
    );
    let after_cancel = engine.diagnostic_snapshot().unwrap();
    assert_eq!(
        after_cancel["state"]["pending_linked_exile_play"], pending_linked_play,
        "canceling a spell proposal leaves its linked-play occurrence pending"
    );
    assert_eq!(
        after_cancel["state"]["linked_exile_records"],
        linked_records
    );
    assert_eq!(
        cancelled
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackResolved(resolved))
                    if resolved.object_id == ability_stack_item_id
            ))
            .count(),
        0,
        "canceling a cast proposal must leave the parked Mosswort ability on the client stack"
    );
    let reoffered = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("cancellation republishes the live SpecialCast offer")
        .clone();
    assert_eq!(reoffered.presentation.choice_kind, ChoiceKind::SpecialCast);
    let tricerules_core::state::ResolutionContinuation::SpecialCast {
        stack: reoffered_stack,
        ..
    } = &reoffered.continuation
    else {
        panic!("cancellation republishes the linked SpecialCast continuation");
    };
    assert_eq!(reoffered_stack.item.id, ability_stack_item_id);
    let reoffered_prompt_id = reoffered.resolution_prompt_id.get();
    assert_ne!(reoffered_prompt_id, 0);
    let stale_prompt_id = if reoffered_prompt_id == first_special_prompt_id {
        reoffered_prompt_id + 1
    } else {
        first_special_prompt_id
    };
    let retry_announcement =
        linked_exile_cast_announcement(&mut engine, 0, linked_spell, target_bear);
    rejects_without_state_change(
        &mut engine,
        0,
        &submit_resolution_cast_at(stale_prompt_id, retry_announcement.clone()),
    );
    let retry_begun = engine
        .apply_scenario_command(
            0,
            &submit_resolution_cast_at(reoffered_prompt_id, retry_announcement),
        )
        .expect("retry the still-live linked cast offer");
    assert_eq!(
        retry_begun
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
                    if reveal.cards.iter().any(|card| card.object_id == linked_spell && card.card_id == "lightning_bolt")
            ))
            .count(),
        1,
        "reoffering the same live linked play publishes one fresh announcement receipt"
    );
    let retry_transaction = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("retry begins a fresh cast proposal")
        .transaction_id;
    let committed = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(
                    tricerules_proto::ruled::v1::CommitSpellCast {
                        transaction_id: retry_transaction,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("commit the free cast from the linked exile play");

    assert_eq!(engine.state.objects[&linked_spell].zone, Zone::Stack);
    assert!(!engine.state.objects[&linked_spell].face_down);
    let spell = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == linked_spell)
        .expect("Lightning Bolt is a normal spell on the stack");
    assert_eq!(spell.cast_by, Some(0));
    assert!(spell.ability_text.is_none());
    assert_eq!(spell.targets.len(), 1);
    assert_eq!(spell.targets[0].object_id, target_bear);
    assert_eq!(spell.mana_colors_spent_to_cast.count(), 0);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert!(
        !committed.events.iter().any(|event| matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
                if reveal.cards.iter().any(|card| card.object_id == linked_spell)
        )),
        "the committed retry does not duplicate the accepted announcement receipt"
    );
    assert!(
        committed.events.iter().any(|event| matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackPushed(stack))
                if stack.object_id == linked_spell && stack.card_id == "lightning_bolt"
        )),
        "the committed card is published as a normal spell"
    );
    assert!(engine.state.pending_resolution.is_none());
    let completed = engine.diagnostic_snapshot().unwrap();
    assert!(completed["state"]["pending_linked_exile_play"].is_null());
    assert_linked_play_root_retired_once_without_moving_source(
        &committed.events,
        ability_stack_item_id,
        bridge,
    );
    assert_eq!(engine.state.objects[&bridge].zone, Zone::Hand);
    assert_eq!(
        engine.state.zone_change_generation[&bridge],
        bridge_generation + 1
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&linked_spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&target_bear].zone, Zone::Graveyard);
}

#[test]
fn mosswort_bridge_does_not_offer_the_linked_card_below_ten_power() {
    let mut engine = game(20_261_013);
    let bridge = inject_card_into_hand(&mut engine, 0, MOSSWORT);
    move_ready_to_battlefield(&mut engine, 0, MOSSWORT);
    let linked_land = put_hideaway_card_on_top(&mut engine, 0, "forest");
    resolve_hideaway_look(&mut engine, 0, linked_land);

    engine.state.objects.get_mut(&bridge).unwrap().tapped = false;
    for _ in 0..4 {
        inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    }
    engine.state.players[0].mana_pool.green = 1;
    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, bridge, 1, Vec::new()))
        .expect("activate Mosswort Bridge below the power threshold");
    pass_priority_round(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_land].face_down);
    assert!(engine.state.objects[&bridge].tapped);
}

#[test]
fn mosswort_bridge_optional_branch_decline_keeps_the_linked_card_hidden() {
    let mut engine = game(20_261_015);
    let bridge = move_ready_to_battlefield(&mut engine, 0, MOSSWORT);
    let linked_land = put_hideaway_card_on_top(&mut engine, 0, "mountain");
    resolve_hideaway_look(&mut engine, 0, linked_land);
    assert!(engine.state.objects[&linked_land].face_down);

    engine.state.objects.get_mut(&bridge).unwrap().tapped = false;
    for _ in 0..5 {
        inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    }
    engine.state.players[0].mana_pool.green = 1;
    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, bridge, 1, Vec::new()))
        .expect("activate Mosswort Bridge with one green");
    pass_priority_round(&mut engine);
    let branch = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("qualifying ability offers its optional branch");
    assert_eq!(
        branch.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    let branch_prompt_id = branch.resolution_prompt_id.get();
    assert_ne!(branch_prompt_id, 0);

    let declined = engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision_at(branch_prompt_id, ResolutionChoiceDecision::Decline),
        )
        .expect("decline to play the linked card");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_land].face_down);
    assert!(!declined.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == linked_land)
    )));
}

#[test]
fn mosswort_linked_choice_decline_clears_occurrence_before_second_bridge_play() {
    let mut engine = game(20_261_021);
    let first_bridge = move_ready_to_battlefield(&mut engine, 0, MOSSWORT);
    let first_linked_land = put_hideaway_card_on_top(&mut engine, 0, "mountain");
    resolve_hideaway_look(&mut engine, 0, first_linked_land);

    let second_bridge_card = inject_card_into_hand(&mut engine, 0, MOSSWORT);
    let second_bridge = move_ready_to_battlefield(&mut engine, 0, MOSSWORT);
    assert_eq!(second_bridge, second_bridge_card);
    let second_linked_land = put_hideaway_card_on_top(&mut engine, 0, "forest");
    resolve_hideaway_look(&mut engine, 0, second_linked_land);
    assert_ne!(first_bridge, second_bridge);
    assert_ne!(first_linked_land, second_linked_land);

    for _ in 0..5 {
        inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    }
    engine.state.objects.get_mut(&first_bridge).unwrap().tapped = false;
    engine.state.objects.get_mut(&second_bridge).unwrap().tapped = false;
    engine.state.players[0].mana_pool.green = 2;

    let linked_records_before_decline =
        engine.diagnostic_snapshot().unwrap()["state"]["linked_exile_records"].clone();
    engine
        .apply_scenario_command(
            0,
            &activate_ability_for(&engine, first_bridge, 1, Vec::new()),
        )
        .expect("activate the first Bridge");
    pass_priority_round(&mut engine);
    let first_branch = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the first Bridge offers its qualifying optional branch");
    assert_eq!(
        first_branch.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    let first_ability_stack_item_id = match &first_branch.continuation {
        tricerules_core::state::ResolutionContinuation::ProgramChoose { stack, .. } => {
            assert_eq!(stack.item.source_permanent_id, Some(first_bridge));
            stack.item.id
        }
        _ => panic!("the first Bridge branch retains its exact Program stack item"),
    };
    let first_branch_prompt_id = first_branch.resolution_prompt_id.get();
    assert_ne!(first_branch_prompt_id, 0);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision_at(
                first_branch_prompt_id,
                ResolutionChoiceDecision::SelectBranch,
            ),
        )
        .expect("select the first Bridge's linked-play branch");

    let first_linked_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the first Bridge prompts for its linked card");
    assert_eq!(
        first_linked_choice.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert_eq!(
        first_linked_choice.presentation.candidates,
        [first_linked_land]
    );
    let first_linked_prompt_id = first_linked_choice.resolution_prompt_id.get();
    assert_ne!(first_linked_prompt_id, 0);
    let decline = engine
        .apply_scenario_command(
            0,
            &submit_resolution_choice_at(first_linked_prompt_id, Vec::new()),
        )
        .expect("decline the direct linked-card choice");
    assert!(decline.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
            if log.text == "P0 declines the linked Exile choice."
    )));

    let after_decline = engine.diagnostic_snapshot().unwrap();
    assert!(
        after_decline["state"]["pending_resolution"].is_null(),
        "declining the linked-card choice completes the prompt"
    );
    assert!(
        after_decline["state"]["pending_linked_exile_play"].is_null(),
        "declining the linked-card choice clears its auxiliary play occurrence"
    );
    assert_eq!(after_decline["state"]["stack"], serde_json::json!([]));
    assert_eq!(
        after_decline["state"]["linked_exile_records"], linked_records_before_decline,
        "decline leaves both persistent Bridge links intact"
    );
    assert_eq!(engine.state.objects[&first_linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&first_linked_land].face_down);
    assert_eq!(engine.state.objects[&second_linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&second_linked_land].face_down);
    assert_linked_play_root_retired_once_without_moving_source(
        &decline.events,
        first_ability_stack_item_id,
        first_bridge,
    );
    assert_eq!(engine.state.objects[&first_bridge].zone, Zone::Battlefield);

    engine
        .apply_scenario_command(
            0,
            &activate_ability_for(&engine, second_bridge, 1, Vec::new()),
        )
        .expect("activate the independent second Bridge");
    pass_priority_round(&mut engine);
    let second_branch = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the second Bridge resolves its qualifying branch");
    assert_eq!(
        second_branch.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    let second_branch_prompt_id = second_branch.resolution_prompt_id.get();
    assert_ne!(second_branch_prompt_id, 0);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision_at(
                second_branch_prompt_id,
                ResolutionChoiceDecision::SelectBranch,
            ),
        )
        .expect("select the second Bridge's linked-play branch");

    let second_linked_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the second Bridge prompts for its own linked card");
    assert_eq!(
        second_linked_choice.presentation.candidates,
        [second_linked_land]
    );
    let second_linked_prompt_id = second_linked_choice.resolution_prompt_id.get();
    assert_ne!(second_linked_prompt_id, 0);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_choice_at(second_linked_prompt_id, vec![second_linked_land]),
        )
        .expect("choose the second Bridge's exact linked land");

    let play_prompt_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the linked land has a live PlayLand offer")
        .resolution_prompt_id
        .get();
    assert_ne!(play_prompt_id, 0);
    let second_land_generation = engine.state.zone_change_generation[&second_linked_land];
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::PlayLand(tricerules_proto::ruled::v1::PlayLand {
                    resolution_prompt_id: play_prompt_id,
                    source: Some(LandSource {
                        location: Some(
                            tricerules_proto::ruled::v1::land_source::Location::ExileObjectId(
                                second_linked_land,
                            ),
                        ),
                        expected_zone_change_generation: Some(second_land_generation),
                    }),
                    face_index: 0,
                })),
            },
        )
        .expect("play the second Bridge's linked Forest with its live prompt");

    assert_eq!(
        engine.state.objects[&second_linked_land].zone,
        Zone::Battlefield
    );
    assert!(!engine.state.objects[&second_linked_land].face_down);
    assert_eq!(engine.state.objects[&first_linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&first_linked_land].face_down);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_linked_exile_play"].is_null());
}

fn buggy_after_combat_hit(seed: u64) -> (GameEngine, u32, u32) {
    let mut engine = game(seed);
    let linked_card = put_hideaway_card_on_top(&mut engine, 0, "opt");
    let _buggy_in_hand = inject_card_into_hand(&mut engine, 0, BUGGY);
    let buggy = move_ready_to_battlefield(&mut engine, 0, BUGGY);
    assert!(!engine.state.objects[&buggy].tapped);
    resolve_hideaway_look(&mut engine, 0, linked_card);

    let crew = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let generation = engine
        .state
        .zone_change_generation
        .get(&crew)
        .copied()
        .unwrap_or(0);
    let mut crew_command = activate_ability_with_costs(
        buggy,
        0,
        Vec::new(),
        vec![one_creature_cost(crew, generation)],
    );
    let Some(Cmd::ActivateAbility(activation)) = crew_command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&buggy)
        .copied()
        .unwrap_or(0);
    engine
        .apply_scenario_command(0, &crew_command)
        .expect("crew Smuggler's Buggy with two power");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.characteristics(buggy).unwrap().is_creature());
    advance_to_combat_damage(&mut engine, buggy);
    assert_eq!(engine.state.players[1].life, 15);
    (engine, buggy, linked_card)
}

#[test]
fn smugglers_buggy_casts_its_linked_card_after_combat_damage_then_returns_itself() {
    let (mut engine, buggy, linked_card) = buggy_after_combat_hit(20_261_012);
    let source_generation = engine.state.zone_change_generation[&buggy];
    pass_priority_round(&mut engine);
    let linked_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Buggy's combat damage trigger offers its linked card");
    assert_eq!(
        linked_choice.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert_eq!(linked_choice.presentation.candidates, [linked_card]);
    let cast_offer = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![linked_card]))
        .expect("choose the linked Opt");
    let cast_offer_logs = cast_offer
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log)) => {
                Some((log.text.clone(), log.visible_to_player_id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        cast_offer_logs.iter().any(
            |(text, visible_to)| text.contains("Face-down exiled card") && visible_to.is_none()
        ),
        "face-down cast offer logs: {cast_offer_logs:?}"
    );
    assert!(!cast_offer.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
            if log.text.contains("cast Opt") && log.visible_to_player_id.is_none()
    )));

    let cast_action = engine.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == linked_card)
        .cloned()
        .expect("Buggy grants a current resolving cast permission");
    let linked_generation = engine.state.zone_change_generation[&linked_card];
    let cast = tricerules_proto::ruled::v1::CastSpell {
        cast_method: cast_action.cast_method,
        casting_permission_id: cast_action.casting_permission_id,
        source: Some(CastSource {
            expected_zone_change_generation: Some(linked_generation),
            location: Some(
                tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(linked_card),
            ),
        }),
        ..Default::default()
    };
    let cast_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice_with_cast(cast))
        .expect("cast Opt during Buggy's resolving trigger");

    assert_eq!(engine.state.objects[&linked_card].zone, Zone::Stack);
    assert!(
        !engine.state.objects[&linked_card].face_down,
        "a successfully cast face-down exiled card is face up on the stack"
    );
    let reveal_index = cast_batch
        .events
        .iter()
        .position(|event| matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
                if reveal.source_zone == tricerules_proto::ruled::v1::ChoiceCandidateSourceZone::Exile as i32
                    && reveal.cards.iter().any(|card| card.object_id == linked_card && card.card_id == "opt")
        ))
        .expect("successful cast publicly reveals the card from Exile");
    let stack_index = cast_batch
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackPushed(stack))
                    if stack.object_id == linked_card && stack.card_id == "opt"
            )
        })
        .expect("successful cast publishes its printed StackPushed identity");
    assert!(reveal_index < stack_index, "reveal precedes StackPushed");
    assert_eq!(engine.state.objects[&buggy].zone, Zone::Hand);
    assert!(engine.state.zone_change_generation[&buggy] > source_generation);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn smugglers_buggy_decline_keeps_the_vehicle_and_linked_card_in_place() {
    let (mut engine, buggy, linked_card) = buggy_after_combat_hit(20_261_014);
    let source_generation = engine.state.zone_change_generation[&buggy];
    pass_priority_round(&mut engine);
    let linked_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Buggy's combat damage trigger offers its linked card");
    assert_eq!(
        linked_choice.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );

    let declined = engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline Buggy's optional linked cast");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&linked_card].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_card].face_down);
    assert!(!declined.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == linked_card)
    )));
    assert_eq!(engine.state.objects[&buggy].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&buggy],
        source_generation
    );
}

#[test]
fn cancelled_face_down_exile_spell_proposal_restores_its_hidden_exiled_card() {
    let mut engine = game(20_261_016);
    let linked_card = inject_card_into_hand(&mut engine, 0, "opt");
    engine.state.players[0]
        .hand
        .retain(|object_id| *object_id != linked_card);
    engine.state.players[0].exile.push(linked_card);
    let card = engine.state.objects.get_mut(&linked_card).unwrap();
    card.zone = Zone::Exile;
    card.face_down = true;
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&linked_card)
        .copied();
    let generation = original_generation.unwrap_or(0);
    engine.state.active_exile_play_permissions.push(
        tricerules_core::state::ActiveExilePlayPermission {
            group_id: 20261016,
            player_id: 0,
            source_label: "test cast permission".into(),
            object_id: linked_card,
            zone_change_generation: generation,
            scope: tricerules_core::state::ExilePlayPermissionScope::CastCard,
            max_face_mana_value: None,
            cast_cost: tricerules_core::state::ExilePermissionCastCost::PrintedManaCost,
            origin: tricerules_core::state::ExilePlayPermissionOrigin::Effect,
            available_after_turn_instance: None,
            expires_at_cleanup_turn_instance: None,
        },
    );
    let cast_action = engine.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == linked_card)
        .cloned()
        .expect("the exact exile permission offers a cast action");
    let announcement = tricerules_proto::ruled::v1::SpellCastAnnouncement {
        source: Some(CastSource {
            expected_zone_change_generation: Some(generation),
            location: Some(
                tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(linked_card),
            ),
        }),
        cast_method: cast_action.cast_method,
        casting_permission_id: cast_action.casting_permission_id,
        ..Default::default()
    };
    let mut invalid_announcement = announcement.clone();
    invalid_announcement
        .source
        .as_mut()
        .unwrap()
        .expected_zone_change_generation = Some(generation.saturating_add(1));
    let before_invalid_begin = serde_json::to_value(&engine.state).unwrap();
    assert!(engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(
                    tricerules_proto::ruled::v1::BeginSpellCast {
                        announcement: Some(invalid_announcement),
                    },
                )),
            },
        )
        .is_err());
    assert_eq!(
        serde_json::to_value(&engine.state).unwrap(),
        before_invalid_begin,
        "a stale BeginSpellCast cannot mutate or reveal the source"
    );
    assert_eq!(engine.state.objects[&linked_card].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_card].face_down);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&linked_card)
            .copied(),
        original_generation
    );
    assert!(engine.state.pending_spell_cast.is_none());

    let begin = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(
                    tricerules_proto::ruled::v1::BeginSpellCast {
                        announcement: Some(announcement.clone()),
                    },
                )),
            },
        )
        .expect("begin a valid linked spell-cast proposal");
    assert_eq!(engine.state.objects[&linked_card].zone, Zone::Stack);
    assert!(
        !engine.state.objects[&linked_card].face_down,
        "accepted announcement turns a face-down exiled source face up"
    );
    assert_eq!(
        begin.events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
                    if reveal.source_zone == tricerules_proto::ruled::v1::ChoiceCandidateSourceZone::Exile as i32
                        && reveal.cards.iter().any(|card| card.object_id == linked_card && card.card_id == "opt")
            ))
            .count(),
        1,
        "accepted announcement publishes exactly one identity receipt"
    );

    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the valid announcement is awaiting payment")
        .transaction_id;
    let cancelled = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(
                    tricerules_proto::ruled::v1::CancelSpellCast { transaction_id },
                )),
            },
        )
        .expect("cancel the uncommitted proposal");
    assert_eq!(engine.state.objects[&linked_card].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_card].face_down);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&linked_card)
            .copied(),
        original_generation,
        "cancel restores the exact original exile generation"
    );
    assert!(!cancelled.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == linked_card)
    )));

    engine.state.players[0].mana_pool.blue = 1;
    let retry_begin = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(
                    tricerules_proto::ruled::v1::BeginSpellCast {
                        announcement: Some(announcement),
                    },
                )),
            },
        )
        .expect("retry the same supported face-up cast after cancellation");
    assert!(!engine.state.objects[&linked_card].face_down);
    assert_eq!(
        retry_begin
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
                    if reveal.cards.iter().any(|card| card.object_id == linked_card && card.card_id == "opt")
            ))
            .count(),
        1,
        "the retried accepted announcement publishes one reveal receipt"
    );
    let retry_transaction = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("retry waits for payment")
        .transaction_id;
    let committed = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(
                    tricerules_proto::ruled::v1::CommitSpellCast {
                        transaction_id: retry_transaction,
                        payment: Some(PaymentSelection {
                            expected_state_revision: engine.state.command_index,
                            source: Some(tricerules_proto::ruled::v1::CostObjectRef {
                                object_id: linked_card,
                                zone_change_generation: engine.state.zone_change_generation
                                    [&linked_card],
                            }),
                            mana: Some(PaymentMana {
                                u: 1,
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("commit the retried spell");
    assert!(!engine.state.objects[&linked_card].face_down);
    assert!(!committed.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == linked_card)
    )));
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
}

fn submit_resolution_choice_with_cast(
    cast: tricerules_proto::ruled::v1::CastSpell,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(
            tricerules_proto::ruled::v1::SubmitResolutionChoice {
                resolution_prompt_id: 0,
                decision: ResolutionChoiceDecision::CastSpell as i32,
                cast_spell: Some(cast),
                ..Default::default()
            },
        )),
    }
}

fn advance_to_combat_damage(engine: &mut GameEngine, attacker: u32) {
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("begin combat");
    for _ in 0..12 {
        if engine.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
            break;
        }
        pass_priority_round(engine);
    }
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("attack with Smuggler's Buggy");
    for _ in 0..12 {
        if engine.state.turn_step == tricerules_core::TurnStep::CombatDamage {
            return;
        }
        pass_priority_round(engine);
    }
    panic!(
        "game did not reach combat damage: {:?}",
        engine.state.turn_step
    );
}
