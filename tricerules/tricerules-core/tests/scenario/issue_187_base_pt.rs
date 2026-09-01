use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
use tricerules_cards::{CardRegistry, CounterKind};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{dev_command::Dev, DevCommand, DevMoveCard, DevZone};

#[test]
fn issue_187_cards_are_registered_with_complete_authored_modes() {
    let registry = CardRegistry::global();
    let mind = registry
        .get("mind_transfer_protocol")
        .expect("Mind Transfer Protocol is registered");
    assert_eq!(mind.primary_face().spell_effect.len(), 3);

    let quandrix = registry
        .get("quandrix_charm")
        .expect("Quandrix Charm is registered");
    assert_eq!(
        quandrix
            .primary_face()
            .modal_spell
            .as_ref()
            .expect("Quandrix Charm is modal")
            .modes
            .len(),
        3
    );

    let galion = registry
        .get("galion,_elvenkings_butler")
        .expect("Galion is registered");
    assert_eq!(galion.primary_face().triggered_abilities.len(), 1);
}

#[test]
fn issue_187_mind_transfer_animates_an_artifact_sets_base_pt_and_draws() {
    let mut engine = GameEngine::new(187_001, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "explosive_apparatus");
    inject_card_into_hand(&mut engine, 0, "mind_transfer_protocol");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "mind_transfer_protocol");
    engine
        .apply_command(0, &cast_spell(slot, target_object(artifact)))
        .expect("cast Mind Transfer Protocol on an artifact");
    let hand_before_resolution = engine.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut engine);

    let characteristics = engine.characteristics(artifact).expect("animated artifact");
    assert!(characteristics.has_type("Artifact"));
    assert!(characteristics.has_type("Creature"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(4), Some(5))
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_resolution + 1
    );

    let timestamp = engine.state.command_index;
    engine
        .state
        .objects
        .get_mut(&artifact)
        .expect("artifact")
        .add_counters(CounterKind::PlusOnePlusOne, 1, timestamp);
    assert_eq!(
        (
            engine.effective_power(artifact),
            engine.effective_toughness(artifact)
        ),
        (Some(5), Some(6)),
        "layer 7c counters apply after the layer 7b base-P/T setter"
    );
}

#[test]
fn issue_187_quandrix_charm_third_mode_sets_base_pt() {
    let mut engine = GameEngine::new(187_002, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "quandrix_charm");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "quandrix_charm");
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(2, target_object(target))]))
        .expect("cast Quandrix Charm's third mode");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        (
            engine.effective_power(target),
            engine.effective_toughness(target)
        ),
        (Some(5), Some(5))
    );
}

#[test]
fn issue_187_galion_samples_source_pt_at_resolution_and_uses_lki() {
    let decks = Some(vec![
        deck_with("forest", &["galion,_elvenkings_butler"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(187_003, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_declare_attackers(&mut engine);
    let target = battlefield_object_for_card(&engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "galion,_elvenkings_butler");
    let galion = put_creature_on_battlefield(&mut engine, 0, "galion,_elvenkings_butler");

    engine
        .apply_command(0, &declare_attackers(vec![galion]))
        .expect("attack with Galion");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: target_object(target),
                    ..Default::default()
                })),
            },
        )
        .expect("target another controlled creature");
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(galion)
    );

    let timestamp = engine.state.command_index;
    engine
        .state
        .objects
        .get_mut(&galion)
        .expect("Galion")
        .add_counters(CounterKind::PlusOnePlusOne, 1, timestamp);
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .add_counters(CounterKind::PlusOnePlusOne, 1, timestamp);
    assert_eq!(
        (
            engine.effective_power(galion),
            engine.effective_toughness(galion)
        ),
        (Some(5), Some(5))
    );

    engine.enable_dev_commands();
    engine
        .apply_command(
            engine.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: "Galion, Elvenking's Butler".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move Galion away before its trigger resolves");
    assert_eq!(engine.state.objects[&galion].zone, Zone::Graveyard);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        (
            engine.effective_power(target),
            engine.effective_toughness(target)
        ),
        (Some(6), Some(6)),
        "Galion's 5/5 LKI becomes the base values and the target's counter applies afterward"
    );
}

#[test]
fn issue_187_layer_7b_setters_preserve_signed_internal_values() {
    let mut engine = GameEngine::new(187_004, &[0, 1], 20, None, true).expect("new engine");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(target),
        kind: ContinuousEffectKind::Layer7bSetPt {
            power: -1,
            toughness: 2,
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    let timestamp = engine.state.command_index;
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("creature")
        .add_counters(CounterKind::PlusOnePlusOne, 1, timestamp);
    assert_eq!(
        (
            engine.effective_power(target),
            engine.effective_toughness(target)
        ),
        (Some(0), Some(3)),
        "the signed -1 base power is not clamped before the layer 7c counter"
    );
}
