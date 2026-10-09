use super::helpers::*;
use tricerules_cards::primitives::{AttackLimitAffected, CounterKind, StaticAbilityDef};
use tricerules_core::state::CopiableValues;
use tricerules_core::TurnStep;

// Unrelated resources and must-attack flags are explicit fixture setup. Every declaration
// below uses current engine-published, generation-bound assignments through apply_command.
fn fixture(seed: u64, players: &[i32]) -> (GameEngine, u32, Vec<u32>) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        players,
        20,
        None,
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 1, "crawlspace");
    let attackers = (0..5)
        .map(|_| inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"))
        .collect();
    (engine, source, attackers)
}

fn begin(engine: &mut GameEngine) {
    let active = engine.state.active_player_id();
    semantic::accepted(engine, active, &primitive_yield());
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn declare_to(engine: &mut GameEngine, destinations: &[(u32, TargetRefKind, u32)]) -> RuledCommand {
    let active = engine.state.active_player_id();
    let batch = engine.initial_response_batch();
    let options = &batch.legal_by_player[&active].legal_attack_assignments;
    RuledCommand {
        cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
            assignments: destinations
                .iter()
                .map(|(attacker, kind, target)| {
                    options
                        .iter()
                        .find(|edge| {
                            edge.attacker_object_id == *attacker
                                && edge.defender.as_ref().is_some_and(|defender| {
                                    defender.kind == *kind as i32 && defender.object_id == *target
                                })
                        })
                        .cloned()
                        .expect("exact engine defender edge")
                })
                .collect(),
        })),
    }
}

fn reject_unchanged(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

fn limit_fixture(
    engine: &mut GameEngine,
    source: u32,
    maximum: u32,
    affected: AttackLimitAffected,
) {
    let mut face = tricerules_cards::registry::global()
        .get("crawlspace")
        .unwrap()
        .primary_face()
        .clone();
    face.static_abilities[0].definition = StaticAbilityDef::LimitAttackers { maximum, affected };
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        source_card_id: "crawlspace".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Declaration limit fixture".into(),
    });
}

#[test]
fn crawlspace_rejects_three_player_attackers_and_accepts_two_without_partial_mutation() {
    let mut engine = semantic::main_phase(50401);
    inject_card_into_hand(&mut engine, 0, "crawlspace");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "crawlspace");
    semantic::accepted(&mut engine, 0, &cast_spell(slot, vec![]));
    semantic::complete(&mut engine, 8, |_| None).require_exercised();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let source = battlefield_object_for_card(&engine, 0, "crawlspace");
    assert_eq!(engine.state.objects[&source].controller, 0);
    for _ in 0..32 {
        if engine.state.active_player_id() == 1 && engine.state.turn_step == TurnStep::Main1 {
            break;
        }
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    let attackers: Vec<_> = (0..3)
        .map(|_| inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears"))
        .collect();
    semantic::accepted(&mut engine, 1, &primitive_yield());
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    let before = engine.diagnostic_snapshot().unwrap();
    engine
        .apply_command(1, &declare_attackers(attackers.clone()))
        .expect_err("Crawlspace limits direct attackers to two");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    semantic::accepted(&mut engine, 1, &declare_attackers(attackers[..2].to_vec()));
    assert_eq!(engine.state.combat.as_ref().unwrap().attacking.len(), 2);
    assert!(!engine.state.objects[&attackers[2]].tapped);
}

#[test]
fn crawlspace_maximizes_must_attack_requirements_under_a_cap() {
    for selected in [vec![0, 1], vec![1, 2], vec![0, 2]] {
        let (mut engine, _, attackers) = fixture(50402, &[0, 1]);
        for attacker in &attackers[..3] {
            engine
                .state
                .objects
                .get_mut(attacker)
                .unwrap()
                .must_attack_if_able = true;
        }
        begin(&mut engine);
        let batch = engine.initial_response_batch();
        let legal = &batch.legal_by_player[&0];
        assert_eq!(legal.attack_requirement_ids.len(), 3);
        assert_eq!(legal.minimum_attack_requirement_count, 2);
        assert_eq!(legal.attack_declaration_limits.len(), 1);
        assert_eq!(
            legal.attack_declaration_limits[0].attacked_player_id,
            Some(1)
        );
        assert_eq!(legal.attack_declaration_limits[0].maximum_attackers, 2);
        assert!(batch.legal_by_player[&1].attack_requirement_ids.is_empty());
        assert_eq!(
            batch.legal_by_player[&1].minimum_attack_requirement_count,
            0
        );
        assert!(batch.legal_by_player[&1]
            .attack_declaration_limits
            .is_empty());
        for bad in [
            vec![],
            vec![attackers[0]],
            vec![attackers[0], attackers[3]],
            attackers[..3].to_vec(),
        ] {
            reject_unchanged(&mut engine, 0, &declare_attackers(bad));
        }
        semantic::accepted(
            &mut engine,
            0,
            &declare_attackers(selected.into_iter().map(|index| attackers[index]).collect()),
        );
    }
}

#[test]
fn crawlspace_does_not_limit_planeswalkers_battles_or_mixed_declarations() {
    for kind in 0..3 {
        let (mut engine, _, attackers) = fixture(50403 + kind, &[0, 1]);
        let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
        engine
            .state
            .objects
            .get_mut(&walker)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 10);
        let battle = inject_permanent_on_battlefield(
            &mut engine,
            1,
            "invasion_of_ulgrotha_grandmother_ravi_sengir",
        );
        engine
            .state
            .objects
            .get_mut(&battle)
            .unwrap()
            .set_counter(CounterKind::Defense, 10);
        engine.state.battle_protectors.insert(battle, 1);
        begin(&mut engine);
        let targets: Vec<_> = attackers[..3]
            .iter()
            .enumerate()
            .map(|(index, attacker)| {
                if kind == 2 && index < 2 {
                    (*attacker, TargetRefKind::Player, 1)
                } else {
                    (
                        *attacker,
                        TargetRefKind::Permanent,
                        if kind == 1 { battle } else { walker },
                    )
                }
            })
            .collect();
        let command = declare_to(&mut engine, &targets);
        semantic::accepted(&mut engine, 0, &command);
        assert_eq!(engine.state.combat.as_ref().unwrap().attacking.len(), 3);
    }
}

#[test]
fn crawlspace_four_seats_counts_only_the_protected_player_and_follows_controller() {
    for transferred in [false, true] {
        let (mut engine, source, attackers) = fixture(50410, &[7, 20, 42, 91]);
        if transferred {
            engine.state.players[1]
                .battlefield
                .retain(|id| *id != source);
            engine.state.players[2].battlefield.push(source);
            engine.state.objects.get_mut(&source).unwrap().controller = 42;
            engine
                .state
                .objects
                .get_mut(&source)
                .unwrap()
                .base_controller = 42;
        }
        let protected = if transferred { 42 } else { 20 };
        let unprotected = if transferred { 20 } else { 42 };
        begin(&mut engine);
        let bad_targets: Vec<_> = attackers
            .iter()
            .enumerate()
            .map(|(index, id)| {
                (
                    *id,
                    TargetRefKind::Player,
                    if index < 3 { protected } else { unprotected },
                )
            })
            .collect();
        let bad = declare_to(&mut engine, &bad_targets);
        reject_unchanged(&mut engine, 7, &bad);
        let good_targets: Vec<_> = attackers
            .iter()
            .enumerate()
            .map(|(index, id)| {
                (
                    *id,
                    TargetRefKind::Player,
                    if index < 2 { protected } else { unprotected },
                )
            })
            .collect();
        let good = declare_to(&mut engine, &good_targets);
        semantic::accepted(&mut engine, 7, &good);
        assert_eq!(engine.state.combat.as_ref().unwrap().attacking.len(), 5);
    }
}

#[test]
fn crawlspace_unbounded_defender_keeps_every_must_attack_requirement() {
    let (mut engine, _, attackers) = fixture(50411, &[0, 1, 2]);
    for attacker in &attackers[..3] {
        engine
            .state
            .objects
            .get_mut(attacker)
            .unwrap()
            .must_attack_if_able = true;
    }
    begin(&mut engine);
    let fewer = declare_to(
        &mut engine,
        &[
            (attackers[0], TargetRefKind::Player, 1),
            (attackers[1], TargetRefKind::Player, 1),
        ],
    );
    reject_unchanged(&mut engine, 0, &fewer);
    let all = declare_to(
        &mut engine,
        &[
            (attackers[0], TargetRefKind::Player, 1),
            (attackers[1], TargetRefKind::Player, 1),
            (attackers[2], TargetRefKind::Player, 2),
        ],
    );
    semantic::accepted(&mut engine, 0, &all);
}

#[test]
fn attack_limit_global_fixture_and_duplicate_minimum_compose_with_requirements() {
    let (mut engine, source, attackers) = fixture(50412, &[0, 1]);
    limit_fixture(&mut engine, source, 1, AttackLimitAffected::All);
    for attacker in &attackers[..3] {
        engine
            .state
            .objects
            .get_mut(attacker)
            .unwrap()
            .must_attack_if_able = true;
    }
    let duplicate = inject_permanent_on_battlefield(&mut engine, 1, "crawlspace");
    limit_fixture(&mut engine, duplicate, 4, AttackLimitAffected::All);
    begin(&mut engine);
    reject_unchanged(&mut engine, 0, &declare_attackers(attackers[..2].to_vec()));
    reject_unchanged(&mut engine, 0, &declare_attackers(vec![attackers[4]]));
    semantic::accepted(&mut engine, 0, &declare_attackers(vec![attackers[2]]));
}

#[test]
fn crawlspace_inactive_face_down_or_removed_source_has_no_limit() {
    for face_down in [false, true] {
        let (mut engine, source, attackers) = fixture(50413, &[0, 1]);
        if face_down {
            engine.state.objects.get_mut(&source).unwrap().face_down = true;
        } else {
            engine.state.players[1]
                .battlefield
                .retain(|id| *id != source);
            engine.state.players[1].graveyard.push(source);
            engine.state.objects.get_mut(&source).unwrap().zone = tricerules_core::Zone::Graveyard;
        }
        begin(&mut engine);
        semantic::accepted(&mut engine, 0, &declare_attackers(attackers[..3].to_vec()));
    }
}

#[test]
fn crawlspace_exact_identity_and_presentation_cover_the_complete_card() {
    let definition = tricerules_cards::registry::global()
        .get("crawlspace")
        .unwrap();
    assert_eq!(definition.name, "Crawlspace");
    assert_eq!(definition.layout, tricerules_cards::Layout::Normal);
    assert_eq!(definition.faces_iter().count(), 1);
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert_eq!((face.power, face.toughness), (None, None));
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(
        face.static_abilities[0].definition,
        StaticAbilityDef::LimitAttackers {
            maximum: 2,
            affected: AttackLimitAffected::AttackingController,
        }
    );
    assert_eq!(
        face.static_abilities[0].presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![1])
    );
}

#[test]
fn attack_limit_zero_filters_edges_and_requirements_without_creating_a_deadlock() {
    let (mut engine, source, attackers) = fixture(50414, &[0, 1]);
    for attacker in &attackers[..3] {
        engine
            .state
            .objects
            .get_mut(attacker)
            .unwrap()
            .must_attack_if_able = true;
    }
    begin(&mut engine);
    limit_fixture(
        &mut engine,
        source,
        0,
        AttackLimitAffected::AttackingController,
    );
    let batch = engine.initial_response_batch();
    let legal = &batch.legal_by_player[&0];
    assert!(legal.selectable_attacker_ids.is_empty());
    assert!(legal.legal_attack_assignments.is_empty());
    assert!(legal.attack_requirement_ids.is_empty());
    assert_eq!(legal.minimum_attack_requirement_count, 0);
    assert_eq!(legal.attack_declaration_limits[0].maximum_attackers, 0);
    reject_unchanged(&mut engine, 0, &declare_attackers(vec![attackers[0]]));
    semantic::accepted(&mut engine, 0, &declare_attackers(vec![]));
    assert_eq!(engine.state.turn_step, TurnStep::EndCombat);
}

#[test]
fn attack_limit_capped_defender_capacities_add() {
    let (mut engine, source, attackers) = fixture(50415, &[7, 20, 42, 91]);
    limit_fixture(
        &mut engine,
        source,
        1,
        AttackLimitAffected::AttackingController,
    );
    for index in [2, 3] {
        let other = inject_permanent_on_battlefield(&mut engine, index, "crawlspace");
        limit_fixture(
            &mut engine,
            other,
            1,
            AttackLimitAffected::AttackingController,
        );
    }
    for attacker in &attackers {
        engine
            .state
            .objects
            .get_mut(attacker)
            .unwrap()
            .must_attack_if_able = true;
    }
    begin(&mut engine);
    assert_eq!(
        engine.initial_response_batch().legal_by_player[&7].minimum_attack_requirement_count,
        3
    );
    let fewer = declare_to(
        &mut engine,
        &[
            (attackers[0], TargetRefKind::Player, 20),
            (attackers[1], TargetRefKind::Player, 42),
        ],
    );
    reject_unchanged(&mut engine, 7, &fewer);
    let command = declare_to(
        &mut engine,
        &[
            (attackers[0], TargetRefKind::Player, 20),
            (attackers[1], TargetRefKind::Player, 42),
            (attackers[2], TargetRefKind::Player, 91),
        ],
    );
    semantic::accepted(&mut engine, 7, &command);
}

#[test]
fn crawlspace_layer_six_ability_loss_removes_the_limit() {
    let (mut engine, source, attackers) = fixture(50416, &[0, 1]);
    engine
        .state
        .continuous_effects
        .push(tricerules_core::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: tricerules_core::AffectedScope::Single(source),
            kind: tricerules_cards::ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: tricerules_cards::EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
    begin(&mut engine);
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .attack_declaration_limits
        .is_empty());
    semantic::accepted(&mut engine, 0, &declare_attackers(attackers[..3].to_vec()));
}

#[test]
fn crawlspace_limit_is_independent_in_each_combat() {
    let (mut engine, _, attackers) = fixture(50417, &[0, 1]);
    begin(&mut engine);
    semantic::accepted(&mut engine, 0, &declare_attackers(attackers[..2].to_vec()));
    for _ in 0..48 {
        if engine.state.active_player_id() == 0 && engine.state.turn_step == TurnStep::Main1 {
            break;
        }
        resolve_cleanup_discards_if_any(&mut engine);
        pass_priority_round(&mut engine);
    }
    assert_eq!(
        (engine.state.active_player_id(), engine.state.turn_step),
        (0, TurnStep::Main1)
    );
    begin(&mut engine);
    reject_unchanged(&mut engine, 0, &declare_attackers(attackers[..3].to_vec()));
    semantic::accepted(&mut engine, 0, &declare_attackers(attackers[..2].to_vec()));
}

#[test]
fn attack_limit_does_not_restrict_mobilize_creatures_added_attacking() {
    let (mut engine, source, _) = fixture(50418, &[0, 1]);
    limit_fixture(&mut engine, source, 1, AttackLimitAffected::All);
    let shock = inject_creature_on_battlefield(&mut engine, 0, "shock_brigade");
    let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 10);
    begin(&mut engine);
    let command = declare_to(&mut engine, &[(shock, TargetRefKind::Player, 1)]);
    semantic::accepted(&mut engine, 0, &command);
    semantic::accepted(&mut engine, 0, &pass());
    let batch = engine.apply_command(1, &pass()).unwrap();
    let choice = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .expect("Mobilize defender choice");
    let player = *choice
        .combat_defender_options
        .iter()
        .find(|option| {
            option
                .defender
                .as_ref()
                .is_some_and(|target| target.kind == TargetRefKind::Player as i32)
        })
        .unwrap();
    let command = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_combat_defender: Some(player),
            ..Default::default()
        })),
    };
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(battlefield_token_oids(&engine, 0, "warrior_r_1_1").len(), 1);
    assert_eq!(engine.state.combat.as_ref().unwrap().attacking.len(), 2);
}

#[test]
fn crawlspace_rejects_duplicate_stale_and_wrong_actor_declarations_atomically() {
    let (mut engine, _, attackers) = fixture(50419, &[0, 1]);
    begin(&mut engine);
    reject_unchanged(
        &mut engine,
        0,
        &declare_attackers(vec![attackers[0], attackers[0]]),
    );
    let good = declare_to(&mut engine, &[(attackers[0], TargetRefKind::Player, 1)]);
    reject_unchanged(&mut engine, 1, &good);
    let mut stale = good.clone();
    let Some(Cmd::DeclareAttackers(declaration)) = &mut stale.cmd else {
        unreachable!()
    };
    declaration.assignments[0].attacker_zone_change_generation += 1;
    reject_unchanged(&mut engine, 0, &stale);
    semantic::accepted(&mut engine, 0, &good);
    let closed = engine.initial_response_batch();
    assert!(closed.legal_by_player[&0].attack_requirement_ids.is_empty());
    assert_eq!(
        closed.legal_by_player[&0].minimum_attack_requirement_count,
        0
    );
    assert!(closed.legal_by_player[&0]
        .attack_declaration_limits
        .is_empty());
}
