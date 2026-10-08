//! Actual-card coverage for Pyreswipe Hawk's attack pump and Expend control trigger.
//!
//! Oracle text and rulings are from the Wizards Bloomburrow release notes. CR 700.14, 611.2a-c,
//! 613.1b/613.7, 400.7, 508.3a, and 603.2 govern these scenarios.

use super::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, ControllerReference, EffectDuration, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};
use tricerules_core::{AffectedScope, ContinuousEffect, GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, AttackAssignment, ChooseTriggerTarget, DeclareAttackers, DevCommand, DevMoveCard,
    DevZone,
};

const HAWK: &str = "pyreswipe_hawk";
const RAY_OF_COMMAND: &str = "ray_of_command";
const ARTIFACT: &str = "sol_ring";

/// Keep four distinct roles: Hawk owner P0, source/ability controller P2, artifact owner P1,
/// and the later control-effect controller P3.
fn four_player_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[2, 0, 1, 3], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn setup_expend(seed: u64) -> (GameEngine, u32, u32) {
    assert!(
        CardRegistry::global().get(HAWK).is_some(),
        "Pyreswipe Hawk is registered"
    );
    let mut engine = four_player_engine(seed);
    let hawk = inject_creature_under_foreign_control(&mut engine, 1, 0, HAWK);
    let artifact = inject_permanent_on_battlefield(&mut engine, 2, ARTIFACT);
    (engine, hawk, artifact)
}

fn cast_two_mana_bear(engine: &mut GameEngine) {
    inject_card_into_hand(engine, 0, "grizzly_bears");
    give_mana(
        engine,
        2,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let cast = cast_spell(hand_index_for_card(engine, 0, "grizzly_bears"), vec![]);
    engine.apply_command(2, &cast).expect("cast two-mana spell");
}

fn expend_six(engine: &mut GameEngine) {
    for spell in 0..3 {
        cast_two_mana_bear(engine);
        if spell < 2 {
            pass_priority_round(engine);
        }
    }
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(2)
            .mana_spent_casting_spells,
        6
    );
    assert_eq!(engine.state.pending_triggers.len(), 1);
}

fn choose_artifact(engine: &mut GameEngine, artifact: u32) {
    engine
        .apply_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: vec![TargetRef {
                        kind: TargetRefKind::Permanent as i32,
                        object_id: artifact,
                        group_index: 0,
                        ..Default::default()
                    }],
                    ..Default::default()
                })),
            },
        )
        .expect("choose the artifact target");
}

fn choose_no_artifact(engine: &mut GameEngine) {
    engine
        .apply_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget::default())),
            },
        )
        .expect("decline the optional target");
}

fn pass_until_player(engine: &mut GameEngine, player: i32) {
    for _ in 0..engine.state.players.len() {
        let actor = engine.state.priority_player_id();
        if actor == player {
            return;
        }
        engine
            .apply_command(actor, &pass())
            .expect("pass priority to the requested player");
    }
    assert_eq!(engine.state.priority_player_id(), player);
}

fn cast_ray_of_command(engine: &mut GameEngine, player_index: usize, player_id: i32, hawk: u32) {
    inject_card_into_hand(engine, player_index, RAY_OF_COMMAND);
    give_mana(
        engine,
        player_id,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let ray = cast_spell(
        hand_index_for_card(engine, player_index, RAY_OF_COMMAND),
        target_object(hawk),
    );
    engine
        .apply_command(player_id, &ray)
        .expect("cast Ray of Command on the Hawk");
}

fn dev_move_owned_card(engine: &mut GameEngine, owner: i32, card_name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move the owned Hawk to the requested zone");
}

#[test]
fn pyreswipe_hawk_has_its_complete_single_face_and_attack_value_is_snapshotted_on_resolution() {
    let card = CardRegistry::global()
        .get(HAWK)
        .expect("Pyreswipe Hawk definition");
    assert_eq!(card.name, "Pyreswipe Hawk");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{R}{R}");
    assert_eq!(face.types, ["Creature", "Elemental", "Bird"]);
    assert_eq!(face.power, Some(4));
    assert_eq!(face.toughness, Some(4));
    assert_eq!(
        face.keywords,
        [
            tricerules_cards::primitives::Keyword::Flying,
            tricerules_cards::primitives::Keyword::Haste
        ]
    );
    assert_eq!(face.triggered_abilities.len(), 2);
    assert_eq!(
        face.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        face.triggered_abilities[0].trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    ));
    assert_eq!(
        face.triggered_abilities[1].presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert!(matches!(
        face.triggered_abilities[1].trigger,
        TriggerCondition::WheneverPlayerExpendsMana { amount: 6, .. }
    ));
    let target_group = &face.triggered_abilities[1]
        .targeting
        .as_ref()
        .expect("the Expend trigger has target selection")
        .groups[0];
    assert_eq!(target_group.min, 0);
    assert_eq!(target_group.max, 1);

    let mut engine = four_player_engine(20_261_008);
    let hawk = inject_creature_on_battlefield(&mut engine, 0, HAWK);
    // The general scenario helper uses a 2/2 fixture override; this card needs its printed 4/4.
    let hawk_object = engine.state.objects.get_mut(&hawk).unwrap();
    hawk_object.power = None;
    hawk_object.toughness = None;
    assert_eq!(
        engine
            .characteristics(hawk)
            .expect("printed Hawk characteristics")
            .power,
        Some(4)
    );
    let source_artifact = inject_permanent_on_battlefield(&mut engine, 0, ARTIFACT);
    let opponent_artifact = inject_permanent_on_battlefield(&mut engine, 1, "darksteel_forge");
    assert_eq!(
        engine
            .characteristics(source_artifact)
            .expect("Sol Ring characteristics")
            .mana_value,
        1
    );
    assert_eq!(
        engine
            .characteristics(opponent_artifact)
            .expect("opponent artifact characteristics")
            .mana_value,
        9
    );

    engine
        .apply_command(2, &primitive_yield())
        .expect("main phase to begin combat");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    let generation = engine
        .state
        .zone_change_generation
        .get(&hawk)
        .copied()
        .unwrap_or(0);
    engine
        .apply_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![AttackAssignment {
                        attacker_object_id: hawk,
                        attacker_zone_change_generation: generation,
                        defender: Some(TargetRef {
                            object_id: 0,
                            kind: TargetRefKind::Player as i32,
                            ..Default::default()
                        }),
                        defending_player_id: 0,
                        ..Default::default()
                    }],
                })),
            },
        )
        .expect("declare Pyreswipe Hawk attacking P0");

    // The stack trigger reads the largest currently controlled artifact mana value at resolution.
    let late_artifact = inject_permanent_on_battlefield(&mut engine, 0, "thran_dynamo");
    assert_eq!(
        engine
            .characteristics(late_artifact)
            .expect("Thran Dynamo characteristics")
            .mana_value,
        4
    );
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .characteristics(hawk)
            .expect("Hawk characteristics")
            .power,
        Some(8),
        "the opponent's mana-value-9 artifact is excluded; P2's mana-value-4 artifact sets X"
    );

    // Once the trigger resolves, later artifacts do not recalculate its one-shot +X bonus.
    let post_resolution_artifact =
        inject_permanent_on_battlefield(&mut engine, 0, "darksteel_forge");
    assert_eq!(
        engine
            .characteristics(post_resolution_artifact)
            .expect("post-resolution Darksteel Forge characteristics")
            .mana_value,
        9
    );
    assert_eq!(
        engine
            .characteristics(hawk)
            .expect("Hawk characteristics after the artifact maximum changes")
            .power,
        Some(8),
        "the resolved +4 remains fixed when the controlled-artifact maximum later rises to 9"
    );
}

#[test]
fn expend_six_target_is_optional_and_requires_the_source_at_resolution() {
    let (mut engine, _hawk, artifact) = setup_expend(20_261_009);
    expend_six(&mut engine);
    choose_no_artifact(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&artifact].controller, 1);

    // This identity-focused case uses a source owned by P2 so the dev fixture can move the exact
    // object through P2's zones while its ability controller remains P2.
    let mut engine = four_player_engine(20_261_010);
    let hawk = inject_creature_on_battlefield(&mut engine, 0, HAWK);
    let artifact = inject_permanent_on_battlefield(&mut engine, 2, ARTIFACT);
    expend_six(&mut engine);
    choose_artifact(&mut engine, artifact);
    dev_move_owned_card(&mut engine, 2, "Pyreswipe Hawk", DevZone::Graveyard);
    dev_move_owned_card(&mut engine, 2, "Pyreswipe Hawk", DevZone::Battlefield);
    assert_eq!(engine.state.objects[&hawk].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&hawk].controller, 2);
    assert_eq!(engine.state.zone_change_generation[&hawk], 2);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&artifact].controller, 1,
        "a returned Hawk with the same numeric id is a new source incarnation"
    );
}

#[test]
fn expend_target_that_leaves_and_returns_is_a_different_artifact_object() {
    let (mut engine, _hawk, artifact) = setup_expend(20_261_014);
    expend_six(&mut engine);
    choose_artifact(&mut engine, artifact);
    dev_move_owned_card(&mut engine, 1, "Sol Ring", DevZone::Graveyard);
    dev_move_owned_card(&mut engine, 1, "Sol Ring", DevZone::Battlefield);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.zone_change_generation[&artifact], 2);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&artifact].controller, 1,
        "the target snapshot does not follow the returned artifact generation"
    );
}

#[test]
fn expend_trigger_needs_hawk_under_its_controller_and_can_start_after_control_returns() {
    // If another player still controls Hawk when the trigger resolves, the duration never starts.
    let (mut engine, hawk, artifact) = setup_expend(20_261_011);
    expend_six(&mut engine);
    choose_artifact(&mut engine, artifact);
    pass_until_player(&mut engine, 3);
    cast_ray_of_command(&mut engine, 3, 3, hawk);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&hawk].controller, 3);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&artifact].controller, 1);

    // A temporary loss before the first application does not end a duration that has not begun.
    let (mut engine, hawk, artifact) = setup_expend(20_261_012);
    expend_six(&mut engine);
    choose_artifact(&mut engine, artifact);
    pass_until_player(&mut engine, 3);
    cast_ray_of_command(&mut engine, 3, 3, hawk);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&hawk].controller, 3);
    cast_ray_of_command(&mut engine, 0, 2, hawk);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&hawk].controller, 2);
    // P3's Ray of Command leaves a delayed tap trigger above Pyreswipe's trigger.
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&artifact].controller, 2,
        "the source is controlled by the ability controller when the effect first applies"
    );
}

#[test]
fn source_control_loss_after_application_expires_the_artifact_lease_permanently() {
    let (mut engine, hawk, artifact) = setup_expend(20_261_013);
    expend_six(&mut engine);
    choose_artifact(&mut engine, artifact);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&artifact].controller, 2);

    // Resolve the third Grizzly Bears spell below the trigger, then open a new response window.
    pass_priority_round(&mut engine);
    cast_two_mana_bear(&mut engine);
    pass_until_player(&mut engine, 3);
    cast_ray_of_command(&mut engine, 3, 3, hawk);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&hawk].controller, 3);
    assert_eq!(
        engine.state.objects[&artifact].controller, 1,
        "the Hawk's control-duration lease ends as soon as P2 loses control of it"
    );

    // P2 later regains Hawk with another Ray; the ended lease must not revive.
    cast_ray_of_command(&mut engine, 0, 2, hawk);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&hawk].controller, 2);
    assert_eq!(engine.state.objects[&artifact].controller, 1);
}

#[test]
fn source_control_loss_preserves_a_later_layer_two_control_effect_on_the_artifact() {
    let (mut engine, hawk, artifact) = setup_expend(20_261_015);
    expend_six(&mut engine);
    choose_artifact(&mut engine, artifact);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&artifact].controller, 2);

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(artifact),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(3),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index.saturating_add(1),
    });
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&artifact].controller, 3);

    cast_two_mana_bear(&mut engine);
    pass_until_player(&mut engine, 3);
    cast_ray_of_command(&mut engine, 3, 3, hawk);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&hawk].controller, 3);
    assert_eq!(
        engine.state.objects[&artifact].controller, 3,
        "expiring Pyreswipe's earlier timestamp cannot remove or overwrite a later control effect"
    );
    assert!(!engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| matches!(
            effect.duration,
            EffectDuration::WhileSourceControlledBy { .. }
        )));
}
