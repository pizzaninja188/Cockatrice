//! The Millennium Calendar's actual-card scenario coverage.

use super::helpers::*;
use tricerules_cards::registry;
use tricerules_cards::{ContinuousEffectKind, CounterKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChooseTriggerTarget, DevCommand, DevMoveCard, DevZone,
    RuledCommand, TargetRef, TargetRefKind,
};

const CALENDAR: &str = "the_millennium_calendar";

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(registry::global(), seed, &[0, 1], 20, None, true)
        .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn start_upkeep(engine: &mut GameEngine, player: i32) {
    let previous = if player == 0 { 1 } else { 0 };
    engine.state.active_player_idx = previous as usize;
    engine.state.priority_idx = previous as usize;
    engine.state.turn_step = TurnStep::EndStep;
    engine.state.passes_since_stack_change = 0;
    engine
        .apply_command(previous, &pass())
        .expect("previous player passes their end step");
    engine
        .apply_command(player, &pass())
        .expect("next player advances through cleanup and untaps");
    assert_eq!(engine.state.active_player_id(), player);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
}

fn threshold_trigger_after_activation(seed: u64) -> (GameEngine, u32, u32) {
    let mut engine = engine(seed);
    let calendar = inject_permanent_on_battlefield(&mut engine, 0, CALENDAR);
    engine
        .state
        .objects
        .get_mut(&calendar)
        .unwrap()
        .set_counter(CounterKind::Time, 500);
    engine.state.players[1].life = 2_000;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, calendar, 0, vec![])
        .expect("pay {2} and tap to double Time counters");
    pass_both_players(&mut engine);
    let threshold = engine
        .state
        .stack
        .iter()
        .find(|item| {
            item.source_permanent_id == Some(calendar)
                && item.triggered_ability.as_ref().is_some_and(|ability| {
                    matches!(
                        ability.trigger,
                        tricerules_cards::TriggerCondition::WhenSourceHasAtLeast1000TimeCounters
                    )
                })
        })
        .expect("crossing 1,000 Time counters triggers the state ability")
        .id;
    assert_eq!(
        engine.state.objects[&calendar].counter_count(CounterKind::Time),
        1_000
    );
    (engine, calendar, threshold)
}

fn choose_stack_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }],
        })),
    }
}

fn counter_threshold_with_tidebinder(engine: &mut GameEngine, threshold: u32) {
    engine
        .apply_command(0, &pass())
        .expect("active player passes priority to the opponent");
    inject_card_into_hand(engine, 1, "tishanas_tidebinder");
    give_mana(
        engine,
        1,
        ManaGift {
            c: 2,
            u: 1,
            ..Default::default()
        },
    );
    let tidebinder_slot = hand_index_for_card(engine, 1, "tishanas_tidebinder");
    engine
        .apply_command(1, &cast_spell(tidebinder_slot, Vec::new()))
        .expect("opponent casts Tishana's Tidebinder in response");
    pass_both_players(engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(1, &choose_stack_trigger_target(threshold))
        .expect("Tidebinder targets the Calendar state-trigger ability");
    pass_both_players(engine);
}

fn dev_move_calendar(engine: &mut GameEngine, zone: DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            engine.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "The Millennium Calendar".into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move Calendar through the engine zone-change path");
}

fn pass_priority_round_recorded(
    engine: &mut GameEngine,
    commands: &mut Vec<(
        i32,
        RuledCommand,
        tricerules_proto::ruled::v1::RuledEventBatch,
    )>,
) {
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_trigger_order.is_none());
    let live_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let remaining = live_players
        .checked_sub(engine.state.passes_since_stack_change as usize)
        .expect("consistent pass count");
    for _ in 0..remaining {
        let actor = engine.state.priority_player_id();
        let command = pass();
        let batch = engine
            .apply_command(actor, &command)
            .expect("record priority pass");
        commands.push((actor, command, batch));
    }
}

#[test]
fn millennium_calendar_is_registered_with_its_complete_card_identity() {
    let card = registry::global()
        .get(CALENDAR)
        .expect("The Millennium Calendar must have a complete rules definition");

    assert_eq!(card.id, CALENDAR);
    assert_eq!(card.name, "The Millennium Calendar");
    assert_eq!(card.faces.len(), 1);
    let face = card.primary_face();
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.mana_cost.to_string(), "{1}");
    assert_eq!(face.triggered_abilities.len(), 2);
    assert_eq!(face.activated_abilities.len(), 1);

    let untap_trigger = &face.triggered_abilities[0];
    assert!(matches!(
        &untap_trigger.trigger,
        tricerules_cards::TriggerCondition::WheneverControllerUntapsOneOrMorePermanents
    ));
    assert!(matches!(
        &untap_trigger.effect[..],
        [tricerules_cards::SpellEffectKind::PutCounters {
            counter: tricerules_cards::CounterKind::Time,
            count: tricerules_cards::Amount::EventCount,
            subject: tricerules_cards::primitives::EffectSubject::Source,
        }]
    ));

    let threshold_trigger = &face.triggered_abilities[1];
    assert!(matches!(
        &threshold_trigger.trigger,
        tricerules_cards::TriggerCondition::WhenSourceHasAtLeast1000TimeCounters
    ));
    assert!(matches!(
        &threshold_trigger.effect[..],
        [
            tricerules_cards::SpellEffectKind::Sacrifice {
                subject: tricerules_cards::primitives::EffectSubject::Source,
            },
            tricerules_cards::SpellEffectKind::LoseLife {
                amount: tricerules_cards::primitives::LifeAmount::Fixed(1000),
                who: tricerules_cards::primitives::PlayerRecipient::EachOpponent,
            },
        ]
    ));

    let activation = &face.activated_abilities[0];
    assert_eq!(activation.costs.len(), 2);
    assert!(matches!(
        &activation.effect[..],
        [tricerules_cards::SpellEffectKind::DoubleTimeCounters]
    ));
}

#[test]
fn controller_untap_trigger_counts_only_successful_untaps_in_the_active_cohort() {
    let mut engine = engine(2_026_101_010);
    let calendar = inject_permanent_on_battlefield(&mut engine, 0, CALENDAR);
    let successful = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let stunned = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let skipped = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let prohibited = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    for object in [successful, stunned, skipped, prohibited] {
        engine.state.objects.get_mut(&object).unwrap().tapped = true;
    }
    engine
        .state
        .objects
        .get_mut(&stunned)
        .unwrap()
        .set_counter(CounterKind::Stun, 1);
    let skipped_generation = engine
        .state
        .zone_change_generation
        .get(&skipped)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .skip_next_untap
        .insert((skipped, skipped_generation));
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(prohibited),
        kind: ContinuousEffectKind::ProhibitUntap,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });

    start_upkeep(&mut engine, 0);

    assert!(!engine.state.objects[&successful].tapped);
    assert!(engine.state.objects[&stunned].tapped);
    assert_eq!(
        engine.state.objects[&stunned].counter_count(CounterKind::Stun),
        0
    );
    assert!(engine.state.objects[&skipped].tapped);
    assert!(engine.state.objects[&prohibited].tapped);
    let trigger = engine
        .state
        .stack
        .iter()
        .find(|item| item.source_permanent_id == Some(calendar))
        .expect("one-or-more successful active-player untaps trigger the Calendar");
    assert_eq!(trigger.trigger_context.event_count, Some(1));

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&calendar].counter_count(CounterKind::Time),
        1,
        "the trigger puts the exact successful untap count on the source"
    );
}

#[test]
fn opponent_untaps_during_this_players_untap_do_not_trigger_the_calendar() {
    let mut engine = engine(2_026_101_011);
    let calendar = inject_permanent_on_battlefield(&mut engine, 0, CALENDAR);
    let clock = inject_permanent_on_battlefield(&mut engine, 1, "unwinding_clock");
    let opponent_artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    engine
        .state
        .objects
        .get_mut(&opponent_artifact)
        .unwrap()
        .tapped = true;

    start_upkeep(&mut engine, 0);

    assert!(!engine.state.objects[&opponent_artifact].tapped);
    assert!(engine.state.objects[&clock].zone == Zone::Battlefield);
    assert!(
        !engine
            .state
            .stack
            .iter()
            .any(|item| item.source_permanent_id == Some(calendar)),
        "the opponent's own artifacts untapping do not count for the active player's Calendar"
    );
    assert_eq!(
        engine.state.objects[&calendar].counter_count(CounterKind::Time),
        0
    );
}

#[test]
fn controller_untap_trigger_does_not_fire_when_no_permanent_untaps() {
    let mut engine = engine(2_026_101_018);
    let calendar = inject_permanent_on_battlefield(&mut engine, 0, CALENDAR);
    let stunned = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    engine.state.objects.get_mut(&stunned).unwrap().tapped = true;
    engine
        .state
        .objects
        .get_mut(&stunned)
        .unwrap()
        .set_counter(CounterKind::Stun, 1);

    start_upkeep(&mut engine, 0);

    assert!(engine.state.objects[&stunned].tapped);
    assert_eq!(
        engine.state.objects[&stunned].counter_count(CounterKind::Stun),
        0
    );
    assert!(engine
        .state
        .stack
        .iter()
        .all(|item| item.source_permanent_id != Some(calendar)));
    assert_eq!(
        engine.state.objects[&calendar].counter_count(CounterKind::Time),
        0
    );
}

#[test]
fn untap_and_opponent_upkeep_triggers_are_stacked_together_in_apnap_order() {
    let mut engine = engine(2_026_101_019);
    let calendar = inject_permanent_on_battlefield(&mut engine, 0, CALENDAR);
    let active_permanent = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    engine
        .state
        .objects
        .get_mut(&active_permanent)
        .unwrap()
        .tapped = true;
    let vortex = inject_permanent_on_battlefield(&mut engine, 1, "sulfuric_vortex");

    start_upkeep(&mut engine, 0);

    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.priority_player_id(), 0);
    assert_eq!(engine.state.stack.len(), 2);
    let calendar_trigger = engine
        .state
        .stack
        .iter()
        .find(|item| item.source_permanent_id == Some(calendar))
        .expect("active player's untap trigger is on the stack");
    assert_eq!(calendar_trigger.trigger_context.event_count, Some(1));
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(vortex),
        "the nonactive player's upkeep trigger is above the active player's trigger"
    );
    assert_eq!(
        engine.state.objects[&calendar].counter_count(CounterKind::Time),
        0
    );
}

#[test]
fn activation_doubles_only_time_counters_and_zero_stays_zero() {
    for (time_counters, other_counters, expected_time) in [(3, 5, 6), (0, 5, 0)] {
        let mut engine = engine(2_026_101_012 + time_counters as u64);
        let calendar = inject_permanent_on_battlefield(&mut engine, 0, CALENDAR);
        engine
            .state
            .objects
            .get_mut(&calendar)
            .unwrap()
            .set_counter(CounterKind::Time, time_counters);
        engine
            .state
            .objects
            .get_mut(&calendar)
            .unwrap()
            .set_counter(CounterKind::Quest, other_counters);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );

        apply_ability(&mut engine, 0, calendar, 0, vec![])
            .expect("pay {2} and tap to double Time counters");
        assert!(engine.state.objects[&calendar].tapped);
        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(
            engine.state.objects[&calendar].counter_count(CounterKind::Time),
            expected_time
        );
        assert_eq!(
            engine.state.objects[&calendar].counter_count(CounterKind::Quest),
            other_counters,
            "other counter kinds are unchanged"
        );
    }
}

#[test]
fn state_trigger_resolves_after_time_counters_fall_below_1000() {
    let (mut engine, calendar, _) = threshold_trigger_after_activation(2_026_101_013);
    engine
        .state
        .objects
        .get_mut(&calendar)
        .unwrap()
        .set_counter(CounterKind::Time, 0);

    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&calendar].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, 1_000);
}

#[test]
fn calendar_control_change_keeps_the_original_trigger_controller_and_retriggers_for_the_new_controller(
) {
    let (mut engine, calendar, original) = threshold_trigger_after_activation(2_026_101_023);
    engine.state.players[0].life = 2_000;
    inject_creature_on_battlefield(&mut engine, 1, "pyreswipe_hawk");

    // The original state trigger belongs to P0. P1's Hawk later gains control of the artifact
    // while that ability is waiting on the stack, which must not change its trigger controller.
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .find(|item| item.id == original)
            .expect("original Calendar threshold ability is on the stack")
            .controller,
        0
    );

    // P1 controls the Hawk, so P1 spends the six mana. This instant can be cast above the waiting
    // state trigger and crosses Pyreswipe Hawk's Expend threshold.
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 3,
            c: 3,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut engine, 1, "blue_suns_zenith");
    engine
        .apply_command(0, &pass())
        .expect("P0 passes priority with the Calendar state trigger waiting");
    let cast = cast_spell_x(
        hand_index_for_card(&engine, 1, "blue_suns_zenith"),
        target_player(0),
        3,
    );
    engine
        .apply_command(1, &cast)
        .expect("cast Blue Sun's Zenith at instant speed to trigger Pyreswipe Hawk");
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(1)
            .mana_spent_casting_spells,
        6
    );
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "Pyreswipe Hawk's Expend trigger waits for its optional target choice"
    );

    engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: vec![TargetRef {
                        kind: TargetRefKind::Permanent as i32,
                        object_id: calendar,
                        group_index: 0,
                        ..Default::default()
                    }],
                    ..Default::default()
                })),
            },
        )
        .expect("P1 chooses Calendar for Pyreswipe Hawk's control trigger");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&calendar].controller, 1);

    // Resolve Blue Sun's Zenith, then the original state trigger. P0 cannot sacrifice a permanent
    // P1 controls (CR 701.21a), but its already-triggered life-loss instruction still resolves.
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[1].life, 1_000);
    assert_eq!(engine.state.objects[&calendar].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&calendar].controller, 1);
    assert_eq!(
        engine.state.objects[&calendar].counter_count(CounterKind::Time),
        1_000
    );

    let replacement = engine
        .state
        .stack
        .iter()
        .find(|item| {
            item.id != original
                && item.source_permanent_id == Some(calendar)
                && item.triggered_ability.as_ref().is_some_and(|ability| {
                    matches!(
                        ability.trigger,
                        tricerules_cards::TriggerCondition::WhenSourceHasAtLeast1000TimeCounters
                    )
                })
        })
        .expect(
            "the current Calendar controller gets a new state trigger after the old one leaves",
        );
    assert_eq!(replacement.controller, 1);

    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&calendar].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, 1_000);
}

#[test]
fn countering_the_state_trigger_retriggers_only_while_threshold_remains_met() {
    for (remaining_time, should_retrigger) in [(1_000, true), (999, false)] {
        let seed = if should_retrigger {
            2_026_101_014
        } else {
            2_026_101_015
        };
        let (mut engine, calendar, original) = threshold_trigger_after_activation(seed);
        engine
            .state
            .objects
            .get_mut(&calendar)
            .unwrap()
            .set_counter(CounterKind::Time, remaining_time);

        counter_threshold_with_tidebinder(&mut engine, original);

        assert!(engine.state.stack.iter().all(|item| item.id != original));
        let replacement = engine.state.stack.iter().find(|item| {
            item.source_permanent_id == Some(calendar)
                && item.triggered_ability.as_ref().is_some_and(|ability| {
                    matches!(
                        ability.trigger,
                        tricerules_cards::TriggerCondition::WhenSourceHasAtLeast1000TimeCounters
                    )
                })
        });
        assert_eq!(replacement.is_some(), should_retrigger);
        if should_retrigger {
            assert_ne!(replacement.unwrap().id, original);
            pass_both_players(&mut engine);
            assert_eq!(engine.state.objects[&calendar].zone, Zone::Graveyard);
            assert_eq!(engine.state.players[1].life, 1_000);
        } else {
            assert_eq!(engine.state.objects[&calendar].zone, Zone::Battlefield);
            assert_eq!(engine.state.players[1].life, 2_000);
        }
    }
}

#[test]
fn copied_threshold_stack_item_does_not_suppress_a_new_original_after_countering() {
    let (mut engine, calendar, original) = threshold_trigger_after_activation(2_026_101_021);
    engine.state.players[1].life = 5_000;

    // The engine does not currently author an ability-copy card. Materialize the copy-shaped
    // stack item in the fixture so this regression isolates CR 603.8 suppression ownership.
    let mut copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == original)
        .expect("original Calendar state trigger is on the stack")
        .clone();
    copy.id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    copy.is_copy = true;
    let copy_id = copy.id;
    engine
        .state
        .stack_presentations
        .insert(copy_id, engine.state.stack_presentations[&original].clone());
    engine.state.stack.push(copy);

    counter_threshold_with_tidebinder(&mut engine, original);

    assert!(engine.state.stack.iter().any(|item| item.id == copy_id));
    let replacement = engine.state.stack.iter().find(|item| {
        item.id != copy_id
            && item.source_permanent_id == Some(calendar)
            && item.triggered_ability.as_ref().is_some_and(|ability| {
                matches!(
                    ability.trigger,
                    tricerules_cards::TriggerCondition::WhenSourceHasAtLeast1000TimeCounters
                )
            })
    });
    assert!(
        replacement.is_some(),
        "the remaining copied item alone does not suppress a fresh original trigger"
    );

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&calendar].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, 3_000);
}

#[test]
fn threshold_crossing_is_retained_when_later_instruction_removes_the_counters() {
    let mut engine = engine(2_026_101_022);
    let calendar = inject_permanent_on_battlefield(&mut engine, 0, CALENDAR);
    engine
        .state
        .objects
        .get_mut(&calendar)
        .unwrap()
        .set_counter(CounterKind::Time, 500);
    engine.state.players[1].life = 2_000;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, calendar, 0, vec![]).expect("activate Calendar");

    // The first instruction crosses the threshold; the second makes it false before the
    // activation finishes. This test-only suffix proves state triggers are checked per instruction.
    engine
        .state
        .stack
        .last_mut()
        .unwrap()
        .activated_ability
        .as_mut()
        .unwrap()
        .effect
        .push(tricerules_cards::SpellEffectKind::RemoveCounters {
            counter: CounterKind::Time,
            count: 1_000,
            subject: tricerules_cards::primitives::EffectSubject::Source,
        });

    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&calendar].counter_count(CounterKind::Time),
        0
    );
    let threshold_items = engine
        .state
        .stack
        .iter()
        .filter(|item| {
            item.source_permanent_id == Some(calendar)
                && item.triggered_ability.as_ref().is_some_and(|ability| {
                    matches!(
                        ability.trigger,
                        tricerules_cards::TriggerCondition::WhenSourceHasAtLeast1000TimeCounters
                    )
                })
        })
        .count();
    assert_eq!(
        threshold_items, 1,
        "one trigger from the intermediate threshold crossing remains on the stack"
    );

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&calendar].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, 1_000);
}

#[test]
fn old_threshold_trigger_cannot_sacrifice_a_returned_calendar() {
    let (mut engine, calendar, old_threshold) = threshold_trigger_after_activation(2_026_101_016);
    let first_generation = engine
        .state
        .zone_change_generation
        .get(&calendar)
        .copied()
        .unwrap_or(0);
    dev_move_calendar(&mut engine, DevZone::Graveyard);
    dev_move_calendar(&mut engine, DevZone::Battlefield);
    assert!(engine.state.zone_change_generation[&calendar] > first_generation);
    assert_eq!(
        engine.state.objects[&calendar].counter_count(CounterKind::Time),
        0,
        "Time counters do not persist through the zone change"
    );

    pass_both_players(&mut engine);

    assert!(engine
        .state
        .stack
        .iter()
        .all(|item| item.id != old_threshold));
    assert!(!engine.state.stack.iter().any(|item| {
        item.source_permanent_id == Some(calendar)
            && item.triggered_ability.as_ref().is_some_and(|ability| {
                matches!(
                    ability.trigger,
                    tricerules_cards::TriggerCondition::WhenSourceHasAtLeast1000TimeCounters
                )
            })
    }));
    assert_eq!(engine.state.objects[&calendar].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[1].life, 1_000);
}

#[test]
fn threshold_life_loss_affects_every_opponent_in_a_three_player_game() {
    let mut engine = GameEngine::new(
        registry::global(),
        2_026_101_017,
        &[0, 1, 2],
        20,
        None,
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    let calendar = inject_permanent_on_battlefield(&mut engine, 0, CALENDAR);
    engine
        .state
        .objects
        .get_mut(&calendar)
        .unwrap()
        .set_counter(CounterKind::Time, 500);
    engine.state.players[1].life = 2_000;
    engine.state.players[2].life = 2_000;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, calendar, 0, vec![]).expect("activate Calendar");
    while !engine.state.stack.is_empty() {
        pass_priority_round(&mut engine);
    }

    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 1_000);
    assert_eq!(engine.state.players[2].life, 1_000);
    assert_eq!(engine.state.objects[&calendar].zone, Zone::Graveyard);
}

#[test]
fn untap_count_activation_threshold_and_life_loss_replay_identically() {
    use prost::Message;

    fn fresh() -> (GameEngine, u32, [u32; 2]) {
        let mut engine = engine(2_026_101_020);
        let calendar = inject_permanent_on_battlefield(&mut engine, 0, CALENDAR);
        engine
            .state
            .objects
            .get_mut(&calendar)
            .unwrap()
            .set_counter(CounterKind::Time, 500);
        let forests = [
            inject_permanent_on_battlefield(&mut engine, 0, "forest"),
            inject_permanent_on_battlefield(&mut engine, 0, "forest"),
        ];
        for forest in forests {
            engine.state.objects.get_mut(&forest).unwrap().tapped = true;
        }
        engine.state.players[1].life = 2_000;
        engine.state.active_player_idx = 1;
        engine.state.priority_idx = 1;
        engine.state.turn_step = TurnStep::EndStep;
        engine.state.passes_since_stack_change = 0;
        (engine, calendar, forests)
    }

    let (mut engine, calendar, forests) = fresh();
    let mut commands = Vec::new();
    for _ in 0..4 {
        pass_priority_round_recorded(&mut engine, &mut commands);
    }
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(
        engine.state.objects[&calendar].counter_count(CounterKind::Time),
        502
    );

    for forest in forests {
        let mana_ability = activate_ability_for(&engine, forest, 0, Vec::new());
        let batch = engine
            .apply_command(0, &mana_ability)
            .expect("tap an untapped Forest for green mana");
        commands.push((0, mana_ability, batch));
    }

    let activation = activate_ability_for(&engine, calendar, 0, Vec::new());
    let batch = engine
        .apply_command(0, &activation)
        .expect("record Calendar activation");
    commands.push((0, activation, batch));
    while !engine.state.stack.is_empty() {
        pass_priority_round_recorded(&mut engine, &mut commands);
    }

    assert_eq!(engine.state.objects[&calendar].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, 1_000);

    let (mut replay, _, _) = fresh();
    for (actor, command, expected_batch) in commands {
        let encoded = command.encode_to_vec();
        let decoded = RuledCommand::decode(encoded.as_slice()).expect("decode command log");
        assert_eq!(
            replay
                .apply_command(actor, &decoded)
                .expect("replay command"),
            expected_batch
        );
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}
