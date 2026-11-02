//! Phase B capability-only staging regression for Narset's next own-hand cast observer.
//! The synthetic Artifact exercises trigger creation only; it does not claim Rebound resolution.
use std::sync::OnceLock;

use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const REBOUND_OBSERVER: &str = "narset_rebound_test_artifact";
const REBOUND_OBSERVER_NAME: &str = "Narset Rebound Test Artifact";

const REBOUND_OBSERVER_RON: &str = r#"(
  id: "narset_rebound_test_artifact",
  name: "Narset Rebound Test Artifact",
  face_id: "narset_rebound_test_artifact",
  mana_cost: "{0}",
  types: ["Artifact"],
  activated_abilities: [(
    ability_id: "activated_01",
    presentation: Fallback,
    costs: [Tap],
    effect: [NextHandInstantOrSorceryGainsReboundThisTurn],
  )],
)"#;

fn narset_rebound_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(
            &[
                REBOUND_OBSERVER_RON,
                include_str!("../../../tricerules-cards/data/opt.ron"),
            ],
            &[],
        )
        .expect("the synthetic Artifact and Opt fixture definitions register")
    })
}

#[test]
fn narset_phase_b_next_own_hand_cast_stages_grant() {
    let decks = Some(vec![
        deck_with("opt", &[REBOUND_OBSERVER]),
        deck_with("opt", &[]),
    ]);
    let mut engine = GameEngine::new(
        narset_rebound_registry(),
        6_110_101,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Narset capability fixture engine");
    advance_to_main1_from_game_start(&mut engine);

    let before = engine.state.players[0].battlefield.clone();
    let player_id = engine.state.players[0].id;
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: REBOUND_OBSERVER_NAME.into(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move the registered synthetic Artifact through the engine");
    let source = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| !before.contains(object_id))
        .expect("synthetic observer entered the battlefield");
    let source_generation = engine.state.zone_change_generation[&source];

    let activation = authoring_actions::activation(&mut engine, player_id, source, 0)
        .expect("engine offers the synthetic observer activation");
    engine
        .apply_command(player_id, &activation)
        .expect("activate the synthetic observer");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "observer activation resolved"
    );

    ensure_in_hand(&mut engine, 0, "opt");
    give_mana(
        &mut engine,
        player_id,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "opt");
    let mut cast = cast_spell(hand_index, vec![]);
    authoring_actions::pay(&engine, player_id, &mut cast).expect("pay Opt from the live offer");
    engine
        .apply_command(player_id, &cast)
        .expect("commit Opt without passing priority");

    assert_eq!(
        engine.state.stack.len(),
        2,
        "the trigger is staged above Opt"
    );
    let opt = &engine.state.stack[0];
    let grant = &engine.state.stack[1];
    assert_eq!(opt.card_id, "opt");
    assert!(!opt.is_triggered);
    assert!(!opt.is_copy, "the committed Opt remains the original spell");
    assert!(
        opt.cast_occurrence.is_some(),
        "Opt has its committed cast occurrence"
    );

    assert!(
        grant.is_triggered,
        "the observer creates an ordinary trigger"
    );
    assert!(!grant.is_copy, "the grant trigger is not a spell copy");
    assert_eq!(grant.controller, player_id);
    assert_eq!(grant.source_permanent_id, Some(source));
    assert_eq!(grant.source_zone_change, source_generation);
    assert!(grant.targets.is_empty(), "the grant trigger has no targets");
    assert!(grant.triggered_ability.is_some());
    let presentation = engine
        .state
        .stack_presentations
        .get(&grant.id)
        .expect("ordinary trigger retains its stack presentation");
    assert_eq!(
        presentation.source_label.as_deref(),
        Some(REBOUND_OBSERVER_NAME)
    );
    assert!(
        engine.state.captured_spell_copies.is_empty(),
        "staging the grant creates no immediate spell copy"
    );

    let serialized_grant = serde_json::to_value(grant).expect("serialize the staged trigger");
    let serialized_effects = &serialized_grant["triggered_ability"]["effect"];
    assert_eq!(
        serialized_effects
            .as_array()
            .expect("trigger effect list")
            .len(),
        1,
        "the staged trigger carries exactly one typed grant effect"
    );
    assert!(
        serialized_effects
            .to_string()
            .contains("GrantCapturedSpellRebound"),
        "the serialized trigger executor is GrantCapturedSpellRebound"
    );
    assert_eq!(
        serialized_grant["trigger_context"]["triggering_spell"],
        serde_json::to_value(opt.cast_occurrence)
            .expect("serialize Opt's full public cast occurrence"),
        "the trigger captures this exact committed Opt occurrence"
    );
}

fn narset_rebound_consumption_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(
            &[
                REBOUND_OBSERVER_RON,
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/generated/t/think_twice.ron"),
                include_str!("../../../tricerules-cards/data/generated/a/alanias_pathmaker.ron"),
            ],
            &[],
        )
        .expect("the synthetic observer and actual origin-test cards register")
    })
}

fn narset_rebound_consumption_fixture(seed: u64) -> (GameEngine, u32, u64) {
    let decks = Some(vec![
        deck_with(
            "opt",
            &[
                REBOUND_OBSERVER,
                "grizzly_bears",
                "think_twice",
                "alanias_pathmaker",
            ],
        ),
        deck_with("opt", &[]),
    ]);
    let mut engine = GameEngine::new(
        narset_rebound_consumption_registry(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Narset capability consumption fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = move_fixture_card_to_zone(&mut engine, 0, REBOUND_OBSERVER, DevZone::Battlefield);
    let source_generation = engine.state.zone_change_generation[&source];
    (engine, source, source_generation)
}

fn fixture_zone_objects(engine: &GameEngine, player: usize, zone: DevZone) -> Vec<u32> {
    match zone {
        DevZone::Battlefield => engine.state.players[player].battlefield.clone(),
        DevZone::Graveyard => engine.state.players[player].graveyard.clone(),
        _ => panic!("unsupported fixture destination zone: {zone:?}"),
    }
}

fn move_fixture_card_to_zone(
    engine: &mut GameEngine,
    player: usize,
    card_id: &str,
    zone: DevZone,
) -> u32 {
    let player_id = engine.state.players[player].id;
    let card_name = narset_rebound_consumption_registry()
        .get(card_id)
        .expect("fixture card is registered")
        .name
        .clone();
    let before = fixture_zone_objects(engine, player, zone);
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name,
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move the one synthetic fixture card through the engine");
    fixture_zone_objects(engine, player, zone)
        .into_iter()
        .find(|object_id| !before.contains(object_id))
        .expect("the requested fixture card entered its destination zone")
}

fn put_card_on_top_of_library(engine: &mut GameEngine, player: usize, card_id: &str) {
    let position = engine.state.players[player]
        .library
        .iter()
        .position(|object_id| {
            engine
                .state
                .objects
                .get(object_id)
                .is_some_and(|object| object.card_id == card_id)
        })
        .expect("fixture card is in the library");
    let object_id = engine.state.players[player]
        .library
        .remove(position)
        .expect("found library position");
    engine.state.players[player].library.insert(0, object_id);
}

fn serialized_observer_count(engine: &GameEngine) -> usize {
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("serialize engine diagnostics for active observer state");
    snapshot["state"]["active_event_observers"]
        .as_array()
        .expect("active event observer list")
        .len()
}

fn serialized_rebound_grants(engine: &GameEngine) -> serde_json::Value {
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("serialize engine diagnostics for private rebound grant state");
    snapshot["state"]["rebound_spell_grants"].clone()
}

fn arm_narset_observer(engine: &mut GameEngine, source: u32) {
    let player_id = engine.state.players[0].id;
    let activation = authoring_actions::activation(engine, player_id, source, 0)
        .expect("engine offers the synthetic observer activation");
    engine
        .apply_command(player_id, &activation)
        .expect("activate the synthetic observer through the current offer");
    resolve_entire_stack_two_player(engine);
    assert_eq!(serialized_observer_count(engine), 1);
    assert_eq!(serialized_rebound_grants(engine), serde_json::json!([]));
}

fn cast_hand_card(
    engine: &mut GameEngine,
    player: usize,
    card_id: &str,
    mana: ManaGift,
) -> tricerules_core::state::StackObjectRef {
    let player_id = engine.state.players[player].id;
    ensure_in_hand(engine, player, card_id);
    give_mana(engine, player_id, mana);
    let hand_index = hand_index_for_card(engine, player, card_id);
    let mut command = cast_spell(hand_index, vec![]);
    let old_stack_ids = engine
        .state
        .stack
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    authoring_actions::pay(engine, player_id, &mut command)
        .expect("pay the cast from the current engine offer");
    engine
        .apply_command(player_id, &command)
        .expect("commit the hand cast through the engine");
    engine
        .state
        .stack
        .iter()
        .find(|item| {
            !old_stack_ids.contains(&item.id)
                && item.card_id == card_id
                && item.cast_by == Some(player_id)
        })
        .and_then(|item| item.cast_occurrence)
        .expect("the newly committed hand spell has a cast occurrence")
}

fn assert_unqualified_cast_leaves_observer_armed(engine: &GameEngine, card_id: &str) {
    assert_eq!(engine.state.stack.len(), 1, "no trigger was staged");
    let spell = &engine.state.stack[0];
    assert_eq!(spell.card_id, card_id);
    assert!(!spell.is_triggered);
    assert!(!spell.is_copy);
    assert_eq!(serialized_observer_count(engine), 1);
    assert_eq!(serialized_rebound_grants(engine), serde_json::json!([]));
}

fn assert_one_rebound_trigger_for_cast(
    engine: &GameEngine,
    source: u32,
    source_generation: u64,
    expected_spell: tricerules_core::state::StackObjectRef,
) {
    let spell_index = engine
        .state
        .stack
        .iter()
        .position(|item| item.cast_occurrence == Some(expected_spell))
        .expect("the qualifying hand Opt is on the stack");
    let triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect::<Vec<_>>();
    assert_eq!(triggers.len(), 1, "one ordinary grant trigger is staged");
    let trigger = triggers[0];
    let trigger_index = engine
        .state
        .stack
        .iter()
        .position(|item| item.id == trigger.id)
        .expect("staged trigger is on the stack");
    assert!(trigger_index > spell_index, "the trigger is above its Opt");
    assert!(!trigger.is_copy);
    assert_eq!(trigger.controller, engine.state.players[0].id);
    assert_eq!(trigger.source_permanent_id, Some(source));
    assert_eq!(trigger.source_zone_change, source_generation);
    assert!(trigger.targets.is_empty());
    let presentation = engine
        .state
        .stack_presentations
        .get(&trigger.id)
        .expect("trigger retains its captured source presentation");
    assert_eq!(
        presentation.source_label.as_deref(),
        Some(REBOUND_OBSERVER_NAME)
    );
    let serialized_trigger = serde_json::to_value(trigger).expect("serialize staged trigger");
    assert_eq!(
        serialized_trigger["trigger_context"]["triggering_spell"],
        serde_json::to_value(Some(expected_spell))
            .expect("serialize the qualifying Opt's full cast occurrence"),
        "the grant trigger captures the qualifying hand Opt, not the earlier spell"
    );
    assert_eq!(
        serialized_rebound_grants(engine),
        serde_json::json!([]),
        "the captured spell is not granted Rebound before its trigger resolves"
    );
    assert_eq!(serialized_observer_count(engine), 0);
}

fn assert_next_hand_opt_consumes_observer_once(
    engine: &mut GameEngine,
    source: u32,
    source_generation: u64,
) {
    let first_opt = cast_hand_card(
        engine,
        0,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_one_rebound_trigger_for_cast(engine, source, source_generation, first_opt);
    let stack_len_with_one_trigger = engine.state.stack.len();

    let second_opt = cast_hand_card(
        engine,
        0,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        engine.state.stack.len(),
        stack_len_with_one_trigger + 1,
        "the next own-hand Opt adds no second trigger"
    );
    let second_spell = engine
        .state
        .stack
        .iter()
        .find(|item| item.cast_occurrence == Some(second_opt))
        .expect("the second Opt was committed");
    assert_eq!(second_spell.card_id, "opt");
    assert!(!second_spell.is_triggered);
    assert!(!second_spell.is_copy);
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_triggered)
            .count(),
        1,
        "the observer consumed only the first qualifying own-hand Opt"
    );
    assert_eq!(serialized_observer_count(engine), 0);
    assert_eq!(serialized_rebound_grants(engine), serde_json::json!([]));
}

fn cast_graveyard_flashback(
    engine: &mut GameEngine,
    object_id: u32,
) -> tricerules_core::state::StackObjectRef {
    let player_id = engine.state.players[0].id;
    let batch = engine.initial_response_batch();
    let action = batch.legal_by_player[&player_id]
        .zone_cast_actions
        .iter()
        .find(|action| {
            action.object_id == object_id && action.cast_method == CastMethod::Flashback as i32
        })
        .expect("current offer includes Think Twice Flashback from the graveyard");
    let generation = engine.state.zone_change_generation[&object_id];
    let mut command = RuledCommand {
        cmd: Some(Cmd::CastSpell(tricerules_proto::ruled::v1::CastSpell {
            source: Some(graveyard_cast_source(object_id, generation)),
            face_index: action.face_index,
            cast_method: action.cast_method,
            casting_permission_id: action.casting_permission_id,
            ..Default::default()
        })),
    };
    let old_stack_ids = engine
        .state
        .stack
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    authoring_actions::pay(engine, player_id, &mut command)
        .expect("pay Flashback from the current graveyard cast offer");
    engine
        .apply_command(player_id, &command)
        .expect("commit the actual Flashback cast");
    engine
        .state
        .stack
        .iter()
        .find(|item| !old_stack_ids.contains(&item.id) && item.card_id == "think_twice")
        .and_then(|item| item.cast_occurrence)
        .expect("Think Twice Flashback has a committed cast occurrence")
}

fn cast_exile_permission_spell(
    engine: &mut GameEngine,
    object_id: u32,
) -> tricerules_core::state::StackObjectRef {
    let player_id = engine.state.players[0].id;
    let batch = engine.initial_response_batch();
    let action = batch.legal_by_player[&player_id]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == object_id)
        .expect("current offer includes Opt's Alania exile-play permission");
    assert!(
        action.casting_permission_id.is_some(),
        "the offered exile cast is bound to Alania's current permission"
    );
    let mut command = RuledCommand {
        cmd: Some(Cmd::CastSpell(tricerules_proto::ruled::v1::CastSpell {
            source: Some(exile_cast_source(object_id, action.zone_change_generation)),
            face_index: action.face_index,
            cast_method: action.cast_method,
            casting_permission_id: action.casting_permission_id,
            ..Default::default()
        })),
    };
    let old_stack_ids = engine
        .state
        .stack
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    authoring_actions::pay(engine, player_id, &mut command)
        .expect("pay Opt from the current exile cast offer");
    engine
        .apply_command(player_id, &command)
        .expect("commit Opt from exile using Alania's permission");
    engine
        .state
        .stack
        .iter()
        .find(|item| !old_stack_ids.contains(&item.id) && item.card_id == "opt")
        .and_then(|item| item.cast_occurrence)
        .expect("exiled Opt has a committed cast occurrence")
}

#[test]
fn opponent_hand_opt_and_own_hand_creature_leave_observer_armed() {
    let (mut engine, source, source_generation) = narset_rebound_consumption_fixture(6_110_102);
    arm_narset_observer(&mut engine, source);
    let opponent_id = engine.state.players[1].id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass priority so the opponent can cast its hand Opt");
    assert_eq!(engine.state.priority_player_id(), opponent_id);
    let opponent_opt = cast_hand_card(
        &mut engine,
        1,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        engine.state.stack.last().unwrap().cast_occurrence,
        Some(opponent_opt)
    );
    assert_unqualified_cast_leaves_observer_armed(&engine, "opt");
    engine
        .apply_scenario_command(opponent_id, &pass())
        .expect("pass back so the observer's controller can respond");
    assert_eq!(engine.state.priority_player_id(), 0);
    assert_next_hand_opt_consumes_observer_once(&mut engine, source, source_generation);

    let (mut engine, source, source_generation) = narset_rebound_consumption_fixture(6_110_103);
    arm_narset_observer(&mut engine, source);
    let grizzly_bears = cast_hand_card(
        &mut engine,
        0,
        "grizzly_bears",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        engine.state.stack.last().unwrap().cast_occurrence,
        Some(grizzly_bears)
    );
    assert_unqualified_cast_leaves_observer_armed(&engine, "grizzly_bears");
    assert_next_hand_opt_consumes_observer_once(&mut engine, source, source_generation);
}

#[test]
fn think_twice_flashback_leaves_observer_armed() {
    let (mut engine, source, source_generation) = narset_rebound_consumption_fixture(6_110_104);
    let normal_think_twice = cast_hand_card(
        &mut engine,
        0,
        "think_twice",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&normal_think_twice.object_id].zone,
        tricerules_core::Zone::Graveyard,
        "the real Think Twice card reaches its graveyard before arming"
    );

    arm_narset_observer(&mut engine, source);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let flashback = cast_graveyard_flashback(&mut engine, normal_think_twice.object_id);
    assert_eq!(
        engine.state.stack.last().unwrap().cast_occurrence,
        Some(flashback)
    );
    assert_unqualified_cast_leaves_observer_armed(&engine, "think_twice");
    assert_next_hand_opt_consumes_observer_once(&mut engine, source, source_generation);
}

#[test]
fn alanias_pathmaker_exile_permission_cast_leaves_observer_armed() {
    let (mut engine, source, source_generation) = narset_rebound_consumption_fixture(6_110_105);
    put_card_on_top_of_library(&mut engine, 0, "opt");
    let pathmaker = cast_hand_card(
        &mut engine,
        0,
        "alanias_pathmaker",
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&pathmaker.object_id].zone,
        tricerules_core::Zone::Battlefield
    );
    let opt_in_exile = engine.state.players[0]
        .exile
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "opt")
        .expect("Alania's actual ETB permission exiles the prepared top Opt");

    arm_narset_observer(&mut engine, source);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let exile_cast = cast_exile_permission_spell(&mut engine, opt_in_exile);
    assert_eq!(
        engine.state.stack.last().unwrap().cast_occurrence,
        Some(exile_cast)
    );
    assert_unqualified_cast_leaves_observer_armed(&engine, "opt");
    assert_next_hand_opt_consumes_observer_once(&mut engine, source, source_generation);
}

#[test]
fn failed_spell_cast_commit_then_cancel_does_not_consume_observer() {
    let (mut engine, source, source_generation) = narset_rebound_consumption_fixture(6_110_106);
    arm_narset_observer(&mut engine, source);
    ensure_in_hand(&mut engine, 0, "opt");
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    let hand_index = hand_index_for_card(&engine, 0, "opt");
    let Some(Cmd::CastSpell(cast)) = cast_spell(hand_index, vec![]).cmd else {
        unreachable!("cast_spell creates a CastSpell announcement")
    };
    let player_id = engine.state.players[0].id;
    engine
        .apply_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(
                    tricerules_proto::ruled::v1::BeginSpellCast {
                        announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                            targets: cast.targets,
                            x_value: cast.x_value,
                            flex_payments: cast.flex_payments,
                            face_index: cast.face_index,
                            selected_modes: cast.selected_modes,
                            source: cast.source,
                            prototyped: cast.prototyped,
                            cost_selections: cast.cost_selections,
                            cast_cost_group_selections: cast.cast_cost_group_selections,
                            cast_method: cast.cast_method,
                            casting_permission_id: cast.casting_permission_id,
                        }),
                    },
                )),
            },
        )
        .expect("begin the current hand-cast announcement");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("BeginSpellCast opened payment")
        .transaction_id;
    assert_eq!(serialized_observer_count(&engine), 1);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    let before_rejected_commit = engine
        .diagnostic_snapshot()
        .expect("serialize diagnostics before failed payment commit");
    assert!(engine
        .apply_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(
                    tricerules_proto::ruled::v1::CommitSpellCast {
                        transaction_id,
                        ..Default::default()
                    },
                )),
            },
        )
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("serialize diagnostics after failed payment commit"),
        before_rejected_commit,
        "the rejected CommitSpellCast reaches the engine and preserves the pending state"
    );

    engine
        .apply_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(
                    tricerules_proto::ruled::v1::CancelSpellCast { transaction_id },
                )),
            },
        )
        .expect("cancel the uncommitted cast transaction");
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(serialized_observer_count(&engine), 1);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
    assert_next_hand_opt_consumes_observer_once(&mut engine, source, source_generation);
}

#[test]
fn prepared_source_departure_does_not_cancel_armed_observer() {
    let (mut engine, source, source_generation) = narset_rebound_consumption_fixture(6_110_107);
    arm_narset_observer(&mut engine, source);

    // Prepared-state setup only: DevMove isolates source independence; this is not legal-path evidence.
    let moved_source =
        move_fixture_card_to_zone(&mut engine, 0, REBOUND_OBSERVER, DevZone::Graveyard);
    assert_eq!(
        moved_source, source,
        "the same source incarnation left the battlefield"
    );
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_ne!(
        engine.state.zone_change_generation[&source],
        source_generation
    );
    assert_eq!(serialized_observer_count(&engine), 1);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    assert_next_hand_opt_consumes_observer_once(&mut engine, source, source_generation);
}

#[test]
fn observer_stays_through_end_step_and_expires_at_cleanup() {
    let (mut engine, source, _) = narset_rebound_consumption_fixture(6_110_108);
    arm_narset_observer(&mut engine, source);
    let active_turn = engine.state.turn_instance;
    let mut reached_end_step = false;
    for _ in 0..40 {
        if engine.state.turn_instance != active_turn {
            break;
        }
        if engine.state.turn_step == tricerules_core::TurnStep::EndStep {
            reached_end_step = true;
            assert_eq!(serialized_observer_count(&engine), 1);
            assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
        }
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("pass through ordinary turn progression");
    }
    assert!(
        reached_end_step,
        "the active turn reached its real end step"
    );
    assert_ne!(engine.state.turn_instance, active_turn);
    assert_eq!(serialized_observer_count(&engine), 0);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    if engine.state.priority_player_id() != 0 {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("give the nonactive observer controller priority in the new turn");
    }
    assert_eq!(engine.state.priority_player_id(), 0);
    let next_turn_opt = cast_hand_card(
        &mut engine,
        0,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_eq!(engine.state.stack.len(), 1);
    let spell = engine.state.stack.last().unwrap();
    assert_eq!(spell.cast_occurrence, Some(next_turn_opt));
    assert_eq!(spell.card_id, "opt");
    assert!(!spell.is_triggered);
    assert_eq!(serialized_observer_count(&engine), 0);
}

// These integration fixtures add only the real interaction spells needed to observe the
// lifetime of a captured own-hand Opt. The test Artifact above remains the observer.
fn narset_rebound_lifetime_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(
            &[
                REBOUND_OBSERVER_RON,
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!("../../../tricerules-cards/data/counterspell.ron"),
                include_str!("../../../tricerules-cards/data/twincast.ron"),
                include_str!("../../../tricerules-cards/data/tishanas_tidebinder.ron"),
            ],
            &[],
        )
        .expect("the observer and actual lifetime-interaction cards register")
    })
}

fn narset_rebound_lifetime_fixture(seed: u64) -> (GameEngine, u32, u64) {
    let decks = Some(vec![
        deck_with("opt", &[REBOUND_OBSERVER, REBOUND_OBSERVER]),
        deck_with("opt", &["tishanas_tidebinder", "counterspell", "twincast"]),
    ]);
    let mut engine = GameEngine::new(
        narset_rebound_lifetime_registry(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Narset grant-lifetime fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = move_fixture_card_to_zone(&mut engine, 0, REBOUND_OBSERVER, DevZone::Battlefield);
    let source_generation = engine.state.zone_change_generation[&source];
    (engine, source, source_generation)
}

fn cast_hand_card_with_targets(
    engine: &mut GameEngine,
    player: usize,
    card_id: &str,
    mana: ManaGift,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) -> tricerules_core::state::StackObjectRef {
    let player_id = engine.state.players[player].id;
    ensure_in_hand(engine, player, card_id);
    give_mana(engine, player_id, mana);
    let hand_index = hand_index_for_card(engine, player, card_id);
    let mut command = cast_spell(hand_index, targets);
    let old_stack_ids = engine
        .state
        .stack
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    authoring_actions::pay(engine, player_id, &mut command)
        .expect("pay the targeted cast from the current engine offer");
    engine
        .apply_command(player_id, &command)
        .expect("commit the targeted hand cast through the engine");
    engine
        .state
        .stack
        .iter()
        .find(|item| {
            !old_stack_ids.contains(&item.id)
                && item.card_id == card_id
                && item.cast_by == Some(player_id)
        })
        .and_then(|item| item.cast_occurrence)
        .expect("the newly committed targeted spell has a cast occurrence")
}

fn stack_target_for_cast(
    occurrence: tricerules_core::state::StackObjectRef,
) -> tricerules_proto::ruled::v1::TargetRef {
    tricerules_proto::ruled::v1::TargetRef {
        object_id: occurrence.object_id,
        expected_zone_change_generation: occurrence.zone_change_generation,
        kind: tricerules_proto::ruled::v1::TargetRefKind::Stack as i32,
        ..Default::default()
    }
}

fn expected_rebound_grants_for_cast(
    occurrence: tricerules_core::state::StackObjectRef,
) -> serde_json::Value {
    serde_json::json!([[
        occurrence.object_id,
        occurrence
            .zone_change_generation
            .map(|generation| generation.to_string())
    ]])
}

fn triggered_stack_ids(engine: &GameEngine) -> Vec<u32> {
    engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .map(|item| item.id)
        .collect()
}

#[test]
fn actual_tishanas_tidebinder_counters_grant_trigger_without_granting_opt() {
    let (mut engine, source, source_generation) = narset_rebound_lifetime_fixture(6_110_111);
    arm_narset_observer(&mut engine, source);
    let opt = cast_hand_card(
        &mut engine,
        0,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, opt);
    let grant_trigger = *triggered_stack_ids(&engine)
        .first()
        .expect("the captured Opt has its ordinary grant trigger");

    let opponent = engine.state.players[1].id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent priority to cast Tishana's Tidebinder");
    let tidebinder = cast_hand_card(
        &mut engine,
        1,
        "tishanas_tidebinder",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&tidebinder.object_id].zone,
        tricerules_core::Zone::Battlefield,
        "the actual Tidebinder creature resolves before its target is chosen"
    );

    let current_offer = engine.initial_response_batch();
    let target_offer = current_offer.legal_by_player[&opponent]
        .valid_targets_by_ability
        .values()
        .find(|ability_targets| {
            ability_targets
                .groups
                .iter()
                .any(|group| group.valid_stack_ids.contains(&grant_trigger))
        })
        .expect("the current Tidebinder offer includes the grant trigger on the stack");
    assert!(target_offer
        .groups
        .iter()
        .any(|group| group.valid_stack_ids.contains(&grant_trigger)));
    let choose_grant = RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(
            tricerules_proto::ruled::v1::ChooseTriggerTarget {
                targets: vec![tricerules_proto::ruled::v1::TargetRef {
                    object_id: grant_trigger,
                    kind: tricerules_proto::ruled::v1::TargetRefKind::Stack as i32,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )),
    };
    engine
        .apply_scenario_command(opponent, &choose_grant)
        .expect("choose the current actual grant trigger for Tidebinder");
    pass_priority_round(&mut engine);

    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == grant_trigger));
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == opt.object_id && item.card_id == "opt"));
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
}

#[test]
fn countered_captured_opt_before_grant_resolution_does_not_leave_grant() {
    let (mut engine, source, source_generation) = narset_rebound_lifetime_fixture(6_110_112);
    arm_narset_observer(&mut engine, source);
    let opt = cast_hand_card(
        &mut engine,
        0,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, opt);
    let grant_trigger = *triggered_stack_ids(&engine)
        .first()
        .expect("the captured Opt has its ordinary grant trigger");

    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent priority to counter the captured Opt");
    cast_hand_card_with_targets(
        &mut engine,
        1,
        "counterspell",
        ManaGift {
            u: 2,
            ..Default::default()
        },
        vec![stack_target_for_cast(opt)],
    );
    pass_priority_round(&mut engine);

    assert_eq!(triggered_stack_ids(&engine), vec![grant_trigger]);
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Graveyard,
        "the actual Counterspell removed the captured Opt before grant resolution"
    );
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
}

#[test]
fn two_observers_share_one_exact_rebound_grant_after_separate_resolution() {
    let (mut engine, first_source, first_generation) = narset_rebound_lifetime_fixture(6_110_113);
    let second_source =
        move_fixture_card_to_zone(&mut engine, 0, REBOUND_OBSERVER, DevZone::Battlefield);
    assert_ne!(first_source, second_source);
    arm_narset_observer(&mut engine, first_source);

    let player = engine.state.players[0].id;
    let activation = authoring_actions::activation(&mut engine, player, second_source, 0)
        .expect("engine offers the second synthetic observer activation");
    engine
        .apply_command(player, &activation)
        .expect("activate the second synthetic observer through the current offer");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(serialized_observer_count(&engine), 2);

    let opt = cast_hand_card(
        &mut engine,
        0,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let _published = engine.initial_response_batch();
    let pending_order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("one controller must order its two simultaneous observer triggers");
    assert_eq!(pending_order.deciding_player, player);
    assert_eq!(pending_order.candidates.len(), 2);
    let candidate_ids = pending_order
        .candidates
        .iter()
        .map(|candidate| candidate.object_id)
        .collect::<Vec<_>>();
    assert_ne!(candidate_ids[0], candidate_ids[1]);
    let mut candidate_sources = pending_order
        .candidates
        .iter()
        .map(|candidate| candidate.source_permanent_id)
        .collect::<Vec<_>>();
    candidate_sources.sort_unstable();
    let mut observer_sources = vec![first_source, second_source];
    observer_sources.sort_unstable();
    assert_eq!(candidate_sources, observer_sources);
    assert!(answer_trigger_order_in_engine_order(&mut engine));
    assert!(engine.state.pending_trigger_order.is_none());
    assert_eq!(triggered_stack_ids(&engine).len(), 2);
    assert_eq!(serialized_observer_count(&engine), 0);

    let expected = expected_rebound_grants_for_cast(opt);
    pass_priority_round(&mut engine);
    assert_eq!(serialized_rebound_grants(&engine), expected);
    assert_eq!(triggered_stack_ids(&engine).len(), 1);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == opt.object_id && item.card_id == "opt"));

    pass_priority_round(&mut engine);
    assert_eq!(serialized_rebound_grants(&engine), expected);
    assert!(triggered_stack_ids(&engine).is_empty());
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == opt.object_id && item.card_id == "opt"));
    assert_eq!(
        first_generation,
        engine.state.zone_change_generation[&first_source]
    );
}

#[test]
fn twincast_copy_does_not_inherit_original_grant_and_cleanup_waits_for_scry() {
    let (mut engine, source, source_generation) = narset_rebound_lifetime_fixture(6_110_114);
    arm_narset_observer(&mut engine, source);
    let opt = cast_hand_card(
        &mut engine,
        0,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, opt);
    pass_priority_round(&mut engine);
    let expected = expected_rebound_grants_for_cast(opt);
    assert_eq!(serialized_rebound_grants(&engine), expected);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, opt.object_id);

    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent priority to cast Twincast at the original Opt");
    cast_hand_card_with_targets(
        &mut engine,
        1,
        "twincast",
        ManaGift {
            u: 2,
            ..Default::default()
        },
        vec![stack_target_for_cast(opt)],
    );
    pass_priority_round(&mut engine);

    let copy_id = {
        let copy = engine
            .state
            .stack
            .iter()
            .find(|item| item.is_copy && item.card_id == "opt")
            .expect("actual Twincast creates a virtual copy of the original Opt");
        assert_ne!(copy.id, opt.object_id);
        assert_eq!(copy.cast_occurrence, None);
        copy.id
    };
    assert_eq!(serialized_rebound_grants(&engine), expected);

    pass_priority_round(&mut engine);
    let (copy_decider, copy_candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the actual Opt copy parks at its Scry choice");
        assert_eq!(
            pending.presentation.source_object_id, copy_id,
            "the parked Scry belongs to the virtual copy"
        );
        assert_eq!(
            pending.presentation.choice_kind,
            tricerules_proto::ruled::v1::ChoiceKind::LibraryTop
        );
        (
            pending.deciding_player,
            pending.presentation.candidates.clone(),
        )
    };
    assert_eq!(serialized_rebound_grants(&engine), expected);
    engine
        .apply_scenario_command(copy_decider, &submit_resolution_choice(copy_candidates))
        .expect("answer the copy's current Scry prompt");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, opt.object_id);
    assert_eq!(serialized_rebound_grants(&engine), expected);

    pass_priority_round(&mut engine);
    let (opt_decider, opt_candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the original Opt parks at its own Scry choice");
        assert_eq!(
            pending.presentation.source_object_id, opt.object_id,
            "the parked Scry belongs to the granted original Opt"
        );
        assert_eq!(
            pending.presentation.choice_kind,
            tricerules_proto::ruled::v1::ChoiceKind::LibraryTop
        );
        (
            pending.deciding_player,
            pending.presentation.candidates.clone(),
        )
    };
    assert_eq!(serialized_rebound_grants(&engine), expected);
    engine
        .apply_scenario_command(opt_decider, &submit_resolution_choice(opt_candidates))
        .expect("answer the original Opt's current Scry prompt");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
}

#[test]
fn countering_opt_after_grant_resolution_clears_exact_grant() {
    let (mut engine, source, source_generation) = narset_rebound_lifetime_fixture(6_110_115);
    arm_narset_observer(&mut engine, source);
    let opt = cast_hand_card(
        &mut engine,
        0,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, opt);
    pass_priority_round(&mut engine);
    let expected = expected_rebound_grants_for_cast(opt);
    assert_eq!(serialized_rebound_grants(&engine), expected);

    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent priority to counter the granted Opt");
    cast_hand_card_with_targets(
        &mut engine,
        1,
        "counterspell",
        ManaGift {
            u: 2,
            ..Default::default()
        },
        vec![stack_target_for_cast(opt)],
    );
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.stack.len(), 0);
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Graveyard,
        "Counterspell removes the exact granted Opt from the stack"
    );
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
}

const NARSET_PHASE_B_MODEL_SPELL_BASELINE: &str = r#"(
  id: "narset_rebound_model_spell_fixture",
  name: "Narset Rebound Model Spell Fixture",
  face_id: "narset_rebound_model_spell_fixture",
  mana_cost: "{1}",
  types: ["Instant"],
  spell_effect: [GainLife(who: Controller, amount: 1)],
)"#;

const NARSET_PHASE_B_MODEL_ACTIVATED_BASELINE: &str = r#"(
  id: "narset_rebound_model_activated_fixture",
  name: "Narset Rebound Model Activated Fixture",
  face_id: "narset_rebound_model_activated_fixture",
  mana_cost: "{1}",
  types: ["Artifact"],
  activated_abilities: [(
    ability_id: "activated_01",
    presentation: Fallback,
    costs: [Tap],
    effect: [GainLife(who: Controller, amount: 1)],
  )],
)"#;

const NARSET_PHASE_B_MODEL_TRIGGERED_BASELINE: &str = r#"(
  id: "narset_rebound_model_triggered_fixture",
  name: "Narset Rebound Model Triggered Fixture",
  face_id: "narset_rebound_model_triggered_fixture",
  mana_cost: "{1}",
  types: ["Enchantment"],
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: Fallback,
    trigger: WhenSelfEntersBattlefield,
    effect: [GainLife(who: Controller, amount: 1)],
  )],
)"#;

fn narset_phase_b_parse_model_baseline(
    ron_text: &str,
) -> tricerules_card_model::card_def::RawCardDefinition {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(ron_text)
        .expect("the approved local baseline parses as RawCardDefinition")
}

fn narset_phase_b_model_registry(
    raw: &tricerules_card_model::card_def::RawCardDefinition,
) -> Result<
    tricerules_card_model::registry::CardRegistry,
    tricerules_card_model::registry::RegistryError,
> {
    let serialized =
        ron::ser::to_string(raw).expect("typed baseline mutation remains serializable");
    tricerules_card_model::registry::CardRegistry::from_chunks_and_tokens(
        &[serialized.as_str()],
        &[],
    )
}

fn narset_phase_b_expect_model_admitted(
    raw: &tricerules_card_model::card_def::RawCardDefinition,
    case: &str,
) {
    narset_phase_b_model_registry(raw)
        .unwrap_or_else(|error| panic!("registry rejected {case}: {error}"));
}

fn narset_phase_b_expect_rebound_rejected(
    raw: &tricerules_card_model::card_def::RawCardDefinition,
    case: &str,
) {
    use tricerules_card_model::registry::RegistryError;

    match narset_phase_b_model_registry(raw) {
        Err(RegistryError::InvalidCard { id, reason }) => {
            assert_eq!(id, raw.id, "{case} must fail for the mutated card itself");
            assert!(
                reason.split_whitespace().count() >= 4,
                "{case} must report a meaningful InvalidCard reason, got {reason:?}"
            );
            let reason = reason.to_ascii_lowercase();
            assert!(
                [
                    "rebound",
                    "grantcapturedspell",
                    "trigger",
                    "target",
                    "program"
                ]
                .iter()
                .any(|marker| reason.contains(marker)),
                "{case} must identify the Rebound profile violation, got {reason:?}"
            );
        }
        Err(RegistryError::Ron(error)) => {
            panic!("{case} unexpectedly failed as a RON parse error: {error}")
        }
        Ok(_) => panic!("registry accepted invalid Rebound profile {case}"),
    }
}

fn narset_phase_b_activated_mut<'a>(
    raw: &'a mut tricerules_card_model::card_def::RawCardDefinition,
    ability_id: &str,
) -> &'a mut tricerules_card_model::primitives::ActivatedAbilityDef {
    raw.activated_abilities
        .iter_mut()
        .find(|ability| ability.ability_id.as_str() == ability_id)
        .unwrap_or_else(|| panic!("baseline has activated ability {ability_id}"))
}

fn narset_phase_b_triggered_mut<'a>(
    raw: &'a mut tricerules_card_model::card_def::RawCardDefinition,
    ability_id: &str,
) -> &'a mut tricerules_card_model::primitives::TriggeredAbilityDef {
    raw.triggered_abilities
        .iter_mut()
        .find(|ability| ability.ability_id.as_str() == ability_id)
        .unwrap_or_else(|| panic!("baseline has triggered ability {ability_id}"))
}

fn narset_phase_b_rebound_cast_trigger(
    caster: tricerules_card_model::primitives::CastTriggerPlayer,
    card_type: tricerules_card_model::primitives::CardTypeFilter,
) -> tricerules_card_model::primitives::TriggerCondition {
    tricerules_card_model::primitives::TriggerCondition::WheneverPlayerCastsSpell {
        caster,
        filter: tricerules_card_model::primitives::SpellCastFilter {
            card_type: Some(card_type),
            ..Default::default()
        },
        ordinal: None,
        ordinal_scope: tricerules_card_model::primitives::CastOrdinalScope::AllSpells,
    }
}

fn narset_phase_b_flat_grant_baseline() -> tricerules_card_model::card_def::RawCardDefinition {
    let mut raw = narset_phase_b_parse_model_baseline(NARSET_PHASE_B_MODEL_TRIGGERED_BASELINE);
    let ability = narset_phase_b_triggered_mut(&mut raw, "triggered_01");
    ability.trigger = narset_phase_b_rebound_cast_trigger(
        tricerules_card_model::primitives::CastTriggerPlayer::Controller,
        tricerules_card_model::primitives::CardTypeFilter::InstantOrSorcery,
    );
    ability.effect =
        vec![tricerules_card_model::primitives::SpellEffectKind::GrantCapturedSpellRebound];
    raw
}

#[test]
fn narset_phase_b_model_accepts_registration_effects_in_spell_activated_and_triggered_lists() {
    use tricerules_card_model::primitives::SpellEffectKind;

    let mut spell = narset_phase_b_parse_model_baseline(NARSET_PHASE_B_MODEL_SPELL_BASELINE);
    narset_phase_b_expect_model_admitted(&spell, "the ordinary spell baseline");
    spell.spell_effect = vec![SpellEffectKind::NextHandInstantOrSorceryGainsReboundThisTurn];
    narset_phase_b_expect_model_admitted(
        &spell,
        "NextHandInstantOrSorceryGainsReboundThisTurn in a spell effect list",
    );

    let mut activated =
        narset_phase_b_parse_model_baseline(NARSET_PHASE_B_MODEL_ACTIVATED_BASELINE);
    narset_phase_b_expect_model_admitted(&activated, "the ordinary activated baseline");
    narset_phase_b_activated_mut(&mut activated, "activated_01").effect =
        vec![SpellEffectKind::NextHandInstantOrSorceryGainsReboundThisTurn];
    narset_phase_b_expect_model_admitted(
        &activated,
        "NextHandInstantOrSorceryGainsReboundThisTurn in an activated effect list",
    );

    let mut triggered =
        narset_phase_b_parse_model_baseline(NARSET_PHASE_B_MODEL_TRIGGERED_BASELINE);
    narset_phase_b_expect_model_admitted(&triggered, "the ordinary triggered baseline");
    narset_phase_b_triggered_mut(&mut triggered, "triggered_01").effect =
        vec![SpellEffectKind::NextHandInstantOrSorceryGainsReboundThisTurn];
    narset_phase_b_expect_model_admitted(
        &triggered,
        "NextHandInstantOrSorceryGainsReboundThisTurn in a triggered effect list",
    );
}

#[test]
fn narset_phase_b_model_admits_only_the_flat_captured_spell_grant_profile() {
    use tricerules_card_model::card_def::{ModalDef, ModeDef};
    use tricerules_card_model::primitives::{
        AuthoredBlock, CardProgram, GameCondition, Instruction, InstructionKind, SpellEffectKind,
        TargetChooser, TargetGroupDef, TargetingDef, TriggerCondition,
    };

    let spell_baseline = narset_phase_b_parse_model_baseline(NARSET_PHASE_B_MODEL_SPELL_BASELINE);
    narset_phase_b_expect_model_admitted(&spell_baseline, "the ordinary spell baseline");
    let activated_baseline =
        narset_phase_b_parse_model_baseline(NARSET_PHASE_B_MODEL_ACTIVATED_BASELINE);
    narset_phase_b_expect_model_admitted(&activated_baseline, "the ordinary activated baseline");
    let triggered_baseline =
        narset_phase_b_parse_model_baseline(NARSET_PHASE_B_MODEL_TRIGGERED_BASELINE);
    narset_phase_b_expect_model_admitted(&triggered_baseline, "the ordinary triggered baseline");

    let grant = narset_phase_b_flat_grant_baseline();
    narset_phase_b_expect_model_admitted(
        &grant,
        "the flat, nonoptional, untargeted controller cast grant",
    );

    let mut bare_spell = spell_baseline.clone();
    bare_spell.spell_effect = vec![SpellEffectKind::GrantCapturedSpellRebound];
    narset_phase_b_expect_rebound_rejected(&bare_spell, "bare spell GrantCapturedSpellRebound");

    let mut bare_activated = activated_baseline.clone();
    narset_phase_b_activated_mut(&mut bare_activated, "activated_01").effect =
        vec![SpellEffectKind::GrantCapturedSpellRebound];
    narset_phase_b_expect_rebound_rejected(&bare_activated, "activated GrantCapturedSpellRebound");

    let mut wrong_trigger = grant.clone();
    narset_phase_b_triggered_mut(&mut wrong_trigger, "triggered_01").trigger =
        TriggerCondition::WhenSelfEntersBattlefield;
    narset_phase_b_expect_rebound_rejected(&wrong_trigger, "grant with a non-cast trigger");

    let mut wrong_caster = grant.clone();
    narset_phase_b_triggered_mut(&mut wrong_caster, "triggered_01").trigger =
        narset_phase_b_rebound_cast_trigger(
            tricerules_card_model::primitives::CastTriggerPlayer::Opponent,
            tricerules_card_model::primitives::CardTypeFilter::InstantOrSorcery,
        );
    narset_phase_b_expect_rebound_rejected(&wrong_caster, "grant with an opponent-cast trigger");

    let mut wrong_filter = grant.clone();
    narset_phase_b_triggered_mut(&mut wrong_filter, "triggered_01").trigger =
        narset_phase_b_rebound_cast_trigger(
            tricerules_card_model::primitives::CastTriggerPlayer::Controller,
            tricerules_card_model::primitives::CardTypeFilter::Creature,
        );
    narset_phase_b_expect_rebound_rejected(
        &wrong_filter,
        "grant with a non-Instant-or-Sorcery filter",
    );

    let mut optional = grant.clone();
    narset_phase_b_triggered_mut(&mut optional, "triggered_01").may = true;
    narset_phase_b_expect_rebound_rejected(&optional, "optional captured-spell grant");

    let mut targeted = grant.clone();
    narset_phase_b_triggered_mut(&mut targeted, "triggered_01").targeting = Some(TargetingDef {
        groups: vec![TargetGroupDef {
            chooser: TargetChooser::Controller,
            min: 1,
            max: 1,
            prompt: "Choose a target".into(),
            effect_indices: vec![0],
            distinct_from: Vec::new(),
            same_graveyard: false,
            cast_cost_expansion: None,
        }],
    });
    narset_phase_b_expect_rebound_rejected(&targeted, "targeted captured-spell grant");

    let mut modal = grant.clone();
    let modal_ability = narset_phase_b_triggered_mut(&mut modal, "triggered_01");
    modal_ability.effect.clear();
    modal_ability.modal = Some(ModalDef {
        min_modes: 1,
        max_modes: 1,
        all_modes_cast_cost: None,
        trigger_mode_selection_restriction: None,
        modes: vec![ModeDef {
            mode_id: tricerules_card_model::ModeId::new("grant_rebound")
                .expect("valid fixture mode id"),
            presentation: tricerules_card_model::AbilityPresentation::Fallback,
            linked_cast_cost: None,
            effects: vec![SpellEffectKind::GrantCapturedSpellRebound],
            program: None,
            targeting: None,
        }],
    });
    narset_phase_b_expect_rebound_rejected(&modal, "modal captured-spell grant");

    let mut nested = grant.clone();
    narset_phase_b_triggered_mut(&mut nested, "triggered_01").effect =
        vec![SpellEffectKind::Conditional {
            condition: GameCondition::ControllerLibraryEmpty,
            effect: Box::new(SpellEffectKind::GrantCapturedSpellRebound),
        }];
    narset_phase_b_expect_rebound_rejected(&nested, "nested conditional captured-spell grant");

    let mut programmed = grant;
    let program_ability = narset_phase_b_triggered_mut(&mut programmed, "triggered_01");
    program_ability.effect.clear();
    program_ability.program = Some(CardProgram {
        body: AuthoredBlock {
            steps: vec![Instruction {
                id: "grant_rebound_01".into(),
                bind: Vec::new(),
                input: None,
                kind: InstructionKind::Perform(Box::new(
                    SpellEffectKind::GrantCapturedSpellRebound,
                )),
            }],
            yields: Vec::new(),
        },
        target_bindings: Vec::new(),
        target_aliases: Vec::new(),
    });
    narset_phase_b_expect_rebound_rejected(&programmed, "Program captured-spell grant");
}
