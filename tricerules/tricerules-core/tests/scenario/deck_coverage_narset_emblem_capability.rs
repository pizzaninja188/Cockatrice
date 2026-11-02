//! Prepared-state capability proof for a source-independent Narset emblem restriction.
//! The inline Artifact is a capability fixture, not an implementation of Narset itself.
use std::sync::OnceLock;

use crate::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_proto::ruled::v1::{
    dev_command, BeginSpellCast, CastMethod, CommitSpellCast, DevCommand, DevMoveCard, DevZone,
    HandActionKind, PaymentMana, PaymentSelection, PreviewPayment, SpellCastAnnouncement,
};

const ARTIFACT_ID: &str = "narset_emblem_test_artifact";
const ARTIFACT_NAME: &str = "Narset Emblem Test Artifact";
const CONTROLLER_INDEX: usize = 0;
const OPPONENT_INDEX: usize = 1;

const EMBLEM_ARTIFACT_RON: &str = r#"(
    id: "narset_emblem_test_artifact",
    name: "Narset Emblem Test Artifact",
    face_id: "narset_emblem_test_artifact",
    mana_cost: "{0}",
    types: ["Artifact"],
    activated_abilities: [(
        ability_id: "activated_01",
        presentation: Fallback,
        costs: [Tap],
        effect: [CreateStaticEmblem(
            emblem_id: "narset_emblem_capability_fixture",
            display_name: "Narset Emblem Capability Fixture",
            effects: [OpponentsCantCastNoncreatureSpells],
        )],
    )],
)"#;

fn narset_emblem_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(
            &[
                EMBLEM_ARTIFACT_RON,
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
            ],
            &[],
        )
        .expect("the synthetic Artifact, shipped Opt, and filler Island register")
    })
}

fn move_fixture_card_with_dev_command(
    engine: &mut GameEngine,
    actor: i32,
    card_name: &str,
    zone: DevZone,
) {
    let command = RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: actor,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: card_name.into(),
                zone: zone as i32,
                ready: true,
            })),
        })),
    };
    engine
        .apply_command(actor, &command)
        .expect("move the fixture card through an engine Dev command");
}

fn prepared_game(seed: u64) -> (GameEngine, u32, u32, i32, i32) {
    let decks = Some(vec![
        deck_with("island", &[ARTIFACT_ID]),
        deck_with("island", &["opt"]),
    ]);
    let mut engine = GameEngine::new(narset_emblem_registry(), seed, &[17, 29], 20, decks, true)
        .expect("Narset emblem capability fixture engine");

    let controller_id = engine.state.players[CONTROLLER_INDEX].id;
    let opponent_id = engine.state.players[OPPONENT_INDEX].id;
    assert_ne!(controller_id, opponent_id);
    assert_ne!(controller_id as usize, CONTROLLER_INDEX);
    assert_ne!(opponent_id as usize, OPPONENT_INDEX);

    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), controller_id);

    let battlefield_before = engine.state.players[CONTROLLER_INDEX].battlefield.clone();
    engine.enable_dev_commands();
    move_fixture_card_with_dev_command(
        &mut engine,
        controller_id,
        ARTIFACT_NAME,
        DevZone::Battlefield,
    );
    let source = engine.state.players[CONTROLLER_INDEX]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| !battlefield_before.contains(object_id))
        .expect("the synthetic Artifact entered the controller's battlefield");

    ensure_in_hand(&mut engine, OPPONENT_INDEX, "opt");
    let opt = engine.state.players[OPPONENT_INDEX]
        .hand
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "opt")
        .expect("the opponent has the physical Opt in hand");
    give_mana(
        &mut engine,
        opponent_id,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    assert!(engine.state.players[OPPONENT_INDEX].hand.contains(&opt));
    assert_eq!(
        engine.state.players[OPPONENT_INDEX].mana_pool,
        tricerules_core::state::ManaPool {
            blue: 1,
            ..Default::default()
        }
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.blocking_choice().is_none());

    (engine, source, opt, controller_id, opponent_id)
}

fn begin_normal_opt(engine: &GameEngine) -> (usize, RuledCommand) {
    let hand_index = hand_index_for_card(engine, OPPONENT_INDEX, "opt");
    let command = RuledCommand {
        cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
            announcement: Some(SpellCastAnnouncement {
                cast_method: CastMethod::Normal as i32,
                source: Some(hand_cast_source(hand_index)),
                ..Default::default()
            }),
        })),
    };
    (hand_index, command)
}

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

#[test]
fn narset_phase_d_emblem_blocks_opponent_noncreature_begin_atomically_after_source_leaves() {
    const SEED: u64 = 6_110_201;

    let (mut control, _control_source, control_opt, control_controller, control_opponent) =
        prepared_game(SEED);
    let (mut restricted, source, opt, controller, opponent) = prepared_game(SEED);
    assert_eq!(control_controller, controller);
    assert_eq!(control_opponent, opponent);
    assert_eq!(control_opt, opt);

    control
        .apply_command(control_controller, &pass())
        .expect("the active controller passes Main1 priority to the opponent");
    assert_eq!(control.state.priority_player_id(), control_opponent);
    let control_hand_index = hand_index_for_card(&control, OPPONENT_INDEX, "opt");
    let control_offer = control.initial_response_batch();
    assert!(
        control_offer.legal_by_player[&control_opponent]
            .hand_actions
            .iter()
            .any(|action| {
                action.kind == HandActionKind::HandActionCastSpell as i32
                    && action.hand_index as usize == control_hand_index
                    && action.face_index == 0
            }),
        "the unrestricted control offers the opponent's Opt cast"
    );

    let (control_hand_index, control_begin) = begin_normal_opt(&control);
    assert_eq!(
        control.state.players[OPPONENT_INDEX].hand[control_hand_index],
        control_opt
    );
    control
        .apply_command(control_opponent, &control_begin)
        .expect("the otherwise-legal targetless normal Opt Begin succeeds");
    let pending = control
        .state
        .pending_spell_cast
        .as_ref()
        .expect("Begin opens the real Opt payment transaction");
    let transaction_id = pending.transaction_id;
    assert_eq!(pending.caster, control_opponent);
    assert_eq!(pending.reserved_object_id, control_opt);
    assert_eq!(
        control.state.objects[&control_opt].zone,
        tricerules_core::Zone::Stack
    );
    assert!(control.state.stack.is_empty());
    assert_eq!(
        control.state.players[OPPONENT_INDEX].mana_pool.blue, 1,
        "Begin reserves the physical card without spending its mana"
    );

    let revision = control.state.command_index;
    let before_preview = control
        .diagnostic_snapshot()
        .expect("snapshot the pending Opt transaction before preview");
    let preview = control.preview_payment_scenario(
        control_opponent,
        &PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(CommitSpellCast {
                transaction_id,
                payment: Some(PaymentSelection {
                    mana: Some(PaymentMana {
                        u: 1,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    assert_eq!(
        control.diagnostic_snapshot().unwrap(),
        before_preview,
        "payment preview leaves the pending transaction untouched"
    );
    let selection = preview
        .selection
        .expect("the successful Opt preview returns its payment selection");
    control
        .apply_command(
            control_opponent,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: Some(selection),
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .expect("commit the previewed normal Opt cast");
    assert!(control.state.pending_spell_cast.is_none());
    assert_eq!(control.state.stack.len(), 1, "one physical Opt is pushed");
    assert_eq!(control.state.stack[0].id, control_opt);
    assert_eq!(control.state.stack[0].card_id, "opt");
    assert!(!control.state.stack[0].is_copy);
    assert!(control.state.stack[0].cast_occurrence.is_some());
    assert_eq!(
        control.state.objects[&control_opt].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        control.state.players[OPPONENT_INDEX].mana_pool,
        Default::default(),
        "the committed Opt spends exactly the opponent's one blue mana"
    );

    let activation = authoring_actions::activation(&mut restricted, controller, source, 0)
        .expect("the engine offers the synthetic Artifact activation");
    restricted
        .apply_command(controller, &activation)
        .expect("activate the offered emblem ability");
    assert_eq!(restricted.state.stack.len(), 1);
    assert_eq!(restricted.state.stack[0].source_permanent_id, Some(source));
    let battlefield_generation = generation(&restricted, source);
    move_fixture_card_with_dev_command(
        &mut restricted,
        controller,
        ARTIFACT_NAME,
        DevZone::Graveyard,
    );
    let graveyard_generation = generation(&restricted, source);
    assert_eq!(
        restricted.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_ne!(graveyard_generation, battlefield_generation);
    assert!(
        restricted.state.players[CONTROLLER_INDEX]
            .graveyard
            .contains(&source),
        "the exact activated source has left the battlefield before resolution"
    );

    resolve_entire_stack_two_player(&mut restricted);
    assert!(restricted.state.stack.is_empty());
    assert!(restricted.state.pending_resolution.is_none());
    assert_eq!(restricted.state.static_emblems.len(), 1);
    let emblem = &restricted.state.static_emblems[0];
    assert_eq!(emblem.controller, controller);
    assert_eq!(
        restricted.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    let marker_id = emblem.object_id;

    restricted
        .apply_command(controller, &pass())
        .expect("the emblem controller passes Main1 priority to the opponent");
    assert_eq!(restricted.state.active_player_id(), controller);
    assert_eq!(restricted.state.turn_step, tricerules_core::TurnStep::Main1);
    assert_eq!(restricted.state.priority_player_id(), opponent);
    assert!(restricted.state.players[OPPONENT_INDEX].hand.contains(&opt));
    assert_eq!(
        restricted.state.players[OPPONENT_INDEX].mana_pool,
        tricerules_core::state::ManaPool {
            blue: 1,
            ..Default::default()
        }
    );
    assert!(restricted.state.stack.is_empty());
    assert!(restricted.state.pending_spell_cast.is_none());
    assert!(restricted.state.blocking_choice().is_none());

    let (hand_index, forged_begin) = begin_normal_opt(&restricted);
    assert_eq!(
        restricted.state.players[OPPONENT_INDEX].hand[hand_index],
        opt
    );
    let before_rejected_begin = restricted
        .diagnostic_snapshot()
        .expect("snapshot the emblem-restricted state before forged Begin");
    let command_index_before = restricted.state.command_index;
    let opponent_state_index = restricted
        .state
        .player_idx(opponent)
        .expect("the opponent's actual PlayerId maps to its state index");
    let hand_before = restricted.state.players[opponent_state_index].hand.clone();
    let mana_before = restricted.state.players[opponent_state_index].mana_pool;
    let opt_zone_before = restricted.state.objects[&opt].zone;
    let opt_generation_before = generation(&restricted, opt);
    let stack_before = serde_json::to_value(&restricted.state.stack)
        .expect("serialize the stack before forged Begin");
    let pending_before = serde_json::to_value(&restricted.state.pending_spell_cast)
        .expect("serialize pending cast state before forged Begin");

    let rejected = restricted.apply_command(opponent, &forged_begin);
    assert!(
        rejected.is_err(),
        "BeginSpellCast must reject the opponent's forbidden Opt before reserving it"
    );
    assert_eq!(
        restricted.diagnostic_snapshot().unwrap(),
        before_rejected_begin,
        "rejected Begin preserves the complete diagnostic state"
    );
    assert_eq!(restricted.state.command_index, command_index_before);
    assert_eq!(
        restricted.state.players[opponent_state_index].hand,
        hand_before
    );
    assert_eq!(
        restricted.state.players[opponent_state_index].mana_pool,
        mana_before
    );
    assert_eq!(restricted.state.objects[&opt].zone, opt_zone_before);
    assert_eq!(generation(&restricted, opt), opt_generation_before);
    assert_eq!(
        serde_json::to_value(&restricted.state.stack).unwrap(),
        stack_before
    );
    assert_eq!(
        serde_json::to_value(&restricted.state.pending_spell_cast).unwrap(),
        pending_before
    );

    let restricted_batch = restricted.initial_response_batch();
    assert!(
        restricted_batch.events.iter().any(|event| {
            matches!(&event.ev, Some(Ev::ZoneView(view)) if view.per_player.iter().any(|player| {
                player.player_id == controller
                    && player.static_emblems.iter().any(|marker| {
                        marker.object_id == marker_id
                            && marker.display_name == "Narset Emblem Capability Fixture"
                    })
            }))
        }),
        "the public per-player view retains the emblem marker after its source leaves"
    );
    assert!(
        !restricted_batch.legal_by_player[&opponent]
            .hand_actions
            .iter()
            .any(|action| {
                action.kind == HandActionKind::HandActionCastSpell as i32
                    && action.hand_index as usize == hand_index
                    && action.face_index == 0
            }),
        "the opponent is not offered the Opt cast under the persistent emblem"
    );
}

fn narset_phase_d_multiplayer_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(
            &[
                EMBLEM_ARTIFACT_RON,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!("../../../tricerules-cards/data/ornithopter.ron"),
                include_str!("../../../tricerules-cards/data/swamp.ron"),
                include_str!("../../../tricerules-cards/data/village_rites.ron"),
            ],
            &[],
        )
        .expect("the emblem fixture and shipped Phase D cards register")
    })
}

fn narset_phase_d_three_player_game(seed: u64, create_emblem: bool) -> (GameEngine, [i32; 3]) {
    let decks = Some(vec![
        deck_with("forest", &[ARTIFACT_ID, "opt"]),
        deck_with("forest", &["opt", "grizzly_bears"]),
        deck_with("forest", &["opt", "ornithopter"]),
    ]);
    let mut engine = GameEngine::new(
        narset_phase_d_multiplayer_registry(),
        seed,
        &[17, 29, 41],
        20,
        decks,
        true,
    )
    .expect("three-player Narset Phase D capability fixture");
    advance_to_main1_from_game_start(&mut engine);

    let player_ids = [
        engine.state.players[0].id,
        engine.state.players[1].id,
        engine.state.players[2].id,
    ];
    for (seat, player_id) in player_ids.iter().copied().enumerate() {
        assert_ne!(player_id as usize, seat, "seat indices are not PlayerIds");
        assert_eq!(
            player_ids
                .iter()
                .filter(|other| **other == player_id)
                .count(),
            1,
            "each multiplayer seat has a distinct PlayerId"
        );
    }
    assert_eq!(engine.state.active_player_id(), player_ids[0]);
    assert_eq!(engine.state.priority_player_id(), player_ids[0]);

    if create_emblem {
        let battlefield_before = engine.state.players[0].battlefield.clone();
        engine.enable_dev_commands();
        move_fixture_card_with_dev_command(
            &mut engine,
            player_ids[0],
            ARTIFACT_NAME,
            DevZone::Battlefield,
        );
        let source = engine.state.players[0]
            .battlefield
            .iter()
            .copied()
            .find(|object_id| !battlefield_before.contains(object_id))
            .expect("the synthetic Artifact entered A's battlefield");
        let activation = authoring_actions::activation(&mut engine, player_ids[0], source, 0)
            .expect("A is offered the synthetic emblem ability");
        engine
            .apply_command(player_ids[0], &activation)
            .expect("A activates the synthetic emblem ability");
        pass_priority_round(&mut engine);
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.static_emblems.len(), 1);
        assert_eq!(engine.state.static_emblems[0].controller, player_ids[0]);
        assert!(engine.state.static_emblems[0]
            .effects
            .iter()
            .any(|effect| matches!(
                effect,
                tricerules_card_model::primitives::StaticEmblemEffect::OpponentsCantCastNoncreatureSpells
            )));
    }

    for (seat, card_id) in [
        (0, "opt"),
        (1, "opt"),
        (2, "opt"),
        (1, "grizzly_bears"),
        (1, "forest"),
        (2, "ornithopter"),
    ] {
        ensure_card_in_hand(&mut engine, seat, card_id);
    }
    for player_id in player_ids {
        give_mana(
            &mut engine,
            player_id,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
    }

    (engine, player_ids)
}

fn narset_phase_d_prepare_main1_priority(
    engine: &mut GameEngine,
    active_seat: usize,
    priority_seat: usize,
    turn: u32,
) {
    assert!(
        engine.state.stack.is_empty(),
        "prepare priority with an empty stack"
    );
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.blocking_choice().is_none());

    // This capability fixture deliberately prepares the phase and priority directly so every
    // active turn/player-priority pair can be covered without a two-player turn driver.
    engine.state.active_player_idx = active_seat;
    engine.state.priority_idx = priority_seat;
    engine.state.turn_step = tricerules_core::TurnStep::Main1;
    engine.state.turn = turn;
    engine.state.turn_instance = u64::from(turn);
    engine.state.passes_since_stack_change = 0;
    engine.state.lands_played_this_turn = 0;
}

fn narset_phase_d_begin_normal_spell(
    engine: &GameEngine,
    caster_seat: usize,
    card_id: &str,
) -> (usize, RuledCommand) {
    let hand_index = hand_index_for_card(engine, caster_seat, card_id);
    let command = RuledCommand {
        cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
            announcement: Some(SpellCastAnnouncement {
                cast_method: CastMethod::Normal as i32,
                source: Some(hand_cast_source(hand_index)),
                ..Default::default()
            }),
        })),
    };
    (hand_index, command)
}

fn narset_phase_d_commit_normal_spell(
    engine: &mut GameEngine,
    caster: i32,
    transaction_id: u64,
    mana: PaymentMana,
) {
    let revision = engine.state.command_index;
    let preview = engine.preview_payment_scenario(
        caster,
        &PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(CommitSpellCast {
                transaction_id,
                payment: Some(PaymentSelection {
                    expected_state_revision: revision,
                    mana: Some(mana),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(
        preview.valid && preview.complete,
        "otherwise-legal cast preview: {preview:?}"
    );
    let selection = preview
        .selection
        .expect("complete cast preview returns its payment selection");
    engine
        .apply_command(
            caster,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: Some(selection),
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .expect("commit the previewed normal spell cast");
}

fn narset_phase_d_inject_emblem(engine: &mut GameEngine, controller: i32) -> u32 {
    let object_id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    engine
        .state
        .static_emblems
        .push(tricerules_core::state::StaticEmblemInstance {
            object_id,
            controller,
            emblem_id: "narset_phase_d_test_emblem".into(),
            display_name: "Narset Phase D Test Emblem".into(),
            effects: vec![
                tricerules_card_model::primitives::StaticEmblemEffect::OpponentsCantCastNoncreatureSpells,
            ],
            triggered_ability: None,
        });
    object_id
}

#[test]
fn narset_phase_d_emblem_blocks_both_opponents_across_turns_but_preserves_other_actions() {
    const SEED: u64 = 6_110_202;

    let (mut control, control_ids) = narset_phase_d_three_player_game(SEED, false);
    let (mut restricted, player_ids) = narset_phase_d_three_player_game(SEED + 1, true);
    assert_ne!(control_ids[0], control_ids[1]);
    assert_ne!(control_ids[1], control_ids[2]);
    assert_ne!(player_ids[0], player_ids[1]);
    assert_ne!(player_ids[1], player_ids[2]);
    assert!(control.state.static_emblems.is_empty());
    assert_eq!(restricted.state.static_emblems.len(), 1);

    // Each opponent holds the actual priority and enough blue mana for Opt in an independent
    // unrestricted control and in the matching restricted state. The direct Main1 preparation
    // above labels the intentional turn/priority fixture; command actors remain PlayerIds.
    for (turn, active_seat) in [(1, 0), (2, 1), (3, 2)] {
        for caster_seat in [1, 2] {
            narset_phase_d_prepare_main1_priority(&mut control, active_seat, caster_seat, turn);
            let control_caster = control_ids[caster_seat];
            assert_eq!(control.state.active_player_id(), control_ids[active_seat]);
            assert_eq!(control.state.priority_player_id(), control_caster);
            assert_eq!(control.state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(
                control.state.players[caster_seat].mana_pool.blue, 1,
                "the control can pay Opt's real blue mana"
            );
            let (control_hand_index, control_begin) =
                narset_phase_d_begin_normal_spell(&control, caster_seat, "opt");
            let control_opt = control.state.players[caster_seat].hand[control_hand_index];
            control
                .apply_command(control_caster, &control_begin)
                .expect("the exact player/turn/mana control begins normal Opt");
            let control_transaction = control
                .state
                .pending_spell_cast
                .as_ref()
                .expect("control Opt opens payment")
                .transaction_id;
            let control_revision = control.state.command_index;
            let control_preview = control.preview_payment_scenario(
                control_caster,
                &PreviewPayment {
                    transaction_id: control_transaction,
                    revision: control_revision,
                    commit_spell_cast: Some(CommitSpellCast {
                        transaction_id: control_transaction,
                        payment: Some(PaymentSelection {
                            expected_state_revision: control_revision,
                            mana: Some(PaymentMana {
                                u: 1,
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            );
            assert!(
                control_preview.valid && control_preview.complete,
                "the independent control can fully pay normal Opt: {control_preview:?}"
            );
            let control_payment = control_preview
                .selection
                .expect("legal control preview returns a complete payment");
            assert_eq!(control_payment.expected_state_revision, control_revision);
            assert_eq!(control_payment.mana.as_ref().unwrap().u, 1);
            assert_eq!(control.state.command_index, control_revision);
            control
                .apply_command(
                    control_caster,
                    &RuledCommand {
                        cmd: Some(Cmd::CancelSpellCast(
                            tricerules_proto::ruled::v1::CancelSpellCast {
                                transaction_id: control_transaction,
                            },
                        )),
                    },
                )
                .expect("cancel the independent legal Opt control");
            assert!(control.state.pending_spell_cast.is_none());
            assert!(control.state.players[caster_seat]
                .hand
                .contains(&control_opt));

            narset_phase_d_prepare_main1_priority(&mut restricted, active_seat, caster_seat, turn);
            let caster = player_ids[caster_seat];
            assert_eq!(restricted.state.active_player_id(), player_ids[active_seat]);
            assert_eq!(restricted.state.priority_player_id(), caster);
            assert_eq!(restricted.state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(restricted.state.players[caster_seat].mana_pool.blue, 1);
            assert!(restricted.state.pending_spell_cast.is_none());
            let (hand_index, forged_begin) =
                narset_phase_d_begin_normal_spell(&restricted, caster_seat, "opt");
            let opt = restricted.state.players[caster_seat].hand[hand_index];
            let command_index_before = restricted.state.command_index;
            let hand_before = restricted.state.players[caster_seat].hand.clone();
            let mana_before = restricted.state.players[caster_seat].mana_pool;
            let zone_before = restricted.state.objects[&opt].zone;
            let generation_before = generation(&restricted, opt);
            let diagnostic_before = restricted
                .diagnostic_snapshot()
                .expect("snapshot the complete multiplayer restricted state");
            let state_before = serde_json::to_value(&restricted.state)
                .expect("serialize all multiplayer state before rejected Begin");

            let rejected = restricted
                .apply_command(caster, &forged_begin)
                .expect_err("an opposing emblem rejects this normal noncreature Begin");
            let reason = format!("{rejected:?}");
            assert!(
                reason
                    .contains("an emblem prohibits this opponent from casting noncreature spells"),
                "the Begin failure is specifically the emblem restriction: {reason}"
            );
            assert_eq!(restricted.state.command_index, command_index_before);
            assert_eq!(restricted.state.players[caster_seat].hand, hand_before);
            assert_eq!(restricted.state.players[caster_seat].mana_pool, mana_before);
            assert_eq!(restricted.state.objects[&opt].zone, zone_before);
            assert_eq!(generation(&restricted, opt), generation_before);
            assert!(restricted.state.pending_spell_cast.is_none());
            assert_eq!(restricted.state.stack.len(), 0);
            assert_eq!(restricted.diagnostic_snapshot().unwrap(), diagnostic_before);
            assert_eq!(
                serde_json::to_value(&restricted.state).unwrap(),
                state_before,
                "rejected Begin preserves the full diagnostic state"
            );
        }
    }

    // B's actual Forest land play and current mana ability remain legal under A's emblem.
    narset_phase_d_prepare_main1_priority(&mut restricted, 1, 1, 4);
    let b_id = player_ids[1];
    let forest_hand_index = hand_index_for_card(&restricted, 1, "forest");
    let forest = restricted.state.players[1].hand[forest_hand_index];
    restricted
        .apply_command(b_id, &play_land(forest_hand_index))
        .expect("the active player may play a real Forest under the emblem");
    assert_eq!(
        restricted.state.objects[&forest].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(restricted.state.lands_played_this_turn, 1);
    let forest_mana = activate_ability_for(&restricted, forest, 0, vec![]);
    restricted
        .apply_command(b_id, &forest_mana)
        .expect("the current Forest mana ability remains legal under the emblem");
    assert!(restricted.state.objects[&forest].tapped);
    assert_eq!(restricted.state.players[1].mana_pool.green, 1);
    give_mana(
        &mut restricted,
        b_id,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    // A real creature cast from each opponent remains available at that player's Main1 priority.
    let (bear_hand_index, bear_begin) =
        narset_phase_d_begin_normal_spell(&restricted, 1, "grizzly_bears");
    let bears = restricted.state.players[1].hand[bear_hand_index];
    restricted
        .apply_command(b_id, &bear_begin)
        .expect("the emblem does not prevent B from beginning Grizzly Bears");
    let bear_transaction = restricted
        .state
        .pending_spell_cast
        .as_ref()
        .expect("Grizzly Bears opens its normal payment")
        .transaction_id;
    narset_phase_d_commit_normal_spell(
        &mut restricted,
        b_id,
        bear_transaction,
        PaymentMana {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    pass_priority_round(&mut restricted);
    assert!(restricted.state.stack.is_empty());
    assert_eq!(
        restricted.state.objects[&bears].zone,
        tricerules_core::Zone::Battlefield
    );

    narset_phase_d_prepare_main1_priority(&mut restricted, 2, 2, 5);
    let c_id = player_ids[2];
    let (thopter_hand_index, thopter_begin) =
        narset_phase_d_begin_normal_spell(&restricted, 2, "ornithopter");
    let thopter = restricted.state.players[2].hand[thopter_hand_index];
    restricted
        .apply_command(c_id, &thopter_begin)
        .expect("the emblem does not prevent C from beginning Ornithopter");
    let thopter_transaction = restricted
        .state
        .pending_spell_cast
        .as_ref()
        .expect("Ornithopter opens its normal payment")
        .transaction_id;
    narset_phase_d_commit_normal_spell(
        &mut restricted,
        c_id,
        thopter_transaction,
        PaymentMana::default(),
    );
    pass_priority_round(&mut restricted);
    assert!(restricted.state.stack.is_empty());
    assert_eq!(
        restricted.state.objects[&thopter].zone,
        tricerules_core::Zone::Battlefield
    );

    narset_phase_d_prepare_main1_priority(&mut restricted, 0, 0, 6);
    let a_id = player_ids[0];
    let (opt_hand_index, opt_begin) = narset_phase_d_begin_normal_spell(&restricted, 0, "opt");
    let opt = restricted.state.players[0].hand[opt_hand_index];
    restricted
        .apply_command(a_id, &opt_begin)
        .expect("the emblem controller A may begin Opt");
    let opt_transaction = restricted
        .state
        .pending_spell_cast
        .as_ref()
        .expect("A's Opt opens normal payment")
        .transaction_id;
    narset_phase_d_commit_normal_spell(
        &mut restricted,
        a_id,
        opt_transaction,
        PaymentMana {
            u: 1,
            ..Default::default()
        },
    );
    assert_eq!(restricted.state.stack.len(), 1);
    assert_eq!(restricted.state.stack[0].id, opt);
    assert_eq!(restricted.state.stack[0].card_id, "opt");
}

#[test]
fn narset_phase_d_emblem_revalidates_locked_payment_without_debit_and_preserves_cancel() {
    const SEED: u64 = 6_110_203;
    const CONTROLLER_SEAT: usize = 0;
    const CASTER_SEAT: usize = 1;

    let decks = Some(vec![
        deck_with("swamp", &["opt"]),
        deck_with("swamp", &["village_rites", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(
        narset_phase_d_multiplayer_registry(),
        SEED,
        &[17, 29],
        20,
        decks,
        true,
    )
    .expect("Narset locked Village Rites payment fixture");
    advance_to_main1_from_game_start(&mut engine);

    let controller = engine.state.players[CONTROLLER_SEAT].id;
    let caster = engine.state.players[CASTER_SEAT].id;
    assert_ne!(controller, caster);
    assert_ne!(controller as usize, CONTROLLER_SEAT);
    assert_ne!(caster as usize, CASTER_SEAT);
    ensure_card_in_hand(&mut engine, CASTER_SEAT, "village_rites");
    let creature = relocate_to_battlefield(&mut engine, CASTER_SEAT, "grizzly_bears", false);
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(controller, &pass())
        .expect("A passes Main1 priority to B for the instant cast");
    assert_eq!(engine.state.active_player_id(), controller);
    assert_eq!(engine.state.priority_player_id(), caster);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    assert!(engine.state.static_emblems.is_empty());

    let (rites_hand_index, mut begin) =
        narset_phase_d_begin_normal_spell(&engine, CASTER_SEAT, "village_rites");
    let rites = engine.state.players[CASTER_SEAT].hand[rites_hand_index];
    let published = engine.initial_response_batch();
    let action = published.legal_by_player[&caster]
        .hand_actions
        .iter()
        .find(|action| {
            action.kind == HandActionKind::HandActionCastSpell as i32
                && action.hand_index as usize == rites_hand_index
                && action.face_index == 0
        })
        .expect("the current priority holder is offered the real Village Rites face");
    let cost_choices = action
        .cost_choices
        .as_ref()
        .expect("Village Rites publishes its additional-cost candidate");
    assert!(cost_choices.non_mana_costs_payable);
    let sacrifice_choice = cost_choices
        .choices
        .iter()
        .find(|choice| choice.cost_index == 0)
        .expect("Village Rites publishes cost index zero");
    assert_eq!(sacrifice_choice.min, 1);
    assert_eq!(sacrifice_choice.max, 1);
    let candidate = sacrifice_choice
        .candidate_objects
        .iter()
        .filter_map(|candidate| candidate.object.as_ref())
        .find(|candidate| candidate.object_id == creature)
        .expect("the live Grizzly Bears is a generation-checked published candidate");
    let creature_generation = candidate.zone_change_generation;
    assert_eq!(creature_generation, generation(&engine, creature));
    assert_eq!(
        sacrifice_choice.candidate_ids,
        [creature],
        "the published cost choice admits this exact PermanentId"
    );
    let Some(Cmd::BeginSpellCast(begin_spell_cast)) = begin.cmd.as_mut() else {
        unreachable!("the helper constructed BeginSpellCast")
    };
    begin_spell_cast
        .announcement
        .as_mut()
        .expect("normal spell announcement")
        .cost_selections
        .push(tricerules_proto::ruled::v1::CostSelection {
            cost_index: sacrifice_choice.cost_index,
            selection: Some(
                tricerules_proto::ruled::v1::cost_selection::Selection::PermanentId(
                    candidate.object_id,
                ),
            ),
        });
    let original_hand = engine.state.players[CASTER_SEAT].hand.clone();
    let original_rites_generation = engine.state.zone_change_generation.get(&rites).copied();
    let original_creature_generation = engine.state.zone_change_generation.get(&creature).copied();
    engine
        .apply_command(caster, &begin)
        .expect("B begins the actual Village Rites from hand with its published sacrifice");
    let pending = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("Village Rites is pending its payment");
    let transaction_id = pending.transaction_id;
    assert_eq!(pending.caster, caster);
    assert_eq!(pending.reserved_object_id, rites);
    assert_eq!(pending.announcement.cost_selections.len(), 1);
    assert_eq!(
        pending.announcement.cost_selections[0].cost_index,
        sacrifice_choice.cost_index
    );
    assert!(matches!(
        pending.announcement.cost_selections[0].selection.as_ref(),
        Some(tricerules_proto::ruled::v1::cost_selection::Selection::PermanentId(object_id))
            if *object_id == creature
    ));
    assert_eq!(
        engine.state.objects[&rites].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.objects[&creature].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.players[CASTER_SEAT].mana_pool.black, 1);
    assert!(engine.state.stack.is_empty());

    let revision = engine.state.command_index;
    let before_preview = engine
        .diagnostic_snapshot()
        .expect("snapshot the complete pending Village Rites payment");
    let valid_preview = engine.preview_payment_scenario(
        caster,
        &PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(CommitSpellCast {
                transaction_id,
                payment: Some(PaymentSelection {
                    expected_state_revision: revision,
                    mana: Some(PaymentMana {
                        b: 1,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(
        valid_preview.valid && valid_preview.complete,
        "{valid_preview:?}"
    );
    assert_eq!(engine.state.command_index, revision);
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_preview);
    let previously_valid_payment = valid_preview
        .selection
        .expect("successful preview returns the complete payment selection");
    let previously_valid_restricted_mana = valid_preview.restricted_mana;
    assert_eq!(previously_valid_payment.expected_state_revision, revision);

    // Prepared payment boundary: inject a persistent emblem controlled by the opponent while
    // Village Rites is pending. This is fixture state, not a claimed legal opponent action.
    let command_index_before_emblem = engine.state.command_index;
    let emblem_id = narset_phase_d_inject_emblem(&mut engine, controller);
    assert_eq!(engine.state.command_index, command_index_before_emblem);
    assert_eq!(engine.state.command_index, revision);
    assert_eq!(engine.state.static_emblems.len(), 1);
    assert_ne!(engine.state.static_emblems[0].controller, caster);
    assert_eq!(engine.state.static_emblems[0].object_id, emblem_id);
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id,
        transaction_id
    );

    let before_invalidated_preview = engine
        .diagnostic_snapshot()
        .expect("snapshot before refreshing payment under the new emblem");
    let state_before_invalidated_preview = serde_json::to_value(&engine.state)
        .expect("serialize state before the invalidated payment preview");
    let invalidated_preview = engine.preview_payment_scenario(
        caster,
        &PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(CommitSpellCast {
                transaction_id,
                payment: Some(previously_valid_payment.clone()),
                restricted_mana: previously_valid_restricted_mana.clone(),
            }),
            ..Default::default()
        },
    );
    assert!(
        !invalidated_preview.valid,
        "current payment preview: {invalidated_preview:?}"
    );
    assert_eq!(engine.state.command_index, revision);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before_invalidated_preview
    );
    assert_eq!(
        serde_json::to_value(&engine.state).unwrap(),
        state_before_invalidated_preview,
        "the newly invalid preview leaves the pending transaction untouched"
    );

    let before_rejected_commit = engine
        .diagnostic_snapshot()
        .expect("snapshot pending payment and persistent opposing emblem");
    let state_before_rejected_commit = serde_json::to_value(&engine.state)
        .expect("serialize complete state before rejected CommitSpellCast");
    let pending_before_rejected_commit = serde_json::to_value(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .expect("transaction remains pending"),
    )
    .expect("serialize the exact pending transaction");
    let source_before_rejected_commit = serde_json::to_value(
        engine
            .state
            .objects
            .get(&rites)
            .expect("reserved Village Rites source"),
    )
    .expect("serialize the exact reserved source object");
    let hand_before_rejected_commit = engine.state.players[CASTER_SEAT].hand.clone();
    let mana_before_rejected_commit = engine.state.players[CASTER_SEAT].mana_pool;
    let life_before_rejected_commit = engine.state.players[CASTER_SEAT].life;
    let stack_before_rejected_commit = serde_json::to_value(&engine.state.stack)
        .expect("serialize the stack before rejected commit");
    let source_generation_before_rejected_commit =
        engine.state.zone_change_generation.get(&rites).copied();
    let creature_generation_before_rejected_commit =
        engine.state.zone_change_generation.get(&creature).copied();

    let rejected_commit = engine.apply_command(
        caster,
        &RuledCommand {
            cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                transaction_id,
                payment: Some(previously_valid_payment),
                restricted_mana: previously_valid_restricted_mana,
            })),
        },
    );
    let rejected_commit =
        rejected_commit.expect_err("the newly prohibited locked cast cannot commit");
    let reason = format!("{rejected_commit:?}");
    assert!(
        reason.contains("an emblem prohibits this opponent from casting noncreature spells"),
        "CommitSpellCast must fail specifically on the current emblem restriction: {reason}"
    );
    assert_eq!(engine.state.command_index, revision);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before_rejected_commit
    );
    assert_eq!(
        serde_json::to_value(&engine.state).unwrap(),
        state_before_rejected_commit,
        "rejected commit leaves every serialized state field unchanged"
    );
    assert_eq!(
        serde_json::to_value(engine.state.pending_spell_cast.as_ref().unwrap()).unwrap(),
        pending_before_rejected_commit,
        "the same transaction remains open"
    );
    assert_eq!(
        serde_json::to_value(engine.state.objects.get(&rites).unwrap()).unwrap(),
        source_before_rejected_commit,
        "the reserved physical source is unchanged"
    );
    assert_eq!(
        engine.state.players[CASTER_SEAT].hand,
        hand_before_rejected_commit
    );
    assert_eq!(
        engine.state.players[CASTER_SEAT].mana_pool,
        mana_before_rejected_commit
    );
    assert_eq!(
        engine.state.players[CASTER_SEAT].life,
        life_before_rejected_commit
    );
    assert_eq!(
        serde_json::to_value(&engine.state.stack).unwrap(),
        stack_before_rejected_commit
    );
    assert_eq!(
        engine.state.zone_change_generation.get(&rites).copied(),
        source_generation_before_rejected_commit
    );
    assert_eq!(
        engine.state.zone_change_generation.get(&creature).copied(),
        creature_generation_before_rejected_commit
    );
    assert_eq!(
        engine.state.objects[&rites].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.objects[&creature].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.players[CASTER_SEAT]
        .battlefield
        .contains(&creature));
    assert!(engine.state.stack.is_empty());

    engine
        .apply_command(
            caster,
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(
                    tricerules_proto::ruled::v1::CancelSpellCast { transaction_id },
                )),
            },
        )
        .expect("B may cancel the still-pending cast after the rejected commit");
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.players[CASTER_SEAT].hand, original_hand);
    assert_eq!(
        engine.state.objects[&rites].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(
        engine.state.zone_change_generation.get(&rites).copied(),
        original_rites_generation
    );
    assert_eq!(
        engine.state.objects[&creature].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.zone_change_generation.get(&creature).copied(),
        original_creature_generation
    );
    assert!(engine.state.players[CASTER_SEAT]
        .battlefield
        .contains(&creature));
    assert_eq!(
        engine.state.players[CASTER_SEAT].mana_pool,
        mana_before_rejected_commit
    );
    assert_eq!(
        engine.state.players[CASTER_SEAT].life,
        life_before_rejected_commit
    );
    assert!(engine.state.stack.is_empty());
}

const NARSET_PHASE_D_REBOUND_OBSERVER_ID: &str = "narset_phase_d_rebound_observer";
const NARSET_PHASE_D_REBOUND_OBSERVER_NAME: &str = "Narset Phase D Rebound Observer";
const NARSET_PHASE_D_REBOUND_OBSERVER_RON: &str = r#"(
    id: "narset_phase_d_rebound_observer",
    name: "Narset Phase D Rebound Observer",
    face_id: "narset_phase_d_rebound_observer",
    mana_cost: "{0}",
    types: ["Artifact"],
    activated_abilities: [(
        ability_id: "activated_01",
        presentation: Fallback,
        costs: [Tap],
        effect: [NextHandInstantOrSorceryGainsReboundThisTurn],
    )],
)"#;

fn narset_phase_d_regression_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(
            &[
                EMBLEM_ARTIFACT_RON,
                NARSET_PHASE_D_REBOUND_OBSERVER_RON,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!(
                    "../../../tricerules-cards/data/multiface/bonecrusher_giant_stomp.ron"
                ),
                include_str!("../../../tricerules-cards/data/skitterbeam_battalion.ron"),
                include_str!("../../../tricerules-cards/data/generated/t/think_twice.ron"),
                include_str!("../../../tricerules-cards/data/generated/a/alanias_pathmaker.ron"),
                include_str!("../../../tricerules-cards/data/case_of_the_locked_hothouse.ron"),
                include_str!("../../../tricerules-cards/data/exploration.ron"),
                include_str!("../../../tricerules-cards/data/twincast.ron"),
            ],
            &[],
        )
        .expect("the emblem fixture and shipped Phase D route cards register")
    })
}

fn narset_phase_d_regression_game(seed: u64) -> (GameEngine, [i32; 2], u32) {
    let decks = Some(vec![
        deck_with("forest", &[ARTIFACT_ID]),
        deck_with(
            "island",
            &[
                NARSET_PHASE_D_REBOUND_OBSERVER_ID,
                "opt",
                "twincast",
                "grizzly_bears",
                "bonecrusher_giant_stomp",
                "skitterbeam_battalion",
                "think_twice",
                "alanias_pathmaker",
                "case_of_the_locked_hothouse",
                "exploration",
            ],
        ),
    ]);
    let mut engine = GameEngine::new(
        narset_phase_d_regression_registry(),
        seed,
        &[17, 29],
        20,
        decks,
        true,
    )
    .expect("the two-player Phase D regression fixture starts");
    advance_to_main1_from_game_start(&mut engine);

    let player_ids = [engine.state.players[0].id, engine.state.players[1].id];
    for (seat, player_id) in player_ids.iter().copied().enumerate() {
        assert_ne!(player_id as usize, seat, "seat indices are not PlayerIds");
        assert_eq!(
            player_ids
                .iter()
                .filter(|other| **other == player_id)
                .count(),
            1,
            "each fixture seat has one distinct PlayerId"
        );
    }

    let battlefield_before = engine.state.players[0].battlefield.clone();
    engine.enable_dev_commands();
    move_fixture_card_with_dev_command(
        &mut engine,
        player_ids[0],
        ARTIFACT_NAME,
        DevZone::Battlefield,
    );
    let emblem_source = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| !battlefield_before.contains(object_id))
        .expect("A's actual synthetic Artifact entered the battlefield");
    assert_eq!(engine.state.objects[&emblem_source].card_id, ARTIFACT_ID);
    (engine, player_ids, emblem_source)
}

fn narset_phase_d_regression_resolve_emblem_ability(
    engine: &mut GameEngine,
    controller: i32,
    source: u32,
) -> u32 {
    if engine.state.priority_player_id() != controller {
        let current_priority = engine.state.priority_player_id();
        engine
            .apply_command(current_priority, &pass())
            .expect("the current player passes priority to the emblem controller");
    }
    assert_eq!(engine.state.priority_player_id(), controller);

    let emblems_before = engine.state.static_emblems.len();
    let activation = authoring_actions::activation(engine, controller, source, 0)
        .expect("the engine offers the actual synthetic emblem ability");
    engine
        .apply_command(controller, &activation)
        .expect("activate the current synthetic emblem ability");
    let ability_id = engine
        .state
        .stack
        .last()
        .expect("the emblem ability is placed above the current stack")
        .id;
    pass_priority_round(engine);
    assert!(engine.state.stack.iter().all(|item| item.id != ability_id));
    assert_eq!(engine.state.static_emblems.len(), emblems_before + 1);
    let emblem = engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.controller == controller)
        .expect("the resolved ability creates its controller's persistent emblem");
    assert!(emblem.effects.iter().any(|effect| matches!(
        effect,
        tricerules_card_model::primitives::StaticEmblemEffect::OpponentsCantCastNoncreatureSpells
    )));
    emblem.object_id
}

fn narset_phase_d_regression_has_hand_cast_offer(
    engine: &mut GameEngine,
    player: i32,
    hand_index: usize,
    face_index: u32,
    prototyped: bool,
) -> bool {
    engine.initial_response_batch().legal_by_player[&player]
        .hand_actions
        .iter()
        .any(|action| {
            action.kind == HandActionKind::HandActionCastSpell as i32
                && action.hand_index as usize == hand_index
                && action.face_index == face_index
                && action.prototyped == prototyped
        })
}

fn narset_phase_d_regression_hand_target(
    engine: &mut GameEngine,
    player: i32,
    hand_index: usize,
    face_index: u32,
    target_object: u32,
) -> tricerules_proto::ruled::v1::TargetRef {
    let key = ((hand_index as u32) << 8) | face_index;
    let offers = engine.initial_response_batch();
    let targets = offers.legal_by_player[&player]
        .valid_targets_by_hand_slot
        .get(&key)
        .expect("the otherwise-legal hand face publishes its current target group");
    let group = targets
        .groups
        .iter()
        .find(|group| group.valid_permanent_ids.contains(&target_object))
        .expect("the prepared permanent is a current legal target");
    tricerules_proto::ruled::v1::TargetRef {
        object_id: target_object,
        expected_zone_change_generation: Some(generation(engine, target_object)),
        group_index: group.group_index,
        kind: tricerules_proto::ruled::v1::TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

fn narset_phase_d_regression_begin_hand_spell(
    engine: &GameEngine,
    player_seat: usize,
    card_id: &str,
    face_index: u32,
    prototyped: bool,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) -> RuledCommand {
    let hand_index = hand_index_for_card(engine, player_seat, card_id);
    let mut cast_command = if face_index == 0 {
        cast_spell_x(hand_index, targets, 0)
    } else {
        cast_spell_face(hand_index, targets, face_index)
    };
    let cast = match cast_command.cmd.take() {
        Some(Cmd::CastSpell(cast)) => cast,
        _ => panic!("the cast helper creates a current CastSpell announcement"),
    };
    RuledCommand {
        cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
            announcement: Some(SpellCastAnnouncement {
                targets: cast.targets,
                x_value: cast.x_value,
                flex_payments: cast.flex_payments,
                face_index: cast.face_index,
                selected_modes: cast.selected_modes,
                source: cast.source,
                prototyped,
                cost_selections: cast.cost_selections,
                cast_cost_group_selections: cast.cast_cost_group_selections,
                cast_method: cast.cast_method,
                casting_permission_id: cast.casting_permission_id,
            }),
        })),
    }
}

fn narset_phase_d_regression_begin_zone_spell(
    source: tricerules_proto::ruled::v1::CastSource,
    face_index: u32,
    cast_method: i32,
    casting_permission_id: Option<u64>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
            announcement: Some(SpellCastAnnouncement {
                source: Some(source),
                face_index,
                cast_method,
                casting_permission_id,
                ..Default::default()
            }),
        })),
    }
}
fn narset_phase_d_regression_cast_hand_spell(
    engine: &mut GameEngine,
    player_seat: usize,
    card_id: &str,
    face_index: u32,
    prototyped: bool,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
    mana: ManaGift,
) -> tricerules_core::state::StackObjectRef {
    let player = engine.state.players[player_seat].id;
    ensure_card_in_hand(engine, player_seat, card_id);
    give_mana(engine, player, mana);
    let hand_index = hand_index_for_card(engine, player_seat, card_id);
    let mut command = if face_index == 0 {
        cast_spell_x(hand_index, targets, 0)
    } else {
        cast_spell_face(hand_index, targets, face_index)
    };
    let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        panic!("the cast helper creates a CastSpell command")
    };
    cast.prototyped = prototyped;
    let old_stack_ids = engine
        .state
        .stack
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    authoring_actions::pay(engine, player, &mut command)
        .expect("pay the hand spell from the current engine offer");
    engine
        .apply_command(player, &command)
        .expect("commit the actual hand spell through the engine");
    engine
        .state
        .stack
        .iter()
        .find(|item| {
            !old_stack_ids.contains(&item.id)
                && item.card_id == card_id
                && item.face_index == face_index as usize
                && item.cast_by == Some(player)
                && item.prototyped == prototyped
                && !item.is_triggered
                && !item.is_copy
        })
        .and_then(|item| item.cast_occurrence)
        .expect("the actual hand cast retains its physical cast occurrence")
}

#[allow(clippy::too_many_arguments)]
fn narset_phase_d_regression_cast_zone_spell(
    engine: &mut GameEngine,
    player: i32,
    object_id: u32,
    source: tricerules_proto::ruled::v1::CastSource,
    face_index: u32,
    cast_method: i32,
    casting_permission_id: Option<u64>,
    mana: ManaGift,
) -> tricerules_core::state::StackObjectRef {
    give_mana(engine, player, mana);
    let mut command = RuledCommand {
        cmd: Some(Cmd::CastSpell(tricerules_proto::ruled::v1::CastSpell {
            source: Some(source),
            face_index,
            cast_method,
            casting_permission_id,
            ..Default::default()
        })),
    };
    authoring_actions::pay(engine, player, &mut command)
        .expect("pay the zone spell from its current engine offer");
    engine
        .apply_command(player, &command)
        .expect("commit the actual zone spell through the engine");
    engine
        .state
        .stack
        .iter()
        .find(|item| item.id == object_id && !item.is_triggered && !item.is_copy)
        .and_then(|item| item.cast_occurrence)
        .expect("the actual zone cast retains its physical cast occurrence")
}

fn narset_phase_d_regression_assert_atomic_begin_rejection(
    engine: &mut GameEngine,
    actor: i32,
    command: &RuledCommand,
    object_id: u32,
    expected_zone: tricerules_core::Zone,
    emblem_reason_required: bool,
) {
    let diagnostics_before = engine
        .diagnostic_snapshot()
        .expect("snapshot the complete state before forged Begin");
    let command_index_before = engine.state.command_index;
    let actor_index = engine
        .state
        .player_idx(actor)
        .expect("the command actor is an actual PlayerId");
    let hand_before = engine.state.players[actor_index].hand.clone();
    let mana_before = engine.state.players[actor_index].mana_pool;
    let object_zone_before = engine.state.objects[&object_id].zone;
    let object_generation_before = generation(engine, object_id);
    let stack_before = serde_json::to_value(&engine.state.stack).expect("serialize stack");
    let pending_cast_before =
        serde_json::to_value(&engine.state.pending_spell_cast).expect("serialize pending cast");
    let pending_resolution_before =
        serde_json::to_value(&engine.state.pending_resolution).expect("serialize continuation");

    let rejection = match engine.apply_command(actor, command) {
        Err(error) => error.to_string(),
        Ok(_) => panic!("the forged spell Begin must be rejected"),
    };
    if emblem_reason_required {
        assert!(
            rejection.contains("an emblem prohibits this opponent from casting noncreature spells"),
            "rejection must identify the emblem prohibition, got: {rejection}"
        );
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        diagnostics_before,
        "rejected Begin preserves the complete diagnostic state"
    );
    assert_eq!(engine.state.command_index, command_index_before);
    assert_eq!(engine.state.players[actor_index].hand, hand_before);
    assert_eq!(engine.state.players[actor_index].mana_pool, mana_before);
    assert_eq!(engine.state.objects[&object_id].zone, object_zone_before);
    assert_eq!(object_zone_before, expected_zone);
    assert_eq!(generation(engine, object_id), object_generation_before);
    assert_eq!(
        serde_json::to_value(&engine.state.stack).unwrap(),
        stack_before
    );
    assert_eq!(
        serde_json::to_value(&engine.state.pending_spell_cast).unwrap(),
        pending_cast_before
    );
    assert_eq!(
        serde_json::to_value(&engine.state.pending_resolution).unwrap(),
        pending_resolution_before
    );
}

fn narset_phase_d_regression_pathmaker_opt_game(
    seed: u64,
) -> (GameEngine, [i32; 2], u32, u32, u64) {
    let (mut engine, player_ids, emblem_source) = narset_phase_d_regression_game(seed);
    let opt = inject_library_card(&mut engine, 1, "opt");
    engine.state.players[1].library.push_front(opt);
    narset_phase_d_prepare_main1_priority(&mut engine, 1, 1, 1);
    narset_phase_d_regression_cast_hand_spell(
        &mut engine,
        1,
        "alanias_pathmaker",
        0,
        false,
        Vec::new(),
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.players[1]
        .battlefield
        .iter()
        .any(|object_id| engine.state.objects[object_id].card_id == "alanias_pathmaker"));
    assert_eq!(
        engine.state.objects[&opt].zone,
        tricerules_core::Zone::Exile
    );
    let exile_generation = generation(&engine, opt);
    let permissions = engine
        .state
        .active_exile_play_permissions
        .iter()
        .filter(|permission| permission.object_id == opt && permission.player_id == player_ids[1])
        .collect::<Vec<_>>();
    assert_eq!(permissions.len(), 1);
    assert_eq!(permissions[0].zone_change_generation, exile_generation);
    (engine, player_ids, emblem_source, opt, exile_generation)
}

fn narset_phase_d_regression_solved_case_enchantment_game(
    seed: u64,
) -> (GameEngine, [i32; 2], u32, u32, u32) {
    let (mut engine, player_ids, emblem_source) = narset_phase_d_regression_game(seed);
    narset_phase_d_prepare_main1_priority(&mut engine, 1, 1, 1);
    narset_phase_d_regression_cast_hand_spell(
        &mut engine,
        1,
        "case_of_the_locked_hothouse",
        0,
        false,
        Vec::new(),
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    resolve_entire_stack_two_player(&mut engine);
    let case = engine.state.players[1]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "case_of_the_locked_hothouse")
        .expect("the real Case resolved onto B's battlefield");
    for _ in 0..7 {
        inject_permanent_on_battlefield(&mut engine, 1, "forest");
    }
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = tricerules_core::TurnStep::Main2;
    engine.state.passes_since_stack_change = 0;
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert!(engine.state.is_case_solved(case));

    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = tricerules_core::TurnStep::Main1;
    engine.state.passes_since_stack_change = 0;
    let top = inject_library_card(&mut engine, 1, "exploration");
    engine.state.players[1].library.push_front(top);
    assert_eq!(engine.state.players[1].library.front(), Some(&top));
    (engine, player_ids, emblem_source, case, top)
}
#[test]
fn narset_phase_d_emblem_checks_selected_faces_and_all_spell_source_routes() {
    const SEED: u64 = 6_110_204;

    narset_phase_d_check_adventure_faces(SEED);
    narset_phase_d_check_prototype(SEED + 2);
    narset_phase_d_check_flashback_route(SEED + 4);
    narset_phase_d_check_pathmaker_route(SEED + 6);
    narset_phase_d_check_library_route(SEED + 8);
    narset_phase_d_check_hand_route(SEED + 10);
}

#[inline(never)]
fn narset_phase_d_check_adventure_faces(seed: u64) {
    // Both Adventure faces use the same physical card. Stomp's control has a current legal
    // target receipt, so losing its action after the emblem cannot be a target failure.
    let (mut bone_control, control_ids, _) = narset_phase_d_regression_game(seed);
    let (mut bone_restricted, restricted_ids, emblem_source) =
        narset_phase_d_regression_game(seed + 1);
    for engine in [&mut bone_control, &mut bone_restricted] {
        ensure_card_in_hand(engine, 1, "bonecrusher_giant_stomp");
        inject_permanent_on_battlefield(engine, 1, "grizzly_bears");
        let caster = engine.state.players[1].id;
        give_mana(
            engine,
            caster,
            ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        );
        narset_phase_d_prepare_main1_priority(engine, 1, 1, 2);
    }
    let control_hand_index = hand_index_for_card(&bone_control, 1, "bonecrusher_giant_stomp");
    let control_bear = bone_control.state.players[1]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| bone_control.state.objects[object_id].card_id == "grizzly_bears")
        .unwrap();
    let control_offer = bone_control.initial_response_batch();
    assert!(narset_phase_d_regression_has_hand_cast_offer(
        &mut bone_control,
        control_ids[1],
        control_hand_index,
        0,
        false
    ));
    assert!(narset_phase_d_regression_has_hand_cast_offer(
        &mut bone_control,
        control_ids[1],
        control_hand_index,
        1,
        false
    ));
    let control_stomp_targets = control_offer.legal_by_player[&control_ids[1]]
        .valid_targets_by_hand_slot
        .get(&(((control_hand_index as u32) << 8) | 1))
        .expect("Stomp face 1 publishes targets before the emblem");
    assert_eq!(control_stomp_targets.groups.len(), 1);
    assert!(control_stomp_targets.groups[0]
        .valid_permanent_ids
        .contains(&control_bear));
    let control_target = narset_phase_d_regression_hand_target(
        &mut bone_control,
        control_ids[1],
        control_hand_index,
        1,
        control_bear,
    );
    let control_stomp = narset_phase_d_regression_cast_hand_spell(
        &mut bone_control,
        1,
        "bonecrusher_giant_stomp",
        1,
        false,
        vec![control_target],
        ManaGift::default(),
    );
    assert_eq!(
        bone_control.state.stack.last().unwrap().id,
        control_stomp.object_id
    );
    assert_eq!(bone_control.state.stack.last().unwrap().face_index, 1);

    let restricted_hand_index = hand_index_for_card(&bone_restricted, 1, "bonecrusher_giant_stomp");
    let restricted_bonecrusher = bone_restricted.state.players[1].hand[restricted_hand_index];
    assert_eq!(
        bone_restricted.state.objects[&restricted_bonecrusher].card_id,
        "bonecrusher_giant_stomp"
    );
    let restricted_bear = bone_restricted.state.players[1]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| bone_restricted.state.objects[object_id].card_id == "grizzly_bears")
        .unwrap();
    assert!(narset_phase_d_regression_has_hand_cast_offer(
        &mut bone_restricted,
        restricted_ids[1],
        restricted_hand_index,
        0,
        false
    ));
    assert!(narset_phase_d_regression_has_hand_cast_offer(
        &mut bone_restricted,
        restricted_ids[1],
        restricted_hand_index,
        1,
        false
    ));
    let restricted_target = narset_phase_d_regression_hand_target(
        &mut bone_restricted,
        restricted_ids[1],
        restricted_hand_index,
        1,
        restricted_bear,
    );
    let target_generation = generation(&bone_restricted, restricted_bear);
    narset_phase_d_regression_resolve_emblem_ability(
        &mut bone_restricted,
        restricted_ids[0],
        emblem_source,
    );
    narset_phase_d_prepare_main1_priority(&mut bone_restricted, 1, 1, 2);
    let after_bone_emblem = bone_restricted.initial_response_batch();
    let legal_bone = &after_bone_emblem.legal_by_player[&restricted_ids[1]];
    assert!(legal_bone.hand_actions.iter().any(|action| {
        action.kind == HandActionKind::HandActionCastSpell as i32
            && action.hand_index as usize == restricted_hand_index
            && action.face_index == 0
            && !action.prototyped
    }));
    assert!(!legal_bone.hand_actions.iter().any(|action| {
        action.kind == HandActionKind::HandActionCastSpell as i32
            && action.hand_index as usize == restricted_hand_index
            && action.face_index == 1
    }));
    // Hand face descriptors are structural; the hand-action map above gates cast eligibility.
    assert!(
        legal_bone
            .valid_targets_by_hand_slot
            .get(&(((restricted_hand_index as u32) << 8) | 1))
            .expect("the Stomp face descriptor persists without a current cast action")
            .groups
            .iter()
            .any(|group| group.valid_permanent_ids.contains(&restricted_bear)),
        "the unchanged Stomp descriptor still has the same legal Bear target"
    );
    assert_eq!(
        bone_restricted.state.players[1].hand[restricted_hand_index],
        restricted_bonecrusher
    );
    assert_eq!(
        generation(&bone_restricted, restricted_bear),
        target_generation
    );
    let forged_stomp = narset_phase_d_regression_begin_hand_spell(
        &bone_restricted,
        1,
        "bonecrusher_giant_stomp",
        1,
        false,
        vec![restricted_target],
    );
    narset_phase_d_regression_assert_atomic_begin_rejection(
        &mut bone_restricted,
        restricted_ids[1],
        &forged_stomp,
        restricted_bonecrusher,
        tricerules_core::Zone::Hand,
        true,
    );
}

#[inline(never)]
fn narset_phase_d_check_prototype(seed: u64) {
    // Both published Skitterbeam modes are creatures. Prototype uses its live flag and cost.
    let (mut skitter_control, skitter_control_ids, _) = narset_phase_d_regression_game(seed);
    let (mut skitter_restricted, skitter_ids, skitter_emblem_source) =
        narset_phase_d_regression_game(seed + 1);
    for engine in [&mut skitter_control, &mut skitter_restricted] {
        ensure_card_in_hand(engine, 1, "skitterbeam_battalion");
        let caster = engine.state.players[1].id;
        give_mana(
            engine,
            caster,
            ManaGift {
                r: 2,
                c: 3,
                ..Default::default()
            },
        );
        narset_phase_d_prepare_main1_priority(engine, 1, 1, 3);
    }
    let skitter_control_slot = hand_index_for_card(&skitter_control, 1, "skitterbeam_battalion");
    let skitter_control_batch = skitter_control.initial_response_batch();
    let skitter_control_actions =
        &skitter_control_batch.legal_by_player[&skitter_control_ids[1]].hand_actions;
    let normal_skitter = skitter_control_actions
        .iter()
        .find(|action| {
            action.kind == HandActionKind::HandActionCastSpell as i32
                && action.hand_index as usize == skitter_control_slot
                && action.face_index == 0
                && !action.prototyped
        })
        .expect("the unrestricted control offers normal Skitterbeam");
    let prototype_skitter = skitter_control_actions
        .iter()
        .find(|action| {
            action.kind == HandActionKind::HandActionCastSpell as i32
                && action.hand_index as usize == skitter_control_slot
                && action.face_index == 0
                && action.prototyped
        })
        .expect("the unrestricted control offers Skitterbeam Prototype");
    assert_eq!(normal_skitter.cost, "{9}");
    assert_eq!(prototype_skitter.cost, "{3}{R}{R}");
    let skitter_slot = hand_index_for_card(&skitter_restricted, 1, "skitterbeam_battalion");
    let skitter_physical = skitter_restricted.state.players[1].hand[skitter_slot];
    let skitter_pre = skitter_restricted.initial_response_batch();
    let pre_actions = &skitter_pre.legal_by_player[&skitter_ids[1]].hand_actions;
    assert!(pre_actions.iter().any(|action| {
        action.kind == HandActionKind::HandActionCastSpell as i32
            && action.hand_index as usize == skitter_slot
            && action.face_index == 0
            && !action.prototyped
            && action.cost == "{9}"
    }));
    assert!(pre_actions.iter().any(|action| {
        action.kind == HandActionKind::HandActionCastSpell as i32
            && action.hand_index as usize == skitter_slot
            && action.face_index == 0
            && action.prototyped
            && action.cost == "{3}{R}{R}"
    }));
    narset_phase_d_regression_resolve_emblem_ability(
        &mut skitter_restricted,
        skitter_ids[0],
        skitter_emblem_source,
    );
    narset_phase_d_prepare_main1_priority(&mut skitter_restricted, 1, 1, 3);
    let skitter_post = skitter_restricted.initial_response_batch();
    let post_actions = &skitter_post.legal_by_player[&skitter_ids[1]].hand_actions;
    let post_normal = post_actions
        .iter()
        .find(|action| {
            action.kind == HandActionKind::HandActionCastSpell as i32
                && action.hand_index as usize == skitter_slot
                && action.face_index == 0
                && !action.prototyped
        })
        .expect("the emblem still offers the normal creature mode");
    let post_prototype = post_actions
        .iter()
        .find(|action| {
            action.kind == HandActionKind::HandActionCastSpell as i32
                && action.hand_index as usize == skitter_slot
                && action.face_index == 0
                && action.prototyped
        })
        .expect("the emblem still offers the Prototype creature mode");
    assert_eq!(post_normal.cost, "{9}");
    assert_eq!(post_prototype.cost, "{3}{R}{R}");
    assert_eq!(
        skitter_restricted.state.players[1].hand[skitter_slot],
        skitter_physical
    );
    let prototype_cast = narset_phase_d_regression_cast_hand_spell(
        &mut skitter_restricted,
        1,
        "skitterbeam_battalion",
        0,
        true,
        Vec::new(),
        ManaGift::default(),
    );
    let prototype_stack = skitter_restricted
        .state
        .stack
        .iter()
        .find(|item| item.id == prototype_cast.object_id)
        .expect("the actual Prototype cast is on the stack");
    assert_eq!(prototype_stack.card_id, "skitterbeam_battalion");
    assert!(prototype_stack.prototyped);
    assert_eq!(prototype_stack.face_index, 0);
}

#[inline(never)]
fn narset_phase_d_check_flashback_route(seed: u64) {
    // Think Twice starts as a real hand cast and resolves to the graveyard before its current
    // Flashback offer is captured. The emblem then removes only the legal Flashback route.
    let (mut flashback_control, flashback_control_ids, _) = narset_phase_d_regression_game(seed);
    let (mut flashback_restricted, flashback_ids, flashback_emblem_source) =
        narset_phase_d_regression_game(seed + 1);
    for engine in [&mut flashback_control, &mut flashback_restricted] {
        narset_phase_d_prepare_main1_priority(engine, 1, 1, 4);
        narset_phase_d_regression_cast_hand_spell(
            engine,
            1,
            "think_twice",
            0,
            false,
            Vec::new(),
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        );
        resolve_entire_stack_two_player(engine);
    }
    let control_think_twice = flashback_control.state.players[1]
        .graveyard
        .iter()
        .copied()
        .find(|object_id| flashback_control.state.objects[object_id].card_id == "think_twice")
        .expect("the real Think Twice reached B's graveyard");
    let control_generation = generation(&flashback_control, control_think_twice);
    give_mana(
        &mut flashback_control,
        flashback_control_ids[1],
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    narset_phase_d_prepare_main1_priority(&mut flashback_control, 1, 1, 4);
    let control_flashback_offer = flashback_control.initial_response_batch();
    let control_flashback = control_flashback_offer.legal_by_player[&flashback_control_ids[1]]
        .zone_cast_actions
        .iter()
        .find(|action| {
            action.object_id == control_think_twice
                && action.source_zone
                    == tricerules_proto::ruled::v1::CastSourceZone::Graveyard as i32
                && action.cast_method == CastMethod::Flashback as i32
        })
        .expect("the control publishes Think Twice's current Flashback offer");
    assert_eq!(control_flashback.zone_change_generation, control_generation);
    narset_phase_d_regression_cast_zone_spell(
        &mut flashback_control,
        flashback_control_ids[1],
        control_think_twice,
        graveyard_cast_source(control_think_twice, control_generation),
        control_flashback.face_index,
        control_flashback.cast_method,
        control_flashback.casting_permission_id,
        ManaGift::default(),
    );

    let restricted_think_twice = flashback_restricted.state.players[1]
        .graveyard
        .iter()
        .copied()
        .find(|object_id| flashback_restricted.state.objects[object_id].card_id == "think_twice")
        .expect("the restricted route begins from the same real graveyard card");
    let flashback_generation = generation(&flashback_restricted, restricted_think_twice);
    give_mana(
        &mut flashback_restricted,
        flashback_ids[1],
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    narset_phase_d_prepare_main1_priority(&mut flashback_restricted, 1, 1, 4);
    let before_flashback_emblem = flashback_restricted.initial_response_batch();
    let flashback_action = before_flashback_emblem.legal_by_player[&flashback_ids[1]]
        .zone_cast_actions
        .iter()
        .find(|action| {
            action.object_id == restricted_think_twice
                && action.source_zone
                    == tricerules_proto::ruled::v1::CastSourceZone::Graveyard as i32
                && action.cast_method == CastMethod::Flashback as i32
        })
        .expect("the actual Flashback offer exists before emblem creation");
    assert_eq!(
        flashback_action.zone_change_generation,
        flashback_generation
    );
    let flashback_face = flashback_action.face_index;
    let flashback_method = flashback_action.cast_method;
    let flashback_permission = flashback_action.casting_permission_id;
    narset_phase_d_regression_resolve_emblem_ability(
        &mut flashback_restricted,
        flashback_ids[0],
        flashback_emblem_source,
    );
    narset_phase_d_prepare_main1_priority(&mut flashback_restricted, 1, 1, 4);
    let after_flashback_emblem = flashback_restricted.initial_response_batch();
    assert!(after_flashback_emblem.legal_by_player[&flashback_ids[1]]
        .zone_cast_actions
        .iter()
        .all(|action| action.object_id != restricted_think_twice));
    let forged_flashback = narset_phase_d_regression_begin_zone_spell(
        graveyard_cast_source(restricted_think_twice, flashback_generation),
        flashback_face,
        flashback_method,
        flashback_permission,
    );
    narset_phase_d_regression_assert_atomic_begin_rejection(
        &mut flashback_restricted,
        flashback_ids[1],
        &forged_flashback,
        restricted_think_twice,
        tricerules_core::Zone::Graveyard,
        true,
    );
}

#[inline(never)]
fn narset_phase_d_check_pathmaker_route(seed: u64) {
    // Alania's actual ETB keeps its exact Opt permission alive through emblem creation; the
    // same current permission ID and exile generation are used for the rejected Begin.
    let (mut alania_control, alania_control_ids, _, control_opt, control_exile_generation) =
        narset_phase_d_regression_pathmaker_opt_game(seed);
    give_mana(
        &mut alania_control,
        alania_control_ids[1],
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let control_alania_batch = alania_control.initial_response_batch();
    let control_alania_action = control_alania_batch.legal_by_player[&alania_control_ids[1]]
        .zone_cast_actions
        .iter()
        .find(|action| {
            action.object_id == control_opt
                && action.source_zone == tricerules_proto::ruled::v1::CastSourceZone::Exile as i32
        })
        .expect("the control publishes the actual Alania exile permission");
    let control_alania_permission = control_alania_action
        .casting_permission_id
        .expect("the current Alania offer carries its permission ID");
    assert_eq!(
        control_alania_action.zone_change_generation,
        control_exile_generation
    );
    assert_eq!(control_alania_action.cast_method, CastMethod::Normal as i32);
    narset_phase_d_regression_cast_zone_spell(
        &mut alania_control,
        alania_control_ids[1],
        control_opt,
        exile_cast_source(control_opt, control_exile_generation),
        control_alania_action.face_index,
        control_alania_action.cast_method,
        Some(control_alania_permission),
        ManaGift::default(),
    );

    let (
        mut alania_restricted,
        alania_ids,
        alania_emblem_source,
        alania_opt,
        alania_exile_generation,
    ) = narset_phase_d_regression_pathmaker_opt_game(seed + 1);
    give_mana(
        &mut alania_restricted,
        alania_ids[1],
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let before_alania_emblem = alania_restricted.initial_response_batch();
    let alania_action = before_alania_emblem.legal_by_player[&alania_ids[1]]
        .zone_cast_actions
        .iter()
        .find(|action| {
            action.object_id == alania_opt
                && action.source_zone == tricerules_proto::ruled::v1::CastSourceZone::Exile as i32
        })
        .expect("the actual Pathmaker permission offers this exact exile incarnation");
    let alania_permission_id = alania_action
        .casting_permission_id
        .expect("the current exile action has an opaque permission ID");
    let alania_face = alania_action.face_index;
    let alania_method = alania_action.cast_method;
    assert_eq!(
        alania_action.zone_change_generation,
        alania_exile_generation
    );
    assert_eq!(alania_method, CastMethod::Normal as i32);
    let permission_expiry_before = alania_restricted
        .state
        .active_exile_play_permissions
        .iter()
        .find(|permission| permission.group_id == alania_permission_id)
        .expect("the exact Pathmaker permission is active before the emblem")
        .expires_at_cleanup_turn_instance;
    narset_phase_d_regression_resolve_emblem_ability(
        &mut alania_restricted,
        alania_ids[0],
        alania_emblem_source,
    );
    narset_phase_d_prepare_main1_priority(&mut alania_restricted, 1, 1, 1);
    let current_permission = alania_restricted
        .state
        .active_exile_play_permissions
        .iter()
        .find(|permission| {
            permission.group_id == alania_permission_id
                && permission.object_id == alania_opt
                && permission.player_id == alania_ids[1]
                && permission.zone_change_generation == alania_exile_generation
        })
        .expect("the exact Pathmaker permission remains unexpired and current");
    assert_eq!(
        current_permission.expires_at_cleanup_turn_instance,
        permission_expiry_before
    );
    assert_eq!(
        generation(&alania_restricted, alania_opt),
        alania_exile_generation
    );
    let after_alania_emblem = alania_restricted.initial_response_batch();
    assert!(after_alania_emblem.legal_by_player[&alania_ids[1]]
        .zone_cast_actions
        .iter()
        .all(|action| action.object_id != alania_opt));
    let forged_alania = narset_phase_d_regression_begin_zone_spell(
        exile_cast_source(alania_opt, alania_exile_generation),
        alania_face,
        alania_method,
        Some(alania_permission_id),
    );
    narset_phase_d_regression_assert_atomic_begin_rejection(
        &mut alania_restricted,
        alania_ids[1],
        &forged_alania,
        alania_opt,
        tricerules_core::Zone::Exile,
        true,
    );
}

#[inline(never)]
fn narset_phase_d_check_library_route(seed: u64) {
    // The library-top permission is produced by a real solved Hothouse and a current top
    // enchantment, not an unrelated Opt or an arbitrary library card.
    let (mut case_control, case_control_ids, _, control_case, control_top) =
        narset_phase_d_regression_solved_case_enchantment_game(seed);
    assert!(case_control.state.is_case_solved(control_case));
    let control_case_generation = generation(&case_control, control_top);
    give_mana(
        &mut case_control,
        case_control_ids[1],
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let control_case_offer = case_control.initial_response_batch();
    let control_case_action = control_case_offer.legal_by_player[&case_control_ids[1]]
        .zone_cast_actions
        .iter()
        .find(|action| {
            action.object_id == control_top
                && action.source_zone == tricerules_proto::ruled::v1::CastSourceZone::Library as i32
        })
        .expect("the actual solved Case offers the top enchantment from its library");
    assert_eq!(
        case_control.state.objects[&control_top].card_id,
        "exploration"
    );
    assert_eq!(
        control_case_action.zone_change_generation,
        control_case_generation
    );
    let control_case_begin = narset_phase_d_regression_begin_zone_spell(
        tricerules_proto::ruled::v1::CastSource {
            location: Some(
                tricerules_proto::ruled::v1::cast_source::Location::LibraryObjectId(control_top),
            ),
            expected_zone_change_generation: Some(control_case_generation),
        },
        control_case_action.face_index,
        control_case_action.cast_method,
        control_case_action.casting_permission_id,
    );
    case_control
        .apply_command(case_control_ids[1], &control_case_begin)
        .expect("the control can Begin Case's current top-enchantment offer");
    let case_control_transaction = case_control
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the legal library Begin opens its real payment")
        .transaction_id;
    assert_eq!(
        case_control
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .reserved_object_id,
        control_top
    );
    case_control
        .apply_command(
            case_control_ids[1],
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(
                    tricerules_proto::ruled::v1::CancelSpellCast {
                        transaction_id: case_control_transaction,
                    },
                )),
            },
        )
        .expect("cancel the independent legal library permission control");
    assert!(case_control.state.pending_spell_cast.is_none());
    let (mut case_restricted, case_ids, case_emblem_source, solved_case, case_top) =
        narset_phase_d_regression_solved_case_enchantment_game(seed + 1);
    assert!(case_restricted.state.is_case_solved(solved_case));
    let case_generation = generation(&case_restricted, case_top);
    give_mana(
        &mut case_restricted,
        case_ids[1],
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let before_case_emblem = case_restricted.initial_response_batch();
    let case_action = before_case_emblem.legal_by_player[&case_ids[1]]
        .zone_cast_actions
        .iter()
        .find(|action| {
            action.object_id == case_top
                && action.source_zone == tricerules_proto::ruled::v1::CastSourceZone::Library as i32
        })
        .expect("the otherwise-legal current Case permission is captured before the emblem");
    assert_eq!(
        case_restricted.state.objects[&case_top].card_id,
        "exploration"
    );
    assert_eq!(case_action.zone_change_generation, case_generation);
    let case_face = case_action.face_index;
    let case_method = case_action.cast_method;
    let case_permission = case_action.casting_permission_id;
    narset_phase_d_regression_resolve_emblem_ability(
        &mut case_restricted,
        case_ids[0],
        case_emblem_source,
    );
    narset_phase_d_prepare_main1_priority(&mut case_restricted, 1, 1, 1);
    assert!(case_restricted.state.is_case_solved(solved_case));
    assert_eq!(
        case_restricted.state.players[1].library.front(),
        Some(&case_top)
    );
    assert_eq!(generation(&case_restricted, case_top), case_generation);
    assert!(
        case_restricted.initial_response_batch().legal_by_player[&case_ids[1]]
            .zone_cast_actions
            .iter()
            .all(|action| action.object_id != case_top)
    );
    let forged_case = narset_phase_d_regression_begin_zone_spell(
        tricerules_proto::ruled::v1::CastSource {
            location: Some(
                tricerules_proto::ruled::v1::cast_source::Location::LibraryObjectId(case_top),
            ),
            expected_zone_change_generation: Some(case_generation),
        },
        case_face,
        case_method,
        case_permission,
    );
    narset_phase_d_regression_assert_atomic_begin_rejection(
        &mut case_restricted,
        case_ids[1],
        &forged_case,
        case_top,
        tricerules_core::Zone::Library,
        true,
    );
}

#[inline(never)]
fn narset_phase_d_check_hand_route(seed: u64) {
    // The ordinary hand route keeps its current physical card and a separate legal control.
    let (mut hand_control, hand_control_ids, _) = narset_phase_d_regression_game(seed);
    let (mut hand_restricted, hand_ids, hand_emblem_source) =
        narset_phase_d_regression_game(seed + 1);
    for engine in [&mut hand_control, &mut hand_restricted] {
        ensure_card_in_hand(engine, 1, "opt");
        let actor = engine.state.players[1].id;
        give_mana(
            engine,
            actor,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        narset_phase_d_prepare_main1_priority(engine, 0, 1, 5);
    }
    let control_opt_slot = hand_index_for_card(&hand_control, 1, "opt");
    assert!(narset_phase_d_regression_has_hand_cast_offer(
        &mut hand_control,
        hand_control_ids[1],
        control_opt_slot,
        0,
        false
    ));
    narset_phase_d_regression_cast_hand_spell(
        &mut hand_control,
        1,
        "opt",
        0,
        false,
        Vec::new(),
        ManaGift::default(),
    );

    let opt_slot = hand_index_for_card(&hand_restricted, 1, "opt");
    let hand_opt = hand_restricted.state.players[1].hand[opt_slot];
    let opt_generation = generation(&hand_restricted, hand_opt);
    assert_eq!(hand_restricted.state.objects[&hand_opt].card_id, "opt");
    assert!(narset_phase_d_regression_has_hand_cast_offer(
        &mut hand_restricted,
        hand_ids[1],
        opt_slot,
        0,
        false
    ));
    narset_phase_d_regression_resolve_emblem_ability(
        &mut hand_restricted,
        hand_ids[0],
        hand_emblem_source,
    );
    narset_phase_d_prepare_main1_priority(&mut hand_restricted, 0, 1, 5);
    assert!(!narset_phase_d_regression_has_hand_cast_offer(
        &mut hand_restricted,
        hand_ids[1],
        opt_slot,
        0,
        false
    ));
    assert_eq!(hand_restricted.state.players[1].hand[opt_slot], hand_opt);
    let forged_hand_opt = narset_phase_d_regression_begin_hand_spell(
        &hand_restricted,
        1,
        "opt",
        0,
        false,
        Vec::new(),
    );
    narset_phase_d_regression_assert_atomic_begin_rejection(
        &mut hand_restricted,
        hand_ids[1],
        &forged_hand_opt,
        hand_opt,
        tricerules_core::Zone::Hand,
        true,
    );
    assert_eq!(generation(&hand_restricted, hand_opt), opt_generation);
}

fn narset_phase_d_regression_rebound_grants(engine: &GameEngine) -> serde_json::Value {
    engine
        .diagnostic_snapshot()
        .expect("snapshot the exact private Rebound grant state")["state"]["rebound_spell_grants"]
        .clone()
}

fn narset_phase_d_regression_scheduled_rebounds(engine: &GameEngine) -> serde_json::Value {
    engine
        .diagnostic_snapshot()
        .expect("snapshot the exact delayed Rebound records")["state"]["scheduled_rebounds"]
        .clone()
}

fn narset_phase_d_regression_capture_priority_round(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let live_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let passes_needed = live_players
        .checked_sub(engine.state.passes_since_stack_change as usize)
        .expect("the prepared two-player priority count is consistent");
    let mut last_batch = None;
    for _ in 0..passes_needed {
        let actor = engine.state.priority_player_id();
        last_batch = Some(
            engine
                .apply_command(actor, &pass())
                .expect("the current player passes through the actual priority round"),
        );
    }
    last_batch.expect("the current priority round contains an unpassed player")
}

fn narset_phase_d_regression_stack_target(
    occurrence: tricerules_core::state::StackObjectRef,
) -> tricerules_proto::ruled::v1::TargetRef {
    tricerules_proto::ruled::v1::TargetRef {
        object_id: occurrence.object_id,
        expected_zone_change_generation: occurrence.zone_change_generation,
        kind: tricerules_proto::ruled::v1::TargetRefKind::Stack as i32,
        ..Default::default()
    }
}

fn narset_phase_d_regression_resolve_opt_choice(
    engine: &mut GameEngine,
    expected_source: u32,
    expected_decider: i32,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let choice_batch = narset_phase_d_regression_capture_priority_round(engine);
    let (decider, prompt_id, candidates) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the actual Opt reaches its printed Scry choice");
        assert_eq!(pending.presentation.source_object_id, expected_source);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryTop);
        assert_eq!(pending.deciding_player, expected_decider);
        (
            pending.deciding_player,
            pending.resolution_prompt_id.get(),
            pending.presentation.candidates.clone(),
        )
    };
    let offer = find_resolution_choice(&choice_batch)
        .expect("the actual Opt Scry publishes its current resolution choice");
    assert_eq!(offer.source_object_id, expected_source);
    assert_eq!(offer.choice_kind, ChoiceKind::LibraryTop as i32);
    assert_eq!(offer.resolution_prompt_id, prompt_id);
    assert_ne!(prompt_id, 0);
    let mut answer = submit_resolution_choice(candidates);
    match answer.cmd.as_mut() {
        Some(Cmd::SubmitResolutionChoice(choice)) => {
            choice.resolution_prompt_id = prompt_id;
        }
        _ => panic!("the actual Scry answer is a resolution-choice command"),
    }
    let completion = engine
        .apply_command(decider, &answer)
        .expect("answer the actual Opt's current Scry choice");
    assert!(engine.state.pending_resolution.is_none());
    completion
}

struct NarsetPhaseDReboundScenario {
    engine: GameEngine,
    player_ids: [i32; 2],
    opt_object_id: u32,
    opt_stack_generation: u64,
    exile_generation: u64,
    rebound_trigger_id: u32,
    rebound_resolution: tricerules_proto::ruled::v1::RuledEventBatch,
    activated_emblem: bool,
}

fn narset_phase_d_regression_run_twincast_rebound_scenario(
    seed: u64,
    activate_emblem: bool,
) -> NarsetPhaseDReboundScenario {
    let (mut engine, player_ids, emblem_source) = narset_phase_d_regression_game(seed);
    narset_phase_d_prepare_main1_priority(&mut engine, 1, 1, 1);
    ensure_card_in_hand(&mut engine, 1, "opt");
    ensure_card_in_hand(&mut engine, 1, "twincast");
    give_mana(
        &mut engine,
        player_ids[1],
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let observer_battlefield_before = engine.state.players[1].battlefield.clone();
    engine.enable_dev_commands();
    move_fixture_card_with_dev_command(
        &mut engine,
        player_ids[1],
        NARSET_PHASE_D_REBOUND_OBSERVER_NAME,
        DevZone::Battlefield,
    );
    let observer = engine.state.players[1]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| !observer_battlefield_before.contains(object_id))
        .expect("B's actual Rebound observer entered the battlefield");
    assert_eq!(
        engine.state.objects[&observer].card_id,
        NARSET_PHASE_D_REBOUND_OBSERVER_ID
    );
    let observer_generation = generation(&engine, observer);
    let observer_activation =
        authoring_actions::activation(&mut engine, player_ids[1], observer, 0)
            .expect("B is offered the real one-shot Rebound observer ability");
    engine
        .apply_command(player_ids[1], &observer_activation)
        .expect("B activates the actual external Rebound grant");
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());

    let hand_before_opt = engine.state.players[1].hand.len();

    let opt = narset_phase_d_regression_cast_hand_spell(
        &mut engine,
        1,
        "opt",
        0,
        false,
        Vec::new(),
        ManaGift::default(),
    );
    let opt_stack_generation = opt
        .zone_change_generation
        .expect("the actual Opt has its exact stack incarnation");
    let opt_index = engine
        .state
        .stack
        .iter()
        .position(|item| item.cast_occurrence == Some(opt))
        .expect("B's physical Opt is on the stack");
    let grant_triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect::<Vec<_>>();
    assert_eq!(
        grant_triggers.len(),
        1,
        "the actual activation grants one trigger"
    );
    let grant_trigger = grant_triggers[0];
    let grant_trigger_index = engine
        .state
        .stack
        .iter()
        .position(|item| item.id == grant_trigger.id)
        .unwrap();
    assert!(grant_trigger_index > opt_index);
    assert!(!grant_trigger.is_copy);
    assert_eq!(grant_trigger.controller, player_ids[1]);
    assert_eq!(grant_trigger.source_permanent_id, Some(observer));
    assert_eq!(grant_trigger.source_zone_change, observer_generation);
    let grant_trigger_id = grant_trigger.id;
    let serialized_trigger =
        serde_json::to_value(grant_trigger).expect("serialize the actual grant trigger");
    assert_eq!(
        serialized_trigger["trigger_context"]["triggering_spell"],
        serde_json::to_value(Some(opt)).expect("serialize the exact Opt cast occurrence")
    );
    assert_eq!(
        narset_phase_d_regression_rebound_grants(&engine),
        serde_json::json!([]),
        "the Opt is granted only after its actual observer trigger resolves"
    );

    let grant_resolution = narset_phase_d_regression_capture_priority_round(&mut engine);
    assert!(grant_resolution.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == grant_trigger_id
        )
    }));
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, opt.object_id);
    assert_eq!(
        narset_phase_d_regression_rebound_grants(&engine),
        serde_json::json!([[opt.object_id, Some(opt_stack_generation.to_string())]]),
        "the resolved external grant binds to this exact physical Opt stack incarnation"
    );

    let twincast = narset_phase_d_regression_cast_hand_spell(
        &mut engine,
        1,
        "twincast",
        0,
        false,
        vec![narset_phase_d_regression_stack_target(opt)],
        ManaGift::default(),
    );
    let twincast_id = twincast.object_id;
    if activate_emblem {
        narset_phase_d_regression_resolve_emblem_ability(&mut engine, player_ids[0], emblem_source);
        assert_eq!(engine.state.static_emblems.len(), 1);
        assert_eq!(engine.state.static_emblems[0].controller, player_ids[0]);
        assert!(engine
            .state
            .stack
            .iter()
            .any(|item| item.id == opt.object_id));
        assert!(engine.state.stack.iter().any(|item| item.id == twincast_id));
        assert!(engine.state.stack.iter().all(|item| !item.is_copy));
    } else {
        assert!(engine.state.static_emblems.is_empty());
    }

    let twincast_resolution = narset_phase_d_regression_capture_priority_round(&mut engine);
    assert!(twincast_resolution.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == twincast_id
        )
    }));
    assert_eq!(
        engine.state.objects[&twincast_id].zone,
        tricerules_core::Zone::Graveyard
    );
    let (copy_id, copy_controller) = {
        let copy = engine
            .state
            .stack
            .iter()
            .find(|item| item.is_copy && item.card_id == "opt")
            .expect("actual Twincast still creates its Opt copy above the original");
        assert_ne!(copy.id, opt.object_id);
        assert_eq!(copy.cast_occurrence, None);
        (copy.id, copy.controller)
    };
    assert_eq!(copy_controller, player_ids[1]);
    assert_eq!(engine.state.stack.len(), 2);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == opt.object_id));
    assert_eq!(
        narset_phase_d_regression_rebound_grants(&engine),
        serde_json::json!([[opt.object_id, Some(opt_stack_generation.to_string())]])
    );
    assert_eq!(
        narset_phase_d_regression_scheduled_rebounds(&engine),
        serde_json::json!([]),
        "neither stack spell schedules Rebound before the physical original resolves"
    );

    let hand_before_copy_draw = engine.state.players[1].hand.len();
    narset_phase_d_regression_resolve_opt_choice(&mut engine, copy_id, player_ids[1]);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, opt.object_id);
    assert_eq!(
        engine.state.players[1].hand.len(),
        hand_before_copy_draw + 1,
        "the actual copy completes its Scry and draws for Twincast's controller"
    );
    assert_eq!(
        narset_phase_d_regression_scheduled_rebounds(&engine),
        serde_json::json!([]),
        "resolving the copy creates no physical Rebound schedule"
    );
    let hand_before_original_draw = engine.state.players[1].hand.len();
    let original_resolution =
        narset_phase_d_regression_resolve_opt_choice(&mut engine, opt.object_id, player_ids[1]);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(original_resolution.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::StackResolved(resolved))
                if resolved.object_id == opt.object_id
                    && resolved.destination
                        == tricerules_proto::ruled::v1::StackResolveDestination::Exile as i32
        )
    }));
    assert_eq!(
        engine.state.players[1].hand.len(),
        hand_before_original_draw + 1,
        "the original physical Opt completes its Scry and draw"
    );
    assert_eq!(
        engine.state.objects[&opt.object_id].zone,
        tricerules_core::Zone::Exile
    );
    let exile_generation = generation(&engine, opt.object_id);
    assert_eq!(exile_generation, opt_stack_generation + 1);
    assert_eq!(
        narset_phase_d_regression_rebound_grants(&engine),
        serde_json::json!([])
    );
    let schedule = narset_phase_d_regression_scheduled_rebounds(&engine);
    let records = schedule
        .as_array()
        .expect("diagnostics expose scheduled-Rebound records");
    assert_eq!(
        records.len(),
        1,
        "only the physical original schedules Rebound"
    );
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
    assert_eq!(records[0]["controller"], serde_json::json!(player_ids[1]));
    assert_eq!(records[0]["card_id"], serde_json::json!("opt"));
    assert_eq!(engine.state.players[1].hand.len(), hand_before_opt);

    end_active_turn(&mut engine, player_ids[1]);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        narset_phase_d_regression_scheduled_rebounds(&engine),
        schedule,
        "A's actual intervening upkeep leaves the exact Opt schedule untouched"
    );
    assert_eq!(generation(&engine, opt.object_id), exile_generation);
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    end_active_turn(&mut engine, player_ids[0]);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    assert_eq!(engine.state.active_player_id(), player_ids[1]);
    assert_eq!(engine.state.stack.len(), 1);
    let rebound_trigger = &engine.state.stack[0];
    assert!(rebound_trigger.is_triggered);
    assert!(!rebound_trigger.is_copy);
    assert_eq!(rebound_trigger.card_id, "opt");
    assert_eq!(rebound_trigger.controller, player_ids[1]);
    assert_eq!(rebound_trigger.source_permanent_id, Some(opt.object_id));
    assert_eq!(rebound_trigger.source_zone_change, opt_stack_generation);
    assert_eq!(
        rebound_trigger.trigger_context.observed_object,
        Some(tricerules_core::state::TriggerObjectRef {
            object_id: opt.object_id,
            zone_change_generation: exile_generation,
            controller_at_event: player_ids[1],
        })
    );
    let rebound_trigger_id = rebound_trigger.id;
    assert_eq!(
        narset_phase_d_regression_scheduled_rebounds(&engine),
        serde_json::json!([]),
        "the actual own-upkeep trigger consumes the one-shot schedule"
    );
    let rebound_resolution = narset_phase_d_regression_capture_priority_round(&mut engine);

    NarsetPhaseDReboundScenario {
        engine,
        player_ids,
        opt_object_id: opt.object_id,
        opt_stack_generation,
        exile_generation,
        rebound_trigger_id,
        rebound_resolution,
        activated_emblem: activate_emblem,
    }
}
#[test]
fn narset_phase_d_emblem_preserves_stack_copies_and_suppresses_later_rebound_offer() {
    const SEED: u64 = 6_110_205;

    // The unrestricted twin proves that the same external grant, physical exile incarnation,
    // and own-upkeep trigger would publish a current SpecialCast offer.
    let mut control = narset_phase_d_regression_run_twincast_rebound_scenario(SEED, false);
    assert!(!control.activated_emblem);
    assert!(control.engine.state.static_emblems.is_empty());
    assert_eq!(
        control.engine.state.objects[&control.opt_object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        generation(&control.engine, control.opt_object_id),
        control.exile_generation
    );
    let control_special = find_resolution_choice(&control.rebound_resolution)
        .expect("the unrestricted actual Rebound trigger publishes its legal cast offer");
    assert_eq!(control_special.choice_kind, ChoiceKind::SpecialCast as i32);
    assert_eq!(
        control_special.source_object_id, control.rebound_trigger_id,
        "the legal offer belongs to the actual delayed trigger"
    );
    let control_pending = control
        .engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the legal control offer parks its real trigger parent");
    assert_eq!(control_pending.deciding_player, control.player_ids[1]);
    assert_eq!(
        control_pending.presentation.source_object_id,
        control.rebound_trigger_id
    );
    assert_eq!(
        control_pending.presentation.candidates,
        vec![control.opt_object_id]
    );
    let control_legal = control.engine.initial_response_batch();
    let control_rebound_action = control_legal.legal_by_player[&control.player_ids[1]]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == control.opt_object_id)
        .expect("the control publishes the current exile cast action");
    assert_eq!(
        control_rebound_action.source_zone,
        tricerules_proto::ruled::v1::CastSourceZone::Exile as i32
    );
    assert_eq!(
        control_rebound_action.zone_change_generation,
        control.exile_generation
    );
    assert_eq!(
        control_rebound_action.cast_method,
        CastMethod::Permission as i32
    );
    assert!(control_rebound_action.casting_permission_id.is_some());

    // A activates the emblem above B's already-cast Twincast. The helper asserts that Twincast
    // remains on the stack and no copy exists until the actual Twincast resolution below.
    let mut restricted = narset_phase_d_regression_run_twincast_rebound_scenario(SEED + 1, true);
    assert!(restricted.activated_emblem);
    assert_eq!(restricted.engine.state.static_emblems.len(), 1);
    assert_eq!(
        restricted.engine.state.static_emblems[0].controller,
        restricted.player_ids[0]
    );
    assert!(restricted.rebound_resolution.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::StackResolved(resolved))
                if resolved.object_id == restricted.rebound_trigger_id
        )
    }));
    assert!(!restricted.rebound_resolution.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::ResolutionChoiceRequired(choice))
                if choice.choice_kind == ChoiceKind::SpecialCast as i32
        )
    }));
    assert!(restricted.engine.state.pending_resolution.is_none());
    assert!(restricted.engine.state.pending_spell_cast.is_none());
    assert!(restricted.engine.state.stack.is_empty());
    assert_eq!(
        narset_phase_d_regression_scheduled_rebounds(&restricted.engine),
        serde_json::json!([])
    );
    assert_eq!(
        restricted.engine.state.objects[&restricted.opt_object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        generation(&restricted.engine, restricted.opt_object_id),
        restricted.exile_generation,
        "the no-offer result still refers to Opt's exact current exile incarnation"
    );
    assert_eq!(
        restricted.engine.state.objects[&restricted.opt_object_id].owner,
        restricted.player_ids[1]
    );
    assert!(restricted
        .engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != restricted.opt_object_id));
    let restricted_legal = restricted.engine.initial_response_batch();
    assert!(restricted_legal.legal_by_player[&restricted.player_ids[1]]
        .zone_cast_actions
        .iter()
        .all(|action| action.object_id != restricted.opt_object_id));
    assert_eq!(
        restricted.engine.state.turn_step,
        tricerules_core::TurnStep::Upkeep
    );
    assert_eq!(
        restricted.engine.state.priority_player_id(),
        restricted.player_ids[1]
    );

    // The no-offer proof is the paired live control above plus the trigger's exact successful
    // consumption, not the error from this deliberately forged, nonexistent permission ID.
    let forged_permission_begin = narset_phase_d_regression_begin_zone_spell(
        exile_cast_source(restricted.opt_object_id, restricted.exile_generation),
        0,
        CastMethod::Permission as i32,
        Some(u64::MAX),
    );
    narset_phase_d_regression_assert_atomic_begin_rejection(
        &mut restricted.engine,
        restricted.player_ids[1],
        &forged_permission_begin,
        restricted.opt_object_id,
        tricerules_core::Zone::Exile,
        false,
    );
    assert_eq!(
        generation(&restricted.engine, restricted.opt_object_id),
        restricted.exile_generation
    );

    pass_priority_round(&mut restricted.engine);
    pass_priority_round(&mut restricted.engine);
    assert_eq!(
        restricted.engine.state.turn_step,
        tricerules_core::TurnStep::Main1
    );
    end_active_turn(&mut restricted.engine, restricted.player_ids[1]);
    assert_eq!(
        restricted.engine.state.turn_step,
        tricerules_core::TurnStep::Upkeep
    );
    assert_eq!(
        restricted.engine.state.active_player_id(),
        restricted.player_ids[0]
    );
    assert!(restricted.engine.state.stack.is_empty());
    pass_priority_round(&mut restricted.engine);
    pass_priority_round(&mut restricted.engine);
    assert_eq!(
        restricted.engine.state.turn_step,
        tricerules_core::TurnStep::Main1
    );
    end_active_turn(&mut restricted.engine, restricted.player_ids[0]);
    assert_eq!(
        restricted.engine.state.turn_step,
        tricerules_core::TurnStep::Upkeep
    );
    assert_eq!(
        restricted.engine.state.active_player_id(),
        restricted.player_ids[1]
    );
    assert!(restricted.engine.state.stack.is_empty());
    assert!(restricted.engine.state.pending_resolution.is_none());
    assert!(restricted.engine.state.pending_spell_cast.is_none());
    assert!(restricted
        .engine
        .state
        .stack
        .iter()
        .all(|item| !item.is_triggered));
    assert_eq!(
        narset_phase_d_regression_scheduled_rebounds(&restricted.engine),
        serde_json::json!([]),
        "the consumed no-offer opportunity does not retry at B's next own upkeep"
    );
    assert_eq!(
        restricted.engine.state.objects[&restricted.opt_object_id].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        generation(&restricted.engine, restricted.opt_object_id),
        restricted.exile_generation
    );
    assert!(restricted
        .engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != restricted.opt_object_id));
    assert_eq!(
        restricted.opt_stack_generation + 1,
        restricted.exile_generation
    );
}
