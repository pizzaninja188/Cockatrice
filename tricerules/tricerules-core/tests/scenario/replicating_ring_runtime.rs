//! Necessary counter contracts before Replicating Ring card admission.
use super::helpers::*;
use tricerules_cards::primitives::{
    AbilityCost, CastTriggerPlayer, CounterKind, GameCondition, SpellEffectKind, TriggerCondition,
};
use tricerules_core::{state::CopiableValues, GameEngine, TurnStep};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, DevCommand, DevMoveCard, DevZone, RuledCommand,
};

#[test]
fn night_counter_has_public_label() {
    let counter: CounterKind = serde_json::from_str("\"Night\"").expect("Night counter kind");
    assert_eq!(counter.label(), "night");
}

#[test]
fn remove_all_charge_counters_preserves_other_kinds_and_zero_is_noop() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        507_001,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
    let mut face = tricerules_cards::registry::global()
        .get("codex_shredder")
        .unwrap()
        .primary_face()
        .clone();
    face.activated_abilities[0].costs = vec![AbilityCost::Tap];
    face.activated_abilities[0].effect = vec![serde_json::from_str::<SpellEffectKind>(
        r#"{"RemoveAllCounters":{"counter":"Charge","subject":"Source"}}"#,
    )
    .expect("remove-all counter instruction")];
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.copiable_values = Some(CopiableValues {
        source_card_id: "codex_shredder".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    object.add_counters(CounterKind::Charge, 123, engine.state.command_index);
    object.add_counters(CounterKind::Quest, 2, engine.state.command_index);
    for _ in 0..2 {
        engine.state.objects.get_mut(&source).unwrap().tapped = false;
        let command = activate_ability_for(&engine, source, 0, vec![]);
        engine.apply_command(0, &command).expect("activate removal");
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            0
        );
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Quest),
            2
        );
    }
}

#[test]
fn source_counter_condition_uses_exact_departed_generation_lki() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        507_002,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "ebony_owl_netsuke");
    let mut face = tricerules_cards::registry::global()
        .get("ebony_owl_netsuke")
        .unwrap()
        .primary_face()
        .clone();
    face.triggered_abilities[0].trigger = TriggerCondition::AtBeginningOfUpkeep {
        player: CastTriggerPlayer::Controller,
    };
    face.triggered_abilities[0].intervening_if = None;
    face.triggered_abilities[0].effect = vec![SpellEffectKind::WinGameIf {
        condition: GameCondition::SourceCounterCount {
            counter: CounterKind::Charge,
            min: Some(8),
            max: None,
        },
    }];
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.copiable_values = Some(CopiableValues {
        source_card_id: "ebony_owl_netsuke".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    object.add_counters(CounterKind::Charge, 8, engine.state.command_index);
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = TurnStep::EndStep;
    engine.apply_command(1, &pass()).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    assert_eq!(engine.state.stack.len(), 1);
    engine.enable_dev_commands();
    for zone in [DevZone::Graveyard, DevZone::Battlefield] {
        engine
            .apply_command(
                engine.state.priority_player_id(),
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
            .expect("source leaves and returns while old trigger remains");
    }
    assert_ne!(
        engine.state.zone_change_generation[&source],
        original_generation
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.winner(),
        Some(0),
        "old trigger reads eight counters from its exact incarnation"
    );
}
