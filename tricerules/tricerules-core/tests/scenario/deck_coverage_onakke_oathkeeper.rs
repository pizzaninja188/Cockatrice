//! Actual Onakke abilities with explicitly synthetic setup objects and mana.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::{ContinuousEffectKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    AbilitySourceZone, AttackAssignment, CancelAttackDeclaration, CommitAttackDeclaration,
    DeclareAttackers, PaymentMana, PaymentSelection, TargetRefKind,
};

const OATHKEEPER: &str = "onakke_oathkeeper";

fn game(seed: u64, players: &[i32]) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    assert!(
        registry.get(OATHKEEPER).is_some(),
        "Onakke Oathkeeper must be registered"
    );
    let decks = players.iter().map(|_| deck_with("forest", &[])).collect();
    let mut engine =
        GameEngine::new(registry, seed, players, 20, Some(decks), true).expect("create game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn published_assignment(
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
        .expect("the engine offers this attacker and defender")
}

fn enter_onakke(engine: &mut GameEngine, player: usize) -> u32 {
    let hand = inject_card_into_hand(engine, player, OATHKEEPER);
    let entered = move_ready_to_battlefield(engine, player, OATHKEEPER);
    assert_eq!(entered, hand);
    entered
}

fn begin_attacks(engine: &mut GameEngine) {
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("begin combat");
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn declare_assignment(assignment: AttackAssignment) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
            assignments: vec![assignment],
        })),
    }
}

fn one_tax_fixture(seed: u64) -> (GameEngine, u32, u32, u32) {
    let mut engine = game(seed, &[0, 1]);
    let onakke = enter_onakke(&mut engine, 1);
    let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    (engine, onakke, walker, attacker)
}

fn commit_tax(engine: &GameEngine, mana: PaymentMana) -> RuledCommand {
    let pending = engine
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending attack tax");
    RuledCommand {
        cmd: Some(Cmd::CommitAttackDeclaration(CommitAttackDeclaration {
            transaction_id: pending.transaction_id,
            expected_revision: pending.revision,
            payment: Some(PaymentSelection {
                expected_state_revision: engine.state.command_index,
                mana: Some(mana),
                ..Default::default()
            }),
            ..Default::default()
        })),
    }
}

fn remove_abilities(engine: &mut GameEngine, object: u32) {
    // A synthetic layer-six effect isolates current ability suppression and restoration.
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
}

#[test]
fn onakke_charges_per_attacker_and_copy_only_for_its_controllers_walkers() {
    let mut engine = game(261_027_200, &[0, 1, 2]);
    // The Onakkes enter through the real dev move path so their statics are installed.
    for _ in 0..2 {
        let hand = inject_card_into_hand(&mut engine, 1, OATHKEEPER);
        assert_eq!(move_ready_to_battlefield(&mut engine, 1, OATHKEEPER), hand);
    }
    // Other battlefield objects, loyalty, battle protector and mana are fixture setup.
    let walkers = [
        inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren"),
        inject_permanent_on_battlefield(&mut engine, 1, "daretti,_scrap_savant"),
    ];
    for walker in walkers {
        engine
            .state
            .objects
            .get_mut(&walker)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 4);
    }
    let other_walker = inject_permanent_on_battlefield(&mut engine, 2, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&other_walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
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
        .set_counter(CounterKind::Defense, 5);
    engine.state.battle_protectors.insert(battle, 1);
    let attackers: Vec<_> = (0..5)
        .map(|_| inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"))
        .collect();
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("begin combat");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);

    let assignments = vec![
        published_assignment(
            &mut engine,
            attackers[0],
            TargetRefKind::Permanent,
            walkers[0],
        ),
        published_assignment(
            &mut engine,
            attackers[1],
            TargetRefKind::Permanent,
            walkers[1],
        ),
        published_assignment(&mut engine, attackers[2], TargetRefKind::Player, 1),
        published_assignment(
            &mut engine,
            attackers[3],
            TargetRefKind::Permanent,
            other_walker,
        ),
        published_assignment(&mut engine, attackers[4], TargetRefKind::Permanent, battle),
    ];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let offered = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: assignments.clone(),
                })),
            },
        )
        .expect("declare the engine-offered split attackers");
    let pending = offered.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .expect("two protected walkers require an attack payment");
    assert_eq!(
        pending.generic_mana_cost, 4,
        "two attackers times two Onakkes"
    );
    assert!(!engine.state.combat.as_ref().unwrap().attackers_declared);
    assert!(attackers_declared_in(&offered).is_empty());
    let pending = engine
        .state
        .pending_attack_declaration
        .as_ref()
        .unwrap()
        .clone();
    let paid = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitAttackDeclaration(CommitAttackDeclaration {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    payment: Some(PaymentSelection {
                        expected_state_revision: engine.state.command_index,
                        mana: Some(PaymentMana {
                            c: 4,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
        )
        .expect("pay the locked attack tax");
    assert!(engine.state.pending_attack_declaration.is_none());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        attackers_declared_in(&paid).pop().unwrap().assignments,
        assignments
    );
}

#[test]
fn onakke_tax_uses_current_controllers_instead_of_card_owners() {
    for source_changes_controller in [false, true] {
        let mut engine = game(
            261_027_202 + u64::from(source_changes_controller),
            &[0, 1, 2],
        );
        let source = enter_onakke(&mut engine, 1);
        let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
        engine
            .state
            .objects
            .get_mut(&walker)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 3);
        // Synthetic owner and control changes make ownership differ from current control.
        engine.state.objects.get_mut(&walker).unwrap().owner = 2;
        if source_changes_controller {
            let object = engine.state.objects.get_mut(&source).unwrap();
            object.base_controller = 2;
            object.controller = 2;
            engine.state.players[1]
                .battlefield
                .retain(|id| *id != source);
            engine.state.players[2].battlefield.push(source);
        }
        let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        begin_attacks(&mut engine);
        let assignment =
            published_assignment(&mut engine, attacker, TargetRefKind::Permanent, walker);
        let batch = engine
            .apply_scenario_command(0, &declare_assignment(assignment))
            .expect("declare attack on the current controller's walker");
        if source_changes_controller {
            assert!(engine.state.pending_attack_declaration.is_none());
            assert_eq!(attackers_declared_in(&batch).len(), 1);
        } else {
            assert_eq!(
                batch.legal_by_player[&0]
                    .pending_attack_declaration
                    .as_ref()
                    .unwrap()
                    .generic_mana_cost,
                1
            );
        }
    }
}

#[test]
fn onakke_tax_tracks_suppression_and_restoration_of_its_ability() {
    for restore in [false, true] {
        let (mut engine, source, walker, attacker) =
            one_tax_fixture(261_027_204 + u64::from(restore));
        remove_abilities(&mut engine, source);
        if restore {
            engine.state.continuous_effects.pop();
        }
        begin_attacks(&mut engine);
        let assignment =
            published_assignment(&mut engine, attacker, TargetRefKind::Permanent, walker);
        let batch = engine
            .apply_scenario_command(0, &declare_assignment(assignment))
            .unwrap();
        if restore {
            assert_eq!(
                batch.legal_by_player[&0]
                    .pending_attack_declaration
                    .as_ref()
                    .unwrap()
                    .generic_mana_cost,
                1
            );
        } else {
            assert!(engine.state.pending_attack_declaration.is_none());
            assert_eq!(attackers_declared_in(&batch).len(), 1);
        }
    }
}

#[test]
fn onakke_attack_requirements_use_untaxed_edges() {
    for every_edge_taxed in [false, true] {
        let (mut engine, _, _walker, attacker) =
            one_tax_fixture(261_027_206 + u64::from(every_edge_taxed));
        // The must-attack flag is synthetic; real Propaganda supplies the player-only tax.
        engine
            .state
            .objects
            .get_mut(&attacker)
            .unwrap()
            .must_attack_if_able = true;
        if every_edge_taxed {
            inject_card_into_hand(&mut engine, 1, "propaganda");
            move_ready_to_battlefield(&mut engine, 1, "propaganda");
        }
        begin_attacks(&mut engine);
        let legal = engine.initial_response_batch();
        assert_eq!(
            legal.legal_by_player[&0]
                .attack_requirement_ids
                .contains(&attacker),
            !every_edge_taxed
        );
        if every_edge_taxed {
            engine
                .apply_scenario_command(0, &declare_attackers(vec![]))
                .expect("a player need not pay to satisfy an attack requirement");
        } else {
            engine
                .apply_scenario_command(0, &declare_attackers(vec![]))
                .expect_err("the untaxed player edge preserves the attack requirement");
            let free = published_assignment(&mut engine, attacker, TargetRefKind::Player, 1);
            engine
                .apply_scenario_command(0, &declare_assignment(free))
                .unwrap();
            assert!(engine.state.pending_attack_declaration.is_none());
        }
    }
}

#[test]
fn onakke_locked_tax_rejects_stale_payers_and_can_be_paid_or_cancelled_after_suppression() {
    for cancel in [false, true] {
        let (mut engine, source, walker, attacker) =
            one_tax_fixture(261_027_208 + u64::from(cancel));
        let forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        begin_attacks(&mut engine);
        let assignment =
            published_assignment(&mut engine, attacker, TargetRefKind::Permanent, walker);
        engine
            .apply_scenario_command(0, &declare_assignment(assignment))
            .unwrap();
        let pending = engine
            .state
            .pending_attack_declaration
            .as_ref()
            .unwrap()
            .clone();
        assert_eq!(pending.generic_mana_cost, 1);
        let command = commit_tax(&engine, PaymentMana::default());
        engine
            .apply_scenario_command(1, &command)
            .expect_err("only the attacker can pay");
        let mut stale = command.clone();
        let Some(Cmd::CommitAttackDeclaration(commit)) = stale.cmd.as_mut() else {
            unreachable!()
        };
        commit.expected_revision += 1;
        engine
            .apply_scenario_command(0, &stale)
            .expect_err("stale transaction revision");
        assert_eq!(
            engine.state.pending_attack_declaration.as_ref(),
            Some(&pending)
        );
        assert!(engine.state.objects[&attacker].tapped);
        // Model a source change after costs were locked; the real Forest mana ability is then used.
        remove_abilities(&mut engine, source);
        let mana = activate_ability_for(&engine, forest, 0, vec![]);
        engine
            .apply_scenario_command(0, &mana)
            .expect("activate mana during the payment window");
        assert_eq!(
            engine
                .state
                .pending_attack_declaration
                .as_ref()
                .unwrap()
                .generic_mana_cost,
            1
        );
        if cancel {
            let pending = engine.state.pending_attack_declaration.as_ref().unwrap();
            let command = RuledCommand {
                cmd: Some(Cmd::CancelAttackDeclaration(CancelAttackDeclaration {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                })),
            };
            engine
                .apply_scenario_command(0, &command)
                .expect("cancel declaration");
            assert!(!engine.state.objects[&attacker].tapped);
            assert!(engine.state.objects[&forest].tapped);
            assert_eq!(engine.state.players[0].mana_pool.green, 1);
            assert!(!engine.state.combat.as_ref().unwrap().attackers_declared);
        } else {
            let payment = commit_tax(
                &engine,
                PaymentMana {
                    g: 1,
                    ..Default::default()
                },
            );
            engine
                .apply_scenario_command(0, &payment)
                .expect("pay the previously locked tax");
            assert_eq!(engine.state.players[0].mana_pool.green, 0);
            assert!(engine.state.combat.as_ref().unwrap().attackers_declared);
        }
        assert!(engine.state.pending_attack_declaration.is_none());
    }
}

#[test]
fn onakke_tax_stays_locked_when_a_mana_ability_restores_another_source() {
    for cancel in [false, true] {
        let (mut engine, _, walker, attacker) = one_tax_fixture(261_027_212 + u64::from(cancel));
        let suppressed = enter_onakke(&mut engine, 1);
        let ironworks = inject_permanent_on_battlefield(&mut engine, 0, "krark-clan_ironworks");
        let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        // Synthetic source-linked suppression ends when Ironworks sacrifices this actual artifact.
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(artifact),
            affected: AffectedScope::Single(suppressed),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp: engine.state.command_index + 1,
        });
        begin_attacks(&mut engine);
        let assignment =
            published_assignment(&mut engine, attacker, TargetRefKind::Permanent, walker);
        engine
            .apply_scenario_command(0, &declare_assignment(assignment))
            .unwrap();
        assert_eq!(
            engine
                .state
                .pending_attack_declaration
                .as_ref()
                .unwrap()
                .generic_mana_cost,
            1
        );
        let mut mana = activate_ability_with_costs(
            ironworks,
            0,
            vec![],
            vec![permanent_cost_selection(0, artifact)],
        );
        let Some(Cmd::ActivateAbility(activation)) = mana.cmd.as_mut() else {
            unreachable!()
        };
        activation.expected_zone_change_generation = engine
            .state
            .zone_change_generation
            .get(&ironworks)
            .copied()
            .unwrap_or(0);
        engine
            .apply_scenario_command(0, &mana)
            .expect("Ironworks sacrifices the artifact for mana");
        assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
        assert_eq!(
            engine
                .state
                .pending_attack_declaration
                .as_ref()
                .unwrap()
                .generic_mana_cost,
            1
        );
        if cancel {
            let pending = engine.state.pending_attack_declaration.as_ref().unwrap();
            let command = RuledCommand {
                cmd: Some(Cmd::CancelAttackDeclaration(CancelAttackDeclaration {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                })),
            };
            engine
                .apply_scenario_command(0, &command)
                .expect("cancel the original declaration");
            let again =
                published_assignment(&mut engine, attacker, TargetRefKind::Permanent, walker);
            let offered = engine
                .apply_scenario_command(0, &declare_assignment(again))
                .unwrap();
            assert_eq!(
                offered.legal_by_player[&0]
                    .pending_attack_declaration
                    .as_ref()
                    .unwrap()
                    .generic_mana_cost,
                2,
                "a new declaration sees both restored Onakkes"
            );
        } else {
            let payment = commit_tax(
                &engine,
                PaymentMana {
                    c: 1,
                    ..Default::default()
                },
            );
            engine
                .apply_scenario_command(0, &payment)
                .expect("pay the original locked amount");
            assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
            assert!(engine.state.combat.as_ref().unwrap().attackers_declared);
        }
    }
}

#[test]
fn onakke_attack_tax_does_not_accept_spell_only_restricted_mana() {
    let (mut engine, _, walker, attacker) = one_tax_fixture(261_027_210);
    let embercat = inject_creature_on_battlefield(&mut engine, 0, "chandras_embercat");
    begin_attacks(&mut engine);
    let assignment = published_assignment(&mut engine, attacker, TargetRefKind::Permanent, walker);
    engine
        .apply_scenario_command(0, &declare_assignment(assignment))
        .unwrap();
    let mana = activate_ability_for(&engine, embercat, 0, vec![]);
    engine
        .apply_scenario_command(0, &mana)
        .expect("produce restricted mana");
    let group = engine.state.players[0].restricted_mana[0].restriction_group_id;
    let mut forbidden = commit_tax(&engine, PaymentMana::default());
    let Some(Cmd::CommitAttackDeclaration(commit)) = forbidden.cmd.as_mut() else {
        unreachable!()
    };
    commit.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: group,
        r: 1,
        ..Default::default()
    });
    engine
        .apply_scenario_command(0, &forbidden)
        .expect_err("spell-only mana cannot pay an attack cost");
    assert_eq!(engine.state.players[0].restricted_mana[0].amount.r, 1);
    assert!(engine.state.pending_attack_declaration.is_some());
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let payment = commit_tax(
        &engine,
        PaymentMana {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &payment)
        .expect("unrestricted mana pays the attack tax");
    assert_eq!(engine.state.players[0].restricted_mana[0].amount.r, 1);
}

#[test]
fn onakke_attack_tax_replays_accepted_wire_commands() {
    use prost::Message;
    fn fixture() -> (GameEngine, u32, u32, u32) {
        let (mut engine, _, walker, attacker) = one_tax_fixture(261_027_211);
        let forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        begin_attacks(&mut engine);
        (engine, walker, attacker, forest)
    }
    fn apply_wire(original: &mut GameEngine, replay: &mut GameEngine, command: RuledCommand) {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let batch = original
            .apply_scenario_command(0, &command)
            .expect("accept command");
        assert_eq!(
            batch,
            replay
                .apply_scenario_command(0, &decoded)
                .expect("replay accepted command")
        );
    }
    let (mut original, walker, attacker, forest) = fixture();
    let (mut replay, replay_walker, replay_attacker, replay_forest) = fixture();
    assert_eq!(
        (walker, attacker, forest),
        (replay_walker, replay_attacker, replay_forest)
    );
    let assignment =
        published_assignment(&mut original, attacker, TargetRefKind::Permanent, walker);
    assert_eq!(
        assignment,
        published_assignment(&mut replay, attacker, TargetRefKind::Permanent, walker)
    );
    apply_wire(&mut original, &mut replay, declare_assignment(assignment));
    let mana = activate_ability_for(&original, forest, 0, vec![]);
    apply_wire(&mut original, &mut replay, mana);
    let payment = commit_tax(
        &original,
        PaymentMana {
            g: 1,
            ..Default::default()
        },
    );
    apply_wire(&mut original, &mut replay, payment);
    apply_wire(&mut original, &mut replay, pass());
    assert_eq!(
        original.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn onakke_graveyard_activation_pays_exact_cost_and_returns_own_planeswalker_at_instant_speed() {
    let mut engine = game(261_027_201, &[0, 1]);
    // The graveyards and mana are synthetic setup; activation and battlefield entry use engine commands.
    let source = inject_graveyard_card(&mut engine, 1, OATHKEEPER);
    let walker = inject_graveyard_card(&mut engine, 1, "jace_beleren");
    let nonwalker = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    let other_walker = inject_graveyard_card(&mut engine, 0, "jace_beleren");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            w: 2,
            c: 4,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent priority in main phase");
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.priority_player_id(), 1);
    let key = u64::from(source) << 32;
    let offer = engine.initial_response_batch();
    let action = offer.legal_by_player[&1]
        .zone_ability_actions
        .iter()
        .find(|action| action.object_id == source && action.ability_index == 0)
        .expect("the graveyard ability is offered on another player's turn");
    assert_eq!(action.source_zone, AbilitySourceZone::Graveyard as i32);
    assert_eq!(
        action.zone_change_generation,
        engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0)
    );
    let presentation = action
        .ability
        .as_ref()
        .unwrap()
        .presentation
        .as_ref()
        .unwrap();
    assert_eq!(presentation.card_id, OATHKEEPER);
    assert_eq!(presentation.path[0].id, "activated_01");
    assert_eq!(presentation.oracle_line_indices, [2]);
    let targets =
        &offer.legal_by_player[&1].valid_targets_by_ability[&key].groups[0].valid_graveyard_ids;
    assert!(targets.contains(&walker));
    assert!(!targets.contains(&nonwalker) && !targets.contains(&other_walker));
    let command = authoring_actions::activation(&mut engine, 1, source, 0)
        .expect("choose the offered own-graveyard planeswalker and payment");
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_ref() else {
        panic!("activation")
    };
    assert_eq!(activation.source_zone, AbilitySourceZone::Graveyard as i32);
    assert_eq!(activation.targets.len(), 1);
    assert_eq!(activation.targets[0].object_id, walker);
    assert_eq!(activation.targets[0].kind, TargetRefKind::Graveyard as i32);

    for invalid in [nonwalker, other_walker] {
        let mut rejected = command.clone();
        let Some(Cmd::ActivateAbility(activation)) = rejected.cmd.as_mut() else {
            unreachable!()
        };
        activation.targets[0].object_id = invalid;
        engine
            .apply_scenario_command(1, &rejected)
            .expect_err("illegal graveyard target");
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(
            (
                engine.state.players[1].mana_pool.colorless,
                engine.state.players[1].mana_pool.white
            ),
            (4, 2)
        );
    }
    let mut stale = command.clone();
    let Some(Cmd::ActivateAbility(activation)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation += 1;
    engine
        .apply_scenario_command(1, &stale)
        .expect_err("stale graveyard source");
    engine.state.players[1].mana_pool.colorless = 3;
    engine
        .apply_scenario_command(1, &command)
        .expect_err("insufficient mana");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    engine.state.players[1].mana_pool.colorless = 4;
    engine
        .apply_scenario_command(1, &command)
        .expect("activate from the graveyard");
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert_eq!(
        (
            engine.state.players[1].mana_pool.colorless,
            engine.state.players[1].mana_pool.white
        ),
        (0, 0)
    );
    assert_eq!(engine.state.objects[&walker].zone, Zone::Graveyard);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&walker].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&walker].controller, 1);
    assert!(!engine.state.objects[&walker].tapped);
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        3
    );
}

fn graveyard_fixture(seed: u64, players: &[i32]) -> (GameEngine, u32, u32) {
    let mut engine = game(seed, players);
    let source = inject_graveyard_card(&mut engine, 0, OATHKEEPER);
    let walker = inject_graveyard_card(&mut engine, 0, "jace_beleren");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 2,
            c: 4,
            ..Default::default()
        },
    );
    (engine, source, walker)
}

#[test]
fn onakke_does_not_refund_costs_when_the_target_leaves_and_returns() {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
    let (mut engine, source, walker) = graveyard_fixture(261_027_214, &[0, 1]);
    let command = authoring_actions::activation(&mut engine, 0, source, 0).unwrap();
    engine
        .apply_scenario_command(0, &command)
        .expect("pay and activate");
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&walker)
        .copied()
        .unwrap_or(0);
    engine.enable_dev_commands();
    for zone in [DevZone::Hand, DevZone::Graveyard] {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 0,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: "Jace Beleren".into(),
                            zone: zone as i32,
                            ready: false,
                        })),
                    })),
                },
            )
            .expect("move the same physical target through both zone changes");
    }
    assert!(engine.state.zone_change_generation[&walker] > old_generation);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&walker].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn onakke_does_not_refund_costs_when_its_ability_is_countered() {
    use tricerules_proto::ruled::v1::ChooseTriggerTarget;
    let (mut engine, source, walker) = graveyard_fixture(261_027_215, &[0, 1]);
    let command = authoring_actions::activation(&mut engine, 0, source, 0).unwrap();
    engine
        .apply_scenario_command(0, &command)
        .expect("pay and activate");
    let ability = engine.state.stack.last().unwrap().id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent priority");
    inject_card_into_hand(&mut engine, 1, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "tishanas_tidebinder");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("cast Tidebinder");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: vec![TargetRef {
                        object_id: ability,
                        kind: TargetRefKind::Stack as i32,
                        ..Default::default()
                    }],
                    ..Default::default()
                })),
            },
        )
        .expect("Tidebinder targets the activated ability");
    let mut events = Vec::new();
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        events.extend(
            engine
                .apply_scenario_command(actor, &pass())
                .unwrap()
                .events,
        );
    }
    assert!(events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == ability)));
    assert_eq!(engine.state.objects[&walker].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
}

fn replacement_fixture() -> (GameEngine, u32, u32) {
    let mut engine = game(261_027_216, &[0, 1, 2]);
    for _ in 0..2 {
        let hand = inject_card_into_hand(&mut engine, 0, "doubling_season");
        assert_eq!(
            move_ready_to_battlefield(&mut engine, 0, "doubling_season"),
            hand
        );
    }
    let source = inject_graveyard_card(&mut engine, 0, OATHKEEPER);
    let walker = inject_graveyard_card(&mut engine, 0, "jace_beleren");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 2,
            c: 4,
            ..Default::default()
        },
    );
    (engine, source, walker)
}

#[test]
fn onakke_replays_a_parked_loyalty_entry_after_another_player_departs() {
    use prost::Message;
    use tricerules_proto::ruled::v1::{ChoiceKind, SubmitResolutionChoice};
    fn apply_wire(
        original: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
    ) -> RuledEventBatch {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let batch = original
            .apply_scenario_command(actor, &command)
            .expect("accept command");
        assert_eq!(
            batch,
            replay
                .apply_scenario_command(actor, &decoded)
                .expect("replay accepted command")
        );
        batch
    }
    let (mut engine, source, walker) = replacement_fixture();
    let (mut replay, other_source, other_walker) = replacement_fixture();
    assert_eq!((source, walker), (other_source, other_walker));
    let command = authoring_actions::activation(&mut engine, 0, source, 0).unwrap();
    assert_eq!(
        command,
        authoring_actions::activation(&mut replay, 0, source, 0).unwrap()
    );
    apply_wire(&mut engine, &mut replay, 0, command);
    let activation = engine.state.stack.last().unwrap().id;
    let mut events = Vec::new();
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        events.extend(apply_wire(&mut engine, &mut replay, actor, pass()).events);
    }
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("choose entry-counter replacement order");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(pending.presentation.candidates.len(), 2);
    assert_eq!(engine.state.objects[&walker].zone, Zone::Graveyard);
    assert!(!events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == activation)));
    events.extend(apply_wire(&mut engine, &mut replay, 2, concede()).events);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("surviving controller's choice remains");
    let prompt = pending.resolution_prompt_id.get();
    let chosen = pending.presentation.candidates[0];
    events.extend(
        apply_wire(
            &mut engine,
            &mut replay,
            0,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: prompt,
                    chosen_object_ids: vec![chosen],
                    ..Default::default()
                })),
            },
        )
        .events,
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&walker].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        12
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == activation))
            .count(),
        1
    );
    assert_eq!(events.iter().filter(|event| matches!(&event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == walker
            && moved.destination() == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield)).count(), 1);
    let actor = engine.state.priority_player_id();
    apply_wire(&mut engine, &mut replay, actor, pass());
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn onakke_parked_entry_is_cleaned_up_when_its_owner_and_controller_depart() {
    let (mut engine, source, walker) = replacement_fixture();
    let command = authoring_actions::activation(&mut engine, 0, source, 0).unwrap();
    engine.apply_scenario_command(0, &command).unwrap();
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&walker].zone, Zone::Graveyard);
    engine
        .apply_scenario_command(0, &concede())
        .expect("entry controller and owner depart");
    assert!(!engine.state.objects.contains_key(&source));
    assert!(!engine.state.objects.contains_key(&walker));
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.players[1].has_lost && !engine.state.players[2].has_lost);
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(actor, &pass())
        .expect("survivors can continue");
}
