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
