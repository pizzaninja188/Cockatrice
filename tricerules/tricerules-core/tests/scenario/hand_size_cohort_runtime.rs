//! Test-only typed faces establish the missing cohort runtime contracts before card admission.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{
    primitives::{
        AbilityCost, Amount, CastTriggerPlayer, ConditionPlayerSet, CountExpression, CounterKind,
        GameCondition, PlayerRecipient, RelativePlayerSet, SpellEffectKind, TriggerCondition,
    },
    ManaCost,
};
use tricerules_core::{state::CopiableValues, TurnStep};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, DevCommand, DevMoveCard, DevZone, RuledCommand,
};

fn set_codex_shredder_effect(engine: &mut GameEngine, source: u32, effect: SpellEffectKind) {
    let mut face = tricerules_cards::registry::global()
        .get("codex_shredder")
        .expect("Codex Shredder is registered")
        .primary_face()
        .clone();
    face.activated_abilities[0].costs = vec![AbilityCost::Tap];
    face.activated_abilities[0].effect = vec![effect];
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "codex_shredder".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
}

#[test]
fn activated_double_x_cost_pays_twice_and_preserves_chosen_x_for_draw() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_260_930,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
    let mut face = tricerules_cards::registry::global()
        .get("codex_shredder")
        .expect("Codex Shredder is registered")
        .primary_face()
        .clone();
    face.activated_abilities[0].costs = vec![
        AbilityCost::Mana(ManaCost::parse("{X}{X}").expect("double X mana cost")),
        AbilityCost::Tap,
    ];
    face.activated_abilities[0].effect = vec![SpellEffectKind::Draw {
        who: PlayerRecipient::EachPlayer,
        count: Amount::X,
    }];
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "codex_shredder".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });

    let hands_before = [
        engine.state.players[0].hand.len(),
        engine.state.players[1].hand.len(),
    ];
    let mut command = activate_ability_for(&engine, source, 0, vec![]);
    if let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() {
        ability.x_value = 2;
    }
    let revision = engine.state.command_index;
    assert!(
        engine.apply_scenario_command(0, &command).is_err(),
        "X = 2 needs four mana"
    );
    assert_eq!(engine.state.command_index, revision);
    assert!(!engine.state.objects[&source].tapped);
    engine.state.players[0].mana_pool.colorless = 4;
    let mut impossible = command.clone();
    if let Some(Cmd::ActivateAbility(ability)) = impossible.cmd.as_mut() {
        ability.x_value = u32::MAX;
    }
    assert!(
        engine.apply_scenario_command(0, &impossible).is_err(),
        "an impossible chosen X must reject without saturating into a legal cost"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 4);
    assert!(!engine.state.objects[&source].tapped);
    engine
        .apply_scenario_command(0, &command)
        .expect("activate with X = 2");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hands_before[0] + 2);
    assert_eq!(engine.state.players[1].hand.len(), hands_before[1] + 2);
}

#[test]
fn conditional_upkeep_win_uses_the_trigger_controller() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_260_931,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "ebony_owl_netsuke");
    let mut face = tricerules_cards::registry::global()
        .get("ebony_owl_netsuke")
        .expect("Ebony Owl Netsuke is registered")
        .primary_face()
        .clone();
    let exactly_thirteen = GameCondition::CardsInHand {
        players: ConditionPlayerSet::Relative(RelativePlayerSet::Controller),
        min: Some(13),
        max: Some(13),
    };
    face.triggered_abilities[0].trigger = TriggerCondition::AtBeginningOfUpkeep {
        player: CastTriggerPlayer::Controller,
    };
    face.triggered_abilities[0].intervening_if = Some(exactly_thirteen.clone());
    face.triggered_abilities[0].effect = vec![SpellEffectKind::WinGameIf {
        condition: exactly_thirteen,
    }];
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "ebony_owl_netsuke".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    while engine.state.players[0].hand.len() < 13 {
        inject_card_into_hand(&mut engine, 0, "forest");
    }

    // Finish the opponent's turn to create P0's own upkeep trigger with the exact hand count.
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = TurnStep::EndStep;
    engine
        .apply_scenario_command(1, &pass())
        .expect("opponent passes end step");
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes end step");
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.winner(), Some(0));
}

#[test]
fn source_total_counter_condition_counts_mixed_kinds_at_resolution() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_260_933,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "ebony_owl_netsuke");
    let mut face = tricerules_cards::registry::global()
        .get("ebony_owl_netsuke")
        .expect("Ebony Owl Netsuke is registered")
        .primary_face()
        .clone();
    face.triggered_abilities[0].trigger = TriggerCondition::AtBeginningOfUpkeep {
        player: CastTriggerPlayer::Controller,
    };
    face.triggered_abilities[0].intervening_if = None;
    face.triggered_abilities[0].effect = vec![SpellEffectKind::WinGameIf {
        condition: GameCondition::SourceTotalCounterCount {
            min: Some(20),
            max: None,
        },
    }];
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "ebony_owl_netsuke".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    object.add_counters(CounterKind::PlusOnePlusOne, 12, engine.state.command_index);
    object.add_counters(CounterKind::Charge, 8, engine.state.command_index);

    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = TurnStep::EndStep;
    engine
        .apply_scenario_command(1, &pass())
        .expect("opponent passes end step");
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes end step");
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.winner(), Some(0));
}

#[test]
fn source_total_counter_condition_uses_departed_generations_last_known_counters() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_260_934,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "ebony_owl_netsuke");
    let mut face = tricerules_cards::registry::global()
        .get("ebony_owl_netsuke")
        .expect("Ebony Owl Netsuke is registered")
        .primary_face()
        .clone();
    face.triggered_abilities[0].trigger = TriggerCondition::AtBeginningOfUpkeep {
        player: CastTriggerPlayer::Controller,
    };
    face.triggered_abilities[0].intervening_if = None;
    face.triggered_abilities[0].effect = vec![SpellEffectKind::WinGameIf {
        condition: GameCondition::SourceTotalCounterCount {
            min: Some(20),
            max: None,
        },
    }];
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "ebony_owl_netsuke".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    object.add_counters(CounterKind::PlusOnePlusOne, 12, engine.state.command_index);
    object.add_counters(CounterKind::Charge, 8, engine.state.command_index);
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);

    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = TurnStep::EndStep;
    engine
        .apply_scenario_command(1, &pass())
        .expect("opponent passes end step");
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes end step");
    assert_eq!(engine.state.stack.len(), 1);

    engine.enable_dev_commands();
    for zone in [DevZone::Graveyard, DevZone::Battlefield] {
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(
                priority,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 0,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: "Ebony Owl Netsuke".into(),
                            zone: zone as i32,
                            ready: true,
                        })),
                    })),
                },
            )
            .expect("move trigger source through a zone change");
    }
    assert_ne!(
        engine.state.zone_change_generation[&source],
        source_generation
    );
    assert_eq!(
        engine.state.objects[&source].counters.values().sum::<u32>(),
        0
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "original trigger stays on stack"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.winner(), Some(0));
}

#[test]
fn each_opponent_mills_its_own_hand_count_in_three_player_game() {
    let decks = Some(vec![
        deck_with("island", &[]),
        deck_with("forest", &[]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_260_932,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("new game");
    engine.state.turn_step = TurnStep::Main1;
    let source = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
    let count: CountExpression = serde_json::from_value(serde_json::json!({
        "Affine": {
            "constant": 0,
            "terms": [
                {"coefficient": 1, "quantity": "CardsInHandForRecipient"},
                {"coefficient": 1, "quantity": {"GraveyardCards": {"owners": "All"}}}
            ]
        }
    }))
    .expect("shared mill count reads the recipient hand and public graveyard snapshot");
    let effect = SpellEffectKind::Mill {
        count: Amount::Count(count),
        who: PlayerRecipient::EachOpponent,
    };
    set_codex_shredder_effect(&mut engine, source, effect);
    for (player, count) in [(1, 8), (2, 10)] {
        while engine.state.players[player].hand.len() < count {
            inject_card_into_hand(&mut engine, player, "forest");
        }
    }
    engine.state.players[1].library.truncate(3);
    let library_before = [
        engine.state.players[0].library.len(),
        engine.state.players[1].library.len(),
        engine.state.players[2].library.len(),
    ];
    apply_ability(&mut engine, 0, source, 0, vec![]).expect("activate mill ability");
    for _ in 0..3 {
        let player = engine.state.priority_player_id();
        engine
            .apply_scenario_command(player, &pass())
            .expect("pass priority to resolve");
    }
    assert_eq!(engine.state.players[0].library.len(), library_before[0]);
    assert_eq!(engine.state.players[1].library.len(), 0);
    assert_eq!(
        engine.state.players[2].library.len(),
        library_before[2] - 10,
        "P2's amount is computed before P1's short-library mill adds cards to all graveyards"
    );
}

#[test]
fn recipient_bound_mill_keeps_trigger_affected_hand_distinct_in_affine_count() {
    let decks = Some(vec![
        deck_with("island", &[]),
        deck_with("forest", &[]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_260_935,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("new three-player game");
    engine.state.turn_step = TurnStep::Main1;
    let source = inject_permanent_on_battlefield(&mut engine, 0, "iron_maiden");
    let mut face = tricerules_cards::registry::global()
        .get("iron_maiden")
        .expect("Iron Maiden is registered")
        .primary_face()
        .clone();
    let count: CountExpression = serde_json::from_value(serde_json::json!({
        "Affine": {
            "constant": 0,
            "terms": [
                {
                    "coefficient": 1,
                    "quantity": {"CardsInHand": {"players": "AffectedPlayer"}}
                },
                {"coefficient": 1, "quantity": "CardsInHandForRecipient"}
            ]
        }
    }))
    .expect("the trigger amount combines the original event player and each mill recipient");
    face.triggered_abilities[0].effect = vec![SpellEffectKind::Mill {
        count: Amount::Count(count),
        who: PlayerRecipient::EachOpponent,
    }];
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "iron_maiden".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    for (player, count) in [(1, 8), (2, 10)] {
        while engine.state.players[player].hand.len() < count {
            inject_card_into_hand(&mut engine, player, "forest");
        }
    }
    for _ in 0..10 {
        inject_library_card(&mut engine, 1, "forest");
        inject_library_card(&mut engine, 2, "plains");
    }

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0].trigger_context.affected_player,
        Some(1),
        "Iron Maiden retains the upkeep player in its trigger context"
    );
    inject_card_into_hand(&mut engine, 1, "forest");
    let library_before = [
        engine.state.players[1].library.len(),
        engine.state.players[2].library.len(),
    ];
    for _ in 0..3 {
        let player = engine.state.priority_player_id();
        engine
            .apply_scenario_command(player, &pass())
            .expect("pass priority to resolve");
    }
    assert_eq!(
        engine.state.players[1].library.len(),
        library_before[0] - 18
    );
    assert_eq!(
        engine.state.players[2].library.len(),
        library_before[1] - 19,
        "the old event player stays P1 while the new count binds to each recipient"
    );
}

#[test]
fn ordinary_scalar_each_opponent_mill_reuses_one_amount_for_all_recipients() {
    let decks = Some(vec![
        deck_with("island", &[]),
        deck_with("forest", &[]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_260_936,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("new three-player game");
    engine.state.turn_step = TurnStep::Main1;
    let source = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
    set_codex_shredder_effect(
        &mut engine,
        source,
        SpellEffectKind::Mill {
            count: Amount::Fixed(3),
            who: PlayerRecipient::EachOpponent,
        },
    );
    let library_before = [
        engine.state.players[1].library.len(),
        engine.state.players[2].library.len(),
    ];
    apply_ability(&mut engine, 0, source, 0, vec![]).expect("activate scalar mill ability");
    for _ in 0..3 {
        let player = engine.state.priority_player_id();
        engine
            .apply_scenario_command(player, &pass())
            .expect("pass priority to resolve");
    }
    assert_eq!(engine.state.players[1].library.len(), library_before[0] - 3);
    assert_eq!(engine.state.players[2].library.len(), library_before[1] - 3);
}

#[test]
fn recipient_bound_controller_mill_uses_the_controller_hand() {
    let decks = Some(vec![
        deck_with("island", &[]),
        deck_with("forest", &[]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        20_260_937,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("new three-player game");
    engine.state.turn_step = TurnStep::Main1;
    let source = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
    let count: CountExpression =
        serde_json::from_value(serde_json::json!("CardsInHandForRecipient"))
            .expect("recipient-bound hand count deserializes");
    set_codex_shredder_effect(
        &mut engine,
        source,
        SpellEffectKind::Mill {
            count: Amount::Count(count),
            who: PlayerRecipient::Controller,
        },
    );
    let count = engine.state.players[0].hand.len() as u32;
    let library_before = [
        engine.state.players[0].library.len(),
        engine.state.players[1].library.len(),
    ];

    apply_ability(&mut engine, 0, source, 0, vec![]).expect("activate controller mill");
    for _ in 0..3 {
        let player = engine.state.priority_player_id();
        engine
            .apply_scenario_command(player, &pass())
            .expect("pass priority to resolve");
    }

    assert_eq!(
        engine.state.players[0].library.len(),
        library_before[0] - count as usize
    );
    assert_eq!(engine.state.players[1].library.len(), library_before[1]);
}
