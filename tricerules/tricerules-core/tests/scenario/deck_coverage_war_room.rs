//! War Room's actual card, frozen commander declaration, and atomic activation payment.

use super::helpers::*;
use tricerules_core::{EngineDeck, GameEngine};
use tricerules_proto::ruled::v1::{self as rv1, PreviewPayment};

fn war_room_engine(commanders: &[&str]) -> (GameEngine, u32) {
    war_room_engine_with_opponent(commanders, &[])
}

fn war_room_engine_with_opponent(commanders: &[&str], opponent: &[&str]) -> (GameEngine, u32) {
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        510_001,
        &[0, 1],
        20,
        Some(vec![
            EngineDeck {
                mainboard: deck_with("island", &["war_room"]),
                commanders: commanders.iter().map(|name| (*name).into()).collect(),
            },
            EngineDeck {
                mainboard: deck_with("forest", &[]),
                commanders: opponent.iter().map(|name| (*name).into()).collect(),
            },
        ]),
        true,
    )
    .expect("War Room and declaration resolve");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "war_room", false);
    (engine, source)
}

fn info(engine: &mut GameEngine, source: u32) -> rv1::AbilityInfo {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| {
            let Some(Ev::ZoneView(view)) = &event.ev else {
                return None;
            };
            view.per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == source)
                .and_then(|object| {
                    object
                        .activated_abilities
                        .iter()
                        .find(|ability| ability.ability_index == 1)
                })
                .cloned()
        })
        .expect("actual draw ability is published")
}

fn reject(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    engine
        .apply_command(actor, command)
        .expect_err("illegal activation must reject");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

fn preview(engine: &GameEngine, actor: i32, command: &RuledCommand) -> rv1::PaymentPreview {
    let Some(Cmd::ActivateAbility(activation)) = &command.cmd else {
        unreachable!()
    };
    let mut activation = activation.clone();
    if activation.payment.is_none() {
        activation.payment = Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(rv1::CostObjectRef {
                object_id: activation.source_object_id,
                zone_change_generation: activation.expected_zone_change_generation,
            }),
            mana: Some(rv1::PaymentMana {
                c: 3,
                ..Default::default()
            }),
            ..Default::default()
        });
    }
    let before = engine.diagnostic_snapshot().unwrap();
    let result = engine.preview_payment(
        actor,
        &PreviewPayment {
            transaction_id: 510,
            revision: engine.state.command_index,
            activate_ability: Some(activation),
            ..Default::default()
        },
    );
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before,
        "preview/cancel never charges"
    );
    result
}

#[test]
fn war_room_resolves_zero_through_five_unique_colors_in_labels_and_actual_payments() {
    // Multiple declarations exercise the constructor's union contract, not partner eligibility.
    let cases: &[(&[&str], i32)] = &[
        (&["viv_vision,_teen_synthezoid"], 0),
        (&["kami_of_the_crescent_moon"], 1),
        (&["bello,_bard_of_the_brambles"], 2),
        (
            &["bello,_bard_of_the_brambles", "kami_of_the_crescent_moon"],
            3,
        ),
        (&["atraxa,_praetors_voice"], 4),
        (&["atraxa,_praetors_voice", "daretti,_scrap_savant"], 5),
        (&["atraxa,_praetors_voice", "kami_of_the_crescent_moon"], 4),
    ];
    for &(commanders, life) in cases {
        let (mut engine, source) = war_room_engine(commanders);
        assert!(engine.state.players[0].has_declared_commander);
        assert_eq!(engine.state.players[0].color_identity.len(), life as usize);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 3,
                ..Default::default()
            },
        );
        let ability = info(&mut engine, source);
        assert!(ability.activatable, "{commanders:?}");
        assert!(!ability.is_mana_ability);
        assert_eq!(ability.cost_label, format!("{{3}}, {{T}}, Pay {life} life"));
        assert_eq!(
            ability.text,
            format!("{{3}}, {{T}}, Pay {life} life: Draw a card.")
        );
        assert_eq!(ability.presentation.unwrap().fallback_text, ability.text);
        engine
            .apply_command(0, &activate_ability_for(&engine, source, 1, vec![]))
            .unwrap();
        assert_eq!(engine.state.players[0].life, 20 - life);
        assert_eq!(
            engine.state.turn_history.current.player(0).life_lost,
            life as u64
        );
        resolve_entire_stack_two_player(&mut engine);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn war_room_missing_declaration_publishes_disabled_draw_and_rejects_preview() {
    let (mut engine, source) = war_room_engine(&[]);
    assert!(!engine.state.players[0].has_declared_commander);
    assert!(engine.state.players[0].color_identity.is_empty());
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let ability = info(&mut engine, source);
    assert!(!ability.activatable);
    assert_eq!(ability.cost_label, "{3}, {T}, Pay life (no commander)");
    assert!(
        !preview(
            &engine,
            0,
            &activate_ability_for(&engine, source, 1, vec![])
        )
        .valid
    );
}

#[test]
fn war_room_preview_cancellation_and_normalized_commit_charge_exactly_once() {
    for commander in ["kami_of_the_crescent_moon", "viv_vision,_teen_synthezoid"] {
        let (mut engine, source) = war_room_engine(&[commander]);
        let life = engine.state.players[0].color_identity.len() as i32;
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 3,
                ..Default::default()
            },
        );
        let mut command = activate_ability_for(&engine, source, 1, vec![]);
        assert!(!preview(&engine, 1, &command).valid);
        let result = preview(&engine, 0, &command);
        assert!(result.valid && result.complete, "{result:?}");
        // Cancel is discarding this proposal: no authoritative command was submitted.
        let again = preview(&engine, 0, &command);
        assert_eq!(result, again);
        let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
            unreachable!()
        };
        activation.payment = result.selection;
        activation.restricted_mana = result.restricted_mana;
        engine.apply_command(0, &command).unwrap();
        assert_eq!(engine.state.players[0].life, 20 - life);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        assert!(engine.state.objects[&source].tapped);
        assert_eq!(engine.state.stack.len(), 1);
    }
}

#[test]
fn war_room_illegal_mana_life_actor_tap_generation_and_stale_preview_are_atomic() {
    for invalid in ["mana", "life", "actor", "tap", "generation", "preview"] {
        let (mut engine, source) = war_room_engine(&["atraxa,_praetors_voice"]);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 3,
                ..Default::default()
            },
        );
        let mut command = activate_ability_for(&engine, source, 1, vec![]);
        let actor = match invalid {
            "mana" => {
                engine.state.players[0].mana_pool.colorless = 2;
                0
            }
            "life" => {
                engine.state.players[0].life = 3;
                0
            }
            "actor" => 1,
            "tap" => {
                engine.state.objects.get_mut(&source).unwrap().tapped = true;
                0
            }
            "generation" => {
                let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
                    unreachable!()
                };
                activation.expected_zone_change_generation += 1;
                0
            }
            "preview" => {
                let result = preview(&engine, 0, &command);
                assert!(result.valid && result.complete);
                let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
                    unreachable!()
                };
                activation.payment = result.selection;
                activation.restricted_mana = result.restricted_mana;
                engine.state.command_index += 1;
                0
            }
            _ => unreachable!(),
        };
        if invalid != "preview" {
            let result = preview(&engine, actor, &command);
            assert!(!result.valid || !result.complete, "{invalid}: {result:?}");
        }
        reject(&mut engine, actor, &command);
    }
}

#[test]
fn war_room_uses_current_controller_identity_and_draw_recipient() {
    let (mut engine, source) =
        war_room_engine_with_opponent(&["atraxa,_praetors_voice"], &["kami_of_the_crescent_moon"]);
    engine.state.players[0]
        .battlefield
        .retain(|oid| *oid != source);
    engine.state.players[1].battlefield.push(source);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .base_controller = 1;
    engine.state.objects.get_mut(&source).unwrap().controller = 1;
    engine.state.priority_idx = 1;
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    assert_eq!(info(&mut engine, source).cost_label, "{3}, {T}, Pay 1 life");
    let hand0 = engine.state.players[0].hand.len();
    let hand1 = engine.state.players[1].hand.len();
    let command = activate_ability_for(&engine, source, 1, vec![]);
    reject(&mut engine, 0, &command);
    engine.apply_command(1, &command).unwrap();
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 19);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand0);
    assert_eq!(engine.state.players[1].hand.len(), hand1 + 1);
}

fn dev_move(name: &str, zone: rv1::DevZone) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(rv1::DevCommand {
            target_player_id: 0,
            dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                card_name: name.into(),
                zone: zone as i32,
                ready: true,
            })),
        })),
    }
}

#[test]
fn war_room_identity_is_frozen_after_commander_color_changes_and_hidden_zone_moves() {
    use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
    use tricerules_cards::Color;
    use tricerules_core::{AffectedScope, ContinuousEffect};
    let (mut engine, source) = war_room_engine(&["atraxa,_praetors_voice"]);
    let commander = engine.state.players[0].command_zone[0];
    engine.enable_dev_commands();
    // Establish the battlefield fixture directly: the dev mover intentionally excludes Command.
    // The constructor unit fixture separately moves a commander through the engine zone path.
    engine.state.players[0].command_zone.clear();
    engine.state.players[0].battlefield.push(commander);
    engine.state.objects.get_mut(&commander).unwrap().zone = tricerules_core::Zone::Battlefield;
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(commander),
        kind: ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    assert_eq!(
        engine.characteristics(commander).unwrap().colors,
        vec![Color::Red]
    );
    for zone in [rv1::DevZone::Hand, rv1::DevZone::Library] {
        engine
            .apply_command(0, &dev_move("Atraxa, Praetors' Voice", zone))
            .unwrap();
        assert!(engine.state.players[0].command_zone.is_empty());
        assert!(engine.state.players[0].has_declared_commander);
        assert_eq!(engine.state.players[0].color_identity.len(), 4);
        assert_eq!(info(&mut engine, source).cost_label, "{3}, {T}, Pay 4 life");
    }
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 1, vec![]))
        .unwrap();
    assert_eq!(engine.state.players[0].life, 16);
}

#[test]
fn war_room_paid_ability_draws_after_source_departure() {
    let (mut engine, source) = war_room_engine(&["kami_of_the_crescent_moon"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let top = engine.state.players[0].library[0];
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 1, vec![]))
        .unwrap();
    engine.enable_dev_commands();
    engine
        .apply_command(0, &dev_move("War Room", rv1::DevZone::Graveyard))
        .unwrap();
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.players[0].hand.contains(&top));
    assert_eq!(engine.state.players[0].life, 19);
}

#[test]
fn war_room_exact_life_payment_is_accepted_before_existing_state_based_loss() {
    let (mut engine, source) = war_room_engine(&["kami_of_the_crescent_moon"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    engine.state.players[0].life = 1;
    assert!(info(&mut engine, source).activatable);
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 1, vec![]))
        .unwrap();
    assert_eq!(engine.state.players[0].life, 0);
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.is_terminal());
    assert_eq!(engine.state.turn_history.current.player(0).life_lost, 1);
}

#[test]
fn war_room_replays_accepted_mana_activation_draw_and_pass_commands() {
    fn game() -> (GameEngine, u32) {
        let (mut engine, source) = war_room_engine(&["kami_of_the_crescent_moon"]);
        engine.enable_dev_commands();
        (engine, source)
    }
    let (mut engine, source) = game();
    let mut log = vec![];
    let mut apply = |engine: &mut GameEngine, actor, command: RuledCommand| {
        let batch = engine.apply_command(actor, &command).unwrap();
        log.push((actor, command, batch, engine.diagnostic_snapshot().unwrap()));
    };
    apply(
        &mut engine,
        0,
        RuledCommand {
            cmd: Some(Cmd::DevCommand(rv1::DevCommand {
                target_player_id: 0,
                dev: Some(rv1::dev_command::Dev::AddMana(rv1::DevAddMana {
                    c: 3,
                    ..Default::default()
                })),
            })),
        },
    );
    let command = activate_ability_for(&engine, source, 1, vec![]);
    apply(&mut engine, 0, command);
    for _ in 0..8 {
        if engine.state.stack.is_empty() {
            break;
        }
        let actor = engine.state.priority_player_id();
        apply(&mut engine, actor, pass());
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].life, 19);
    let (mut replay, replay_source) = game();
    assert_eq!(source, replay_source);
    for (actor, command, batch, snapshot) in log {
        assert_eq!(replay.apply_command(actor, &command).unwrap(), batch);
        assert_eq!(replay.diagnostic_snapshot().unwrap(), snapshot);
    }
}

#[test]
fn war_room_pays_one_life_taps_and_draws_only_on_resolution() {
    let (mut engine, source) = war_room_engine(&["kami_of_the_crescent_moon"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let before_hand = engine.state.players[0].hand.len();
    let top = engine.state.players[0].library[0];
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 1, vec![]))
        .expect("pay three mana, tap, and one life");
    assert_eq!(engine.state.players[0].life, 19);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].hand.len(), before_hand);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.players[0].hand.contains(&top));
    assert_eq!(engine.state.players[0].hand.len(), before_hand + 1);
}

#[test]
fn war_room_without_commander_rejects_draw_purely_but_produces_colorless_mana() {
    let (mut engine, source) = war_room_engine(&[]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let before = engine.diagnostic_snapshot().unwrap();
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 1, vec![]))
        .expect_err("an absent commander is not a colorless commander");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, vec![]))
        .expect("ordinary colorless mana needs no commander");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 4);
    assert_eq!(engine.state.players[0].life, 20);
    assert!(engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());
}
