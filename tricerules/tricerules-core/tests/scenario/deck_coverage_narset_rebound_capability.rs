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

#[test]
fn narset_phase_c1_granted_hand_opt_exits_to_exact_exile() {
    let (mut engine, source, source_generation) = narset_rebound_lifetime_fixture(6_110_116);
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

    let (controller, card_id, face_index, presentation) = {
        let item = engine
            .state
            .stack
            .iter()
            .find(|item| item.cast_occurrence == Some(opt))
            .expect("the qualifying hand Opt remains on the stack under its grant trigger");
        assert_eq!(item.card_id, "opt");
        let presentation = engine
            .state
            .stack_presentations
            .get(&item.id)
            .expect("the actual Opt has a captured stack presentation")
            .primary
            .clone();
        (
            item.controller,
            item.card_id.clone(),
            item.face_index,
            presentation,
        )
    };
    let physical_owner = engine.state.objects[&opt.object_id].owner;
    let card_name = narset_rebound_lifetime_registry()
        .get(&card_id)
        .expect("the actual Opt definition is registered")
        .name
        .clone();
    let original_stack_generation = opt
        .zone_change_generation
        .expect("the physical Opt has its exact stack-entry generation");

    pass_priority_round(&mut engine);
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(opt),
        "the ordinary grant trigger resolves before the granted Opt"
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, opt.object_id);

    pass_priority_round(&mut engine);
    let (scry_decider, scry_candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the actual Opt parks at its real Scry choice");
        assert_eq!(pending.presentation.source_object_id, opt.object_id);
        assert_eq!(
            pending.presentation.choice_kind,
            tricerules_proto::ruled::v1::ChoiceKind::LibraryTop
        );
        (
            pending.deciding_player,
            pending.presentation.candidates.clone(),
        )
    };
    let hand_count_before_draw = engine.state.players[0].hand.len();
    let completion = engine
        .apply_scenario_command(scry_decider, &submit_resolution_choice(scry_candidates))
        .expect("answer the actual Opt's Scry choice and finish its resolution");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_count_before_draw + 1,
        "the actual Opt completes its draw after the Scry choice"
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile,
        "the granted physical Opt exits the stack to exile"
    );

    let exile_generation = engine.state.zone_change_generation[&opt.object_id];
    assert_ne!(exile_generation, original_stack_generation);
    assert_eq!(engine.state.objects[&opt.object_id].owner, physical_owner);
    let resolved_opt = completion
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::StackResolved(resolved)) if resolved.object_id == opt.object_id => {
                Some(resolved)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        resolved_opt.len(),
        1,
        "the final choice batch resolves this Opt once"
    );
    assert_eq!(
        resolved_opt[0].destination,
        tricerules_proto::ruled::v1::StackResolveDestination::Exile as i32
    );
    assert_eq!(resolved_opt[0].owner_player_id, Some(physical_owner));

    let snapshot = engine
        .diagnostic_snapshot()
        .expect("serialize the resolved Opt's private delayed-rebound record");
    let scheduled = snapshot["state"]["scheduled_rebounds"]
        .as_array()
        .expect("diagnostics expose the private scheduled-rebound records");
    assert_eq!(scheduled.len(), 1, "the exact Opt schedules Rebound once");
    let record = &scheduled[0];
    assert_eq!(
        record["exiled"]["object_id"],
        serde_json::json!(opt.object_id)
    );
    assert_eq!(
        record["exiled"]["zone_change_generation"],
        serde_json::json!(exile_generation.to_string())
    );
    assert_eq!(
        record["source"]["object_id"],
        serde_json::json!(opt.object_id)
    );
    assert_eq!(
        record["source"]["zone_change_generation"],
        serde_json::json!(original_stack_generation.to_string())
    );
    assert_eq!(record["controller"], serde_json::json!(controller));
    assert_eq!(record["card_id"], serde_json::json!(card_id));
    assert_eq!(record["card_name"], serde_json::json!(card_name));
    assert_eq!(
        record["source_face_index"],
        serde_json::json!(face_index.to_string())
    );
    assert_eq!(
        record["presentation"],
        serde_json::to_value(&presentation).expect("serialize the captured Opt presentation")
    );
    assert!(record.get("owner").is_none());
}

const NARSET_C1_OMEN_CARD_ID: &str = "dirgur_island_dragon_skimming_strike";
const NARSET_C1_ADVENTURE_CARD_ID: &str = "bonecrusher_giant_stomp";
const NARSET_C1_REBOUND_SUMMARY: &str = "Exile it with Rebound.";
const NARSET_C1_OMEN_SUMMARY: &str = "Shuffle it into its owner's library.";
const NARSET_C1_ADVENTURE_SUMMARY: &str = "Exile it with its Adventure permission.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NarsetC1CompetingSpell {
    Omen,
    Adventure,
}

impl NarsetC1CompetingSpell {
    fn card_id(self) -> &'static str {
        match self {
            Self::Omen => NARSET_C1_OMEN_CARD_ID,
            Self::Adventure => NARSET_C1_ADVENTURE_CARD_ID,
        }
    }

    fn printed_exit_summary(self) -> &'static str {
        match self {
            Self::Omen => NARSET_C1_OMEN_SUMMARY,
            Self::Adventure => NARSET_C1_ADVENTURE_SUMMARY,
        }
    }

    fn mana_gift(self) -> ManaGift {
        match self {
            Self::Omen => ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            Self::Adventure => ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        }
    }

    fn targets(self, engine: &GameEngine) -> Vec<TargetRef> {
        match self {
            Self::Omen => Vec::new(),
            Self::Adventure => target_player(engine.state.players[1].id),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NarsetC1SelectedExit {
    Printed,
    Rebound,
}

fn narset_rebound_stack_exit_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(
            &[
                REBOUND_OBSERVER_RON,
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!(
                    "../../../tricerules-cards/data/multiface/dirgur_island_dragon_skimming_strike.ron"
                ),
                include_str!(
                    "../../../tricerules-cards/data/multiface/bonecrusher_giant_stomp.ron"
                ),
            ],
            &[],
        )
        .expect("the synthetic observer and actual Omen/Adventure fixtures register")
    })
}

fn narset_rebound_stack_exit_fixture(seed: u64) -> (GameEngine, u32, u64) {
    let decks = Some(vec![
        deck_with(
            "opt",
            &[
                REBOUND_OBSERVER,
                NARSET_C1_OMEN_CARD_ID,
                NARSET_C1_ADVENTURE_CARD_ID,
            ],
        ),
        deck_with("opt", &[]),
    ]);
    let mut engine = GameEngine::new(
        narset_rebound_stack_exit_registry(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Narset C1 stack-exit competition fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = move_fixture_card_to_zone(&mut engine, 0, REBOUND_OBSERVER, DevZone::Battlefield);
    let source_generation = engine.state.zone_change_generation[&source];
    (engine, source, source_generation)
}

fn narset_c1_complete_ungranted_opt_scry(engine: &mut GameEngine) -> u64 {
    assert_eq!(serialized_observer_count(engine), 0);
    let opt = cast_hand_card(
        engine,
        0,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_eq!(serialized_rebound_grants(engine), serde_json::json!([]));
    pass_priority_round(engine);

    let (decider, prompt_id, candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the ungranted actual Opt reaches its real Scry choice");
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::LibraryTop,
            "the stale nonce comes from a completed actual Opt Scry"
        );
        (
            pending.deciding_player,
            pending.resolution_prompt_id.get(),
            pending.presentation.candidates.clone(),
        )
    };
    assert_ne!(
        prompt_id, 0,
        "the completed Scry has a genuine prompt nonce"
    );
    let mut answer = submit_resolution_choice(candidates);
    match answer.cmd.as_mut() {
        Some(Cmd::SubmitResolutionChoice(answer)) => answer.resolution_prompt_id = prompt_id,
        _ => panic!("the Scry answer is a resolution-choice command"),
    }
    engine
        .apply_command(decider, &answer)
        .expect("answer the genuine Scry prompt before arming the observer");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Graveyard,
        "the actual ungranted Opt finishes before the later terminal prompt"
    );
    assert_eq!(serialized_rebound_grants(engine), serde_json::json!([]));
    prompt_id
}

fn narset_c1_cast_competing_spell(
    engine: &mut GameEngine,
    spell: NarsetC1CompetingSpell,
) -> (tricerules_core::state::StackObjectRef, RuledEventBatch) {
    let player_id = engine.state.players[0].id;
    let card_id = spell.card_id();
    ensure_in_hand(engine, 0, card_id);
    give_mana(engine, player_id, spell.mana_gift());
    let hand_index = hand_index_for_card(engine, 0, card_id);
    let mut command = cast_spell_face(hand_index, spell.targets(engine), 1);
    authoring_actions::pay(engine, player_id, &mut command)
        .expect("pay the actual face cast from the current engine offer");
    let batch = engine
        .apply_scenario_command(player_id, &command)
        .expect("commit the actual Omen or Adventure face from hand");
    let occurrence = engine
        .state
        .stack
        .iter()
        .find(|item| {
            item.card_id == card_id
                && item.face_index == 1
                && item.cast_by == Some(player_id)
                && !item.is_triggered
                && !item.is_copy
        })
        .and_then(|item| item.cast_occurrence)
        .expect("the actual printed face has its committed physical cast occurrence");
    let matching_stack_pushes = batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(Ev::StackPushed(pushed)) if pushed.object_id == occurrence.object_id
            )
        })
        .count();
    assert_eq!(
        matching_stack_pushes, 1,
        "the face cast publishes its original StackPushed event exactly once"
    );
    (occurrence, batch)
}

fn narset_c1_capture_priority_round(engine: &mut GameEngine) -> RuledEventBatch {
    let active_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let passes_needed = active_players
        .checked_sub(engine.state.passes_since_stack_change as usize)
        .expect("the fixture has a consistent priority pass count");
    let mut last_batch = None;
    for _ in 0..passes_needed {
        let actor = engine.state.priority_player_id();
        last_batch = Some(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("the current priority holder passes to resolve the actual spell"),
        );
    }
    last_batch.expect("the current priority round has an unpassed player")
}

fn narset_c1_application_id(
    choice: &tricerules_proto::ruled::v1::ResolutionChoiceRequired,
    summary: &str,
    card_name: &str,
    source_object_id: u32,
    source_generation: u64,
) -> u32 {
    let matches = choice
        .replacement_options
        .iter()
        .filter(|option| {
            option.effect_summary == summary
                && option.source_card_name == card_name
                && option.source_object_id == source_object_id
                && option.source_zone_change_generation == source_generation
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "one published option matches its full receipt"
    );
    matches[0].application_id
}

fn narset_c1_assert_terminal_prompt(
    engine: &GameEngine,
    batch: &RuledEventBatch,
    spell: NarsetC1CompetingSpell,
    occurrence: tricerules_core::state::StackObjectRef,
) -> (u64, u32, u32) {
    let card_id = spell.card_id();
    let controller = engine.state.players[0].id;
    let card_name = narset_rebound_stack_exit_registry()
        .get(card_id)
        .expect("the actual face's card definition is registered")
        .name
        .clone();
    let stack_generation = occurrence
        .zone_change_generation
        .expect("the physical stack occurrence retains its generation");

    let prompt_events = batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        prompt_events.len(),
        1,
        "one terminal replacement prompt is published"
    );
    let choice = prompt_events[0];
    assert_eq!(choice.choice_kind, ChoiceKind::ReplacementEffect as i32);
    assert_eq!(choice.source_object_id, occurrence.object_id);
    assert_eq!(choice.deciding_player_id, controller);
    assert_eq!(choice.min, 1);
    assert_eq!(choice.max, 1);
    assert_eq!(choice.replacement_options.len(), 2);
    assert_eq!(
        choice.candidate_object_ids,
        choice
            .replacement_options
            .iter()
            .map(|option| option.application_id)
            .collect::<Vec<_>>(),
        "the candidate IDs are the published application IDs in the prompt"
    );
    assert_eq!(
        choice.candidate_names,
        choice
            .replacement_options
            .iter()
            .map(|option| format!("{} — {}", option.source_card_name, option.effect_summary))
            .collect::<Vec<_>>(),
        "the public names describe the same published replacement options"
    );
    for option in &choice.replacement_options {
        assert_eq!(option.source_card_name, card_name);
        assert_eq!(option.source_object_id, occurrence.object_id);
        assert_eq!(option.source_zone_change_generation, stack_generation);
    }
    let printed_application_id = narset_c1_application_id(
        choice,
        spell.printed_exit_summary(),
        &card_name,
        occurrence.object_id,
        stack_generation,
    );
    let rebound_application_id = narset_c1_application_id(
        choice,
        NARSET_C1_REBOUND_SUMMARY,
        &card_name,
        occurrence.object_id,
        stack_generation,
    );
    assert_ne!(printed_application_id, rebound_application_id);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the terminal stack-exit continuation remains parked");
    assert!(matches!(
        &pending.continuation,
        ResolutionContinuation::StackExitReplacement { .. }
    ));
    assert_eq!(
        pending.resolution_prompt_id.get(),
        choice.resolution_prompt_id
    );
    assert_ne!(pending.resolution_prompt_id.get(), 0);
    assert_eq!(pending.deciding_player, controller);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(pending.presentation.source_object_id, occurrence.object_id);
    assert_eq!(pending.presentation.candidates, choice.candidate_object_ids);
    let parked = pending
        .continuation
        .stack()
        .expect("the public continuation accessor exposes the exact parked stack receipt");
    assert_eq!(parked.item.id, occurrence.object_id);
    assert_eq!(parked.item.cast_occurrence, Some(occurrence));
    assert_eq!(parked.item.card_id, card_id);
    assert_eq!(parked.item.face_index, 1);
    assert_eq!(parked.item.controller, controller);
    assert_eq!(parked.item.cast_by, Some(controller));
    assert!(!parked.item.is_copy);
    assert_eq!(
        engine.state.objects[&occurrence.object_id].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.zone_change_generation[&occurrence.object_id],
        stack_generation
    );
    assert_eq!(
        serialized_rebound_grants(engine),
        expected_rebound_grants_for_cast(occurrence),
        "the Rebound grant remains live until the exact stack exit commits"
    );
    assert!(
        engine
            .state
            .active_exile_play_permissions
            .iter()
            .all(|permission| permission.object_id != occurrence.object_id),
        "neither competing permission is committed while the prompt is pending"
    );
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("diagnostics retain the uncommitted terminal state");
    assert!(snapshot["state"]["scheduled_rebounds"]
        .as_array()
        .expect("diagnostics expose scheduled Rebound records")
        .is_empty());
    assert!(
        batch.events.iter().all(|event| {
            !matches!(
                &event.ev,
                Some(Ev::StackResolved(resolved)) if resolved.object_id == occurrence.object_id
            )
        }),
        "the terminal spell has no premature StackResolved payload"
    );

    (
        pending.resolution_prompt_id.get(),
        printed_application_id,
        rebound_application_id,
    )
}

fn narset_c1_replacement_command(
    prompt_id: u64,
    chosen_application_ids: Vec<u32>,
    chosen_player_ids: Vec<i32>,
) -> RuledCommand {
    let mut command = submit_resolution_choice(chosen_application_ids);
    match command.cmd.as_mut() {
        Some(Cmd::SubmitResolutionChoice(answer)) => {
            answer.resolution_prompt_id = prompt_id;
            answer.chosen_player_ids = chosen_player_ids;
        }
        _ => panic!("the replacement answer is a resolution-choice command"),
    }
    command
}

fn narset_c1_assert_rejected_answer_is_atomic(
    engine: &mut GameEngine,
    actor: i32,
    command: &RuledCommand,
) {
    let before = engine
        .diagnostic_snapshot()
        .expect("snapshot the complete private fixture state before a rejected answer");
    let command_index = engine.state.command_index;
    let prompt_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the terminal choice remains pending")
        .resolution_prompt_id
        .get();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot state after the rejected answer"),
        before,
        "a rejected answer leaves the full diagnostic state unchanged"
    );
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the rejected answer leaves the prompt pending")
            .resolution_prompt_id
            .get(),
        prompt_id,
        "a rejected answer preserves the exact prompt nonce"
    );
}

fn narset_c1_assert_atomic_answer_rejections(
    engine: &mut GameEngine,
    old_prompt_id: u64,
    current_prompt_id: u64,
    valid_application_id: u32,
) {
    let controller = engine.state.players[0].id;
    let opponent = engine.state.players[1].id;
    assert_ne!(old_prompt_id, 0);
    assert_ne!(current_prompt_id, 0);
    assert_ne!(old_prompt_id, current_prompt_id);

    narset_c1_assert_rejected_answer_is_atomic(
        engine,
        opponent,
        &narset_c1_replacement_command(current_prompt_id, vec![valid_application_id], Vec::new()),
    );
    narset_c1_assert_rejected_answer_is_atomic(
        engine,
        controller,
        &narset_c1_replacement_command(old_prompt_id, vec![valid_application_id], Vec::new()),
    );

    let options = &engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the current replacement prompt remains pending")
        .presentation
        .candidates;
    let unknown_application_id = (1..)
        .find(|candidate| !options.contains(candidate))
        .expect("the published application IDs leave an unknown ID available");
    narset_c1_assert_rejected_answer_is_atomic(
        engine,
        controller,
        &narset_c1_replacement_command(current_prompt_id, vec![unknown_application_id], Vec::new()),
    );
    narset_c1_assert_rejected_answer_is_atomic(
        engine,
        controller,
        &narset_c1_replacement_command(
            current_prompt_id,
            vec![valid_application_id, valid_application_id],
            Vec::new(),
        ),
    );
    narset_c1_assert_rejected_answer_is_atomic(
        engine,
        controller,
        &narset_c1_replacement_command(
            current_prompt_id,
            vec![valid_application_id],
            vec![opponent],
        ),
    );
}

fn narset_c1_run_stack_exit_competition(
    seed: u64,
    spell: NarsetC1CompetingSpell,
    selected: NarsetC1SelectedExit,
    check_atomic_answers: bool,
) {
    let (mut engine, source, source_generation) = narset_rebound_stack_exit_fixture(seed);
    let old_prompt_id =
        check_atomic_answers.then(|| narset_c1_complete_ungranted_opt_scry(&mut engine));
    arm_narset_observer(&mut engine, source);

    let (occurrence, _cast_batch) = narset_c1_cast_competing_spell(&mut engine, spell);
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, occurrence);
    pass_priority_round(&mut engine);
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(occurrence),
        "the ordinary grant trigger resolves before the actual Omen or Adventure spell"
    );

    let hand_before_effect = engine.state.players[0].hand.len();
    let library_before_effect = engine.state.players[0].library.len();
    let library_top_before_effect = (spell == NarsetC1CompetingSpell::Omen).then(|| {
        *engine.state.players[0]
            .library
            .front()
            .expect("the actual Omen Draw has a physical library card")
    });
    let opponent_life_before_effect = engine.state.players[1].life;
    let terminal_batch = narset_c1_capture_priority_round(&mut engine);
    match spell {
        NarsetC1CompetingSpell::Omen => {
            let drawn = library_top_before_effect.expect("Omen recorded the exact top card");
            assert_eq!(engine.state.players[0].hand.len(), hand_before_effect + 1);
            assert_eq!(
                engine.state.players[0].library.len(),
                library_before_effect - 1
            );
            assert_eq!(
                engine.state.objects[&drawn].zone,
                tricerules_core::Zone::Hand
            );
            assert!(engine.state.players[0].hand.contains(&drawn));
        }
        NarsetC1CompetingSpell::Adventure => {
            assert_eq!(
                engine.state.players[1].life,
                opponent_life_before_effect - 2,
                "the actual Stomp face deals its 2 damage before the terminal choice"
            );
        }
    }
    let (prompt_id, printed_application_id, rebound_application_id) =
        narset_c1_assert_terminal_prompt(&engine, &terminal_batch, spell, occurrence);
    if let Some(old_prompt_id) = old_prompt_id {
        assert_ne!(old_prompt_id, prompt_id);
    }

    let selected_application_id = match selected {
        NarsetC1SelectedExit::Printed => printed_application_id,
        NarsetC1SelectedExit::Rebound => rebound_application_id,
    };
    if check_atomic_answers {
        narset_c1_assert_atomic_answer_rejections(
            &mut engine,
            old_prompt_id.expect("the atomic case completed the prior ungranted Scry"),
            prompt_id,
            selected_application_id,
        );
    }
    let valid_answer =
        narset_c1_replacement_command(prompt_id, vec![selected_application_id], Vec::new());
    let controller = engine.state.players[0].id;
    let physical_owner = engine.state.objects[&occurrence.object_id].owner;
    let hand_at_commit = engine.state.players[0].hand.len();
    let life_at_commit = engine.state.players[1].life;
    let stack_generation = occurrence
        .zone_change_generation
        .expect("the spell is bound to its exact stack generation");
    let presentation = engine
        .state
        .stack_presentations
        .get(&occurrence.object_id)
        .expect("the actual spell retains its stack presentation while parked")
        .primary
        .clone();
    let committed = engine
        .apply_scenario_command(controller, &valid_answer)
        .expect("the current deciding player commits the selected published application ID");

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine
        .state
        .stack
        .iter()
        .all(|item| item.id != occurrence.object_id));
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_at_commit,
        "committing the terminal replacement does not repeat an earlier draw"
    );
    assert_eq!(
        engine.state.players[1].life, life_at_commit,
        "committing the terminal replacement does not repeat Stomp damage"
    );
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
    let exile_selected =
        selected == NarsetC1SelectedExit::Rebound || spell == NarsetC1CompetingSpell::Adventure;
    let expected_zone = if exile_selected {
        tricerules_core::Zone::Exile
    } else {
        tricerules_core::Zone::Library
    };
    assert_eq!(
        engine.state.objects[&occurrence.object_id].zone,
        expected_zone
    );
    assert_eq!(
        engine.state.objects[&occurrence.object_id].owner,
        physical_owner
    );
    assert_eq!(
        physical_owner, controller,
        "the selected stack exit preserves the physical owner's identity"
    );
    let new_generation = engine.state.zone_change_generation[&occurrence.object_id];
    assert_eq!(
        new_generation,
        stack_generation + 1,
        "the exact physical card makes one stack-to-destination zone change"
    );
    let owner_index = engine
        .state
        .player_idx(physical_owner)
        .expect("the physical owner remains in the game");
    match expected_zone {
        tricerules_core::Zone::Library => {
            assert_eq!(
                engine.state.players[owner_index]
                    .library
                    .iter()
                    .filter(|object_id| **object_id == occurrence.object_id)
                    .count(),
                1,
                "the exact Omen card is placed in its owner's library once"
            );
            assert_eq!(
                engine.state.players[0].library.len(),
                library_before_effect,
                "the Omen draw is followed by one card returning to its library"
            );
        }
        tricerules_core::Zone::Exile => {
            assert_eq!(
                engine.state.players[owner_index]
                    .exile
                    .iter()
                    .filter(|object_id| **object_id == occurrence.object_id)
                    .count(),
                1,
                "the exact physical card is placed in exile once"
            );
        }
        _ => unreachable!("C1 stack exits only choose Library or Exile"),
    }

    let resolved = committed
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::StackResolved(resolved)) if resolved.object_id == occurrence.object_id => {
                Some(resolved)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(resolved.len(), 1, "the exact stack payload commits once");
    assert_eq!(
        resolved[0].destination,
        match expected_zone {
            tricerules_core::Zone::Library => {
                tricerules_proto::ruled::v1::StackResolveDestination::Library as i32
            }
            tricerules_core::Zone::Exile => {
                tricerules_proto::ruled::v1::StackResolveDestination::Exile as i32
            }
            _ => unreachable!("C1 stack exits only choose Library or Exile"),
        }
    );
    assert_eq!(resolved[0].owner_player_id, Some(physical_owner));
    let expected_omen_shuffle =
        spell == NarsetC1CompetingSpell::Omen && selected == NarsetC1SelectedExit::Printed;
    let shuffle_log = format!(
        "P{} shuffles the resolving Omen into P{}'s library.",
        controller, physical_owner
    );
    assert_eq!(
        committed
            .events
            .iter()
            .filter(|event| { matches!(&event.ev, Some(Ev::Log(log)) if log.text == shuffle_log) })
            .count(),
        if expected_omen_shuffle { 1 } else { 0 },
        "only the selected Omen replacement shuffles the physical card once"
    );

    let card_id = spell.card_id();
    let card_name = narset_rebound_stack_exit_registry()
        .get(card_id)
        .expect("the actual card definition remains registered")
        .name
        .clone();
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("serialize exact post-commit Rebound and permission receipts");
    let scheduled = snapshot["state"]["scheduled_rebounds"]
        .as_array()
        .expect("diagnostics expose scheduled Rebound records");
    if selected == NarsetC1SelectedExit::Rebound {
        assert_eq!(scheduled.len(), 1, "the selected Rebound schedules once");
        let record = &scheduled[0];
        assert_eq!(
            record["source"]["object_id"],
            serde_json::json!(occurrence.object_id)
        );
        assert_eq!(
            record["source"]["zone_change_generation"],
            serde_json::json!(stack_generation.to_string())
        );
        assert_eq!(
            record["source"]["controller_at_event"],
            serde_json::json!(controller)
        );
        assert_eq!(
            record["exiled"]["object_id"],
            serde_json::json!(occurrence.object_id)
        );
        assert_eq!(
            record["exiled"]["zone_change_generation"],
            serde_json::json!(new_generation.to_string())
        );
        assert_eq!(
            record["exiled"]["controller_at_event"],
            serde_json::json!(controller)
        );
        assert_eq!(record["controller"], serde_json::json!(controller));
        assert_eq!(record["card_id"], serde_json::json!(card_id));
        assert_eq!(record["card_name"], serde_json::json!(card_name));
        assert_eq!(record["source_face_index"], serde_json::json!("1"));
        assert_eq!(
            record["presentation"],
            serde_json::to_value(&presentation)
                .expect("serialize the actual printed face's captured stack presentation")
        );
    } else {
        assert!(scheduled.is_empty(), "the printed exit suppresses Rebound");
    }

    let permissions = engine
        .state
        .active_exile_play_permissions
        .iter()
        .filter(|permission| permission.object_id == occurrence.object_id)
        .cloned()
        .collect::<Vec<_>>();
    let current = engine.initial_response_batch();
    let legal = current
        .legal_by_player
        .get(&controller)
        .expect("the spell's controller has a current legal-action view");
    let zone_casts = legal
        .zone_cast_actions
        .iter()
        .filter(|action| action.object_id == occurrence.object_id)
        .collect::<Vec<_>>();
    if spell == NarsetC1CompetingSpell::Adventure && selected == NarsetC1SelectedExit::Printed {
        assert_eq!(
            permissions.len(),
            1,
            "Adventure grants exactly one play permission"
        );
        let permission = &permissions[0];
        assert_eq!(permission.player_id, controller);
        assert_eq!(permission.source_label, card_name);
        assert_eq!(permission.zone_change_generation, new_generation);
        assert_eq!(
            permission.scope,
            tricerules_core::state::ExilePlayPermissionScope::CastFace(0)
        );
        assert_eq!(
            permission.cast_cost,
            tricerules_core::state::ExilePermissionCastCost::PrintedManaCost
        );
        assert_eq!(
            permission.origin,
            tricerules_core::state::ExilePlayPermissionOrigin::Effect
        );
        assert_eq!(permission.available_after_turn_instance, None);
        assert_eq!(permission.expires_at_cleanup_turn_instance, None);
        assert_eq!(
            zone_casts.len(),
            1,
            "the current legal view offers the Adventure card"
        );
        assert_eq!(
            zone_casts[0].source_zone,
            tricerules_proto::ruled::v1::CastSourceZone::Exile as i32
        );
        assert_eq!(zone_casts[0].face_index, 0);
        let adventure_face_name = narset_rebound_stack_exit_registry()
            .get(card_id)
            .expect("the Adventure definition is registered")
            .face(0)
            .expect("Bonecrusher Giant is the Adventure card's permanent face")
            .name
            .clone();
        assert_eq!(zone_casts[0].card_name, adventure_face_name);
        assert_eq!(zone_casts[0].cost, "{2}{R}");
        assert_eq!(
            zone_casts[0].casting_permission_id,
            Some(permission.group_id)
        );
    } else {
        assert!(
            permissions.is_empty(),
            "no unselected Adventure permission is created"
        );
        assert!(
            zone_casts.is_empty(),
            "the current legal view has no cast offer for this exited spell"
        );
    }

    if check_atomic_answers {
        let before_replay = engine
            .diagnostic_snapshot()
            .expect("snapshot committed state before replaying the retired answer");
        let command_index = engine.state.command_index;
        assert!(engine.apply_command(controller, &valid_answer).is_err());
        assert_eq!(
            engine
                .diagnostic_snapshot()
                .expect("snapshot state after replaying the retired answer"),
            before_replay,
            "replaying the committed answer has no duplicate move or payload"
        );
        assert_eq!(engine.state.command_index, command_index);
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn narset_phase_c1_omen_can_choose_its_library_exit_over_rebound_atomically() {
    narset_c1_run_stack_exit_competition(
        6_110_201,
        NarsetC1CompetingSpell::Omen,
        NarsetC1SelectedExit::Printed,
        true,
    );
}

#[test]
fn narset_phase_c1_rebound_can_replace_omen_library_exit() {
    narset_c1_run_stack_exit_competition(
        6_110_202,
        NarsetC1CompetingSpell::Omen,
        NarsetC1SelectedExit::Rebound,
        false,
    );
}

#[test]
fn narset_phase_c1_adventure_can_choose_its_permission_over_rebound() {
    narset_c1_run_stack_exit_competition(
        6_110_203,
        NarsetC1CompetingSpell::Adventure,
        NarsetC1SelectedExit::Printed,
        false,
    );
}

#[test]
fn narset_phase_c1_rebound_can_replace_adventure_permission() {
    narset_c1_run_stack_exit_competition(
        6_110_204,
        NarsetC1CompetingSpell::Adventure,
        NarsetC1SelectedExit::Rebound,
        false,
    );
}

// These focused exclusions exercise actual granted spells and one separate, model-admitted
// non-hand grant provider. They intentionally stop at engine integration evidence.
const NARSET_C1_EXCLUSION_PROVIDER_ID: &str = "narset_rebound_model_triggered_fixture";

fn narset_c1_exclusion_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let grant_provider = ron::ser::to_string(&narset_phase_b_flat_grant_baseline())
            .expect("the already-admitted flat grant provider serializes");
        CardRegistry::from_chunks_and_tokens(
            &[
                REBOUND_OBSERVER_RON,
                grant_provider.as_str(),
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!("../../../tricerules-cards/data/counterspell.ron"),
                include_str!("../../../tricerules-cards/data/lightning_bolt.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/twincast.ron"),
                include_str!("../../../tricerules-cards/data/ascend_from_avernus.ron"),
                include_str!("../../../tricerules-cards/data/generated/t/think_twice.ron"),
                include_str!(
                    "../../../tricerules-cards/data/multiface/dirgur_island_dragon_skimming_strike.ron"
                ),
            ],
            &[],
        )
        .expect("the admitted synthetic providers and actual exclusion cards register")
    })
}

fn narset_c1_exclusion_fixture(seed: u64) -> (GameEngine, u32, u64) {
    let decks = Some(vec![
        deck_with(
            "opt",
            &[
                REBOUND_OBSERVER,
                "opt",
                NARSET_C1_OMEN_CARD_ID,
                "grizzly_bears",
                "twincast",
                "ascend_from_avernus",
                "think_twice",
            ],
        ),
        deck_with("opt", &["counterspell", "lightning_bolt"]),
    ]);
    let mut engine = GameEngine::new(
        narset_c1_exclusion_registry(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Narset C1 exclusion fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = narset_c1_exclusion_move_card_to_zone(
        &mut engine,
        0,
        REBOUND_OBSERVER,
        DevZone::Battlefield,
    );
    let source_generation = engine.state.zone_change_generation[&source];
    (engine, source, source_generation)
}

fn narset_c1_exclusion_zone_objects(engine: &GameEngine, player: usize, zone: DevZone) -> Vec<u32> {
    match zone {
        DevZone::Battlefield => engine.state.players[player].battlefield.clone(),
        DevZone::Graveyard => engine.state.players[player].graveyard.clone(),
        DevZone::Exile => engine.state.players[player].exile.clone(),
        _ => panic!("unsupported Narset C1 fixture zone: {zone:?}"),
    }
}

fn narset_c1_exclusion_move_card_to_zone(
    engine: &mut GameEngine,
    player: usize,
    card_id: &str,
    zone: DevZone,
) -> u32 {
    let player_id = engine.state.players[player].id;
    let card_name = narset_c1_exclusion_registry()
        .get(card_id)
        .expect("fixture card is registered in the exclusion registry")
        .name
        .clone();
    let before = narset_c1_exclusion_zone_objects(engine, player, zone);
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
        .expect("move the exact registered fixture card through the engine");
    narset_c1_exclusion_zone_objects(engine, player, zone)
        .into_iter()
        .find(|object_id| !before.contains(object_id))
        .expect("the exact fixture card entered the requested zone")
}

fn narset_c1_cast_actual_hand_spell(
    engine: &mut GameEngine,
    player: usize,
    card_id: &str,
    face_index: u32,
    x_value: u32,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
    mana: ManaGift,
) -> (tricerules_core::state::StackObjectRef, RuledEventBatch) {
    let player_id = engine.state.players[player].id;
    let face_index_usize =
        usize::try_from(face_index).expect("the test face index fits the host index type");
    ensure_in_hand(engine, player, card_id);
    give_mana(engine, player_id, mana);
    let hand_index = hand_index_for_card(engine, player, card_id);
    let mut command = if face_index == 0 {
        cast_spell_x(hand_index, targets, x_value)
    } else {
        cast_spell_face(hand_index, targets, face_index)
    };
    match command.cmd.as_mut() {
        Some(Cmd::CastSpell(cast)) => {
            cast.x_value = x_value;
            assert_eq!(cast.face_index, face_index);
        }
        _ => panic!("the actual hand cast is a CastSpell command"),
    }
    let old_stack_ids = engine
        .state
        .stack
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    authoring_actions::pay(engine, player_id, &mut command)
        .expect("pay the actual hand spell from the current engine offer");
    let batch = engine
        .apply_scenario_command(player_id, &command)
        .expect("commit the actual hand spell through the engine");
    let occurrence = engine
        .state
        .stack
        .iter()
        .find(|item| {
            !old_stack_ids.contains(&item.id)
                && item.card_id == card_id
                && item.face_index == face_index_usize
                && item.cast_by == Some(player_id)
                && !item.is_triggered
                && !item.is_copy
        })
        .and_then(|item| item.cast_occurrence)
        .expect("the actual hand spell has its exact physical cast occurrence");
    let pushes = batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(Ev::StackPushed(pushed)) if pushed.object_id == occurrence.object_id
            )
        })
        .count();
    assert_eq!(
        pushes, 1,
        "the actual physical cast publishes one StackPushed"
    );
    (occurrence, batch)
}

fn narset_c1_assert_one_stack_resolved(
    batch: &RuledEventBatch,
    object_id: u32,
    owner: i32,
    destination: tricerules_proto::ruled::v1::StackResolveDestination,
) {
    let resolved = batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::StackResolved(resolved)) => Some(resolved),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(resolved.len(), 1, "one physical stack payload resolves");
    assert_eq!(resolved[0].object_id, object_id);
    assert_eq!(resolved[0].destination, destination as i32);
    assert_eq!(resolved[0].owner_player_id, Some(owner));
}

fn narset_c1_exclusion_scheduled_rebounds(engine: &GameEngine) -> serde_json::Value {
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("serialize private scheduled-Rebound evidence");
    snapshot["state"]["scheduled_rebounds"].clone()
}

fn narset_c1_assert_no_scheduled_rebounds(engine: &GameEngine, context: &str) {
    let scheduled = narset_c1_exclusion_scheduled_rebounds(engine);
    assert!(
        scheduled.is_array(),
        "diagnostics expose scheduled Rebound records"
    );
    assert_eq!(
        scheduled,
        serde_json::json!([]),
        "{context} schedules no Rebound"
    );
}

#[test]
fn narset_c1_actual_counterspell_clears_granted_opt_without_rebound_schedule() {
    let (mut engine, source, source_generation) = narset_rebound_lifetime_fixture(6_110_301);
    arm_narset_observer(&mut engine, source);
    let (opt, opt_cast_batch) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        0,
        "opt",
        0,
        0,
        Vec::new(),
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, opt);
    assert_eq!(
        opt_cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(_))))
            .count(),
        2,
        "the physical Opt and its ordinary grant trigger are each pushed once"
    );
    narset_c1_capture_priority_round(&mut engine);
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(opt),
        "the grant is live on the exact physical Opt before Counterspell resolves"
    );

    let controller = engine.state.players[0].id;
    engine
        .apply_scenario_command(controller, &pass())
        .expect("pass priority so the opponent can counter the granted Opt");
    let (counterspell, counterspell_cast_batch) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        1,
        "counterspell",
        0,
        0,
        vec![stack_target_for_cast(opt)],
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    assert_eq!(
        counterspell_cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(_))))
            .count(),
        1,
        "Counterspell adds one physical stack object and no grant trigger"
    );
    assert_eq!(engine.state.stack.len(), 2);
    assert!(engine.state.stack.iter().any(|item| {
        item.id == opt.object_id && item.cast_occurrence == Some(opt) && !item.is_copy
    }));
    assert!(engine.state.stack.iter().any(|item| {
        item.id == counterspell.object_id
            && item.cast_occurrence == Some(counterspell)
            && !item.is_copy
    }));
    let completion = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &completion,
        counterspell.object_id,
        engine.state.players[1].id,
        tricerules_proto::ruled::v1::StackResolveDestination::Graveyard,
    );
    assert_eq!(
        completion
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(Ev::StackObjectCountered(countered)) => Some(countered.object_id),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![opt.object_id],
        "Counterspell counters exactly the granted physical Opt once"
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].owner,
        engine.state.players[0].id
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt.object_id],
        opt.zone_change_generation.expect("Opt stack generation") + 1
    );
    assert_eq!(
        engine.state.objects[&counterspell.object_id].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&counterspell.object_id].owner,
        engine.state.players[1].id
    );
    assert_eq!(
        engine.state.zone_change_generation[&counterspell.object_id],
        counterspell
            .zone_change_generation
            .expect("Counterspell stack generation")
            + 1
    );
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
    narset_c1_assert_no_scheduled_rebounds(&engine, "countering the granted Opt");
}

#[test]
fn narset_c1_granted_skimming_strike_fizzles_after_actual_bolt_kills_its_target() {
    let (mut engine, source, source_generation) = narset_c1_exclusion_fixture(6_110_302);
    let target = narset_c1_exclusion_move_card_to_zone(
        &mut engine,
        0,
        "grizzly_bears",
        DevZone::Battlefield,
    );
    arm_narset_observer(&mut engine, source);
    ensure_in_hand(&mut engine, 0, NARSET_C1_OMEN_CARD_ID);
    let hand_before_cast = engine.state.players[0].hand.len();
    let library_before_cast = engine.state.players[0].library.len();
    let (omen, _) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        0,
        NARSET_C1_OMEN_CARD_ID,
        1,
        0,
        target_object(target),
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, omen);
    narset_c1_capture_priority_round(&mut engine);
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(omen),
        "the actual Skimming Strike has a live Rebound grant before it resolves"
    );

    let controller = engine.state.players[0].id;
    engine
        .apply_scenario_command(controller, &pass())
        .expect("pass priority so the opponent can kill the actual target");
    let (bolt, _) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        1,
        "lightning_bolt",
        0,
        0,
        target_object(target),
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_completion = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &bolt_completion,
        bolt.object_id,
        engine.state.players[1].id,
        tricerules_proto::ruled::v1::StackResolveDestination::Graveyard,
    );
    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Graveyard,
        "Lightning Bolt removes Skimming Strike's sole creature target first"
    );
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(omen),
        "the exact grant stays live until the now-illegal Omen exits the stack"
    );

    let omen_completion = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &omen_completion,
        omen.object_id,
        engine.state.players[0].id,
        tricerules_proto::ruled::v1::StackResolveDestination::Graveyard,
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&omen.object_id].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&omen.object_id].owner,
        engine.state.players[0].id
    );
    assert_eq!(
        engine.state.zone_change_generation[&omen.object_id],
        omen.zone_change_generation
            .expect("Skimming Strike stack generation")
            + 1
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_cast - 1,
        "fizzling Skimming Strike draws no card"
    );
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before_cast,
        "fizzling Skimming Strike does not shuffle or draw from its library"
    );
    assert!(omen_completion
        .events
        .iter()
        .all(|event| { !matches!(&event.ev, Some(Ev::ResolutionChoiceRequired(_))) }));
    assert_eq!(
        omen_completion
            .events
            .iter()
            .filter(|event| {
                matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("shuffles the resolving Omen"))
            })
            .count(),
        0,
        "the fizzling Omen produces no shuffle log"
    );
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
    narset_c1_assert_no_scheduled_rebounds(&engine, "the fizzled granted Omen");
}

#[test]
fn narset_c1_actual_twincast_copy_finishes_opt_without_its_grant_or_schedule() {
    let (mut engine, source, source_generation) = narset_rebound_lifetime_fixture(6_110_303);
    arm_narset_observer(&mut engine, source);
    ensure_in_hand(&mut engine, 0, "opt");
    let hand_before_opt = engine.state.players[0].hand.len();
    let (opt, _) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        0,
        "opt",
        0,
        0,
        Vec::new(),
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, opt);
    narset_c1_capture_priority_round(&mut engine);
    let exact_grant = expected_rebound_grants_for_cast(opt);
    assert_eq!(serialized_rebound_grants(&engine), exact_grant);
    narset_c1_assert_no_scheduled_rebounds(&engine, "before either Opt resolves");

    let controller = engine.state.players[0].id;
    engine
        .apply_scenario_command(controller, &pass())
        .expect("pass priority so the opponent can copy the granted Opt");
    let (twincast, _) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        1,
        "twincast",
        0,
        0,
        vec![stack_target_for_cast(opt)],
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let twincast_completion = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &twincast_completion,
        twincast.object_id,
        engine.state.players[1].id,
        tricerules_proto::ruled::v1::StackResolveDestination::Graveyard,
    );
    assert_eq!(
        engine.state.objects[&twincast.object_id].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&twincast.object_id].owner,
        engine.state.players[1].id
    );
    let (copy_id, copy_controller) = {
        let copy = engine
            .state
            .stack
            .iter()
            .find(|item| item.is_copy && item.card_id == "opt")
            .expect("actual Twincast creates a virtual copy of the granted Opt");
        assert_ne!(copy.id, opt.object_id);
        assert_eq!(copy.cast_occurrence, None);
        (copy.id, copy.controller)
    };
    let copy_controller_index = engine
        .state
        .player_idx(copy_controller)
        .expect("the Twincast copy's controller remains in the game");
    assert_ne!(
        copy_controller, engine.state.players[0].id,
        "the opponent who cast Twincast controls its actual copy"
    );
    assert_eq!(serialized_rebound_grants(&engine), exact_grant);
    narset_c1_assert_no_scheduled_rebounds(&engine, "while only the virtual Opt copy is resolving");

    let copy_choice_batch = narset_c1_capture_priority_round(&mut engine);
    let (copy_decider, copy_candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the actual Opt copy reaches its Scry choice");
        assert_eq!(pending.presentation.source_object_id, copy_id);
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::LibraryTop,
            "the virtual Opt copy parks at its printed Scry"
        );
        (
            pending.deciding_player,
            pending.presentation.candidates.clone(),
        )
    };
    assert!(copy_choice_batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::ResolutionChoiceRequired(choice))
                if choice.source_object_id == copy_id
                    && choice.choice_kind == ChoiceKind::LibraryTop as i32
        )
    }));
    let player_zero_hand_before_copy_draw = engine.state.players[0].hand.len();
    let copy_controller_hand_before_draw = engine.state.players[copy_controller_index].hand.len();
    let copy_completion = engine
        .apply_scenario_command(copy_decider, &submit_resolution_choice(copy_candidates))
        .expect("answer the actual copy's current Scry choice and finish its draw");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, opt.object_id);
    assert_eq!(
        engine.state.players[copy_controller_index].hand.len(),
        copy_controller_hand_before_draw + 1,
        "the virtual copy draws for Twincast's controller"
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        player_zero_hand_before_copy_draw,
        "the opponent-controlled copy does not draw for the original Opt's controller"
    );
    assert_eq!(serialized_rebound_grants(&engine), exact_grant);
    narset_c1_assert_no_scheduled_rebounds(&engine, "after the virtual Opt copy finishes");
    assert!(copy_completion.events.iter().all(|event| {
        !matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == opt.object_id)
    }));

    let original_choice_batch = narset_c1_capture_priority_round(&mut engine);
    let (original_decider, original_candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the physical granted Opt reaches its own Scry choice");
        assert_eq!(pending.presentation.source_object_id, opt.object_id);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryTop);
        (
            pending.deciding_player,
            pending.presentation.candidates.clone(),
        )
    };
    assert!(original_choice_batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::ResolutionChoiceRequired(choice))
                if choice.source_object_id == opt.object_id
                    && choice.choice_kind == ChoiceKind::LibraryTop as i32
        )
    }));
    let hand_before_original_draw = engine.state.players[0].hand.len();
    let stack_generation = opt
        .zone_change_generation
        .expect("the physical Opt carries its exact stack generation");
    let presentation = engine
        .state
        .stack_presentations
        .get(&opt.object_id)
        .expect("the original Opt retains its captured presentation while parked")
        .primary
        .clone();
    let original_completion = engine
        .apply_scenario_command(
            original_decider,
            &submit_resolution_choice(original_candidates),
        )
        .expect("answer the original Opt's current Scry choice and finish its draw");
    narset_c1_assert_one_stack_resolved(
        &original_completion,
        opt.object_id,
        engine.state.players[0].id,
        tricerules_proto::ruled::v1::StackResolveDestination::Exile,
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_original_draw + 1,
        "the physical original performs its own printed draw"
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_opt,
        "the physical Opt's own draw offsets the card cast from its hand"
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].owner,
        engine.state.players[0].id
    );
    let exile_generation = engine.state.zone_change_generation[&opt.object_id];
    assert_eq!(exile_generation, stack_generation + 1);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    let scheduled = narset_c1_exclusion_scheduled_rebounds(&engine);
    let scheduled = scheduled
        .as_array()
        .expect("diagnostics expose the private scheduled-Rebound records");
    assert_eq!(
        scheduled.len(),
        1,
        "only the physical original schedules Rebound; its copy creates no record"
    );
    let record = &scheduled[0];
    assert_eq!(
        record["source"]["object_id"],
        serde_json::json!(opt.object_id)
    );
    assert_eq!(
        record["source"]["zone_change_generation"],
        serde_json::json!(stack_generation.to_string())
    );
    assert_eq!(
        record["exiled"]["object_id"],
        serde_json::json!(opt.object_id)
    );
    assert_eq!(
        record["exiled"]["zone_change_generation"],
        serde_json::json!(exile_generation.to_string())
    );
    assert_eq!(
        record["controller"],
        serde_json::json!(engine.state.players[0].id)
    );
    assert_eq!(record["card_id"], serde_json::json!("opt"));
    assert_eq!(record["card_name"], serde_json::json!("Opt"));
    assert_eq!(record["source_face_index"], serde_json::json!("0"));
    assert_eq!(
        record["presentation"],
        serde_json::to_value(presentation).expect("serialize the captured physical Opt face")
    );
}

#[test]
fn narset_c1_granted_ascend_from_avernus_uses_its_own_exile_once_at_x_zero() {
    let (mut engine, source, source_generation) = narset_c1_exclusion_fixture(6_110_304);
    arm_narset_observer(&mut engine, source);
    ensure_in_hand(&mut engine, 0, "ascend_from_avernus");
    let hand_before_cast = engine.state.players[0].hand.len();
    let (ascend, cast_batch) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        0,
        "ascend_from_avernus",
        0,
        0,
        Vec::new(),
        ManaGift {
            w: 3,
            ..Default::default()
        },
    );
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(_))))
            .count(),
        2,
        "the physical X=0 Ascend and its grant trigger are each pushed once"
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, ascend);
    narset_c1_capture_priority_round(&mut engine);
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(ascend),
        "the actual X=0 Ascend has a live grant before its own exile instruction"
    );

    let completion = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &completion,
        ascend.object_id,
        engine.state.players[0].id,
        tricerules_proto::ruled::v1::StackResolveDestination::Exile,
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&ascend.object_id].zone,
        tricerules_core::Zone::Exile,
        "Ascend from Avernus exiles itself through ExileResolvingSpell"
    );
    assert_eq!(
        engine.state.objects[&ascend.object_id].owner,
        engine.state.players[0].id
    );
    assert_eq!(
        engine.state.zone_change_generation[&ascend.object_id],
        ascend
            .zone_change_generation
            .expect("Ascend's exact stack generation")
            + 1,
        "the exact physical Ascend makes one stack-to-exile zone change"
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_cast - 1,
        "X=0 Ascend draws no card"
    );
    assert!(completion
        .events
        .iter()
        .all(|event| { !matches!(&event.ev, Some(Ev::ResolutionChoiceRequired(_))) }));
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
    narset_c1_assert_no_scheduled_rebounds(&engine, "Ascend's explicit self-exile");
}

fn narset_c1_nonhand_grant_fixture(seed: u64) -> (GameEngine, u32, u64, u32, u64) {
    let decks = Some(vec![
        deck_with("opt", &[NARSET_C1_EXCLUSION_PROVIDER_ID, "think_twice"]),
        deck_with("opt", &[]),
    ]);
    let mut engine = GameEngine::new(
        narset_c1_exclusion_registry(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Narset C1 non-hand provider fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let provider = narset_c1_exclusion_move_card_to_zone(
        &mut engine,
        0,
        NARSET_C1_EXCLUSION_PROVIDER_ID,
        DevZone::Battlefield,
    );
    let provider_generation = engine.state.zone_change_generation[&provider];
    let think_twice =
        narset_c1_exclusion_move_card_to_zone(&mut engine, 0, "think_twice", DevZone::Graveyard);
    let graveyard_generation = engine.state.zone_change_generation[&think_twice];
    (
        engine,
        provider,
        provider_generation,
        think_twice,
        graveyard_generation,
    )
}

#[test]
fn narset_c1_admitted_flat_grant_captures_flashback_without_rebound_exit() {
    let (mut engine, provider, provider_generation, think_twice, graveyard_generation) =
        narset_c1_nonhand_grant_fixture(6_110_305);
    assert_eq!(
        engine.state.objects[&provider].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&think_twice].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(serialized_observer_count(&engine), 0);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    let flashback_controller = engine.state.players[0].id;
    give_mana(
        &mut engine,
        flashback_controller,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let flashback = cast_graveyard_flashback(&mut engine, think_twice);
    assert!(
        flashback
            .zone_change_generation
            .expect("Flashback stack generation")
            > graveyard_generation,
        "the exact Think Twice moves from its graveyard incarnation to the stack"
    );
    let spell_index = engine
        .state
        .stack
        .iter()
        .position(|item| item.cast_occurrence == Some(flashback))
        .expect("the actual Think Twice Flashback is on the stack");
    let triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect::<Vec<_>>();
    assert_eq!(
        triggers.len(),
        1,
        "the admitted provider stages one live trigger"
    );
    let trigger = triggers[0];
    let trigger_index = engine
        .state
        .stack
        .iter()
        .position(|item| item.id == trigger.id)
        .expect("the actual provider's trigger is on the stack");
    assert!(trigger_index > spell_index);
    assert_eq!(trigger.controller, engine.state.players[0].id);
    assert_eq!(trigger.source_permanent_id, Some(provider));
    assert_eq!(trigger.source_zone_change, provider_generation);
    assert_eq!(
        serde_json::to_value(trigger).expect("serialize the live provider trigger")
            ["trigger_context"]["triggering_spell"],
        serde_json::to_value(Some(flashback))
            .expect("serialize the exact actual Flashback occurrence")
    );
    assert_eq!(
        serialized_rebound_grants(&engine),
        serde_json::json!([]),
        "the actual Flashback grant is not installed before its trigger resolves"
    );
    let trigger_completion = narset_c1_capture_priority_round(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, think_twice);
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(flashback),
        "the separately admitted provider grants Rebound to this exact non-hand cast"
    );
    narset_c1_assert_no_scheduled_rebounds(&engine, "while the live Flashback grant is pending");
    assert!(trigger_completion.events.iter().all(|event| {
        !matches!(&event.ev, Some(Ev::StackObjectCountered(countered)) if countered.object_id == think_twice)
    }));

    let hand_before_draw = engine.state.players[0].hand.len();
    let completion = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &completion,
        think_twice,
        engine.state.players[0].id,
        tricerules_proto::ruled::v1::StackResolveDestination::Exile,
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(completion
        .events
        .iter()
        .all(|event| { !matches!(&event.ev, Some(Ev::ResolutionChoiceRequired(_))) }));
    assert_eq!(engine.state.players[0].hand.len(), hand_before_draw + 1);
    assert_eq!(
        engine.state.objects[&think_twice].zone,
        tricerules_core::Zone::Exile,
        "Think Twice keeps Flashback's own exile destination"
    );
    let owner = engine.state.objects[&think_twice].owner;
    assert_eq!(
        owner, engine.state.players[0].id,
        "the physical Flashback card keeps its original owner"
    );
    assert_eq!(
        engine.state.zone_change_generation[&think_twice],
        flashback
            .zone_change_generation
            .expect("Think Twice stack generation")
            + 1
    );
    let owner_index = engine
        .state
        .player_idx(owner)
        .expect("the Flashback card's physical owner remains in the game");
    assert_eq!(
        engine.state.players[owner_index]
            .exile
            .iter()
            .filter(|object_id| **object_id == think_twice)
            .count(),
        1,
        "the exact physical Think Twice is exiled once"
    );
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
    assert_eq!(
        engine.state.zone_change_generation[&provider], provider_generation,
        "the live provider keeps the same battlefield incarnation"
    );
    narset_c1_assert_no_scheduled_rebounds(&engine, "the non-hand Flashback resolution");
}

#[test]
fn narset_phase_c2_rebound_waits_for_own_upkeep_then_declines_once() {
    let (mut engine, source, source_generation) = narset_rebound_lifetime_fixture(6_110_401);
    let controller = engine.state.players[0].id;
    let opponent = engine.state.players[1].id;
    arm_narset_observer(&mut engine, source);
    ensure_in_hand(&mut engine, 0, "opt");
    let hand_before_opt = engine.state.players[0].hand.len();
    let (opt, cast_batch) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        0,
        "opt",
        0,
        0,
        Vec::new(),
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, opt);
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(_))))
            .count(),
        2,
        "the actual Opt and its ordinary grant trigger are each pushed once"
    );
    narset_c1_assert_no_scheduled_rebounds(&engine, "before the actual Opt resolves");

    narset_c1_capture_priority_round(&mut engine);
    let exact_grant = expected_rebound_grants_for_cast(opt);
    assert_eq!(serialized_rebound_grants(&engine), exact_grant);
    narset_c1_assert_no_scheduled_rebounds(&engine, "while the actual Opt's grant is live");

    let scry_batch = narset_c1_capture_priority_round(&mut engine);
    let (scry_decider, scry_prompt_id, scry_candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the actual Opt reaches its printed Scry choice");
        assert_eq!(pending.presentation.source_object_id, opt.object_id);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryTop);
        (
            pending.deciding_player,
            pending.resolution_prompt_id.get(),
            pending.presentation.candidates.clone(),
        )
    };
    let scry_offer = find_resolution_choice(&scry_batch)
        .expect("the actual Scry publishes its current resolution choice");
    assert_eq!(scry_offer.source_object_id, opt.object_id);
    assert_eq!(
        scry_offer.choice_kind,
        ChoiceKind::LibraryTop as i32,
        "the actual Opt parks at its printed Scry"
    );
    assert_ne!(
        scry_prompt_id, 0,
        "the Scry offer has a genuine prompt nonce"
    );
    assert_eq!(scry_offer.resolution_prompt_id, scry_prompt_id);
    let mut scry_answer = submit_resolution_choice(scry_candidates);
    match scry_answer.cmd.as_mut() {
        Some(Cmd::SubmitResolutionChoice(answer)) => {
            answer.resolution_prompt_id = scry_offer.resolution_prompt_id;
        }
        _ => panic!("the actual Scry answer is a resolution-choice command"),
    }
    let opt_completion = engine
        .apply_command(scry_decider, &scry_answer)
        .expect("answer the current Scry offer and finish the actual Opt");
    narset_c1_assert_one_stack_resolved(
        &opt_completion,
        opt.object_id,
        controller,
        tricerules_proto::ruled::v1::StackResolveDestination::Exile,
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].hand.len(), hand_before_opt);
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(engine.state.objects[&opt.object_id].owner, controller);
    let opt_stack_generation = opt
        .zone_change_generation
        .expect("the actual Opt retains its exact stack incarnation");
    let exile_generation = engine.state.zone_change_generation[&opt.object_id];
    assert_eq!(exile_generation, opt_stack_generation + 1);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    let saved_schedule = narset_c1_exclusion_scheduled_rebounds(&engine);
    let saved_records = saved_schedule
        .as_array()
        .expect("diagnostics expose the private scheduled-Rebound records");
    assert_eq!(
        saved_records.len(),
        1,
        "the exact Opt schedules Rebound once"
    );
    let saved_record = saved_records[0].clone();
    assert_eq!(
        saved_record["source"]["object_id"],
        serde_json::json!(opt.object_id)
    );
    assert_eq!(
        saved_record["source"]["zone_change_generation"],
        serde_json::json!(opt_stack_generation.to_string())
    );
    assert_eq!(
        saved_record["exiled"]["object_id"],
        serde_json::json!(opt.object_id)
    );
    assert_eq!(
        saved_record["exiled"]["zone_change_generation"],
        serde_json::json!(exile_generation.to_string())
    );
    assert_eq!(saved_record["controller"], serde_json::json!(controller));
    assert_eq!(saved_record["card_id"], serde_json::json!("opt"));
    assert_eq!(saved_record["card_name"], serde_json::json!("Opt"));
    assert_eq!(saved_record["source_face_index"], serde_json::json!("0"));

    end_active_turn(&mut engine, controller);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(&engine),
        saved_schedule,
        "the opponent's upkeep leaves the exact Opt schedule untouched"
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt.object_id],
        exile_generation
    );

    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(&mut engine, opponent);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);

    let rebound_triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| {
            item.is_triggered
                && !item.is_copy
                && item.card_id == "opt"
                && item.controller == controller
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rebound_triggers.len(),
        1,
        "the exact exiled Opt creates one ordinary noncopy Rebound trigger at its controller's next own upkeep"
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the delayed Rebound trigger is the only object on the stack"
    );
    let rebound_trigger = &engine.state.stack[0];
    assert!(rebound_trigger.is_triggered);
    assert!(!rebound_trigger.is_copy);
    assert_eq!(rebound_trigger.card_id, "opt");
    assert_eq!(rebound_trigger.face_index, 0);
    assert_eq!(rebound_trigger.controller, controller);
    assert_eq!(
        rebound_trigger.source_permanent_id,
        Some(opt.object_id),
        "the delayed trigger keeps the old physical Opt as its source"
    );
    assert_eq!(
        rebound_trigger.source_zone_change, opt_stack_generation,
        "the trigger source is the Opt's old stack incarnation"
    );
    assert_eq!(
        rebound_trigger.trigger_context.observed_object,
        Some(tricerules_core::state::TriggerObjectRef {
            object_id: opt.object_id,
            zone_change_generation: exile_generation,
            controller_at_event: controller,
        }),
        "the trigger watches the exact current exile incarnation"
    );
    let rebound_trigger_id = rebound_trigger.id;
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(&engine),
        serde_json::json!([]),
        "the matched one-shot schedule is consumed when the trigger is staged"
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt.object_id],
        exile_generation
    );

    let trigger_resolution = narset_c1_capture_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    let special_offer = find_resolution_choice(&trigger_resolution)
        .expect("resolving only the delayed trigger publishes its optional cast offer");
    assert_eq!(special_offer.choice_kind, ChoiceKind::SpecialCast as i32);
    assert_ne!(special_offer.resolution_prompt_id, 0);
    let (deciding_player, current_prompt_id) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the delayed trigger parks at the optional Rebound cast offer");
        assert_eq!(pending.deciding_player, controller);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
        assert_eq!(pending.presentation.min, 0);
        assert_eq!(pending.presentation.max, 1);
        assert_eq!(
            pending.presentation.candidates,
            vec![opt.object_id],
            "the offer names only the exact exiled Opt"
        );
        assert_eq!(
            pending.presentation.source_object_id, rebound_trigger_id,
            "the parked parent and prompt source are the delayed trigger"
        );
        assert_ne!(
            pending.presentation.source_object_id, opt.object_id,
            "the prompt source is not assumed to be the physical Opt"
        );
        let tricerules_core::state::ResolutionContinuation::SpecialCast {
            stack,
            exiled,
            face_index,
            method,
            cost,
            permission_scope,
            completion,
            ..
        } = &pending.continuation
        else {
            panic!("the delayed trigger owns an existing SpecialCast continuation");
        };
        assert_eq!(stack.item.id, rebound_trigger_id);
        assert!(stack.item.is_triggered);
        assert!(!stack.item.is_copy);
        assert_eq!(stack.item.source_permanent_id, Some(opt.object_id));
        assert_eq!(stack.item.source_zone_change, opt_stack_generation);
        assert_eq!(stack.item.card_id, "opt");
        assert_eq!(stack.item.face_index, 0);
        assert_eq!(stack.item.controller, controller);
        assert_eq!(exiled.object_id, opt.object_id);
        assert_eq!(exiled.zone_change_generation, exile_generation);
        assert_eq!(exiled.controller_at_event, controller);
        assert_eq!(*face_index, 0);
        assert_eq!(*method, tricerules_core::state::SpellCastMethod::Permission);
        assert_eq!(
            *cost,
            tricerules_cards::mana::ManaCost::default(),
            "the Rebound offer uses zero alternate mana cost"
        );
        assert_eq!(
            *permission_scope,
            Some(tricerules_core::state::ExilePlayPermissionScope::CastCard)
        );
        assert!(matches!(
            completion,
            tricerules_core::state::SpecialCastCompletion::ResumeStack
        ));
        (pending.deciding_player, pending.resolution_prompt_id.get())
    };
    assert_eq!(
        special_offer.source_object_id, rebound_trigger_id,
        "the current wire offer uses the delayed trigger as its parent"
    );
    assert_eq!(special_offer.resolution_prompt_id, current_prompt_id);
    assert_ne!(
        current_prompt_id, 0,
        "the current cast offer has a real nonce"
    );

    let mut decline = submit_resolution_decision(ResolutionChoiceDecision::Decline);
    match decline.cmd.as_mut() {
        Some(Cmd::SubmitResolutionChoice(answer)) => {
            answer.resolution_prompt_id = special_offer.resolution_prompt_id;
        }
        _ => panic!("the Rebound decline is a resolution-choice command"),
    }
    let decline_batch = engine
        .apply_command(deciding_player, &decline)
        .expect("decline the genuine current Rebound offer");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt.object_id],
        exile_generation
    );
    assert_eq!(engine.state.objects[&opt.object_id].owner, controller);
    assert!(
        decline_batch.events.iter().all(|event| {
            !matches!(
                &event.ev,
                Some(Ev::StackPushed(pushed)) if pushed.object_id == opt.object_id
            )
        }),
        "declining does not cast the exact exiled Opt"
    );
    assert!(
        engine
            .state
            .active_exile_play_permissions
            .iter()
            .all(|permission| permission.object_id != opt.object_id),
        "the decline leaves no persistent permission for this Opt"
    );
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(&engine),
        serde_json::json!([])
    );
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(&mut engine, controller);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(&engine),
        serde_json::json!([]),
        "the declined opportunity does not retry at an opponent's upkeep"
    );
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(&mut engine, opponent);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(&engine),
        serde_json::json!([]),
        "the one-shot Rebound opportunity does not return at the next own upkeep"
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt.object_id],
        exile_generation
    );
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != opt.object_id));
}

struct NarsetC2ReboundTriggerFixture {
    engine: GameEngine,
    controller: i32,
    opponent: i32,
    opt_object_id: u32,
    exile_generation: u64,
    rebound_trigger_id: u32,
}

fn narset_c2_rebound_trigger_fixture(seed: u64) -> NarsetC2ReboundTriggerFixture {
    let (mut engine, source, source_generation) = narset_rebound_lifetime_fixture(seed);
    let controller = engine.state.players[0].id;
    let opponent = engine.state.players[1].id;
    arm_narset_observer(&mut engine, source);

    ensure_in_hand(&mut engine, 0, "opt");
    let hand_before_opt = engine.state.players[0].hand.len();
    let (opt, cast_batch) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        0,
        "opt",
        0,
        0,
        Vec::new(),
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, opt);
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(_))))
            .count(),
        2,
        "the actual Opt and its ordinary grant trigger are each pushed once"
    );

    narset_c1_capture_priority_round(&mut engine);
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(opt),
        "resolving the actual grant trigger records this exact Opt cast"
    );
    let scry_batch = narset_c1_capture_priority_round(&mut engine);
    let (scry_decider, scry_candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the actual Opt reaches its printed Scry choice");
        assert_eq!(pending.presentation.source_object_id, opt.object_id);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryTop);
        (
            pending.deciding_player,
            pending.presentation.candidates.clone(),
        )
    };
    let scry_offer = find_resolution_choice(&scry_batch)
        .expect("the actual Scry publishes its current resolution choice");
    assert_eq!(scry_offer.choice_kind, ChoiceKind::LibraryTop as i32);
    let mut scry_answer = submit_resolution_choice(scry_candidates);
    match scry_answer.cmd.as_mut() {
        Some(Cmd::SubmitResolutionChoice(answer)) => {
            answer.resolution_prompt_id = scry_offer.resolution_prompt_id;
        }
        _ => panic!("the actual Scry answer is a resolution-choice command"),
    }
    let opt_completion = engine
        .apply_command(scry_decider, &scry_answer)
        .expect("answer the current Scry choice and finish the actual Opt");
    narset_c1_assert_one_stack_resolved(
        &opt_completion,
        opt.object_id,
        controller,
        tricerules_proto::ruled::v1::StackResolveDestination::Exile,
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].hand.len(), hand_before_opt);
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(engine.state.objects[&opt.object_id].owner, controller);
    let opt_stack_generation = opt
        .zone_change_generation
        .expect("the actual Opt retains its exact stack incarnation");
    let exile_generation = engine.state.zone_change_generation[&opt.object_id];
    assert_eq!(exile_generation, opt_stack_generation + 1);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    let schedule = narset_c1_exclusion_scheduled_rebounds(&engine);
    let records = schedule
        .as_array()
        .expect("diagnostics expose the scheduled-Rebound records");
    assert_eq!(records.len(), 1, "the exact Opt schedules Rebound once");
    assert_eq!(
        records[0]["source"]["object_id"],
        serde_json::json!(opt.object_id)
    );
    assert_eq!(
        records[0]["source"]["zone_change_generation"],
        serde_json::json!(opt_stack_generation.to_string())
    );
    assert_eq!(
        records[0]["exiled"]["object_id"],
        serde_json::json!(opt.object_id)
    );
    assert_eq!(
        records[0]["exiled"]["zone_change_generation"],
        serde_json::json!(exile_generation.to_string())
    );
    assert_eq!(records[0]["controller"], serde_json::json!(controller));

    end_active_turn(&mut engine, controller);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(&engine),
        schedule,
        "the opponent's upkeep leaves the exact Opt schedule untouched"
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt.object_id],
        exile_generation
    );
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(&mut engine, opponent);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);

    assert_eq!(engine.state.stack.len(), 1);
    let rebound_trigger = &engine.state.stack[0];
    assert!(rebound_trigger.is_triggered);
    assert!(!rebound_trigger.is_copy);
    assert_eq!(rebound_trigger.card_id, "opt");
    assert_eq!(rebound_trigger.controller, controller);
    assert_eq!(rebound_trigger.source_permanent_id, Some(opt.object_id));
    assert_eq!(rebound_trigger.source_zone_change, opt_stack_generation);
    assert_eq!(
        rebound_trigger.trigger_context.observed_object,
        Some(tricerules_core::state::TriggerObjectRef {
            object_id: opt.object_id,
            zone_change_generation: exile_generation,
            controller_at_event: controller,
        })
    );
    let rebound_trigger_id = rebound_trigger.id;
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(&engine),
        serde_json::json!([]),
        "staging the own-upkeep trigger consumes the one-shot schedule"
    );

    NarsetC2ReboundTriggerFixture {
        engine,
        controller,
        opponent,
        opt_object_id: opt.object_id,
        exile_generation,
        rebound_trigger_id,
    }
}

#[test]
fn narset_phase_c2_accepts_rebound_opt_and_completes_scry_draw() {
    use tricerules_proto::ruled::v1 as rv1;

    let NarsetC2ReboundTriggerFixture {
        mut engine,
        controller,
        opt_object_id,
        exile_generation,
        rebound_trigger_id,
        opponent,
        ..
    } = narset_c2_rebound_trigger_fixture(6_110_402);
    let trigger_resolution = narset_c1_capture_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    let special_offer = find_resolution_choice(&trigger_resolution)
        .expect("resolving the genuine delayed trigger publishes its optional cast offer");
    assert_eq!(special_offer.choice_kind, ChoiceKind::SpecialCast as i32);
    assert_eq!(special_offer.source_object_id, rebound_trigger_id);
    assert_eq!(special_offer.candidate_object_ids, vec![opt_object_id]);
    assert_eq!(special_offer.candidate_selectable, [true]);
    assert_ne!(special_offer.resolution_prompt_id, 0);

    let (deciding_player, current_prompt_id) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the delayed trigger parks at the optional Rebound cast offer");
        assert_eq!(pending.deciding_player, controller);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
        assert_eq!(pending.presentation.candidates, vec![opt_object_id]);
        let tricerules_core::state::ResolutionContinuation::SpecialCast {
            stack,
            exiled,
            face_index,
            method,
            cost,
            permission_scope,
            completion,
            ..
        } = &pending.continuation
        else {
            panic!("the delayed trigger owns an existing SpecialCast continuation");
        };
        assert_eq!(stack.item.id, rebound_trigger_id);
        assert_eq!(exiled.object_id, opt_object_id);
        assert_eq!(exiled.zone_change_generation, exile_generation);
        assert_eq!(*face_index, 0);
        assert_eq!(*method, tricerules_core::state::SpellCastMethod::Permission);
        assert_eq!(*cost, tricerules_cards::mana::ManaCost::default());
        assert_eq!(
            *permission_scope,
            Some(tricerules_core::state::ExilePlayPermissionScope::CastCard)
        );
        assert!(matches!(
            completion,
            tricerules_core::state::SpecialCastCompletion::ResumeStack
        ));
        (pending.deciding_player, pending.resolution_prompt_id.get())
    };
    assert_eq!(special_offer.resolution_prompt_id, current_prompt_id);

    let current_legal = engine.initial_response_batch();
    let offered_actions = current_legal.legal_by_player[&controller]
        .zone_cast_actions
        .iter()
        .filter(|action| action.object_id == opt_object_id)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(offered_actions.len(), 1);
    let action = &offered_actions[0];
    assert_eq!(action.source_zone, rv1::CastSourceZone::Exile as i32);
    assert_eq!(action.object_id, opt_object_id);
    assert_eq!(action.zone_change_generation, exile_generation);
    assert_eq!(action.face_index, 0);
    assert_eq!(action.cast_method, rv1::CastMethod::Permission as i32);
    assert_eq!(action.cost, "", "zero Rebound cost is the empty string");
    let offered_permission_id = action
        .casting_permission_id
        .expect("the current legal offer carries its permission id");
    assert_ne!(offered_permission_id, 0);
    let announcement = rv1::SpellCastAnnouncement {
        source: Some(exile_cast_source(opt_object_id, exile_generation)),
        face_index: action.face_index,
        cast_method: action.cast_method,
        casting_permission_id: Some(offered_permission_id),
        ..Default::default()
    };
    let begin_batch = engine
        .apply_command(
            deciding_player,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                    resolution_prompt_id: current_prompt_id,
                    decision: ResolutionChoiceDecision::CastSpell as i32,
                    spell_cast_announcement: Some(announcement),
                    ..Default::default()
                })),
            },
        )
        .expect("accept the current genuine Rebound cast offer");
    assert_eq!(
        begin_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.object_id == opt_object_id))
            .count(),
        0,
        "beginning the child cast does not publish StackPushed"
    );
    assert_eq!(
        engine.state.objects[&opt_object_id].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt_object_id],
        exile_generation + 1
    );
    let pending_cast = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the accepted Rebound choice creates a pending spell transaction");
    assert!(pending_cast.resolution_time_offer);
    assert_eq!(pending_cast.caster, controller);
    assert_eq!(pending_cast.reserved_object_id, opt_object_id);
    assert_eq!(pending_cast.original_zone, tricerules_core::Zone::Exile);
    assert_eq!(
        pending_cast.original_zone_change_generation,
        exile_generation
    );
    assert_eq!(
        pending_cast.announcement.casting_permission_id,
        Some(offered_permission_id)
    );
    let parked_parent = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the parent resolution stays suspended while the child is paid");
    let tricerules_core::state::ResolutionContinuation::SpecialCast { stack, .. } =
        &parked_parent.continuation
    else {
        panic!("the parked parent remains the delayed Rebound trigger");
    };
    assert_eq!(stack.item.id, rebound_trigger_id);

    let caster_index = engine
        .state
        .player_idx(controller)
        .expect("the Rebound controller remains in the game");
    let mana_before_commit = engine.state.players[caster_index].mana_pool;
    let transaction_id = pending_cast.transaction_id;
    let revision = engine.state.command_index;
    let preview = engine.preview_payment_scenario(
        controller,
        &rv1::PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(rv1::CommitSpellCast {
                transaction_id,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    assert_eq!(
        engine.state.command_index, revision,
        "preview is query-only"
    );
    assert_eq!(
        engine.state.players[caster_index].mana_pool,
        mana_before_commit
    );
    let cast_batch = engine
        .apply_scenario_command(
            controller,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                    transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .expect("commit the free physical Opt cast from Exile");
    assert_eq!(
        engine.state.players[caster_index].mana_pool,
        mana_before_commit
    );
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == rebound_trigger_id));
    let parent_resolved_index = cast_batch
        .events
        .iter()
        .position(|event| {
            matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == rebound_trigger_id)
        })
        .expect("the delayed Rebound parent resolves exactly once after its accepted child");
    assert!(
        cast_batch
            .events
            .iter()
            .any(|event| matches!(&event.ev, Some(Ev::PriorityChanged(_)))),
        "parent completion publishes priority after resolving the delayed trigger"
    );
    for (event_index, event) in cast_batch.events.iter().enumerate() {
        if matches!(&event.ev, Some(Ev::PriorityChanged(_))) {
            assert!(
                parent_resolved_index < event_index,
                "the parent StackResolved must precede every PriorityChanged in the child commit batch"
            );
        }
    }
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.object_id == opt_object_id))
            .count(),
        1,
        "committing publishes one physical Opt spell push"
    );
    assert_eq!(
        engine.state.objects[&opt_object_id].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt_object_id],
        exile_generation + 1
    );
    let recast = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == opt_object_id)
        .expect("the exact exiled Opt is the committed physical spell");
    assert_eq!(recast.card_id, "opt");
    assert_eq!(recast.controller, controller);
    assert_eq!(recast.cast_by, Some(controller));
    assert_eq!(recast.face_index, 0);
    assert_eq!(
        recast.cast_method,
        tricerules_core::state::SpellCastMethod::Permission
    );
    assert!(!recast.is_copy);
    assert!(!recast.is_triggered);
    assert!(recast.cast_occurrence.is_some());
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != opt_object_id));
    narset_c1_assert_no_scheduled_rebounds(&engine, "the accepted Rebound cast");
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    let hand_before_recast_resolution = engine.state.players[caster_index].hand.len();
    let recast_resolution = narset_c1_capture_priority_round(&mut engine);
    let scry_offer = find_resolution_choice(&recast_resolution)
        .expect("the actual recast Opt reaches its printed Scry choice");
    assert_eq!(scry_offer.source_object_id, opt_object_id);
    assert_eq!(scry_offer.choice_kind, ChoiceKind::LibraryTop as i32);
    let (scry_decider, scry_candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the recast Opt parks at its Scry choice");
        assert_eq!(pending.presentation.source_object_id, opt_object_id);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryTop);
        (
            pending.deciding_player,
            pending.presentation.candidates.clone(),
        )
    };
    let mut scry_answer = submit_resolution_choice(scry_candidates);
    match scry_answer.cmd.as_mut() {
        Some(Cmd::SubmitResolutionChoice(answer)) => {
            answer.resolution_prompt_id = scry_offer.resolution_prompt_id;
        }
        _ => panic!("the recast Scry answer is a resolution-choice command"),
    }
    let opt_completion = engine
        .apply_command(scry_decider, &scry_answer)
        .expect("finish the actual recast Scry and Draw");
    narset_c1_assert_one_stack_resolved(
        &opt_completion,
        opt_object_id,
        controller,
        rv1::StackResolveDestination::Graveyard,
    );
    assert_eq!(
        engine.state.players[caster_index].hand.len(),
        hand_before_recast_resolution + 1
    );
    assert_eq!(
        engine.state.objects[&opt_object_id].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt_object_id],
        exile_generation + 2
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_spell_cast.is_none());
    narset_c1_assert_no_scheduled_rebounds(&engine, "the completed Rebound Opt recast");
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != opt_object_id));

    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(&mut engine, controller);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    narset_c1_assert_no_scheduled_rebounds(
        &engine,
        "the opponent's upkeep after the accepted Rebound cast",
    );
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(&mut engine, opponent);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(triggered_stack_ids(&engine).is_empty());
    narset_c1_assert_no_scheduled_rebounds(
        &engine,
        "the next own upkeep after the accepted Rebound cast",
    );
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != opt_object_id));
}

#[test]
fn narset_phase_c2_actual_tishanas_tidebinder_counters_delayed_trigger() {
    use tricerules_proto::ruled::v1 as rv1;

    let NarsetC2ReboundTriggerFixture {
        mut engine,
        controller,
        opponent,
        opt_object_id,
        exile_generation,
        rebound_trigger_id,
        ..
    } = narset_c2_rebound_trigger_fixture(6_110_403);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, rebound_trigger_id);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != opt_object_id));

    engine
        .apply_scenario_command(controller, &pass())
        .expect("give the opponent priority before the delayed trigger resolves");
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
        "the actual Tidebinder resolves before its target is chosen"
    );

    let current_offer = engine.initial_response_batch();
    let target_offer = current_offer.legal_by_player[&opponent]
        .valid_targets_by_ability
        .values()
        .find(|ability_targets| {
            ability_targets
                .groups
                .iter()
                .any(|group| group.valid_stack_ids.contains(&rebound_trigger_id))
        })
        .expect("the current Tidebinder offer includes the delayed trigger on the stack");
    assert!(target_offer
        .groups
        .iter()
        .any(|group| group.valid_stack_ids.contains(&rebound_trigger_id)));
    let choose_trigger = RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(rv1::ChooseTriggerTarget {
            targets: vec![TargetRef {
                object_id: rebound_trigger_id,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(opponent, &choose_trigger)
        .expect("target the current ordinary Rebound trigger with Tidebinder");
    let counter_batch = narset_c1_capture_priority_round(&mut engine);
    assert_eq!(
        counter_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackObjectCountered(countered)) if countered.object_id == rebound_trigger_id))
            .count(),
        1,
        "Tidebinder counters the exact ordinary delayed Rebound trigger"
    );
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == rebound_trigger_id));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(!counter_batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == ChoiceKind::SpecialCast as i32
    )));
    assert_eq!(
        engine.state.objects[&opt_object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt_object_id],
        exile_generation
    );
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != opt_object_id));
    assert!(engine.initial_response_batch().legal_by_player[&controller]
        .zone_cast_actions
        .iter()
        .all(|action| action.object_id != opt_object_id));
    narset_c1_assert_no_scheduled_rebounds(&engine, "the countered delayed Rebound trigger");

    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(&mut engine, controller);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    narset_c1_assert_no_scheduled_rebounds(
        &engine,
        "the opponent's upkeep after Tidebinder counters Rebound",
    );
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(&mut engine, opponent);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(triggered_stack_ids(&engine).is_empty());
    assert_eq!(
        engine.state.objects[&opt_object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt_object_id],
        exile_generation
    );
    narset_c1_assert_no_scheduled_rebounds(
        &engine,
        "the next own upkeep after Tidebinder counters Rebound",
    );
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != opt_object_id));
}

#[test]
fn narset_phase_c2_printed_artful_maneuver_resolves_and_schedules_once() {
    use tricerules_proto::ruled::v1::StackResolveDestination;

    const ARTFUL_MANEUVER_RON: &str = r#"(
      id: "artful_maneuver",
      name: "Artful Maneuver",
      face_id: "artful_maneuver",
      mana_cost: "{1}{W}",
      types: ["Instant"],
      spell_keywords: [Rebound],
      spell_effect: [PumpTarget(power: 2, toughness: 2)],
    )"#;
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(
            &[
                ARTFUL_MANEUVER_RON,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect(
            "the printed Artful Maneuver and shipped Grizzly Bears fixture definitions register",
        )
    });

    let decks = Some(vec![
        deck_with("forest", &["artful_maneuver", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(registry, 6_110_404, &[0, 1], 20, decks, true)
        .expect("printed Artful Maneuver prepared-state fixture engine");
    advance_to_main1_from_game_start(&mut engine);

    let grizzly = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let grizzly_generation = engine
        .state
        .zone_change_generation
        .get(&grizzly)
        .copied()
        .unwrap_or(0);
    let controller = engine.state.players[0].id;
    assert_eq!(
        engine.state.objects[&grizzly].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.effective_power(grizzly), Some(2));
    assert_eq!(engine.effective_toughness(grizzly), Some(2));

    let (artful, cast_batch) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        0,
        "artful_maneuver",
        0,
        0,
        vec![TargetRef {
            expected_zone_change_generation: Some(grizzly_generation),
            object_id: grizzly,
            damage_amount: 0,
            group_index: 0,
            kind: 0,
        }],
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let stack_generation = artful
        .zone_change_generation
        .expect("the actual Artful Maneuver hand cast has a stack generation");
    let artful_owner = engine.state.objects[&artful.object_id].owner;
    assert_eq!(artful_owner, controller);
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.object_id == artful.object_id))
            .count(),
        1,
        "the physical hand cast publishes only its own StackPushed event"
    );
    assert!(engine.state.stack.iter().all(|item| !item.is_triggered));
    assert_eq!(serialized_observer_count(&engine), 0);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
    narset_c1_assert_no_scheduled_rebounds(&engine, "before printed Artful Maneuver resolves");
    assert_eq!(
        engine.state.objects[&artful.object_id].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.zone_change_generation[&artful.object_id],
        stack_generation
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&grizzly)
            .copied()
            .unwrap_or(0),
        grizzly_generation,
        "casting does not change the targeted Grizzly incarnation"
    );
    assert_eq!(engine.effective_power(grizzly), Some(2));
    assert_eq!(engine.effective_toughness(grizzly), Some(2));

    let presentation = engine.state.stack_presentations[&artful.object_id]
        .primary
        .clone();
    let resolution = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &resolution,
        artful.object_id,
        artful_owner,
        StackResolveDestination::Exile,
    );

    let exile_generation = engine.state.zone_change_generation[&artful.object_id];
    assert_eq!(exile_generation, stack_generation + 1);
    assert_eq!(
        engine.state.objects[&artful.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(engine.state.objects[&artful.object_id].owner, artful_owner);
    assert_eq!(
        engine.state.players[0]
            .exile
            .iter()
            .filter(|object_id| **object_id == artful.object_id)
            .count(),
        1,
        "the exact physical Artful Maneuver is exiled once"
    );
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&grizzly)
            .copied()
            .unwrap_or(0),
        grizzly_generation,
        "resolving the pump keeps the same Grizzly incarnation"
    );
    assert_eq!(engine.effective_power(grizzly), Some(4));
    assert_eq!(engine.effective_toughness(grizzly), Some(4));
    assert_eq!(serialized_observer_count(&engine), 0);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    let snapshot = engine
        .diagnostic_snapshot()
        .expect("serialize complete state with the printed Rebound schedule");
    let scheduled = snapshot["state"]["scheduled_rebounds"]
        .as_array()
        .expect("diagnostics expose scheduled-Rebound records");
    assert_eq!(
        scheduled.len(),
        1,
        "printed Artful Maneuver schedules Rebound once"
    );
    let record = &scheduled[0];
    assert_eq!(
        record["source"]["object_id"],
        serde_json::json!(artful.object_id)
    );
    assert_eq!(
        record["source"]["zone_change_generation"],
        serde_json::json!(stack_generation.to_string())
    );
    assert_eq!(
        record["source"]["controller_at_event"],
        serde_json::json!(controller)
    );
    assert_eq!(
        record["exiled"]["object_id"],
        serde_json::json!(artful.object_id)
    );
    assert_eq!(
        record["exiled"]["zone_change_generation"],
        serde_json::json!(exile_generation.to_string())
    );
    assert_eq!(
        record["exiled"]["controller_at_event"],
        serde_json::json!(controller)
    );
    assert_eq!(record["controller"], serde_json::json!(controller));
    assert_eq!(record["card_id"], serde_json::json!("artful_maneuver"));
    assert_eq!(record["card_name"], serde_json::json!("Artful Maneuver"));
    assert_eq!(record["source_face_index"], serde_json::json!("0"));
    assert_eq!(
        record["presentation"],
        serde_json::to_value(&presentation)
            .expect("serialize the captured Artful Maneuver presentation")
    );
}
#[test]
fn narset_phase_c2_model_keeps_rebound_grants_and_executor_closed() {
    use tricerules_card_model::card_def::{ModalDef, ModeDef, RawCardDefinition};
    use tricerules_card_model::primitives::{
        Amount, AuthoredBlock, CardProgram, CardTypeFilter, CompiledProgram, EffectContext,
        Instruction, InstructionKind, PlayerRecipient, SpellCastFilter, SpellEffectKind,
        SpellKeyword, StaticAbilityDef, StaticEmblemEffect,
    };
    use tricerules_card_model::registry::RegistryError;
    use tricerules_card_model::{AbilityPresentation, CardFaceId, ModeId, ProgramOwnerId};

    let expect_rebound_rejection =
        |raw: &RawCardDefinition, case: &str| match narset_phase_b_model_registry(raw) {
            Err(RegistryError::InvalidCard { id, reason }) => {
                assert_eq!(id, raw.id, "{case} must fail for the mutated card itself");
                assert!(
                    reason.contains("Rebound"),
                    "{case} must identify the Rebound profile violation, got {reason:?}"
                );
            }
            Err(RegistryError::Ron(error)) => {
                panic!("{case} unexpectedly failed as a RON parse error: {error}")
            }
            Ok(_) => panic!("registry accepted invalid Rebound grant {case}"),
        };

    let mut static_grant = narset_phase_b_parse_model_baseline(
        r#"(
            id: "narset_rebound_static_grant_fixture",
            name: "Narset Rebound Static Grant Fixture",
            face_id: "narset_rebound_static_grant_fixture",
            mana_cost: "{1}",
            types: ["Enchantment"],
            static_abilities: [(
                ability_id: "grant_01",
                presentation: Fallback,
                definition: GrantSpellKeyword(
                    filter: (card_type: Some(InstantOrSorcery)),
                    keyword: Cascade,
                ),
            )],
        )"#,
    );
    narset_phase_b_expect_model_admitted(
        &static_grant,
        "the enchantment's static Instant-or-Sorcery Cascade grant",
    );
    let static_ability = static_grant
        .static_abilities
        .iter_mut()
        .find(|ability| ability.ability_id.as_str() == "grant_01")
        .expect("the static grant is found by its authored ability ID");
    match &mut static_ability.definition {
        StaticAbilityDef::GrantSpellKeyword { keyword, .. } => {
            *keyword = SpellKeyword::Rebound;
        }
        other => panic!("grant_01 is the expected GrantSpellKeyword, got {other:?}"),
    }
    expect_rebound_rejection(&static_grant, "static GrantSpellKeyword(Rebound)");

    let storm_emblem = || SpellEffectKind::CreateStaticEmblem {
        emblem_id: "narset_rebound_model_emblem".into(),
        display_name: "Narset Rebound Model Emblem".into(),
        effects: vec![StaticEmblemEffect::GrantSpellKeyword {
            filter: SpellCastFilter {
                card_type: Some(CardTypeFilter::InstantOrSorcery),
                ..Default::default()
            },
            keyword: SpellKeyword::Storm,
        }],
    };
    let set_emblem_grant_to_rebound = |effect: &mut SpellEffectKind, case: &str| {
        let SpellEffectKind::CreateStaticEmblem { effects, .. } = effect else {
            panic!("{case} contains the expected CreateStaticEmblem")
        };
        let StaticEmblemEffect::GrantSpellKeyword { keyword, .. } = effects
            .get_mut(0)
            .unwrap_or_else(|| panic!("{case} emblem has its grant effect"))
        else {
            panic!("{case} emblem has the expected GrantSpellKeyword")
        };
        *keyword = SpellKeyword::Rebound;
    };

    let mut activated_emblem =
        narset_phase_b_parse_model_baseline(NARSET_PHASE_B_MODEL_ACTIVATED_BASELINE);
    narset_phase_b_activated_mut(&mut activated_emblem, "activated_01").effect =
        vec![storm_emblem()];
    narset_phase_b_expect_model_admitted(
        &activated_emblem,
        "the activated artifact's static Storm emblem grant",
    );
    let activated = narset_phase_b_activated_mut(&mut activated_emblem, "activated_01");
    let [emblem_effect] = activated.effect.as_mut_slice() else {
        panic!("activated_01 retains its single static emblem instruction")
    };
    set_emblem_grant_to_rebound(emblem_effect, "activated_01");
    expect_rebound_rejection(&activated_emblem, "activated Storm emblem Rebound grant");

    let mut triggered_modal_emblem =
        narset_phase_b_parse_model_baseline(NARSET_PHASE_B_MODEL_TRIGGERED_BASELINE);
    let triggered = narset_phase_b_triggered_mut(&mut triggered_modal_emblem, "triggered_01");
    triggered.effect.clear();
    triggered.modal = Some(ModalDef {
        min_modes: 1,
        max_modes: 1,
        all_modes_cast_cost: None,
        trigger_mode_selection_restriction: None,
        modes: vec![ModeDef {
            mode_id: ModeId::new("emblem_01").expect("valid stable model mode ID"),
            presentation: AbilityPresentation::Fallback,
            linked_cast_cost: None,
            effects: vec![storm_emblem()],
            program: None,
            targeting: None,
        }],
    });
    narset_phase_b_expect_model_admitted(
        &triggered_modal_emblem,
        "the triggered ability's one-mode Storm emblem grant",
    );

    let mut rejected_triggered_modal = triggered_modal_emblem.clone();
    let mode = narset_phase_b_triggered_mut(&mut rejected_triggered_modal, "triggered_01")
        .modal
        .as_mut()
        .expect("triggered_01 owns the admitted modal")
        .modes
        .iter_mut()
        .find(|mode| mode.mode_id.as_str() == "emblem_01")
        .expect("the Storm emblem mode is found by its authored mode ID");
    let [emblem_effect] = mode.effects.as_mut_slice() else {
        panic!("emblem_01 retains its single static emblem instruction")
    };
    set_emblem_grant_to_rebound(emblem_effect, "triggered_01/emblem_01");
    expect_rebound_rejection(
        &rejected_triggered_modal,
        "triggered modal Storm emblem Rebound grant",
    );

    let gain_life = SpellEffectKind::GainLife {
        who: PlayerRecipient::Controller,
        amount: Amount::Fixed(1),
    };
    SpellEffectKind::validate_list(std::slice::from_ref(&gain_life))
        .expect("ordinary GainLife remains valid in a legacy effect list");
    gain_life
        .validate(EffectContext::Ability)
        .expect("ordinary GainLife remains valid in an ability context");

    let cast_rebounded = SpellEffectKind::CastReboundedCard;
    let list_error = SpellEffectKind::validate_list(std::slice::from_ref(&cast_rebounded))
        .expect_err("CastReboundedCard is reserved outside authored effect lists");
    assert!(
        list_error.contains("CastReboundedCard"),
        "effect-list rejection identifies the reserved executor, got {list_error:?}"
    );
    let ability_error = cast_rebounded
        .validate(EffectContext::Ability)
        .expect_err("CastReboundedCard is reserved outside engine-created delayed triggers");
    assert!(
        ability_error.contains("CastReboundedCard"),
        "ability-context rejection identifies the reserved executor, got {ability_error:?}"
    );

    let one_perform_program = |effect| CardProgram {
        body: AuthoredBlock {
            steps: vec![Instruction {
                id: "perform_01".into(),
                bind: Vec::new(),
                input: None,
                kind: InstructionKind::Perform(Box::new(effect)),
            }],
            yields: Vec::new(),
        },
        target_bindings: Vec::new(),
        target_aliases: Vec::new(),
    };
    let owner = ProgramOwnerId::face(
        "narset_rebound_model_spell_fixture",
        CardFaceId::new("narset_rebound_model_spell_fixture").expect("valid stable model face ID"),
    );
    CompiledProgram::compile(
        owner.clone(),
        EffectContext::Spell,
        &one_perform_program(gain_life.clone()),
    )
    .expect("the existing one-instruction Program admits ordinary GainLife");
    assert!(
        CompiledProgram::compile(
            owner,
            EffectContext::Spell,
            &one_perform_program(cast_rebounded),
        )
        .is_err(),
        "the otherwise-identical one-instruction Program rejects CastReboundedCard"
    );
}

const NARSET_C2_ARTFUL_MANEUVER_RON: &str = r#"(
  id: "artful_maneuver",
  name: "Artful Maneuver",
  face_id: "artful_maneuver",
  mana_cost: "{1}{W}",
  types: ["Instant"],
  spell_keywords: [Rebound],
  spell_effect: [PumpTarget(power: 2, toughness: 2)],
)"#;

fn narset_phase_c2_interaction_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(
            &[
                REBOUND_OBSERVER_RON,
                NARSET_C2_ARTFUL_MANEUVER_RON,
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!("../../../tricerules-cards/data/village_rites.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect(
            "the C2 observer, printed Rebound spell, shipped cost spell, and creatures register",
        )
    })
}

struct NarsetC2ArtfulScheduleFixture {
    engine: GameEngine,
    controller: i32,
    opponent: i32,
    artful_object_id: u32,
    artful_stack_generation: u64,
    artful_exile_generation: u64,
    targets: Vec<u32>,
}

fn narset_phase_c2_artful_schedule_fixture(
    seed: u64,
    include_second_creature: bool,
) -> NarsetC2ArtfulScheduleFixture {
    let deck_cards = if include_second_creature {
        vec![
            REBOUND_OBSERVER,
            "artful_maneuver",
            "grizzly_bears",
            "grizzly_bears",
        ]
    } else {
        vec![REBOUND_OBSERVER, "artful_maneuver", "grizzly_bears"]
    };
    let decks = Some(vec![
        deck_with("forest", &deck_cards),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        narset_phase_c2_interaction_registry(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("C2 printed Artful Maneuver fixture engine");
    advance_to_main1_from_game_start(&mut engine);

    let source = move_fixture_card_to_zone(&mut engine, 0, REBOUND_OBSERVER, DevZone::Battlefield);
    let source_generation = engine.state.zone_change_generation[&source];
    arm_narset_observer(&mut engine, source);

    let target_a = move_fixture_card_to_zone(&mut engine, 0, "grizzly_bears", DevZone::Battlefield);
    let target_a_generation = engine.state.zone_change_generation[&target_a];
    let mut targets = vec![target_a];
    if include_second_creature {
        let target_b =
            move_fixture_card_to_zone(&mut engine, 0, "grizzly_bears", DevZone::Battlefield);
        assert_ne!(target_a, target_b);
        targets.push(target_b);
    }
    let controller = engine.state.players[0].id;
    let opponent = engine.state.players[1].id;
    let (artful, cast_batch) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        0,
        "artful_maneuver",
        0,
        0,
        vec![TargetRef {
            expected_zone_change_generation: Some(target_a_generation),
            object_id: target_a,
            damage_amount: 0,
            group_index: 0,
            kind: TargetRefKind::Permanent as i32,
        }],
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let artful_stack_generation = artful
        .zone_change_generation
        .expect("the actual Artful Maneuver hand cast has its stack generation");
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == artful.object_id && !item.is_triggered));
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(_))))
            .count(),
        2,
        "the actual Artful Maneuver and its external observer trigger are each pushed once"
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, artful);
    assert_eq!(
        serialized_rebound_grants(&engine),
        serde_json::json!([]),
        "the external Rebound grant is not installed before its actual trigger resolves"
    );

    narset_c1_capture_priority_round(&mut engine);
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(artful),
        "the external observer grants Rebound to this exact printed Artful Maneuver cast"
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, artful.object_id);

    let resolution = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &resolution,
        artful.object_id,
        controller,
        tricerules_proto::ruled::v1::StackResolveDestination::Exile,
    );
    assert_eq!(
        engine.state.objects[&artful.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&artful.object_id],
        artful_stack_generation + 1
    );
    assert_eq!(engine.effective_power(target_a), Some(4));
    assert_eq!(engine.effective_toughness(target_a), Some(4));
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    let schedule = narset_c1_exclusion_scheduled_rebounds(&engine);
    let records = schedule
        .as_array()
        .expect("diagnostics expose the scheduled-Rebound records");
    assert_eq!(
        records.len(),
        1,
        "printed and externally granted Rebound schedule this exact cast only once"
    );
    assert_eq!(
        records[0]["source"]["object_id"],
        serde_json::json!(artful.object_id)
    );
    assert_eq!(
        records[0]["source"]["zone_change_generation"],
        serde_json::json!(artful_stack_generation.to_string())
    );
    let artful_exile_generation = engine.state.zone_change_generation[&artful.object_id];
    assert_eq!(
        records[0]["exiled"]["zone_change_generation"],
        serde_json::json!(artful_exile_generation.to_string())
    );
    assert_eq!(records[0]["card_id"], serde_json::json!("artful_maneuver"));
    assert_eq!(records[0]["controller"], serde_json::json!(controller));

    NarsetC2ArtfulScheduleFixture {
        engine,
        controller,
        opponent,
        artful_object_id: artful.object_id,
        artful_stack_generation,
        artful_exile_generation,
        targets,
    }
}

fn narset_phase_c2_advance_to_next_own_rebound_upkeep(
    engine: &mut GameEngine,
    controller: i32,
    opponent: i32,
    card_id: &str,
    object_id: u32,
    stack_generation: u64,
    expected_exile_generations: (u64, u64),
) -> u32 {
    let (current_exile_generation, watched_rebound_exile_generation) = expected_exile_generations;
    let scheduled_before_opponent_turn = narset_c1_exclusion_scheduled_rebounds(engine);
    assert_eq!(
        scheduled_before_opponent_turn.as_array().map(Vec::len),
        Some(1),
        "one exact Rebound schedule remains before the opponent's turn"
    );

    end_active_turn(engine, controller);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(engine),
        scheduled_before_opponent_turn,
        "the opponent's upkeep leaves the current Rebound schedule untouched"
    );
    assert_eq!(
        engine.state.objects[&object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&object_id],
        current_exile_generation
    );

    pass_priority_round(engine);
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(engine, opponent);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);

    assert_eq!(
        engine.state.stack.len(),
        1,
        "the one-shot Rebound schedule stages one ordinary trigger at its controller's own upkeep"
    );
    let trigger = &engine.state.stack[0];
    assert!(trigger.is_triggered);
    assert!(!trigger.is_copy);
    assert_eq!(trigger.card_id, card_id);
    assert_eq!(trigger.controller, controller);
    assert_eq!(trigger.source_permanent_id, Some(object_id));
    assert_eq!(trigger.source_zone_change, stack_generation);
    assert_eq!(
        trigger.trigger_context.observed_object,
        Some(tricerules_core::state::TriggerObjectRef {
            object_id,
            zone_change_generation: watched_rebound_exile_generation,
            controller_at_event: controller,
        })
    );
    let trigger_id = trigger.id;
    assert_eq!(triggered_stack_ids(engine), vec![trigger_id]);
    narset_c1_assert_no_scheduled_rebounds(engine, "the one-shot own-upkeep Rebound trigger");
    trigger_id
}

fn narset_phase_c2_assert_no_next_own_upkeep_rebound(
    engine: &mut GameEngine,
    controller: i32,
    opponent: i32,
    object_id: u32,
) {
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    narset_c1_assert_no_scheduled_rebounds(engine, "after the one-shot Rebound opportunity");

    pass_priority_round(engine);
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(engine, controller);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert!(triggered_stack_ids(engine).is_empty());
    narset_c1_assert_no_scheduled_rebounds(engine, "the opponent's upkeep after the opportunity");
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != object_id));

    pass_priority_round(engine);
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(engine, opponent);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(triggered_stack_ids(engine).is_empty());
    narset_c1_assert_no_scheduled_rebounds(engine, "the next own upkeep after the opportunity");
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != object_id));
}

fn narset_phase_c2_dev_move_card(
    engine: &mut GameEngine,
    player_id: i32,
    card_name: &str,
    zone: DevZone,
) -> RuledEventBatch {
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("perform the explicitly prepared physical-zone movement")
}

fn narset_phase_c2_village_rites_sacrifice_selection(
    choices: &tricerules_proto::ruled::v1::LegalCostChoices,
    object_id: u32,
    generation: u64,
) -> tricerules_proto::ruled::v1::CostSelection {
    use tricerules_proto::ruled::v1 as rv1;

    assert!(choices.non_mana_costs_payable);
    assert_eq!(choices.choices.len(), 1);
    let choice = &choices.choices[0];
    assert_eq!(choice.kind, rv1::CostChoiceKind::Sacrifice as i32);
    assert_eq!(choice.zone, rv1::CostChoiceZone::Battlefield as i32);
    assert_eq!(choice.min, 1);
    assert_eq!(choice.max, 1);
    assert_eq!(choice.cost_index, 0);
    let candidates = choice
        .candidate_objects
        .iter()
        .filter_map(|candidate| candidate.object.as_ref())
        .filter(|candidate| candidate.object_id == object_id)
        .collect::<Vec<_>>();
    assert_eq!(
        candidates.len(),
        1,
        "the live cost offer publishes the chosen creature exactly once"
    );
    assert_eq!(
        candidates[0].zone_change_generation, generation,
        "the sacrifice uses the exact current candidate generation"
    );
    rv1::CostSelection {
        cost_index: choice.cost_index,
        selection: Some(rv1::cost_selection::Selection::PermanentId(
            candidates[0].object_id,
        )),
    }
}

fn narset_phase_c2_village_rites_zone_action(
    engine: &mut GameEngine,
    controller: i32,
    village_rites: u32,
) -> tricerules_proto::ruled::v1::LegalZoneCastAction {
    let legal = engine.initial_response_batch();
    let actions = legal
        .legal_by_player
        .get(&controller)
        .expect("the Rebound controller has current legal actions")
        .zone_cast_actions
        .iter()
        .filter(|action| action.object_id == village_rites)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        actions.len(),
        1,
        "Village Rites has one current Rebound cast action"
    );
    actions.into_iter().next().unwrap()
}

#[test]
fn narset_phase_c2_printed_artful_and_external_rebound_deduplicate_and_use_fresh_target() {
    use tricerules_proto::ruled::v1 as rv1;

    let NarsetC2ArtfulScheduleFixture {
        mut engine,
        controller,
        opponent,
        artful_object_id,
        artful_stack_generation,
        artful_exile_generation,
        targets,
    } = narset_phase_c2_artful_schedule_fixture(6_110_405, true);
    let target_a = targets[0];
    let target_b = targets[1];
    assert_ne!(target_a, target_b);

    let trigger_id = narset_phase_c2_advance_to_next_own_rebound_upkeep(
        &mut engine,
        controller,
        opponent,
        "artful_maneuver",
        artful_object_id,
        artful_stack_generation,
        (artful_exile_generation, artful_exile_generation),
    );
    assert_eq!(
        engine.effective_power(target_a),
        Some(2),
        "the original target's first-turn pump expires before the own-upkeep recast"
    );
    assert_eq!(
        engine.effective_toughness(target_a),
        Some(2),
        "the original target's first-turn toughness bonus expires before the own-upkeep recast"
    );

    let trigger_resolution = narset_c1_capture_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    let special_offer = find_resolution_choice(&trigger_resolution)
        .expect("the exact exiled Artful Maneuver has a current Rebound offer");
    assert_eq!(
        special_offer.choice_kind,
        rv1::ChoiceKind::SpecialCast as i32
    );
    assert_eq!(special_offer.source_object_id, trigger_id);
    assert_eq!(special_offer.candidate_object_ids, vec![artful_object_id]);
    assert_eq!(special_offer.candidate_selectable, vec![true]);
    let (deciding_player, current_prompt_id) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the delayed trigger parks at the printed Artful Maneuver offer");
        assert_eq!(pending.deciding_player, controller);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
        assert_eq!(pending.presentation.candidates, vec![artful_object_id]);
        let tricerules_core::state::ResolutionContinuation::SpecialCast {
            stack,
            exiled,
            method,
            cost,
            ..
        } = &pending.continuation
        else {
            panic!("the delayed trigger owns the SpecialCast continuation");
        };
        assert_eq!(stack.item.id, trigger_id);
        assert_eq!(exiled.object_id, artful_object_id);
        assert_eq!(exiled.zone_change_generation, artful_exile_generation);
        assert_eq!(*method, tricerules_core::state::SpellCastMethod::Permission);
        assert_eq!(*cost, tricerules_cards::mana::ManaCost::default());
        (pending.deciding_player, pending.resolution_prompt_id.get())
    };
    assert_eq!(special_offer.resolution_prompt_id, current_prompt_id);

    let (face_index, cast_method, permission_id, fresh_target) = {
        let current = engine.initial_response_batch();
        let legal = current
            .legal_by_player
            .get(&controller)
            .expect("the Rebound controller receives the current target and cast offers");
        let actions = legal
            .zone_cast_actions
            .iter()
            .filter(|action| action.object_id == artful_object_id)
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 1);
        let action = actions[0];
        assert_eq!(action.source_zone, rv1::CastSourceZone::Exile as i32);
        assert_eq!(action.zone_change_generation, artful_exile_generation);
        assert_eq!(action.face_index, 0);
        assert_eq!(action.cast_method, rv1::CastMethod::Permission as i32);
        assert_eq!(
            action.cost, "",
            "Rebound waives Artful Maneuver's mana cost"
        );
        let permission_id = action
            .casting_permission_id
            .expect("the current permission action carries its opaque permission id");
        assert_ne!(permission_id, 0);

        let targets_key = (u64::from(artful_object_id) << 8) | u64::from(action.face_index);
        let offered_targets = legal
            .valid_targets_by_zone_object
            .get(&targets_key)
            .expect("the current exile action publishes Artful Maneuver's target groups");
        assert_eq!(offered_targets.groups.len(), 1);
        let group = &offered_targets.groups[0];
        assert_eq!(group.min, 1);
        assert_eq!(group.max, 1);
        assert!(group.valid_permanent_ids.contains(&target_b));
        assert!(group.valid_permanent_ids.contains(&target_a));
        let target_generation = engine.state.zone_change_generation[&target_b];
        (
            action.face_index,
            action.cast_method,
            permission_id,
            rv1::TargetRef {
                object_id: target_b,
                expected_zone_change_generation: Some(target_generation),
                damage_amount: 0,
                group_index: group.group_index,
                kind: rv1::TargetRefKind::Permanent as i32,
            },
        )
    };
    assert_eq!(fresh_target.object_id, target_b);
    assert_ne!(fresh_target.object_id, target_a);

    let announcement = rv1::SpellCastAnnouncement {
        targets: vec![fresh_target],
        face_index,
        source: Some(exile_cast_source(artful_object_id, artful_exile_generation)),
        cast_method,
        casting_permission_id: Some(permission_id),
        ..Default::default()
    };
    let begin_batch = engine
        .apply_command(
            deciding_player,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                    resolution_prompt_id: current_prompt_id,
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    spell_cast_announcement: Some(announcement),
                    ..Default::default()
                })),
            },
        )
        .expect("accept the current published Artful Maneuver Rebound offer and target B");
    assert!(begin_batch.events.iter().all(|event| {
        !matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.object_id == artful_object_id)
    }));
    assert_eq!(
        engine.state.objects[&artful_object_id].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.zone_change_generation[&artful_object_id],
        artful_exile_generation + 1
    );
    let transaction_id = {
        let pending_cast = engine
            .state
            .pending_spell_cast
            .as_ref()
            .expect("the current Rebound choice starts a payment transaction");
        assert!(pending_cast.resolution_time_offer);
        assert_eq!(pending_cast.caster, controller);
        assert_eq!(pending_cast.reserved_object_id, artful_object_id);
        assert_eq!(pending_cast.original_zone, tricerules_core::Zone::Exile);
        assert_eq!(
            pending_cast.original_zone_change_generation,
            artful_exile_generation
        );
        pending_cast.transaction_id
    };
    let parked_parent = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the delayed parent remains suspended during the child cast");
    let tricerules_core::state::ResolutionContinuation::SpecialCast { stack, .. } =
        &parked_parent.continuation
    else {
        panic!("the parked parent remains the delayed Rebound trigger");
    };
    assert_eq!(stack.item.id, trigger_id);

    let controller_index = engine
        .state
        .player_idx(controller)
        .expect("the Rebound controller remains in the game");
    let mana_before_commit = engine.state.players[controller_index].mana_pool;
    let revision = engine.state.command_index;
    let preview = engine.preview_payment_scenario(
        controller,
        &rv1::PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(rv1::CommitSpellCast {
                transaction_id,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    assert_eq!(engine.state.command_index, revision);
    assert_eq!(
        engine.state.players[controller_index].mana_pool,
        mana_before_commit
    );
    let cast_batch = engine
        .apply_scenario_command(
            controller,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                    transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .expect("commit the actual Rebound Artful Maneuver cast");
    assert_eq!(
        engine.state.players[controller_index].mana_pool,
        mana_before_commit
    );
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.pending_resolution.is_none());
    let parent_resolved_index = cast_batch
        .events
        .iter()
        .position(|event| {
            matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger_id)
        })
        .expect("the delayed Rebound parent resolves exactly once after its child commits");
    assert!(cast_batch
        .events
        .iter()
        .any(|event| { matches!(&event.ev, Some(Ev::PriorityChanged(_))) }));
    for (event_index, event) in cast_batch.events.iter().enumerate() {
        if matches!(&event.ev, Some(Ev::PriorityChanged(_))) {
            assert!(
                parent_resolved_index < event_index,
                "the delayed parent resolves before priority is published"
            );
        }
    }
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.object_id == artful_object_id))
            .count(),
        1
    );
    let recast = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == artful_object_id)
        .expect("the exact Artful Maneuver is the committed physical spell");
    assert_eq!(recast.card_id, "artful_maneuver");
    assert_eq!(recast.controller, controller);
    assert_eq!(recast.cast_by, Some(controller));
    assert_eq!(
        recast.cast_method,
        tricerules_core::state::SpellCastMethod::Permission
    );
    assert_eq!(recast.targets.len(), 1);
    assert_eq!(recast.targets[0].object_id, target_b);
    assert_eq!(
        recast.targets[0].zone_change_generation,
        Some(engine.state.zone_change_generation[&target_b])
    );
    assert!(!recast.is_copy);
    assert!(!recast.is_triggered);
    assert!(recast.cast_occurrence.is_some());
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != artful_object_id));
    narset_c1_assert_no_scheduled_rebounds(&engine, "the accepted printed Artful Maneuver recast");

    let resolution = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &resolution,
        artful_object_id,
        controller,
        rv1::StackResolveDestination::Graveyard,
    );
    assert_eq!(
        engine.state.objects[&artful_object_id].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.zone_change_generation[&artful_object_id],
        artful_exile_generation + 2
    );
    assert_eq!(engine.effective_power(target_a), Some(2));
    assert_eq!(engine.effective_toughness(target_a), Some(2));
    assert_eq!(engine.effective_power(target_b), Some(4));
    assert_eq!(engine.effective_toughness(target_b), Some(4));
    narset_c1_assert_no_scheduled_rebounds(&engine, "the completed Rebound Artful Maneuver recast");
    narset_phase_c2_assert_no_next_own_upkeep_rebound(
        &mut engine,
        controller,
        opponent,
        artful_object_id,
    );
    assert_eq!(
        engine.state.objects[&artful_object_id].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.zone_change_generation[&artful_object_id],
        artful_exile_generation + 2
    );
}
#[test]
fn narset_phase_c2_rebound_village_rites_keeps_mandatory_sacrifice_across_cancel_and_retry() {
    use tricerules_proto::ruled::v1 as rv1;

    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                REBOUND_OBSERVER,
                "village_rites",
                "grizzly_bears",
                "grizzly_bears",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        narset_phase_c2_interaction_registry(),
        6_110_406,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Village Rites C2 interaction fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = move_fixture_card_to_zone(&mut engine, 0, REBOUND_OBSERVER, DevZone::Battlefield);
    let source_generation = engine.state.zone_change_generation[&source];
    arm_narset_observer(&mut engine, source);
    let sacrifice_a =
        move_fixture_card_to_zone(&mut engine, 0, "grizzly_bears", DevZone::Battlefield);
    let sacrifice_b =
        move_fixture_card_to_zone(&mut engine, 0, "grizzly_bears", DevZone::Battlefield);
    assert_ne!(sacrifice_a, sacrifice_b);
    let sacrifice_a_generation = engine.state.zone_change_generation[&sacrifice_a];
    ensure_in_hand(&mut engine, 0, "village_rites");
    let controller = engine.state.players[0].id;
    let opponent = engine.state.players[1].id;

    let (hand_action, first_cost_selection) = {
        let current = engine.initial_response_batch();
        let legal = current
            .legal_by_player
            .get(&controller)
            .expect("the active Rebound controller has current hand actions");
        let actions = legal
            .hand_actions
            .iter()
            .filter(|action| action.card_name == "Village Rites")
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 1);
        let action = actions.into_iter().next().unwrap();
        assert_eq!(action.cost, "{B}");
        let costs = action
            .cost_choices
            .as_ref()
            .expect("the current Village Rites hand action publishes its additional cost");
        (
            action.clone(),
            narset_phase_c2_village_rites_sacrifice_selection(
                costs,
                sacrifice_a,
                sacrifice_a_generation,
            ),
        )
    };
    give_mana(
        &mut engine,
        controller,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let mut first_cast = cast_spell_with_costs(
        hand_action.hand_index as usize,
        Vec::new(),
        vec![first_cost_selection.clone()],
    );
    authoring_actions::pay(&engine, controller, &mut first_cast)
        .expect("preview payment for the actual hand Village Rites cast");
    let first_cast_batch = engine
        .apply_scenario_command(controller, &first_cast)
        .expect("cast shipped Village Rites with its live sacrifice receipt");
    let village_rites = engine
        .state
        .stack
        .iter()
        .find(|item| {
            item.card_id == "village_rites"
                && item.cast_by == Some(controller)
                && !item.is_triggered
                && !item.is_copy
        })
        .and_then(|item| item.cast_occurrence)
        .expect("the physical hand-cast Village Rites has its actual occurrence");
    assert_eq!(
        first_cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.object_id == village_rites.object_id))
            .count(),
        1
    );
    assert_eq!(
        engine.state.objects[&sacrifice_a].zone,
        tricerules_core::Zone::Graveyard
    );
    let first_sacrifice_generation = sacrifice_a_generation + 1;
    assert_eq!(
        engine.state.zone_change_generation[&sacrifice_a],
        first_sacrifice_generation
    );
    assert_eq!(
        engine.state.objects[&sacrifice_b].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.stack.len(), 2);
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, village_rites);
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    narset_c1_capture_priority_round(&mut engine);
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(village_rites),
        "the observer grants Rebound to this exact shipped Village Rites cast"
    );
    let first_resolution_baseline = engine.state.players[0].hand.len();
    let first_resolution = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &first_resolution,
        village_rites.object_id,
        controller,
        rv1::StackResolveDestination::Exile,
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        first_resolution_baseline + 2,
        "the actual first Village Rites resolution draws two cards"
    );
    let village_exile_generation = engine.state.zone_change_generation[&village_rites.object_id];
    assert_eq!(
        engine.state.objects[&village_rites.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        village_exile_generation,
        village_rites
            .zone_change_generation
            .expect("the exact Village Rites stack occurrence has its generation")
            + 1
    );
    let first_schedule = narset_c1_exclusion_scheduled_rebounds(&engine);
    assert_eq!(first_schedule.as_array().map(Vec::len), Some(1));
    assert_eq!(
        first_schedule[0]["exiled"]["zone_change_generation"],
        serde_json::json!(village_exile_generation.to_string())
    );
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));

    let trigger_id = narset_phase_c2_advance_to_next_own_rebound_upkeep(
        &mut engine,
        controller,
        opponent,
        "village_rites",
        village_rites.object_id,
        village_rites.zone_change_generation.unwrap(),
        (village_exile_generation, village_exile_generation),
    );
    let trigger_resolution = narset_c1_capture_priority_round(&mut engine);
    let special_offer = find_resolution_choice(&trigger_resolution)
        .expect("the exact exiled Village Rites has its current Rebound cast offer");
    assert_eq!(
        special_offer.choice_kind,
        rv1::ChoiceKind::SpecialCast as i32
    );
    assert_eq!(special_offer.source_object_id, trigger_id);
    assert_eq!(
        special_offer.candidate_object_ids,
        vec![village_rites.object_id]
    );
    assert_eq!(special_offer.candidate_selectable, vec![true]);
    let (deciding_player, first_prompt_id) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the delayed trigger parks at the shipped Village Rites cast offer");
        assert_eq!(pending.deciding_player, controller);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
        let tricerules_core::state::ResolutionContinuation::SpecialCast {
            stack,
            exiled,
            method,
            cost,
            ..
        } = &pending.continuation
        else {
            panic!("the delayed trigger owns the Village Rites SpecialCast continuation");
        };
        assert_eq!(stack.item.id, trigger_id);
        assert_eq!(exiled.object_id, village_rites.object_id);
        assert_eq!(exiled.zone_change_generation, village_exile_generation);
        assert_eq!(*method, tricerules_core::state::SpellCastMethod::Permission);
        assert_eq!(*cost, tricerules_cards::mana::ManaCost::default());
        (pending.deciding_player, pending.resolution_prompt_id.get())
    };
    assert_eq!(special_offer.resolution_prompt_id, first_prompt_id);

    let first_action =
        narset_phase_c2_village_rites_zone_action(&mut engine, controller, village_rites.object_id);
    assert_eq!(first_action.source_zone, rv1::CastSourceZone::Exile as i32);
    assert_eq!(
        first_action.zone_change_generation,
        village_exile_generation
    );
    assert_eq!(first_action.face_index, 0);
    assert_eq!(first_action.cast_method, rv1::CastMethod::Permission as i32);
    assert_eq!(first_action.cost, "", "Rebound waives only the mana cost");
    let first_permission_id = first_action
        .casting_permission_id
        .expect("the live Rebound action publishes its permission id");
    assert_ne!(first_permission_id, 0);
    let first_cost_choices = first_action
        .cost_choices
        .as_ref()
        .expect("the current Rebound action retains Village Rites' additional cost");
    let first_retry_cost_selection = narset_phase_c2_village_rites_sacrifice_selection(
        first_cost_choices,
        sacrifice_b,
        engine.state.zone_change_generation[&sacrifice_b],
    );
    assert!(
        first_cost_choices.choices[0]
            .candidate_objects
            .iter()
            .filter_map(|candidate| candidate.object.as_ref())
            .any(|receipt| receipt.object_id == sacrifice_b),
        "the zero-mana Rebound offer still publishes the required sacrifice"
    );
    let first_announcement = rv1::SpellCastAnnouncement {
        source: Some(exile_cast_source(
            village_rites.object_id,
            village_exile_generation,
        )),
        face_index: first_action.face_index,
        cast_method: first_action.cast_method,
        casting_permission_id: Some(first_permission_id),
        cost_selections: vec![first_retry_cost_selection],
        ..Default::default()
    };
    let begin_batch = engine
        .apply_command(
            deciding_player,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                    resolution_prompt_id: first_prompt_id,
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    spell_cast_announcement: Some(first_announcement),
                    ..Default::default()
                })),
            },
        )
        .expect("begin the real Village Rites Rebound payment transaction");
    assert!(begin_batch.events.iter().all(|event| {
        !matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.object_id == village_rites.object_id)
    }));
    assert_eq!(
        engine.state.objects[&village_rites.object_id].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.zone_change_generation[&village_rites.object_id],
        village_exile_generation + 1
    );
    assert_eq!(
        engine.state.objects[&sacrifice_b].zone,
        tricerules_core::Zone::Battlefield
    );
    let hand_before_cancel = engine.state.players[0].hand.clone();
    let mana_before_cancel = engine.state.players[0].mana_pool;
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the accepted offer creates a pending spell payment")
        .transaction_id;
    assert!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .resolution_time_offer
    );
    assert_eq!(
        engine.state.pending_spell_cast.as_ref().unwrap().caster,
        controller
    );
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .original_zone_change_generation,
        village_exile_generation
    );
    assert!(engine.state.pending_resolution.is_some());

    let cancel_batch = engine
        .apply_scenario_command(
            controller,
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(rv1::CancelSpellCast {
                    transaction_id,
                })),
            },
        )
        .expect("cancel the current Village Rites cast without paying its costs");
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(
        engine.state.objects[&village_rites.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&village_rites.object_id],
        village_exile_generation
    );
    assert_eq!(
        engine.state.objects[&sacrifice_b].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.players[0].hand, hand_before_cancel);
    assert_eq!(engine.state.players[0].mana_pool, mana_before_cancel);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the delayed parent is restored")
            .deciding_player,
        controller
    );
    assert!(engine.state.stack.is_empty());
    assert!(cancel_batch.events.iter().all(|event| {
        !matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger_id)
    }));
    narset_c1_assert_no_scheduled_rebounds(&engine, "after cancelling the pending cast");
    let restored_offer = find_resolution_choice(&cancel_batch)
        .expect("cancellation republishes the current Rebound offer");
    assert_eq!(
        restored_offer.choice_kind,
        rv1::ChoiceKind::SpecialCast as i32
    );
    assert_eq!(restored_offer.source_object_id, trigger_id);
    assert_eq!(
        restored_offer.candidate_object_ids,
        vec![village_rites.object_id]
    );
    let restored_prompt_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the parent resolution is restored after cancellation")
        .resolution_prompt_id
        .get();
    assert_eq!(restored_offer.resolution_prompt_id, restored_prompt_id);

    let retry_action =
        narset_phase_c2_village_rites_zone_action(&mut engine, controller, village_rites.object_id);
    assert_eq!(retry_action.source_zone, rv1::CastSourceZone::Exile as i32);
    assert_eq!(
        retry_action.zone_change_generation,
        village_exile_generation
    );
    assert_eq!(retry_action.cast_method, rv1::CastMethod::Permission as i32);
    assert_eq!(retry_action.cost, "");
    let retry_permission_id = retry_action
        .casting_permission_id
        .expect("the reissued Rebound action has a current permission id");
    let retry_cost_choices = retry_action
        .cost_choices
        .as_ref()
        .expect("the reissued action retains the mandatory sacrifice cost");
    let retry_cost_selection = narset_phase_c2_village_rites_sacrifice_selection(
        retry_cost_choices,
        sacrifice_b,
        engine.state.zone_change_generation[&sacrifice_b],
    );
    assert!(retry_cost_choices.choices[0]
        .candidate_objects
        .iter()
        .filter_map(|candidate| candidate.object.as_ref())
        .any(|candidate| {
            candidate.object_id == sacrifice_b
                && candidate.zone_change_generation
                    == engine.state.zone_change_generation[&sacrifice_b]
        }));
    let retry_announcement = rv1::SpellCastAnnouncement {
        source: Some(exile_cast_source(
            village_rites.object_id,
            village_exile_generation,
        )),
        face_index: retry_action.face_index,
        cast_method: retry_action.cast_method,
        casting_permission_id: Some(retry_permission_id),
        cost_selections: vec![retry_cost_selection],
        ..Default::default()
    };
    let retry_begin = engine
        .apply_command(
            controller,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                    resolution_prompt_id: restored_prompt_id,
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    spell_cast_announcement: Some(retry_announcement),
                    ..Default::default()
                })),
            },
        )
        .expect("retry Village Rites with the reissued prompt and current cost receipt");
    assert!(retry_begin.events.iter().all(|event| {
        !matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.object_id == village_rites.object_id)
    }));
    assert!(engine.state.pending_resolution.is_some());

    let (transaction_id, controller_index) = {
        let pending = engine
            .state
            .pending_spell_cast
            .as_ref()
            .expect("the retried actual cast awaits payment");
        assert!(pending.resolution_time_offer);
        assert_eq!(pending.caster, controller);
        assert_eq!(pending.reserved_object_id, village_rites.object_id);
        assert_eq!(pending.original_zone, tricerules_core::Zone::Exile);
        assert_eq!(
            pending.original_zone_change_generation,
            village_exile_generation
        );
        (
            pending.transaction_id,
            engine
                .state
                .player_idx(controller)
                .expect("the current PlayerId is valid"),
        )
    };
    let mana_before_commit = engine.state.players[controller_index].mana_pool;
    let revision = engine.state.command_index;
    let preview = engine.preview_payment_scenario(
        controller,
        &rv1::PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(rv1::CommitSpellCast {
                transaction_id,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    assert_eq!(
        engine.state.command_index, revision,
        "preview remains query-only"
    );
    assert_eq!(
        engine.state.players[controller_index].mana_pool,
        mana_before_commit
    );
    let cast_batch = engine
        .apply_scenario_command(
            controller,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                    transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .expect("commit the retried Village Rites cast using the actual PlayerId payment preview");
    assert_eq!(
        engine.state.players[controller_index].mana_pool,
        mana_before_commit
    );
    assert_eq!(
        engine.state.objects[&sacrifice_b].zone,
        tricerules_core::Zone::Graveyard
    );
    let second_sacrifice_generation = retry_cost_choices.choices[0]
        .candidate_objects
        .iter()
        .filter_map(|candidate| candidate.object.as_ref())
        .find(|candidate| candidate.object_id == sacrifice_b)
        .expect("the reissued offer had B's generation-bound receipt")
        .zone_change_generation
        + 1;
    assert_eq!(
        engine.state.zone_change_generation[&sacrifice_b],
        second_sacrifice_generation
    );
    assert_eq!(
        engine.state.objects[&village_rites.object_id].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.zone_change_generation[&village_rites.object_id],
        village_exile_generation + 1
    );
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.pending_resolution.is_none());
    let parent_resolved_indices = cast_batch
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger_id)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        parent_resolved_indices.len(),
        1,
        "the parked delayed trigger resumes and resolves exactly once"
    );
    assert!(cast_batch
        .events
        .iter()
        .any(|event| { matches!(&event.ev, Some(Ev::PriorityChanged(_))) }));
    for (event_index, event) in cast_batch.events.iter().enumerate() {
        if matches!(&event.ev, Some(Ev::PriorityChanged(_))) {
            assert!(
                parent_resolved_indices[0] < event_index,
                "the Rebound parent resolves before priority is published"
            );
        }
    }
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.object_id == village_rites.object_id))
            .count(),
        1,
        "the physical Village Rites recast is pushed exactly once"
    );
    let recast = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == village_rites.object_id)
        .expect("the exact exiled Village Rites is committed as a physical spell");
    assert_eq!(recast.card_id, "village_rites");
    assert_eq!(recast.cast_by, Some(controller));
    assert_eq!(
        recast.cast_method,
        tricerules_core::state::SpellCastMethod::Permission
    );
    assert!(!recast.is_triggered);
    assert!(!recast.is_copy);
    assert!(recast.cast_occurrence.is_some());
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != village_rites.object_id));

    let hand_before_rebound_resolution = engine.state.players[controller_index].hand.len();
    let recast_resolution = narset_c1_capture_priority_round(&mut engine);
    narset_c1_assert_one_stack_resolved(
        &recast_resolution,
        village_rites.object_id,
        controller,
        rv1::StackResolveDestination::Graveyard,
    );
    assert_eq!(
        engine.state.players[controller_index].hand.len(),
        hand_before_rebound_resolution + 2,
        "the actual recast Village Rites resolution draws two cards"
    );
    assert_eq!(
        engine.state.objects[&village_rites.object_id].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.zone_change_generation[&village_rites.object_id],
        village_exile_generation + 2
    );
    assert_eq!(
        engine.state.objects[&sacrifice_a].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&sacrifice_b].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_spell_cast.is_none());
    narset_c1_assert_no_scheduled_rebounds(&engine, "the completed Village Rites Rebound recast");
    narset_phase_c2_assert_no_next_own_upkeep_rebound(
        &mut engine,
        controller,
        opponent,
        village_rites.object_id,
    );
    assert_eq!(
        engine.state.objects[&village_rites.object_id].zone,
        tricerules_core::Zone::Graveyard
    );
}
#[test]
fn narset_phase_c2_stale_rebound_exile_generation_does_not_cast_returned_opt() {
    use tricerules_proto::ruled::v1 as rv1;

    let decks = Some(vec![
        deck_with("forest", &[REBOUND_OBSERVER, "opt"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        narset_phase_c2_interaction_registry(),
        6_110_407,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("single-Opt stale-generation fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let controller = engine.state.players[0].id;
    let opponent = engine.state.players[1].id;
    let source = move_fixture_card_to_zone(&mut engine, 0, REBOUND_OBSERVER, DevZone::Battlefield);
    let source_generation = engine.state.zone_change_generation[&source];
    arm_narset_observer(&mut engine, source);
    ensure_in_hand(&mut engine, 0, "opt");
    assert_eq!(
        engine
            .state
            .objects
            .values()
            .filter(|object| object.card_id == "opt")
            .count(),
        1,
        "the physical deck contains exactly one Opt"
    );
    let (opt, cast_batch) = narset_c1_cast_actual_hand_spell(
        &mut engine,
        0,
        "opt",
        0,
        0,
        Vec::new(),
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(_))))
            .count(),
        2,
        "the actual Opt and its external observer trigger are each pushed once"
    );
    assert_one_rebound_trigger_for_cast(&engine, source, source_generation, opt);
    narset_c1_capture_priority_round(&mut engine);
    assert_eq!(
        serialized_rebound_grants(&engine),
        expected_rebound_grants_for_cast(opt)
    );

    let scry_batch = narset_c1_capture_priority_round(&mut engine);
    let scry_offer =
        find_resolution_choice(&scry_batch).expect("the actual Opt publishes its real Scry choice");
    assert_eq!(scry_offer.source_object_id, opt.object_id);
    assert_eq!(scry_offer.choice_kind, rv1::ChoiceKind::LibraryTop as i32);
    let (deciding_player, scry_prompt_id, candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the actual Opt parks at Scry");
        assert_eq!(pending.deciding_player, controller);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryTop);
        (
            pending.deciding_player,
            pending.resolution_prompt_id.get(),
            pending.presentation.candidates.clone(),
        )
    };
    assert_eq!(scry_offer.resolution_prompt_id, scry_prompt_id);
    let mut scry_answer = submit_resolution_choice(candidates);
    let Some(Cmd::SubmitResolutionChoice(answer)) = scry_answer.cmd.as_mut() else {
        panic!("the current Opt Scry answer is a SubmitResolutionChoice");
    };
    answer.resolution_prompt_id = scry_offer.resolution_prompt_id;
    let opt_resolution = engine
        .apply_command(deciding_player, &scry_answer)
        .expect("answer the actual Opt's currently published Scry choice");
    narset_c1_assert_one_stack_resolved(
        &opt_resolution,
        opt.object_id,
        controller,
        rv1::StackResolveDestination::Exile,
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    let opt_stack_generation = opt
        .zone_change_generation
        .expect("the actual Opt retains its exact stack incarnation");
    let old_exile_generation = engine.state.zone_change_generation[&opt.object_id];
    assert_eq!(old_exile_generation, opt_stack_generation + 1);
    assert_eq!(engine.state.objects[&opt.object_id].owner, controller);
    assert_eq!(
        engine.state.players[0]
            .exile
            .iter()
            .filter(|id| **id == opt.object_id)
            .count(),
        1
    );
    let saved_schedule = narset_c1_exclusion_scheduled_rebounds(&engine);
    assert_eq!(saved_schedule.as_array().map(Vec::len), Some(1));
    assert_eq!(
        saved_schedule[0]["exiled"]["object_id"],
        serde_json::json!(opt.object_id)
    );
    assert_eq!(
        saved_schedule[0]["exiled"]["zone_change_generation"],
        serde_json::json!(old_exile_generation.to_string())
    );
    assert_eq!(serialized_rebound_grants(&engine), serde_json::json!([]));
    let filler = engine.state.players[0]
        .library
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "forest")
        .expect("a different registered filler remains in the library");
    let filler_generation = engine
        .state
        .zone_change_generation
        .get(&filler)
        .copied()
        .unwrap_or(0);

    let card_name = narset_phase_c2_interaction_registry()
        .get("opt")
        .expect("Opt remains registered")
        .name
        .clone();
    let to_hand = narset_phase_c2_dev_move_card(&mut engine, controller, &card_name, DevZone::Hand);
    assert!(to_hand.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::PermanentMoved(moved)) if moved.object_id == opt.object_id)
    ));
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Hand
    );
    assert!(engine.state.players[0].hand.contains(&opt.object_id));
    assert_eq!(
        engine.state.zone_change_generation[&opt.object_id],
        old_exile_generation + 1
    );
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(&engine),
        saved_schedule,
        "leaving the watched exile incarnation does not rewrite its saved receipt"
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&filler)
            .copied()
            .unwrap_or(0),
        filler_generation
    );
    assert!(engine.state.players[0].library.contains(&filler));

    let to_exile =
        narset_phase_c2_dev_move_card(&mut engine, controller, &card_name, DevZone::Exile);
    assert!(to_exile.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::PermanentMoved(moved)) if moved.object_id == opt.object_id)
    ));
    let new_exile_generation = engine.state.zone_change_generation[&opt.object_id];
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert!(engine.state.players[0].exile.contains(&opt.object_id));
    assert_eq!(new_exile_generation, old_exile_generation + 2);
    assert_ne!(new_exile_generation, old_exile_generation);
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(&engine),
        saved_schedule,
        "the saved Rebound watches only the old exile incarnation"
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&filler)
            .copied()
            .unwrap_or(0),
        filler_generation
    );
    assert!(engine.state.players[0].library.contains(&filler));
    assert_eq!(
        engine
            .state
            .objects
            .values()
            .filter(|object| object.card_id == "opt")
            .count(),
        1,
        "DevMoveCard cannot select a different Opt from the library"
    );

    let trigger_id = narset_phase_c2_advance_to_next_own_rebound_upkeep(
        &mut engine,
        controller,
        opponent,
        "opt",
        opt.object_id,
        opt_stack_generation,
        (new_exile_generation, old_exile_generation),
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt.object_id],
        new_exile_generation
    );
    let trigger_resolution = narset_c1_capture_priority_round(&mut engine);
    let resolved_triggers = trigger_resolution
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger_id => Some(resolved),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(resolved_triggers.len(), 1);
    assert!(trigger_resolution.events.iter().all(|event| {
        !matches!(
            &event.ev,
            Some(Ev::ResolutionChoiceRequired(choice))
                if choice.choice_kind == ChoiceKind::SpecialCast as i32
        )
    }));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_spell_cast.is_none());
    narset_c1_assert_no_scheduled_rebounds(&engine, "the stale watched exile incarnation");
    let current_legal = engine.initial_response_batch();
    let legal = &current_legal.legal_by_player[&controller];
    assert!(
        legal
            .zone_cast_actions
            .iter()
            .all(|action| action.object_id != opt.object_id),
        "the returned Opt's new exile incarnation has no Rebound cast permission"
    );
    assert!(
        !legal
            .valid_targets_by_zone_object
            .contains_key(&(u64::from(opt.object_id) << 8)),
        "no cast target group is published for the stale Rebound opportunity"
    );
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != opt.object_id));
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt.object_id],
        new_exile_generation
    );

    narset_phase_c2_assert_no_next_own_upkeep_rebound(
        &mut engine,
        controller,
        opponent,
        opt.object_id,
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&opt.object_id],
        new_exile_generation
    );
}

#[test]
fn narset_phase_c2_printed_artful_without_legal_creature_target_consumes_rebound_once() {
    let NarsetC2ArtfulScheduleFixture {
        mut engine,
        controller,
        opponent,
        artful_object_id,
        artful_stack_generation,
        artful_exile_generation,
        targets,
    } = narset_phase_c2_artful_schedule_fixture(6_110_408, false);
    let removed_creature = targets[0];
    let removed_generation = engine.state.zone_change_generation[&removed_creature];
    let schedule_before_removal = narset_c1_exclusion_scheduled_rebounds(&engine);

    let removed = move_fixture_card_to_zone(&mut engine, 0, "grizzly_bears", DevZone::Graveyard);
    assert_eq!(removed, removed_creature);
    assert_eq!(
        engine.state.objects[&removed_creature].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.zone_change_generation[&removed_creature],
        removed_generation + 1,
        "the exact previously targeted creature has a new graveyard incarnation"
    );
    assert!(engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter())
        .all(|object_id| !engine
            .characteristics(*object_id)
            .expect("every battlefield object has characteristics")
            .is_creature()));
    assert_eq!(
        narset_c1_exclusion_scheduled_rebounds(&engine),
        schedule_before_removal,
        "prepared creature movement leaves the exact delayed Rebound receipt unchanged"
    );
    assert_eq!(
        engine.state.zone_change_generation[&artful_object_id],
        artful_exile_generation
    );

    let trigger_id = narset_phase_c2_advance_to_next_own_rebound_upkeep(
        &mut engine,
        controller,
        opponent,
        "artful_maneuver",
        artful_object_id,
        artful_stack_generation,
        (artful_exile_generation, artful_exile_generation),
    );
    let trigger_resolution = narset_c1_capture_priority_round(&mut engine);
    let resolved_trigger = trigger_resolution
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger_id => Some(resolved),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        resolved_trigger.len(),
        1,
        "the ordinary delayed trigger resolves once despite there being no legal creature targets"
    );
    assert!(
        trigger_resolution.events.iter().all(|event| {
            !matches!(
                &event.ev,
                Some(Ev::ResolutionChoiceRequired(choice))
                    if choice.choice_kind == ChoiceKind::SpecialCast as i32
            )
        }),
        "the targetless Rebound trigger publishes no cast choice"
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(
        engine.state.objects[&artful_object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&artful_object_id],
        artful_exile_generation
    );
    assert_eq!(
        engine.state.objects[&removed_creature].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.zone_change_generation[&removed_creature],
        removed_generation + 1
    );
    narset_c1_assert_no_scheduled_rebounds(
        &engine,
        "the printed Artful trigger with no legal target",
    );
    let current_legal = engine.initial_response_batch();
    let legal = &current_legal.legal_by_player[&controller];
    assert!(legal
        .zone_cast_actions
        .iter()
        .all(|action| action.object_id != artful_object_id));
    assert!(
        !legal
            .valid_targets_by_zone_object
            .contains_key(&(u64::from(artful_object_id) << 8)),
        "the empty current creature target set prevents a Rebound cast action"
    );
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != artful_object_id));

    narset_phase_c2_assert_no_next_own_upkeep_rebound(
        &mut engine,
        controller,
        opponent,
        artful_object_id,
    );
    assert_eq!(
        engine.state.objects[&artful_object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&artful_object_id],
        artful_exile_generation
    );
}
