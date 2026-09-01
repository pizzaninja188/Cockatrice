//! CR 701.26, 603.2c/e: instructed-player tap attribution and action-local multiplicity.
use crate::helpers::*;
use tricerules_cards::primitives::{
    CastTriggerPlayer, ContinuousEffectKind, EffectDuration, RelativePlayerSet,
    TapTriggerCardinality, TriggerCondition,
};
use tricerules_cards::{CardRegistry, CounterKind};
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, ResolutionChoiceDecision, SubmitResolutionChoice,
};

fn choose_target(oid: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(oid),
        })),
    }
}

fn setup() -> GameEngine {
    let mut engine = GameEngine::new(169020, &[0, 1], 20, None, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_card(engine: &mut GameEngine, card: &str) {
    assert!(
        CardRegistry::global().get(card).is_some(),
        "issue #169 card is authored: {card}"
    );
    inject_card_into_hand(engine, 0, card);
    give_mana(
        engine,
        0,
        ManaGift {
            w: 5,
            u: 5,
            c: 5,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, card);
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(engine);
}

#[test]
fn sanctuary_etb_uses_one_target_and_publishes_a_separate_friendly_counter_target() {
    for already_tapped in [false, true] {
        let mut engine = setup();
        let friend = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let enemy = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        engine.state.objects.get_mut(&enemy).unwrap().tapped = already_tapped;
        cast_card(&mut engine, "solitary_sanctuary");
        assert_eq!(engine.state.pending_triggers.len(), 1);
        assert!(engine.apply_command(0, &choose_target(friend)).is_err());
        engine.apply_command(0, &choose_target(enemy)).unwrap();
        pass_both_players(&mut engine);
        assert!(engine.state.objects[&enemy].tapped);
        assert_eq!(
            engine.state.objects[&enemy].counter_count(CounterKind::Stun),
            1
        );
        if already_tapped {
            assert!(engine.state.pending_triggers.is_empty());
            assert!(engine.state.stack.is_empty());
        } else {
            assert_eq!(engine.state.pending_triggers.len(), 1);
            assert!(engine.apply_command(0, &choose_target(enemy)).is_err());
            engine.apply_command(0, &choose_target(friend)).unwrap();
            pass_both_players(&mut engine);
            assert_eq!(
                engine.state.objects[&friend].counter_count(CounterKind::PlusOnePlusOne),
                1
            );
        }
    }
}

#[test]
fn sharae_etb_stuns_already_tapped_targets_and_only_real_taps_spend_its_limit() {
    for already_tapped in [false, true] {
        let mut engine = setup();
        let enemy = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        engine.state.objects.get_mut(&enemy).unwrap().tapped = already_tapped;
        cast_card(&mut engine, "sharae_of_numbing_depths");
        engine.apply_command(0, &choose_target(enemy)).unwrap();
        let hand = engine.state.players[0].hand.len();
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&enemy].counter_count(CounterKind::Stun),
            1
        );
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand + usize::from(!already_tapped)
        );
        assert_eq!(
            engine.state.trigger_uses_this_turn.len(),
            usize::from(!already_tapped)
        );
    }
}

#[test]
fn icewrought_payment_precedes_reflexive_targeting_and_tap_precedes_pump() {
    let mut engine = setup();
    let enemy = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_card(&mut engine, "icewrought_sentry");
    let sentry = battlefield_object_for_card(&engine, 0, "icewrought_sentry");
    engine
        .state
        .objects
        .get_mut(&sentry)
        .unwrap()
        .summoning_sick = false;
    while engine.state.turn_step != tricerules_core::TurnStep::DeclareAttackers {
        pass_both_players(&mut engine);
    }
    engine
        .apply_command(0, &declare_attackers(vec![sentry]))
        .unwrap();
    assert!(!engine.state.objects[&sentry].tapped, "vigilance");
    assert!(
        engine.state.pending_triggers.is_empty(),
        "no target before paying"
    );
    pass_both_players(&mut engine);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    assert!(
        submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana).is_err()
    );
    assert!(!engine.state.objects[&enemy].tapped);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana).unwrap();
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine.apply_command(0, &choose_target(enemy)).unwrap();
    assert!(
        !engine.state.objects[&enemy].tapped,
        "reflexive trigger may be responded to"
    );
    pass_both_players(&mut engine);
    assert!(engine.state.objects[&enemy].tapped);
    assert_eq!(engine.characteristics(sentry).unwrap().power, Some(2));
    pass_both_players(&mut engine);
    assert_eq!(engine.characteristics(sentry).unwrap().power, Some(4));
    assert_eq!(engine.characteristics(sentry).unwrap().toughness, Some(4));
}

fn grant_tap_counter(engine: &mut GameEngine, source: u32, cardinality: TapTriggerCardinality) {
    let mut ability = CardRegistry::global()
        .get("ajanis_pridemate")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::WheneverPlayerTapsCreature {
        player: CastTriggerPlayer::Controller,
        controllers: RelativePlayerSet::Opponents,
        cardinality,
    };
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

fn frost_breath(engine: &mut GameEngine, targets: &[u32]) {
    ensure_in_hand(engine, 0, "frost_breath");
    give_mana(
        engine,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "frost_breath");
    engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                targets.iter().flat_map(|oid| target_object(*oid)).collect(),
            ),
        )
        .unwrap();
    resolve_entire_stack_two_player(engine);
}

#[test]
fn each_creature_and_one_or_more_are_distinct_without_a_turn_cap() {
    let decks = Some(vec![
        deck_with("island", &["frost_breath"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(169001, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let each = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let grouped = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_tap_counter(&mut engine, each, TapTriggerCardinality::EachObject);
    grant_tap_counter(
        &mut engine,
        grouped,
        TapTriggerCardinality::OneOrMorePerAction,
    );
    let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    frost_breath(&mut engine, &[first, second]);
    assert_eq!(
        engine.state.objects[&each].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        engine.state.objects[&grouped].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn no_op_and_own_creature_taps_do_not_qualify() {
    let decks = Some(vec![
        deck_with("island", &["frost_breath"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(169002, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_tap_counter(&mut engine, source, TapTriggerCardinality::EachObject);
    let enemy = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.objects.get_mut(&enemy).unwrap().tapped = true;
    frost_breath(&mut engine, &[source, enemy]);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn cryptic_mass_tap_counts_creatures_once_and_sharae_once() {
    let mut engine = setup();
    let sentry = inject_creature_on_battlefield(&mut engine, 0, "icewrought_sentry");
    inject_creature_on_battlefield(&mut engine, 0, "sharae_of_numbing_depths");
    let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "cryptic_command");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 4,
            ..Default::default()
        },
    );
    let hand = engine.state.players[0].hand.len();
    let slot = hand_index_for_card(&engine, 0, "cryptic_command");
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(2, vec![]), (3, vec![])]))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&first].tapped && engine.state.objects[&second].tapped);
    assert!(!engine.state.objects[&sentry].tapped);
    assert_eq!(engine.characteristics(sentry).unwrap().power, Some(6));
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand + 1,
        "cast, Cryptic draw, Sharae draw"
    );
}

#[test]
fn resolution_payment_uses_the_deciding_player_not_the_stack_controller() {
    let mut engine = setup();
    let sentry = inject_creature_on_battlefield(&mut engine, 0, "icewrought_sentry");
    let enemy = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "command_bridge");
    let slot = hand_index_for_card(&engine, 0, "command_bridge");
    engine.apply_command(0, &play_land(slot)).unwrap();
    pass_both_players(&mut engine);
    // Tangle Wire-shaped fixture: the ability belongs to P0, but P1 is instructed to tap.
    // Exercise the existing resolution payment path, without introducing a new card/picker.
    engine
        .state
        .pending_resolution
        .as_mut()
        .unwrap()
        .deciding_player = 1;
    engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    engine
        .apply_command(1, &submit_resolution_choice(vec![enemy]))
        .unwrap();
    assert!(engine.state.objects[&enemy].tapped);
    assert!(engine.state.stack.is_empty(), "P0 did not tap the creature");
    assert_eq!(engine.characteristics(sentry).unwrap().power, Some(2));
}

#[test]
fn sentry_can_decline_payment_and_illegal_reflexive_targets_do_not_tap() {
    for decline in [true, false] {
        let mut engine = setup();
        let sentry = inject_creature_on_battlefield(&mut engine, 0, "icewrought_sentry");
        let enemy = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        engine
            .state
            .objects
            .get_mut(&sentry)
            .unwrap()
            .summoning_sick = false;
        while engine.state.turn_step != tricerules_core::TurnStep::DeclareAttackers {
            pass_both_players(&mut engine);
        }
        engine
            .apply_command(0, &declare_attackers(vec![sentry]))
            .unwrap();
        pass_both_players(&mut engine);
        if decline {
            engine
                .apply_command(
                    0,
                    &submit_resolution_decision(ResolutionChoiceDecision::Decline),
                )
                .unwrap();
        } else {
            engine
                .apply_command(
                    0,
                    &RuledCommand {
                        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                            decision: ResolutionChoiceDecision::SelectBranch as i32,
                            selected_branch_index: 0,
                            ..Default::default()
                        })),
                    },
                )
                .unwrap();
            give_mana(
                &mut engine,
                0,
                ManaGift {
                    u: 2,
                    ..Default::default()
                },
            );
            submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
                .unwrap();
            assert!(engine.apply_command(0, &choose_target(sentry)).is_err());
            engine.apply_command(0, &choose_target(enemy)).unwrap();
            // Target ceases to be legal before the separate reflexive ability resolves.
            engine.state.objects.get_mut(&enemy).unwrap().zone = tricerules_core::Zone::Graveyard;
            engine.state.players[1]
                .battlefield
                .retain(|oid| *oid != enemy);
            engine.state.players[1].graveyard.push(enemy);
            resolve_entire_stack_two_player(&mut engine);
        }
        assert!(!engine.state.objects[&enemy].tapped);
        assert_eq!(engine.characteristics(sentry).unwrap().power, Some(2));
    }
}

#[test]
fn accepted_tap_commands_replay_with_identical_batches_and_action_ids() {
    use tricerules_proto::ruled::v1::{
        dev_command::Dev, DevAddMana, DevCommand, DevPutCardInZone, DevZone,
    };
    fn fresh() -> GameEngine {
        let mut engine = GameEngine::new(169099, &[0, 1], 20, None, true).unwrap();
        engine.enable_dev_commands();
        engine
    }
    fn record(
        engine: &mut GameEngine,
        log: &mut Vec<(i32, RuledCommand, RuledEventBatch)>,
        player: i32,
        command: RuledCommand,
    ) {
        let batch = engine.apply_command(player, &command).unwrap();
        log.push((player, command, batch));
    }
    let dev = |target, payload| RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(payload),
        })),
    };
    let put = |target, name: &str, zone: DevZone| {
        dev(
            target,
            Dev::PutCardInZone(DevPutCardInZone {
                card_name: name.into(),
                zone: zone as i32,
                ready: true,
            }),
        )
    };
    let mut engine = fresh();
    let mut log = Vec::new();
    for player in [0, 1, 0, 1] {
        record(&mut engine, &mut log, player, pass());
    }
    record(
        &mut engine,
        &mut log,
        0,
        put(0, "Sharae of Numbing Depths", DevZone::Battlefield),
    );
    record(
        &mut engine,
        &mut log,
        0,
        put(1, "Grizzly Bears", DevZone::Battlefield),
    );
    let first = *engine.state.players[1].battlefield.last().unwrap();
    record(
        &mut engine,
        &mut log,
        0,
        put(1, "Grizzly Bears", DevZone::Battlefield),
    );
    let second = *engine.state.players[1].battlefield.last().unwrap();
    record(
        &mut engine,
        &mut log,
        0,
        put(0, "Frost Breath", DevZone::Hand),
    );
    record(
        &mut engine,
        &mut log,
        0,
        dev(
            0,
            Dev::AddMana(DevAddMana {
                u: 3,
                ..Default::default()
            }),
        ),
    );
    let slot = hand_index_for_card(&engine, 0, "frost_breath");
    let before = engine.initial_response_batch();
    assert!(engine
        .apply_command(
            0,
            &cast_spell(slot, [target_object(first), target_object(first)].concat())
        )
        .is_err());
    assert_eq!(engine.initial_response_batch(), before);
    record(
        &mut engine,
        &mut log,
        0,
        cast_spell(slot, [target_object(first), target_object(second)].concat()),
    );
    for player in [0, 1, 0, 1] {
        record(&mut engine, &mut log, player, pass());
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.next_tap_action_id, 1);
    let mut replay = fresh();
    for (player, command, batch) in log {
        assert_eq!(replay.apply_command(player, &command).unwrap(), batch);
    }
    assert_eq!(
        replay.state.next_tap_action_id,
        engine.state.next_tap_action_id
    );
    assert_eq!(
        replay.state.trigger_uses_this_turn,
        engine.state.trigger_uses_this_turn
    );
    assert_eq!(
        replay.initial_response_batch(),
        engine.initial_response_batch()
    );
}
