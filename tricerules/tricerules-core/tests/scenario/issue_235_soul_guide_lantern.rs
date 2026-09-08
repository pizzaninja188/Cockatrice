//! Soul-Guide Lantern: targeted ETB, untargeted graveyard cohort, and distinct paid abilities.
use super::helpers::*;
use tricerules_cards::primitives::{
    CardTypeFilter, CastTriggerPlayer, ContinuousEffectKind, EffectDuration, RelativePlayerSet,
    SpellEffectKind, TriggerCondition, ZoneCardFilter, ZoneEventCardinality,
};
use tricerules_cards::{CardRegistry, CounterKind};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    self as rv1, dev_command::Dev, DevCommand, DevMoveCard, DevPutCardInZone, DevZone,
};

fn engine(seats: &[i32]) -> GameEngine {
    let decks = Some(vec![vec!["forest".to_string(); 20]; 2]);
    let mut engine = GameEngine::new(235_001, &seats[..2], 20, decks, true).unwrap();
    // Session creation still supports two seats. Extend the fixture to prove the
    // same player-set-generic resolution used by existing multiplayer scenarios.
    for &player in &seats[2..] {
        engine
            .state
            .players
            .push(tricerules_core::state::PlayerState::new(player, 20));
    }
    pass_round(&mut engine);
    pass_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    engine
}

fn pass_round(engine: &mut GameEngine) -> RuledEventBatch {
    answer_trigger_order_in_engine_order(engine);
    let mut batch = RuledEventBatch::default();
    for _ in 0..engine.state.players.len() {
        let player = engine.state.priority_player_id();
        batch = engine.apply_command(player, &pass()).unwrap();
    }
    batch
}

#[test]
fn mass_exile_selects_every_opponent_at_resolution_and_pays_sacrifice_once() {
    let mut engine = engine(&[7, 19, 31]);
    let lantern = inject_permanent_on_battlefield(&mut engine, 0, "soul-guide_lantern");
    let own = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let first = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    let second = inject_graveyard_card(&mut engine, 2, "grizzly_bears");
    engine
        .apply_command(7, &activate_ability(lantern, 0, vec![]))
        .expect("mass exile needs no targets or mana");
    assert_eq!(engine.state.objects[&lantern].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert!(engine
        .apply_command(7, &activate_ability(lantern, 0, vec![]))
        .is_err());
    let late = inject_graveyard_card(&mut engine, 1, "forest");
    let batch = pass_round(&mut engine);
    for oid in [first, second, late] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Exile);
        assert_eq!(engine.state.zone_change_generation[&oid], 1);
        assert!(batch.events.iter().any(|event| matches!(&event.ev,
            Some(Ev::PermanentMoved(moved)) if moved.object_id == oid
                && moved.destination == rv1::permanent_moved::Destination::Exile as i32
                && moved.owner_player_id == engine.state.objects[&oid].owner)));
    }
    for oid in [own, lantern] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
    }
    assert_eq!(engine.state.zone_change_generation[&lantern], 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn mass_exile_rejects_forged_targets_and_stale_activation_without_changing_state() {
    let mut engine = engine(&[0, 1]);
    let lantern = inject_permanent_on_battlefield(&mut engine, 0, "soul-guide_lantern");
    let target = inject_graveyard_card(&mut engine, 1, "gladecover_scout");
    for targets in [target_object(target), target_player(1)] {
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(0, &activate_ability(lantern, 0, targets))
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    engine.state.zone_change_generation.insert(lantern, 2);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(0, &activate_ability(lantern, 0, vec![]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine
        .apply_command(0, &activate_ability_for(&engine, lantern, 0, vec![]))
        .unwrap();
    pass_round(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}

fn dev_move(player: i32, card_name: &str, zone: DevZone) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player,
            dev: Some(Dev::MoveCard(DevMoveCard {
                card_name: card_name.into(),
                zone: zone as i32,
                ready: false,
            })),
        })),
    }
}

#[test]
fn etb_does_not_exile_a_card_that_left_and_returned_to_the_graveyard() {
    let mut engine = engine(&[0, 1]);
    engine.enable_dev_commands();
    let target = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "soul-guide_lantern");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "soul-guide_lantern");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_round(&mut engine);
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
        .unwrap();
    engine
        .apply_command(0, &dev_move(1, "Grizzly Bears", DevZone::Hand))
        .unwrap();
    engine
        .apply_command(0, &dev_move(1, "Grizzly Bears", DevZone::Graveyard))
        .unwrap();
    assert_eq!(engine.state.zone_change_generation[&target], 2);
    pass_round(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn finality_replaces_the_sacrifice_without_repaying_or_disabling_mass_exile() {
    let mut engine = engine(&[0, 1]);
    let lantern = inject_permanent_on_battlefield(&mut engine, 0, "soul-guide_lantern");
    let target = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&lantern)
        .unwrap()
        .add_counters(CounterKind::Finality, 1, 0);
    engine
        .apply_command(0, &activate_ability(lantern, 0, vec![]))
        .unwrap();
    assert_eq!(engine.state.objects[&lantern].zone, Zone::Exile);
    pass_round(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.zone_change_generation[&lantern], 1);
    assert_eq!(
        engine.state.objects[&lantern].counter_count(CounterKind::Finality),
        0
    );
}

#[test]
fn graveyard_exile_primitive_filters_printed_cards_for_each_relative_player_set() {
    for players in [
        RelativePlayerSet::Controller,
        RelativePlayerSet::Opponents,
        RelativePlayerSet::All,
    ] {
        let mut engine = engine(&[0, 1]);
        let lantern = inject_permanent_on_battlefield(&mut engine, 0, "soul-guide_lantern");
        let own = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
        let opposing = inject_graveyard_card(&mut engine, 1, "bonecrusher_giant_stomp");
        let land = inject_graveyard_card(&mut engine, 1, "forest");
        // A stale Adventure face must not turn this printed creature card into an instant.
        engine
            .state
            .objects
            .get_mut(&opposing)
            .unwrap()
            .face_up_index = 1;
        engine
            .apply_command(0, &activate_ability(lantern, 0, vec![]))
            .unwrap();
        // Probe the reusable primitive's parameters on an otherwise real activation;
        // the authored Lantern keeps its unfiltered Opponents definition.
        engine
            .state
            .stack
            .last_mut()
            .unwrap()
            .activated_ability
            .as_mut()
            .unwrap()
            .effect = vec![SpellEffectKind::ExileGraveyards {
            players,
            filter: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..Default::default()
            }),
        }];
        pass_round(&mut engine);
        assert_eq!(
            engine.state.objects[&own].zone,
            if players == RelativePlayerSet::Opponents {
                Zone::Graveyard
            } else {
                Zone::Exile
            }
        );
        assert_eq!(
            engine.state.objects[&opposing].zone,
            if players == RelativePlayerSet::Controller {
                Zone::Graveyard
            } else {
                Zone::Exile
            }
        );
        assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&lantern].zone, Zone::Graveyard);
    }
}

#[test]
fn all_opponents_depart_in_one_trigger_batch() {
    for (cardinality, expected) in [
        (ZoneEventCardinality::OneOrMore, 1),
        (ZoneEventCardinality::EachObject, 3),
    ] {
        let mut engine = engine(&[7, 19, 31]);
        let observer = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let mut ability = CardRegistry::global()
            .get("ajanis_pridemate")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        ability.trigger = TriggerCondition::WheneverCardsLeaveGraveyard {
            owner: CastTriggerPlayer::Opponent,
            filter: Default::default(),
            cardinality,
        };
        engine.state.add_triggered_ability_grant(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(observer),
            kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
        let lantern = inject_permanent_on_battlefield(&mut engine, 0, "soul-guide_lantern");
        for owner in [1, 1, 2] {
            inject_graveyard_card(&mut engine, owner, "grizzly_bears");
        }
        engine
            .apply_command(7, &activate_ability(lantern, 0, vec![]))
            .unwrap();
        pass_round(&mut engine);
        answer_trigger_order_in_engine_order(&mut engine);
        assert_eq!(engine.state.stack.len(), expected);
        while !engine.state.stack.is_empty() {
            pass_round(&mut engine);
        }
        assert_eq!(
            engine.state.objects[&observer].counter_count(CounterKind::PlusOnePlusOne),
            expected as u32
        );
    }
}

#[test]
fn logged_dev_setup_and_lantern_activation_replay_identically() {
    let mut original = engine(&[0, 1]);
    original.enable_dev_commands();
    let put = RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 0,
            dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                card_name: "Soul-Guide Lantern".into(),
                zone: DevZone::Battlefield as i32,
                ready: true,
            })),
        })),
    };
    let mut commands = vec![(0, put), (0, dev_move(1, "Forest", DevZone::Graveyard))];
    let mut batches = Vec::new();
    for (actor, command) in &commands {
        batches.push(original.apply_command(*actor, command).unwrap());
    }
    let lantern = battlefield_object_for_card(&original, 0, "soul-guide_lantern");
    let activate = activate_ability_for(&original, lantern, 0, vec![]);
    batches.push(original.apply_command(0, &activate).unwrap());
    commands.push((0, activate));
    for _ in 0..2 {
        let actor = original.state.priority_player_id();
        let command = pass();
        batches.push(original.apply_command(actor, &command).unwrap());
        commands.push((actor, command));
    }
    assert_eq!(original.state.players[1].exile.len(), 1);
    let mut replay = engine(&[0, 1]);
    replay.enable_dev_commands();
    for ((actor, command), expected) in commands.iter().zip(batches) {
        assert_eq!(replay.apply_command(*actor, command).unwrap(), expected);
    }
    assert_eq!(
        original.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn empty_graveyards_and_stolen_lantern_follow_ability_controller_and_card_owner() {
    for stolen in [false, true] {
        let mut engine = engine(&[0, 1]);
        let lantern = inject_permanent_on_battlefield(&mut engine, 0, "soul-guide_lantern");
        if stolen {
            engine.state.objects.get_mut(&lantern).unwrap().owner = 1;
        }
        engine
            .apply_command(0, &activate_ability(lantern, 0, vec![]))
            .unwrap();
        assert_eq!(engine.state.objects[&lantern].zone, Zone::Graveyard);
        pass_round(&mut engine);
        assert_eq!(
            engine.state.objects[&lantern].zone,
            if stolen { Zone::Exile } else { Zone::Graveyard }
        );
    }
}

#[test]
fn draw_requires_mana_and_tap_and_does_not_exile_graveyards() {
    let mut engine = engine(&[0, 1]);
    let lantern = inject_permanent_on_battlefield(&mut engine, 0, "soul-guide_lantern");
    let opposing = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    let hand = engine.state.players[0].hand.len();
    let drawn = *engine.state.players[0].library.front().unwrap();
    assert!(engine
        .apply_command(0, &activate_ability(lantern, 1, vec![]))
        .is_err());
    assert_eq!(engine.state.objects[&lantern].zone, Zone::Battlefield);
    grant_pool(&mut engine, 0);
    engine.state.objects.get_mut(&lantern).unwrap().tapped = true;
    assert!(engine
        .apply_command(0, &activate_ability(lantern, 1, vec![]))
        .is_err());
    engine.state.objects.get_mut(&lantern).unwrap().tapped = false;
    engine
        .apply_command(0, &activate_ability(lantern, 1, vec![]))
        .unwrap();
    assert_eq!(engine.state.players[0].hand.len(), hand);
    let batch = pass_round(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand + 1);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&lantern].zone, Zone::Graveyard);
    let view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("draw synchronizes the concealed hand through the server-only zone fields");
    assert!(view
        .per_player
        .iter()
        .find(|player| player.player_id == 0)
        .unwrap()
        .hand_cards
        .iter()
        .any(|card| card.object_id == drawn));
    assert!(view
        .per_player
        .iter()
        .filter(|player| player.player_id != 0)
        .all(|player| player.hand_cards.iter().all(|card| card.object_id != drawn)));
    assert!(
        batch.events.iter().all(|event| match &event.ev {
            Some(Ev::PermanentMoved(moved)) => moved.object_id != drawn,
            Some(Ev::Log(log)) => !log.text.contains("Forest"),
            _ => true,
        }),
        "draw does not publish the concealed card in movement events or log text"
    );
}

#[test]
fn etb_exiles_a_card_from_either_graveyard_and_rejects_wrong_zone_targets() {
    for owner in [0, 1] {
        let mut engine = engine(&[0, 1]);
        let target = inject_graveyard_card(&mut engine, owner, "grizzly_bears");
        let lantern = inject_card_into_hand(&mut engine, 0, "soul-guide_lantern");
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, "soul-guide_lantern");
        engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
        pass_round(&mut engine);
        let choose = |oid| RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                targets: vec![TargetRef {
                    object_id: oid,
                    ..Default::default()
                }],
                ..Default::default()
            })),
        };
        assert!(engine.apply_command(0, &choose(lantern)).is_err());
        engine.apply_command(0, &choose(target)).unwrap();
        pass_round(&mut engine);
        assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
        assert_eq!(engine.state.objects[&lantern].zone, Zone::Battlefield);
    }
}
