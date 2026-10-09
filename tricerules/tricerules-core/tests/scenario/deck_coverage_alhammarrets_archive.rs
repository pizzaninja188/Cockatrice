use super::helpers::*;
use prost::Message;
use tricerules_cards::{ContinuousEffectKind, ControllerReference, CounterKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, EngineError, TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

fn setup(seed: u64) -> GameEngine {
    let deck = deck_with(
        "island",
        &["alhammarrets_archive", "chaplains_blessing", "divination"],
    );
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[4, 9, 27],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn paid(engine: &mut GameEngine, card: &str) -> u32 {
    let oid = inject_card_into_hand(engine, 0, card);
    let gift = if card == "alhammarrets_archive" {
        ManaGift {
            c: 5,
            ..Default::default()
        }
    } else {
        ManaGift {
            c: 5,
            w: 1,
            u: 3,
            b: 1,
            ..Default::default()
        }
    };
    give_mana(engine, 4, gift);
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == oid)
        .unwrap();
    engine.apply_command(4, &cast_spell(slot, vec![])).unwrap();
    oid
}

fn resolve(engine: &mut GameEngine, oid: u32) -> Vec<rv1::RuledEvent> {
    let mut events = Vec::new();
    for _ in 0..12 {
        if !engine.state.stack.iter().any(|item| item.id == oid) {
            return events;
        }
        assert!(engine.state.blocking_choice().is_none());
        let actor = engine.state.priority_player_id();
        events.extend(engine.apply_command(actor, &pass()).unwrap().events);
    }
    panic!("bounded resolution did not finish");
}

#[test]
fn archive_paid_entry_gain_history_single_trigger_and_divination_draws() {
    let mut engine = setup(104_810);
    let archive = paid(&mut engine, "alhammarrets_archive");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    resolve(&mut engine, archive);
    assert_eq!(engine.state.objects[&archive].zone, Zone::Battlefield);
    let pridemate = inject_creature_on_battlefield(&mut engine, 0, "ajanis_pridemate");
    let blessing = paid(&mut engine, "chaplains_blessing");
    let events = resolve(&mut engine, blessing);
    assert_eq!(engine.state.players[0].life, 30);
    assert_eq!(engine.state.turn_history.current.player(4).life_gained, 10);
    let gains = events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::LifeChanged(change)) if change.player_id == 4 => Some(change.delta),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(gains, vec![10]);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one gain event creates one Pridemate trigger"
    );
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&pridemate].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    let divination = paid(&mut engine, "divination");
    let hand = engine.state.players[0].hand.len();
    let library = engine.state.players[0].library.len();
    resolve(&mut engine, divination);
    assert_eq!(engine.state.players[0].hand.len(), hand + 4);
    assert_eq!(engine.state.players[0].library.len(), library - 4);
}

#[test]
fn archive_three_player_blood_tithe_doubles_one_aggregate_gain() {
    let mut engine = setup(104_811);
    let archive = paid(&mut engine, "alhammarrets_archive");
    resolve(&mut engine, archive);
    let tithe = paid(&mut engine, "blood_tithe");
    let events = resolve(&mut engine, tithe);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        vec![32, 17, 17]
    );
    assert_eq!(engine.state.turn_history.current.player(4).life_gained, 12);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.ev.as_ref(),
        Some(Ev::LifeChanged(change)) if change.player_id == 4 && change.delta == 12))
            .count(),
        1
    );
}

#[test]
fn archive_first_own_draw_exception_later_draws_and_insight_ordering() {
    for prefer in ["Alhammarret's Archive", "Teferi's Ageless Insight"] {
        let mut engine = setup(104_812);
        for _ in 0..20 {
            inject_library_card(&mut engine, 0, "island");
        }
        let archive = paid(&mut engine, "alhammarrets_archive");
        resolve(&mut engine, archive);
        let insight = paid(&mut engine, "teferis_ageless_insight");
        resolve(&mut engine, insight);
        engine.state.turn_step = TurnStep::Upkeep;
        engine.state.priority_idx = 0;
        let hand = engine.state.players[0].hand.len();
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.turn_step, TurnStep::Draw);
        assert_eq!(engine.state.players[0].hand.len(), hand + 1);
        let brainstorm = paid(&mut engine, "brainstorm");
        let hand = engine.state.players[0].hand.len();
        let mut choice = None;
        for _ in 0..12 {
            let actor = engine.state.priority_player_id();
            let batch = engine.apply_command(actor, &pass()).unwrap();
            if let Some(prompt) = find_resolution_choice(&batch) {
                choice = Some(prompt.clone());
                break;
            }
        }
        let mut choice = choice.expect("two draw replacements require ordering");
        assert_eq!(choice.choice_kind(), rv1::ChoiceKind::ReplacementEffect);
        let before = engine.diagnostic_snapshot().unwrap();
        let id = choice.replacement_options[0].application_id;
        assert!(engine
            .apply_command(9, &submit_resolution_choice(vec![id]))
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        for _ in 0..40 {
            if choice.choice_kind() != rv1::ChoiceKind::ReplacementEffect {
                break;
            }
            let id = choice
                .replacement_options
                .iter()
                .find(|option| option.source_card_name == prefer)
                .unwrap_or(&choice.replacement_options[0])
                .application_id;
            let batch = engine
                .apply_command(4, &submit_resolution_choice(vec![id]))
                .unwrap();
            choice = find_resolution_choice(&batch)
                .expect("Brainstorm put-back follows replacement draws")
                .clone();
        }
        assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
        assert_eq!(engine.state.players[0].hand.len(), hand + 12);
        engine
            .apply_command(
                4,
                &submit_resolution_choice(choice.candidate_object_ids[..2].to_vec()),
            )
            .unwrap();
        assert_eq!(engine.state.objects[&brainstorm].zone, Zone::Graveyard);
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn archive_both_clauses_replay_serialized_accepted_commands_from_identical_fixture() {
    fn fixture() -> GameEngine {
        let mut engine = setup(104_813);
        for card in ["alhammarrets_archive", "chaplains_blessing", "divination"] {
            inject_card_into_hand(&mut engine, 0, card);
        }
        give_mana(
            &mut engine,
            4,
            ManaGift {
                c: 10,
                w: 2,
                u: 3,
                ..Default::default()
            },
        );
        engine
    }
    let mut engine = fixture();
    let mut log = Vec::new();
    for card in ["alhammarrets_archive", "chaplains_blessing", "divination"] {
        let command = cast_spell(hand_index_for_card(&engine, 0, card), vec![]);
        engine.apply_command(4, &command).unwrap();
        log.push((4, command.encode_to_vec()));
        for _ in 0..12 {
            if engine.state.stack.is_empty() {
                break;
            }
            let actor = engine.state.priority_player_id();
            let command = pass();
            engine.apply_command(actor, &command).unwrap();
            log.push((actor, command.encode_to_vec()));
        }
        assert!(engine.state.stack.is_empty());
    }
    assert_eq!(engine.state.players[0].life, 30);
    assert_eq!(engine.state.turn_history.current.player(4).life_gained, 10);
    let mut replay = fixture();
    for (actor, bytes) in log {
        let command = rv1::RuledCommand::decode(bytes.as_slice()).unwrap();
        replay.apply_command(actor, &command).unwrap();
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn archive_swords_gain_follows_derived_target_controller_and_overflow_restores_exile() {
    for overflow in [false, true] {
        let mut engine = setup(104_814);
        let archive = paid(&mut engine, "alhammarrets_archive");
        resolve(&mut engine, archive);
        let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        for oid in [archive, bear] {
            engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::Fixed(9),
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index,
            });
        }
        if overflow {
            engine.state.players[1].life = i32::MAX - 3;
        }
        inject_card_into_hand(&mut engine, 0, "swords_to_plowshares");
        give_mana(
            &mut engine,
            4,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "swords_to_plowshares");
        engine
            .apply_command(
                4,
                &cast_spell(
                    slot,
                    vec![TargetRef {
                        object_id: bear,
                        kind: TargetRefKind::Permanent as i32,
                        ..Default::default()
                    }],
                ),
            )
            .unwrap();
        for _ in 0..2 {
            let actor = engine.state.priority_player_id();
            engine.apply_command(actor, &pass()).unwrap();
        }
        let before = engine.diagnostic_snapshot().unwrap();
        let actor = engine.state.priority_player_id();
        let result = engine.apply_command(actor, &pass());
        if overflow {
            assert!(matches!(result, Err(EngineError::LifeNumericRange(_))));
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
            assert_eq!(engine.state.objects[&bear].zone, Zone::Battlefield);
        } else {
            result.unwrap();
            assert_eq!(engine.state.players[1].life, 24);
            assert_eq!(engine.state.players[0].life, 20);
            assert_eq!(engine.state.objects[&bear].zone, Zone::Exile);
            assert_eq!(engine.state.turn_history.current.player(9).life_gained, 4);
        }
    }
}

#[test]
fn archive_actual_lifelink_after_prevention_and_overflow_restore_damage_and_shield() {
    for overflow in [false, true] {
        let mut engine = setup(104_815);
        let archive = paid(&mut engine, "alhammarrets_archive");
        resolve(&mut engine, archive);
        let source = inject_creature_on_battlefield(&mut engine, 0, "vampire_nighthawk");
        let recipient = inject_creature_on_battlefield(&mut engine, 1, "colossal_dreadmaw");
        engine.state.add_damage_prevention_shield(recipient, 1);
        if overflow {
            engine.state.players[0].life = i32::MAX - 1;
        }
        inject_card_into_hand(&mut engine, 0, "rabid_bite");
        give_mana(
            &mut engine,
            4,
            ManaGift {
                g: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "rabid_bite");
        engine
            .apply_command(
                4,
                &cast_spell(
                    slot,
                    vec![
                        TargetRef {
                            object_id: source,
                            group_index: 0,
                            kind: TargetRefKind::Permanent as i32,
                            ..Default::default()
                        },
                        TargetRef {
                            object_id: recipient,
                            group_index: 1,
                            kind: TargetRefKind::Permanent as i32,
                            ..Default::default()
                        },
                    ],
                ),
            )
            .unwrap();
        for _ in 0..2 {
            let actor = engine.state.priority_player_id();
            engine.apply_command(actor, &pass()).unwrap();
        }
        let before = engine.diagnostic_snapshot().unwrap();
        let actor = engine.state.priority_player_id();
        let result = engine.apply_command(actor, &pass());
        if overflow {
            assert!(matches!(result, Err(EngineError::LifeNumericRange(_))));
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
            assert_eq!(engine.state.objects[&recipient].zone, Zone::Battlefield);
        } else {
            result.unwrap();
            assert_eq!(
                engine.state.players[0].life, 22,
                "one actual damage becomes two life"
            );
            assert_eq!(
                engine.state.objects[&recipient].zone,
                Zone::Graveyard,
                "deathtouch still applies"
            );
            assert_eq!(engine.state.turn_history.current.player(4).life_gained, 2);
        }
    }
}

#[test]
fn archive_combat_sources_gain_separately_and_canonical_settlement_rolls_back_overflow() {
    for overflow in [false, true] {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            104_816,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        advance_to_declare_attackers(&mut engine);
        inject_permanent_on_battlefield(&mut engine, 0, "alhammarrets_archive");
        let first = inject_creature_on_battlefield(&mut engine, 0, "vampire_nighthawk");
        let second = inject_creature_on_battlefield(&mut engine, 0, "vampire_nighthawk");
        engine
            .apply_command(0, &declare_attackers(vec![first, second]))
            .unwrap();
        engine.apply_command(0, &pass()).unwrap();
        engine.apply_command(1, &pass()).unwrap();
        assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
        if overflow {
            engine.state.players[0].life = i32::MAX - 3;
        }
        let before = engine.diagnostic_snapshot().unwrap();
        // Inner dispatch only records the first pass. Automatic settlement supplies the second
        // pass, deals damage and must restore the entire outer command on overflow.
        let command = RuledCommand {
            cmd: Some(Cmd::CanonicalGameplay(rv1::CanonicalGameplayCommand {
                command: pass().encode_to_vec(),
                auto_pass_policies: [0, 1]
                    .into_iter()
                    .map(|player_id| rv1::AutoPassPolicy {
                        player_id,
                        stop_on_own_turn: vec![rv1::PhaseId::CombatDamage as i32],
                        stop_on_opponent_turn: vec![rv1::PhaseId::CombatDamage as i32],
                    })
                    .collect(),
            })),
        };
        let result = engine.apply_command(0, &command);
        if overflow {
            assert!(matches!(result, Err(EngineError::LifeNumericRange(_))));
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        } else {
            let gains = life_changes_in(&result.unwrap())
                .into_iter()
                .filter(|change| change.player_id == 0)
                .map(|change| change.delta)
                .collect::<Vec<_>>();
            assert_eq!(gains, vec![4, 4]);
            assert_eq!(engine.state.players[0].life, 28);
            assert_eq!(engine.state.players[1].life, 16);
            assert_eq!(engine.state.turn_history.current.player(0).life_gained, 8);
        }
    }
}

#[test]
fn archive_targeted_healing_salve_uses_recipient_and_opponent_turn_draws_double() {
    for recipient in [4, 9] {
        let mut engine = setup(104_820);
        let archive = paid(&mut engine, "alhammarrets_archive");
        resolve(&mut engine, archive);
        let salve = inject_card_into_hand(&mut engine, 0, "healing_salve");
        give_mana(
            &mut engine,
            4,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let slot = engine.state.players[0]
            .hand
            .iter()
            .position(|id| *id == salve)
            .unwrap();
        let command = cast_modal_spell(slot, vec![(0, target_player(recipient))]);
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_command(9, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        engine.apply_command(4, &command).unwrap();
        resolve(&mut engine, salve);
        assert_eq!(
            engine.state.players[0].life,
            if recipient == 4 { 26 } else { 20 }
        );
        assert_eq!(
            engine.state.players[1].life,
            if recipient == 9 { 23 } else { 20 }
        );
        engine.state.active_player_idx = 1;
        engine.state.turn_step = TurnStep::Draw;
        engine.state.priority_idx = 0;
        let brainstorm = paid(&mut engine, "brainstorm");
        let hand = engine.state.players[0].hand.len();
        let mut prompt = None;
        for _ in 0..12 {
            let actor = engine.state.priority_player_id();
            let batch = engine.apply_command(actor, &pass()).unwrap();
            if let Some(choice) = find_resolution_choice(&batch) {
                prompt = Some(choice.clone());
                break;
            }
        }
        let prompt = prompt.expect("Brainstorm put-back follows all six opponent-turn draws");
        assert_eq!(prompt.choice_kind(), rv1::ChoiceKind::HandCards);
        assert_eq!(engine.state.players[0].hand.len(), hand + 6);
        engine
            .apply_command(
                4,
                &submit_resolution_choice(prompt.candidate_object_ids[..2].to_vec()),
            )
            .unwrap();
        assert_eq!(engine.state.objects[&brainstorm].zone, Zone::Graveyard);
        assert!(engine.state.pending_resolution.is_none());
    }
}
