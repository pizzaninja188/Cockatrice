use crate::helpers::*;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChooseTriggerTarget, RuledCommand,
};

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn any_target_candidates(engine: &mut GameEngine, hand_slot: usize) -> Vec<u32> {
    engine.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((hand_slot as u32) << 8)]
        .groups[0]
        .valid_permanent_ids
        .clone()
}

#[test]
fn any_target_publishes_planeswalkers_and_battles_but_not_lands() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        deck_with(
            "forest",
            &[
                "jace_beleren",
                "invasion_of_ulgrotha_grandmother_ravi_sengir",
            ],
        ),
    ]);
    let mut engine = GameEngine::new(72_001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    let jace = relocate_to_battlefield(&mut engine, 1, "jace_beleren", false);
    let battle = relocate_to_battlefield(
        &mut engine,
        1,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
        false,
    );
    let forest = relocate_to_battlefield(&mut engine, 1, "forest", false);

    let slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    let candidates = any_target_candidates(&mut engine, slot);
    assert!(
        candidates.contains(&jace),
        "planeswalker must be an any-target candidate"
    );
    assert!(
        candidates.contains(&battle),
        "Battle must be an any-target candidate"
    );
    assert!(
        !candidates.contains(&forest),
        "ordinary land must not be an any-target candidate"
    );
}

fn cast_bolt_at(engine: &mut GameEngine, target: u32) {
    ensure_in_hand(engine, 0, "lightning_bolt");
    give_mana(
        engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "lightning_bolt");
    engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![TargetRef {
                    object_id: target,
                    group_index: 0,
                    kind: TargetRefKind::Permanent as i32,
                    ..Default::default()
                }],
            ),
        )
        .expect("cast Lightning Bolt");
    resolve_entire_stack_two_player(engine);
}

#[test]
fn damage_removes_loyalty_and_defense_counters() {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                "lightning_bolt",
                "lightning_bolt",
                "jace_beleren",
                "invasion_of_ulgrotha_grandmother_ravi_sengir",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(72_002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "jace_beleren");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "jace_beleren");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Jace Beleren");
    resolve_entire_stack_two_player(&mut engine);
    let jace = battlefield_object_for_card(&engine, 0, "jace_beleren");

    ensure_in_hand(
        &mut engine,
        0,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 5,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "invasion_of_ulgrotha_grandmother_ravi_sengir");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Invasion of Ulgrotha");
    resolve_entire_stack_two_player(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![1]))
        .expect("choose Siege protector");
    let trigger_batch = engine
        .apply_command(0, &choose_trigger_targets(target_player(1)))
        .expect("target opponent with Invasion ETB");
    let stack_display = trigger_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(pushed)) => Some(pushed),
            _ => None,
        })
        .expect("Invasion trigger on stack");
    assert_eq!(
        stack_display.description, "Invasion of Ulgrotha",
        "transform ability cards need the display database's face identity"
    );
    resolve_entire_stack_two_player(&mut engine);
    let battle =
        battlefield_object_for_card(&engine, 0, "invasion_of_ulgrotha_grandmother_ravi_sengir");

    assert_eq!(
        engine.state.objects[&jace].counters.values().sum::<u32>(),
        3
    );
    assert_eq!(
        engine.state.objects[&battle].counters.values().sum::<u32>(),
        5
    );
    cast_bolt_at(&mut engine, jace);
    cast_bolt_at(&mut engine, battle);
    assert_eq!(
        engine.state.objects[&jace].counters.values().sum::<u32>(),
        0
    );
    assert_eq!(
        engine.state.objects[&battle].counters.values().sum::<u32>(),
        2
    );
    assert_eq!(engine.state.objects[&jace].damage, 0);
    assert_eq!(engine.state.objects[&battle].damage, 0);
    assert_eq!(
        engine.state.objects[&jace].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&battle].zone,
        tricerules_core::Zone::Battlefield
    );
}

#[test]
fn loyalty_cost_is_atomic_sorcery_speed_and_shared_across_abilities() {
    let decks = Some(vec![
        deck_with("island", &["jace_beleren"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(72_003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "jace_beleren");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "jace_beleren");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Jace");
    resolve_entire_stack_two_player(&mut engine);
    let jace = battlefield_object_for_card(&engine, 0, "jace_beleren");

    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("activate +2");
    assert_eq!(
        engine.state.objects[&jace].counters.values().sum::<u32>(),
        5
    );
    resolve_entire_stack_two_player(&mut engine);
    let before = engine.state.objects[&jace].counters.clone();
    assert!(
        apply_ability(&mut engine, 0, jace, 1, vec![target_player(0)[0]]).is_err(),
        "a second loyalty ability on the same permanent this turn must be rejected"
    );
    assert_eq!(engine.state.objects[&jace].counters, before);
}

#[test]
fn siege_protector_is_chosen_before_the_battle_enters() {
    let mut duel = GameEngine::new(
        72_004,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["invasion_of_ulgrotha_grandmother_ravi_sengir"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("duel");
    advance_to_main1_from_game_start(&mut duel);
    ensure_in_hand(&mut duel, 0, "invasion_of_ulgrotha_grandmother_ravi_sengir");
    give_mana(
        &mut duel,
        0,
        ManaGift {
            b: 5,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&duel, 0, "invasion_of_ulgrotha_grandmother_ravi_sengir");
    let battle = duel.state.players[0].hand[slot];
    duel.apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Invasion");
    duel.apply_command(0, &pass()).expect("caster pass");
    let resolving = duel.apply_command(1, &pass()).expect("opponent pass");

    let choice = find_resolution_choice(&resolving).expect("protector choice");
    assert_eq!(choice.choice_kind, ChoiceKind::BattleProtector as i32);
    assert_eq!(choice.candidate_object_ids, vec![1]);
    assert_eq!(
        duel.state.objects[&battle].zone,
        tricerules_core::Zone::Stack
    );
    assert!(!duel.state.players[0].battlefield.contains(&battle));

    duel.apply_command(0, &submit_resolution_choice(vec![1]))
        .expect("choose opponent to protect the Siege");
    assert_eq!(
        duel.state.objects[&battle].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(duel.state.battle_protectors.get(&battle), Some(&1));
}

fn siege_ready_to_choose(seed: u64) -> (GameEngine, u32, u64) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with(
                "swamp",
                &[
                    "invasion_of_ulgrotha_grandmother_ravi_sengir",
                    "lightning_bolt",
                ],
            ),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let battle = relocate_to_battlefield(
        &mut engine,
        0,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
        false,
    );
    engine
        .state
        .objects
        .get_mut(&battle)
        .expect("Battle")
        .set_counter(tricerules_cards::primitives::CounterKind::Defense, 3);
    engine.apply_command(0, &pass()).expect("assign protector");
    let battlefield_generation = engine
        .state
        .zone_change_generation
        .get(&battle)
        .copied()
        .unwrap_or(0);

    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![TargetRef {
                    object_id: battle,
                    group_index: 0,
                    kind: TargetRefKind::Permanent as i32,
                    ..Default::default()
                }],
            ),
        )
        .expect("cast lethal Bolt");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&battle].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&battle].counters.values().sum::<u32>(),
        0
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&battle)
            .copied()
            .unwrap_or(0),
        battlefield_generation
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "defeat is an intrinsic trigger"
    );

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&battle].zone,
        tricerules_core::Zone::Exile
    );
    let exile_generation = engine.state.zone_change_generation[&battle];
    assert!(
        engine.state.pending_resolution.is_some(),
        "cast choice must park resolution"
    );
    (engine, battle, exile_generation)
}

#[test]
fn defeated_siege_decline_leaves_exact_card_in_exile() {
    let (mut engine, battle, exile_generation) = siege_ready_to_choose(72_005);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
            ),
        )
        .expect("decline transformed cast");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&battle].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.zone_change_generation[&battle],
        exile_generation
    );
}

#[test]
fn defeated_siege_casts_back_face_with_exact_physical_identity() {
    let (mut engine, battle, exile_generation) = siege_ready_to_choose(72_006);
    let announcement = CastSpell {
        cast_method: CastMethod::SiegeDefeat as i32,
        source: Some(exile_cast_source(battle)),
        face_index: 1,
        ..Default::default()
    };
    let command = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::CastTransformed as i32,
            selected_branch_index: 0,
            cast_spell: Some(announcement),
            chosen_combat_defender: None,
            payment: None,
            restricted_mana: vec![],
        })),
    };
    let mut forged_payment = command.clone();
    if let Some(Cmd::SubmitResolutionChoice(choice)) = &mut forged_payment.cmd {
        choice.cast_spell.as_mut().unwrap().payment = Some(Default::default());
    }
    let before = format!("{:?}", engine.state);
    assert!(
        engine.apply_command(0, &forged_payment).is_err(),
        "a free Siege offer must not ignore an explicit payment payload"
    );
    assert_eq!(format!("{:?}", engine.state), before);
    engine
        .apply_command(0, &command)
        .expect("cast transformed back face");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&battle].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.stack.last().expect("back face on stack").id,
        battle
    );
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .expect("back face on stack")
            .face_index,
        1
    );
    assert_eq!(
        engine.state.zone_change_generation[&battle],
        exile_generation + 1
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&battle].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&battle].face_up_index, 1);
    assert!(
        engine.state.objects[&battle].counters.is_empty(),
        "the transformed creature must not inherit the front face's defense counters"
    );
    assert_eq!(
        engine.state.zone_change_generation[&battle],
        exile_generation + 2
    );
}

fn published_attack_assignment(
    engine: &mut GameEngine,
    attacker: u32,
    kind: TargetRefKind,
    defender: u32,
) -> AttackAssignment {
    engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|assignment| {
            assignment.attacker_object_id == attacker
                && assignment.defender.as_ref().is_some_and(|target| {
                    target.kind == kind as i32 && target.object_id == defender
                })
        })
        .cloned()
        .expect("published defender assignment")
}

#[test]
fn split_attackers_damage_player_planeswalker_and_battle() {
    let mut engine = GameEngine::new(
        72_007,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &[]),
            deck_with(
                "island",
                &[
                    "jace_beleren",
                    "invasion_of_ulgrotha_grandmother_ravi_sengir",
                ],
            ),
        ]),
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut engine);
    let attack_player = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let attack_jace = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let attack_battle = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let jace = relocate_to_battlefield(&mut engine, 1, "jace_beleren", false);
    engine
        .state
        .objects
        .get_mut(&jace)
        .expect("Jace")
        .set_counter(tricerules_cards::primitives::CounterKind::Loyalty, 3);
    let battle = relocate_to_battlefield(
        &mut engine,
        1,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
        false,
    );
    engine
        .state
        .objects
        .get_mut(&battle)
        .expect("Battle")
        .set_counter(tricerules_cards::primitives::CounterKind::Defense, 5);
    engine.state.battle_protectors.insert(battle, 1);

    let assignments = vec![
        published_attack_assignment(&mut engine, attack_player, TargetRefKind::Player, 1),
        published_attack_assignment(&mut engine, attack_jace, TargetRefKind::Permanent, jace),
        published_attack_assignment(&mut engine, attack_battle, TargetRefKind::Permanent, battle),
    ];
    let declared = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: assignments.clone(),
                })),
            },
        )
        .expect("split attack declaration");
    let event = attackers_declared_in(&declared)
        .pop()
        .expect("declared event");
    assert_eq!(event.assignments, assignments);

    engine.apply_command(0, &pass()).expect("AP pass attackers");
    engine
        .apply_command(1, &pass())
        .expect("NAP pass attackers");
    engine.apply_command(0, &pass()).expect("AP pass blockers");
    engine
        .apply_command(1, &pass())
        .expect("NAP pass blockers and damage");
    assert_eq!(engine.state.players[1].life, 18);
    assert_eq!(
        engine.state.objects[&jace]
            .counter_count(tricerules_cards::primitives::CounterKind::Loyalty),
        1
    );
    assert_eq!(
        engine.state.objects[&battle]
            .counter_count(tricerules_cards::primitives::CounterKind::Defense),
        3
    );
}

#[test]
fn scorch_spitter_damages_attacked_planeswalker_but_not_battle() {
    let make_engine = |seed| {
        let mut engine = GameEngine::new(
            seed,
            &[0, 1],
            20,
            Some(vec![
                deck_with("mountain", &["scorch_spitter"]),
                deck_with(
                    "island",
                    &[
                        "jace_beleren",
                        "invasion_of_ulgrotha_grandmother_ravi_sengir",
                    ],
                ),
            ]),
            true,
        )
        .expect("new");
        advance_to_declare_attackers(&mut engine);
        let spitter = relocate_to_battlefield(&mut engine, 0, "scorch_spitter", false);
        (engine, spitter)
    };

    let (mut planeswalker_game, spitter) = make_engine(72_008);
    let jace = relocate_to_battlefield(&mut planeswalker_game, 1, "jace_beleren", false);
    planeswalker_game
        .state
        .objects
        .get_mut(&jace)
        .expect("Jace")
        .set_counter(tricerules_cards::primitives::CounterKind::Loyalty, 3);
    let assignment = published_attack_assignment(
        &mut planeswalker_game,
        spitter,
        TargetRefKind::Permanent,
        jace,
    );
    planeswalker_game
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .expect("attack Jace");
    resolve_entire_stack_two_player(&mut planeswalker_game);
    assert_eq!(
        planeswalker_game.state.objects[&jace]
            .counter_count(tricerules_cards::primitives::CounterKind::Loyalty),
        2
    );

    let (mut battle_game, spitter) = make_engine(72_009);
    let battle = relocate_to_battlefield(
        &mut battle_game,
        1,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
        false,
    );
    battle_game
        .state
        .objects
        .get_mut(&battle)
        .expect("Battle")
        .set_counter(tricerules_cards::primitives::CounterKind::Defense, 5);
    battle_game.state.battle_protectors.insert(battle, 1);
    let assignment =
        published_attack_assignment(&mut battle_game, spitter, TargetRefKind::Permanent, battle);
    battle_game
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .expect("attack Battle");
    resolve_entire_stack_two_player(&mut battle_game);
    assert_eq!(
        battle_game.state.objects[&battle]
            .counter_count(tricerules_cards::primitives::CounterKind::Defense),
        5,
        "Scorch Spitter does not damage an attacked Battle"
    );
}

#[test]
fn attacker_stays_in_combat_but_deals_no_damage_when_permanent_defender_disappears() {
    let mut engine = GameEngine::new(
        72_010,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &[]),
            deck_with("island", &["jace_beleren"]),
        ]),
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut engine);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let jace = relocate_to_battlefield(&mut engine, 1, "jace_beleren", false);
    engine
        .state
        .objects
        .get_mut(&jace)
        .expect("Jace")
        .set_counter(tricerules_cards::primitives::CounterKind::Loyalty, 3);
    let assignment =
        published_attack_assignment(&mut engine, attacker, TargetRefKind::Permanent, jace);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .expect("attack Jace");

    engine.state.players[1]
        .battlefield
        .retain(|object| *object != jace);
    engine.state.players[1].graveyard.push(jace);
    engine.state.objects.get_mut(&jace).expect("Jace").zone = tricerules_core::Zone::Graveyard;
    *engine.state.zone_change_generation.entry(jace).or_default() += 1;
    assert!(engine
        .state
        .combat
        .as_ref()
        .expect("combat")
        .attacking
        .contains(&attacker));

    engine.apply_command(0, &pass()).expect("AP pass attackers");
    engine
        .apply_command(1, &pass())
        .expect("NAP pass attackers");
    engine.apply_command(0, &pass()).expect("AP pass blockers");
    engine.apply_command(1, &pass()).expect("NAP pass blockers");
    assert_eq!(engine.state.players[1].life, 20);
}
