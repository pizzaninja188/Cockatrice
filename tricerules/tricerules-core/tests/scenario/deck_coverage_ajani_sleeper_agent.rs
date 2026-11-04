//! Actual registered Ajani; explicitly prepared zones/pools/counters. Normal capture and network
//! transport evidence are separate. No synthetic tail or synthetic planeswalker definition.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

const AJANI: &str = "ajani,_sleeper_agent";

fn game() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_110_317,
        &[17, 29, 41],
        20,
        Some(vec![deck_with("forest", &[]); 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}
fn ajani(engine: &mut GameEngine, loyalty: u32) -> u32 {
    let source = inject_permanent_on_battlefield(engine, 0, AJANI);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, loyalty);
    source
}
fn generation(engine: &GameEngine, oid: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&oid)
        .copied()
        .unwrap_or(0)
}
fn resolve(engine: &mut GameEngine) -> rv1::RuledEventBatch {
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let mut result = None;
    for _ in 0..count {
        result = Some(
            engine
                .apply_scenario_command(engine.state.priority_player_id(), &pass())
                .unwrap(),
        );
    }
    result.unwrap()
}
fn reject(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}
fn allocation(engine: &GameEngine, oid: u32, count: u32) -> rv1::TargetRef {
    rv1::TargetRef {
        object_id: oid,
        kind: rv1::TargetRefKind::Permanent as i32,
        expected_zone_change_generation: Some(generation(engine, oid)),
        counter_amount: count,
        ..Default::default()
    }
}
fn player_target(player: i32) -> rv1::TargetRef {
    rv1::TargetRef {
        object_id: player as u32,
        kind: rv1::TargetRefKind::Player as i32,
        ..Default::default()
    }
}
fn target_opponent(engine: &mut GameEngine, player: i32) -> rv1::RuledEventBatch {
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(rv1::ChooseTriggerTarget {
                    targets: vec![player_target(player)],
                    ..Default::default()
                })),
            },
        )
        .unwrap()
}
fn emblem(engine: &mut GameEngine) -> (u32, u32) {
    let source = ajani(engine, 6);
    apply_ability(engine, 17, source, 2, vec![]).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    resolve(engine);
    (
        source,
        engine.state.static_emblems.last().unwrap().object_id,
    )
}
fn cast_actual(engine: &mut GameEngine, actor: i32, id: &str) -> u32 {
    let index = engine.state.player_idx(actor).unwrap();
    engine.state.active_player_idx = index;
    engine.state.priority_idx = index;
    engine.state.turn_step = TurnStep::Main1;
    let object = inject_card_into_hand(engine, index, id);
    give_mana(
        engine,
        actor,
        ManaGift {
            c: 10,
            w: 10,
            u: 10,
            g: 10,
            ..Default::default()
        },
    );
    let choices = if id == AJANI {
        vec![rv1::FlexPipPayment {
            pip_index: 2,
            mana_color: rv1::ManaColor::Green as i32,
            pay_life: false,
        }]
    } else {
        vec![]
    };
    let command = cast_spell_flex(hand_index_for_card(engine, index, id), vec![], choices);
    engine.apply_scenario_command(actor, &command).unwrap();
    object
}

#[test]
fn actual_ajani_three_payment_branches_and_compleated_season_orders() {
    for (color, life, season, compleated_first, expected) in [
        (rv1::ManaColor::Green, false, false, false, 4),
        (rv1::ManaColor::White, false, false, false, 4),
        (rv1::ManaColor::Unspecified, true, false, false, 2),
        (rv1::ManaColor::Green, false, true, false, 8),
        (rv1::ManaColor::White, false, true, false, 8),
        (rv1::ManaColor::Unspecified, true, true, true, 4),
        (rv1::ManaColor::Unspecified, true, true, false, 6),
    ] {
        let mut engine = game();
        if season {
            inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
        }
        let source = inject_card_into_hand(&mut engine, 0, AJANI);
        give_mana(
            &mut engine,
            17,
            ManaGift {
                c: 1,
                w: 2,
                g: 2,
                ..Default::default()
            },
        );
        let command = cast_spell_flex(
            hand_index_for_card(&engine, 0, AJANI),
            vec![],
            vec![rv1::FlexPipPayment {
                pip_index: 2,
                pay_life: life,
                mana_color: color as i32,
            }],
        );
        engine.apply_scenario_command(17, &command).unwrap();
        assert_eq!(engine.state.players[0].life, if life { 18 } else { 20 });
        let batch = resolve(&mut engine);
        if life && season {
            let choice = find_resolution_choice(&batch).unwrap();
            let handle = choice
                .replacement_options
                .iter()
                .find(|option| option.effect_summary.contains("Compleated") == compleated_first)
                .unwrap()
                .application_id;
            let answer = RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                    resolution_prompt_id: choice.resolution_prompt_id,
                    chosen_object_ids: vec![handle],
                    ..Default::default()
                })),
            };
            reject(&mut engine, 29, &answer);
            engine.apply_command(17, &answer).unwrap();
        }
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Loyalty),
            expected
        );
        assert_eq!(engine.characteristics(source).unwrap().mana_value, 4);
    }
}

#[test]
fn actual_ajani_plus_one_reveals_creature_or_planeswalker_into_hand_and_handles_empty_library() {
    for id in [Some("grizzly_bears"), Some(AJANI), None] {
        let mut engine = game();
        let source = ajani(&mut engine, 4);
        let top = id.map(|id| {
            let top = inject_library_card(&mut engine, 0, id);
            engine.state.players[0].library.retain(|oid| *oid != top);
            engine.state.players[0].library.push_front(top);
            top
        });
        if id.is_none() {
            engine.state.players[0].library.clear();
        }
        let hand = engine.state.players[0].hand.len();
        apply_ability(&mut engine, 17, source, 0, vec![]).unwrap();
        let batch = resolve(&mut engine);
        assert!(find_resolution_choice(&batch).is_none());
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Loyalty),
            5
        );
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand + usize::from(top.is_some())
        );
        assert!(!engine.state.players[0].pending_library_loss);
        if let Some(top) = top {
            assert!(engine.state.players[0].hand.contains(&top));
            assert_eq!(engine.state.objects[&top].zone, Zone::Hand);
            assert_eq!(generation(&engine, top), 1);
            assert!(batch.events.iter().any(|event| matches!(&event.ev,Some(Ev::CardsRevealed(reveal))
                if reveal.cards.len()==1 && reveal.cards[0].object_id==top && reveal.cards[0].zone_change_generation==0)));
        } else {
            assert!(!batch
                .events
                .iter()
                .any(|event| matches!(event.ev, Some(Ev::CardsRevealed(_)))));
        }
        assert_eq!(engine.state.players[0].life, 20);
    }
}

#[test]
fn actual_ajani_unmatched_public_reveal_keeps_or_bottoms_exact_card_and_rejects_stale_answers() {
    for branch in [0, 1] {
        let mut engine = game();
        let source = ajani(&mut engine, 4);
        let before = engine.state.players[0].library.clone();
        let top = before[0];
        apply_ability(&mut engine, 17, source, 0, vec![]).unwrap();
        let batch = resolve(&mut engine);
        let choice = find_resolution_choice(&batch).unwrap();
        assert_eq!(choice.choice_kind, rv1::ChoiceKind::ResolutionBranch as i32);
        let reveal = choice.public_reveal.as_ref().unwrap();
        assert_eq!(reveal.cards[0].object_id, top);
        assert_eq!(
            reveal.cards[0].zone_change_generation,
            generation(&engine, top)
        );
        let answer = RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                resolution_prompt_id: choice.resolution_prompt_id,
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: branch,
                ..Default::default()
            })),
        };
        reject(&mut engine, 29, &answer);
        engine.apply_command(17, &answer).unwrap();
        if branch == 0 {
            assert_eq!(engine.state.players[0].library, before);
        } else {
            assert_eq!(engine.state.players[0].library.back(), Some(&top));
            assert_eq!(engine.state.players[0].library[0], before[1]);
            assert_eq!(generation(&engine, top), 1);
        }
        assert!(engine.state.pending_resolution.is_none());
        reject(&mut engine, 17, &answer);
        assert_eq!(engine.state.players[0].life, 20);
    }
}

#[test]
fn actual_ajani_minus_three_distributes_zero_to_three_targets_and_vigilance_expires() {
    for amounts in [vec![], vec![3], vec![2, 1], vec![1, 1, 1]] {
        let mut engine = game();
        let source = ajani(&mut engine, 3);
        let creatures = (0..3)
            .map(|index| inject_creature_on_battlefield(&mut engine, index, "grizzly_bears"))
            .collect::<Vec<_>>();
        let targets = creatures
            .iter()
            .zip(&amounts)
            .map(|(&oid, &count)| allocation(&engine, oid, count))
            .collect();
        let batch = apply_ability(&mut engine, 17, source, 1, targets).unwrap();
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        let shares = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::StackPushed(push)) => Some(
                    push.targets
                        .iter()
                        .map(|target| target.counter_amount)
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .unwrap();
        assert_eq!(shares, amounts);
        resolve(&mut engine);
        for (index, &oid) in creatures.iter().enumerate() {
            let expected = amounts.get(index).copied().unwrap_or(0);
            assert_eq!(
                engine.state.objects[&oid].counter_count(CounterKind::PlusOnePlusOne),
                expected
            );
            assert_eq!(
                engine
                    .characteristics(oid)
                    .unwrap()
                    .has_keyword(Keyword::Vigilance),
                expected > 0
            );
        }
        end_active_turn(&mut engine, 17);
        for (index, &oid) in creatures.iter().enumerate() {
            assert!(!engine
                .characteristics(oid)
                .unwrap()
                .has_keyword(Keyword::Vigilance));
            assert_eq!(
                engine.state.objects[&oid].counter_count(CounterKind::PlusOnePlusOne),
                amounts.get(index).copied().unwrap_or(0)
            );
        }
    }
}

#[test]
fn actual_ajani_minus_three_rejects_bad_announcements_and_does_not_redistribute_stale_shares() {
    for mutation in 0..10 {
        let mut engine = game();
        let source = ajani(&mut engine, 4);
        let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let mut targets = vec![
            allocation(&engine, first, 2),
            allocation(&engine, second, 1),
        ];
        let mut actor = 17;
        match mutation {
            0 => targets[0].counter_amount = 0,
            1 => targets[0].counter_amount = 1,
            2 => targets[0].counter_amount = u32::MAX,
            3 => targets[0].expected_zone_change_generation = None,
            4 => targets[0].expected_zone_change_generation = Some(9),
            5 => targets[0].damage_amount = 1,
            6 => targets[0].kind = rv1::TargetRefKind::Player as i32,
            7 => targets[1] = allocation(&engine, first, 1),
            8 => targets[0].object_id = source,
            9 => actor = 29,
            _ => unreachable!(),
        }
        let command = activate_ability_for(&engine, source, 1, targets);
        reject(&mut engine, actor, &command);
    }
    for stale in [1, 2] {
        let mut engine = game();
        let source = ajani(&mut engine, 4);
        let creatures = [
            inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"),
            inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears"),
        ];
        let targets = vec![
            allocation(&engine, creatures[0], 2),
            allocation(&engine, creatures[1], 1),
        ];
        apply_ability(&mut engine, 17, source, 1, targets).unwrap();
        // Prepared stale incarnation; unchanged ID alone cannot make this target legal.
        for &oid in &creatures[..stale] {
            engine.state.zone_change_generation.insert(oid, 1);
        }
        resolve(&mut engine);
        assert_eq!(
            engine.state.objects[&creatures[0]].counter_count(CounterKind::PlusOnePlusOne),
            0
        );
        assert!(!engine
            .characteristics(creatures[0])
            .unwrap()
            .has_keyword(Keyword::Vigilance));
        assert_eq!(
            engine.state.objects[&creatures[1]].counter_count(CounterKind::PlusOnePlusOne),
            u32::from(stale == 1)
        );
        assert_eq!(
            engine
                .characteristics(creatures[1])
                .unwrap()
                .has_keyword(Keyword::Vigilance),
            stale == 1
        );
    }
}

#[test]
fn actual_ajani_emblem_outlives_source_matches_own_permanent_casts_and_uses_counter_replacements() {
    for (actor, id, matches, double, initial) in [
        (17, "grizzly_bears", true, false, 0),
        (17, AJANI, true, false, 0),
        (17, "sol_ring", false, false, 0),
        (29, "grizzly_bears", false, false, 0),
        (17, "grizzly_bears", true, true, 0),
        (17, "grizzly_bears", true, false, 8),
    ] {
        let mut engine = game();
        let (source, marker) = emblem(&mut engine);
        if double {
            inject_permanent_on_battlefield(&mut engine, 0, "vorinclex,_monstrous_raider");
        }
        if initial > 0 {
            engine.state.players[1]
                .counters
                .insert(CounterKind::Poison, initial);
        }
        let original = cast_actual(&mut engine, actor, id);
        assert_eq!(engine.state.pending_triggers.len(), usize::from(matches));
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        if matches {
            let invalid = RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(rv1::ChooseTriggerTarget {
                    targets: vec![player_target(17)],
                    ..Default::default()
                })),
            };
            reject(&mut engine, 17, &invalid);
            let batch = target_opponent(&mut engine, 29);
            let pushed = batch
                .events
                .iter()
                .find_map(|event| match &event.ev {
                    Some(Ev::StackPushed(push)) => Some(push),
                    _ => None,
                })
                .unwrap();
            assert!(pushed.ability_source.is_none());
            assert_eq!(pushed.targets, vec![player_target(29)]);
            resolve(&mut engine);
            assert_eq!(
                engine.state.players[1].counters[&CounterKind::Poison],
                initial + if double { 4 } else { 2 }
            );
            assert_eq!(engine.state.players[1].has_lost, initial == 8);
            assert_eq!(engine.state.stack.last().unwrap().id, original);
        }
        assert!(engine
            .state
            .static_emblems
            .iter()
            .any(|emblem| emblem.object_id == marker));
    }
}

#[test]
fn actual_ajani_two_emblems_choose_opponents_independently() {
    let mut engine = game();
    let (_, first) = emblem(&mut engine);
    let (_, second) = emblem(&mut engine);
    cast_actual(&mut engine, 17, "grizzly_bears");
    let pending = engine.state.pending_trigger_order.as_ref().unwrap();
    assert_eq!(pending.candidates.len(), 2);
    let selected = pending
        .candidates
        .iter()
        .find(|trigger| trigger.source_permanent_id == first)
        .unwrap()
        .object_id;
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(Cmd::SubmitTriggerOrder(rv1::SubmitTriggerOrder {
                    trigger_object_id: selected,
                })),
            },
        )
        .unwrap();
    target_opponent(&mut engine, 29);
    assert_eq!(
        engine
            .state
            .pending_triggers
            .front()
            .unwrap()
            .source_permanent_id,
        second
    );
    target_opponent(&mut engine, 41);
    resolve(&mut engine);
    assert_eq!(engine.state.players[2].counters[&CounterKind::Poison], 2);
    assert!(!engine.state.players[1]
        .counters
        .contains_key(&CounterKind::Poison));
    resolve(&mut engine);
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 2);
    assert_eq!(engine.state.static_emblems.len(), 2);
}

#[test]
fn actual_ajani_handles_a_dual_type_top_and_grants_vigilance_despite_prevented_counters() {
    // The planeswalker is the unchanged actual RON; only the subjects are synthetic seam fixtures.
    const SUBJECTS: &[&str] = &[
        include_str!("../../../tricerules-cards/data/ajani,_sleeper_agent.ron"),
        include_str!("../../../tricerules-cards/data/forest.ron"),
        include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
        r#"(id:"ajani_dual_subject",name:"Ajani Dual Subject",face_id:"ajani_dual_subject",
            mana_cost:"{0}",types:["Creature","Planeswalker"],power:1,toughness:1,loyalty:1)"#,
        r#"(id:"ajani_counterless_subject",name:"Ajani Counterless Subject",face_id:"ajani_counterless_subject",
            mana_cost:"{0}",types:["Creature"],power:2,toughness:2,
            static_abilities:[(ability_id:"prohibit",presentation:Fallback,definition:ProhibitCounters(affected:Self_))])"#,
    ];
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(SUBJECTS, &[]).unwrap(),
    ));
    for dual_top in [true, false] {
        let mut engine = GameEngine::new(
            registry,
            26_110_318,
            &[17, 29, 41],
            20,
            Some(vec![deck_with("forest", &[]); 3]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let source = ajani(&mut engine, 4);
        if dual_top {
            let top = inject_library_card(&mut engine, 0, "ajani_dual_subject");
            engine.state.players[0].library.retain(|oid| *oid != top);
            engine.state.players[0].library.push_front(top);
            apply_ability(&mut engine, 17, source, 0, vec![]).unwrap();
            let response = resolve(&mut engine);
            assert!(find_resolution_choice(&response).is_none());
            assert!(engine.state.players[0].hand.contains(&top));
        } else {
            let prevented =
                inject_creature_on_battlefield(&mut engine, 0, "ajani_counterless_subject");
            let placed = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
            let targets = vec![
                allocation(&engine, prevented, 2),
                allocation(&engine, placed, 1),
            ];
            apply_ability(&mut engine, 17, source, 1, targets).unwrap();
            resolve(&mut engine);
            assert_eq!(
                engine.state.objects[&prevented].counter_count(CounterKind::PlusOnePlusOne),
                0
            );
            assert_eq!(
                engine.state.objects[&placed].counter_count(CounterKind::PlusOnePlusOne),
                1
            );
            for oid in [prevented, placed] {
                assert!(engine
                    .characteristics(oid)
                    .unwrap()
                    .has_keyword(Keyword::Vigilance));
            }
        }
    }
}
